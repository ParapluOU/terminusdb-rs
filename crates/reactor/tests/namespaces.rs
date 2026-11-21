#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Namespaces registered while the process runs.
//!
//! Reactor's configuration file lists the databases it tracks, which is the
//! right shape for a handful named by an operator and the wrong one for an
//! application that *creates* databases. A room-per-tenant or
//! publication-per-database layout has thousands of namespaces, appearing while
//! the process runs, and nobody can list them in TOML.
//!
//! These tests are about that: an unregistered database is proxied and cached
//! for nobody, a registered one becomes an ordinary tracked namespace with its
//! own capture task and writer, and untracking takes its cached state with it.
//!
//! The second database in each test is created by hand rather than by the
//! harness, because the thing under test is precisely a database the harness —
//! and the configuration — did not know about.

mod common;

use std::time::{Duration, Instant};

use anyhow::Result;
use common::models::{person, Person};
use common::Reactor;
use serde_json::json;
use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
use terminusdb_reactor::ids::Namespace;

/// Create a database the configuration never mentioned, with a schema and one
/// document in it.
async fn second_database(direct: &TerminusDBHttpClient, name: &str) -> Result<BranchSpec> {
    direct.ensure_database(name).await?;
    let spec = BranchSpec::with_branch(name, "main");
    let args = DocumentInsertArgs::from(spec.clone());
    direct.insert_schemas::<(Person,)>(args.clone()).await?;
    direct
        .insert_instance(&person("alice", "Ann", 30)?, args)
        .await?;
    Ok(spec)
}

/// Wait for a namespace to acquire a watermark, which is what makes it
/// cacheable: until capture establishes one there is no position to pin a fill
/// at, so reads are forwarded however tracked the namespace is.
async fn await_watermark(reactor: &Reactor, ns: &Namespace) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if reactor.engine.watermark(ns).is_some() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

fn ns_of(spec: &BranchSpec) -> Namespace {
    Namespace::new("admin", spec.db.clone(), "main")
}

fn read(spec: &BranchSpec) -> String {
    format!("/api/document/admin/{}?id=Person/alice", spec.db)
}

/// The whole feature: a database nobody configured becomes cacheable on
/// request, and stops being cacheable when asked.
#[tokio::test]
async fn a_namespace_registered_at_run_time_becomes_cacheable() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_register", |direct, spec| async move {
            let reactor = Reactor::start(up, &spec.db).await?;
            let room = second_database(&direct, &format!("{}_room", spec.db)).await?;
            let room_ns = ns_of(&room);

            // Untracked: forwarded, and cached for nobody. This is invariant 6,
            // and it is the default the registration endpoint exists to change.
            let before = reactor.get(&read(&room)).await?;
            assert_eq!(
                before.status,
                200,
                "the read still works: {}",
                before.text()
            );
            assert!(
                before.is_bypass(),
                "an unconfigured database must not be cached: {}",
                before.cache_status()
            );

            // Register it.
            let registered = reactor
                .post(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": room.db}),
                )
                .await?;
            assert_eq!(registered.status, 200, "{}", registered.text());
            let report = registered.json()?;
            assert_eq!(report["added"], json!(true));
            assert_eq!(report["namespace"], json!(room_ns.path()));

            // Registration is idempotent: an application that creates databases
            // should be able to call this on every request without bookkeeping.
            let again = reactor
                .post(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": room.db}),
                )
                .await?;
            assert_eq!(again.json()?["added"], json!(false));

            assert!(
                await_watermark(&reactor, &room_ns).await,
                "capture must start for a namespace registered at run time — which also means \
                 a writer was started for it, since nothing did that at boot"
            );

            // Now it caches.
            let miss = reactor.get(&read(&room)).await?;
            assert!(
                miss.is_miss(),
                "the first tracked read is a miss: {}",
                miss.cache_status()
            );
            let hit = reactor.get(&read(&room)).await?;
            assert!(hit.is_hit(), "the second is a hit: {}", hit.cache_status());
            assert_eq!(hit.body, miss.body, "and the same bytes");

            // And capture is live: a write to the new namespace invalidates.
            let (_, commit) = direct
                .insert_instance_with_commit_id(
                    &person("alice", "Anne", 31)?,
                    DocumentInsertArgs::from(room.clone()),
                )
                .await?;
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut saw = false;
            while Instant::now() < deadline {
                if reactor.engine.watermark(&room_ns).map(|w| w.commit) == Some(commit.clone()) {
                    saw = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(
                saw,
                "the commit must reach the new namespace's capture task"
            );
            let after = reactor.get(&read(&room)).await?;
            assert_eq!(after.json()?["name"], json!("Anne"), "{}", after.text());

            direct.delete_database(&room.db).await?;
            Ok(())
        })
        .await
}

/// Untracking has to take the cached state with it: an entry in a namespace
/// nothing watches would be invalidated by nothing.
#[tokio::test]
async fn untracking_stops_caching_and_drops_what_was_cached() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_untrack", |direct, spec| async move {
            let reactor = Reactor::start(up, &spec.db).await?;
            let room = second_database(&direct, &format!("{}_room", spec.db)).await?;
            let room_ns = ns_of(&room);

            reactor
                .post(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": room.db}),
                )
                .await?;
            assert!(await_watermark(&reactor, &room_ns).await);
            reactor.get(&read(&room)).await?;
            assert!(reactor.get(&read(&room)).await?.is_hit());

            let listed = reactor.get("/_reactor/namespace").await?;
            let entries = listed.json()?;
            let names: Vec<String> = entries
                .as_array()
                .expect("a list")
                .iter()
                .map(|entry| entry["namespace"].as_str().unwrap_or_default().to_owned())
                .collect();
            assert!(names.contains(&room_ns.path()), "{names:?}");
            assert!(
                names.contains(&ns_of(&spec).path()),
                "the configured namespace is listed too: {names:?}"
            );
            let room_entry = entries
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["namespace"] == json!(room_ns.path()))
                .expect("the room is listed");
            assert_eq!(room_entry["source"], json!("registered"));
            assert!(
                room_entry["idle_secs"].is_u64(),
                "it has been read, so its warmth is known: {room_entry}"
            );

            let dropped = reactor
                .delete(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": room.db}),
                )
                .await?;
            assert_eq!(dropped.status, 200, "{}", dropped.text());

            let after = reactor.get(&read(&room)).await?;
            assert!(
                after.is_bypass(),
                "an untracked namespace is forwarded again: {}",
                after.cache_status()
            );
            assert_eq!(after.status, 200, "and still answered: {}", after.text());

            direct.delete_database(&room.db).await?;
            Ok(())
        })
        .await
}

/// A configured namespace is the operator's statement of intent, and a process
/// that forgot it on request would track it again on restart anyway.
#[tokio::test]
async fn a_configured_namespace_cannot_be_untracked() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_configured", |_direct, spec| async move {
            let reactor = Reactor::start(up, &spec.db).await?;
            let refused = reactor
                .delete(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": spec.db}),
                )
                .await?;
            assert_eq!(refused.status, 409, "{}", refused.text());
            assert!(
                refused.text().contains("configuration file"),
                "the reason has to say why: {}",
                refused.text()
            );
            Ok(())
        })
        .await
}

/// The cap refuses rather than evicting a namespace something may be using:
/// a refused registration costs hit rate, an eviction would risk correctness.
#[tokio::test]
async fn the_namespace_cap_refuses_rather_than_evicts() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_cap", |direct, spec| async move {
            // One configured namespace, and a cap of one.
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.capture.max_tracked_namespaces = 1;
            })
            .await?;
            let room = second_database(&direct, &format!("{}_room", spec.db)).await?;

            let refused = reactor
                .post(
                    "/_reactor/namespace",
                    json!({"org": "admin", "db": room.db}),
                )
                .await?;
            assert_eq!(
                refused.status,
                507,
                "over the cap, registration is refused: {}",
                refused.text()
            );
            assert!(
                refused.text().contains("max_tracked_namespaces"),
                "and names the knob: {}",
                refused.text()
            );

            // The read still works. That is the whole point of refusing rather
            // than evicting: the database is proxied, not broken.
            let read = reactor.get(&read(&room)).await?;
            assert_eq!(read.status, 200, "{}", read.text());
            assert!(read.is_bypass(), "{}", read.cache_status());

            // And the namespace that was already tracked is untouched.
            let configured = reactor
                .get(&format!("/api/document/admin/{}?type=Person", spec.db))
                .await?;
            assert!(!configured.is_bypass(), "{}", configured.cache_status());

            direct.delete_database(&room.db).await?;
            Ok(())
        })
        .await
}

/// `auto_register` is the ergonomic form for an application that creates
/// databases: the first read registers, and the second one caches.
#[tokio::test]
async fn auto_registration_tracks_a_namespace_on_first_read() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_auto", |direct, spec| async move {
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.capture.auto_register = true;
            })
            .await?;
            let room = second_database(&direct, &format!("{}_room", spec.db)).await?;
            let room_ns = ns_of(&room);

            // The first read is still forwarded — registration has to establish
            // a watermark first, and there is no position to pin a fill at
            // until it does.
            let first = reactor.get(&read(&room)).await?;
            assert!(first.is_bypass(), "{}", first.cache_status());
            assert_eq!(first.status, 200, "{}", first.text());

            assert!(
                await_watermark(&reactor, &room_ns).await,
                "the read itself registered the namespace"
            );

            reactor.get(&read(&room)).await?;
            let hit = reactor.get(&read(&room)).await?;
            assert!(
                hit.is_hit(),
                "once the watermark lands it caches like any other namespace: {}",
                hit.cache_status()
            );

            let listed = reactor.get("/_reactor/namespace").await?;
            let entries = listed.json()?;
            let room_entry = entries
                .as_array()
                .expect("a list")
                .iter()
                .find(|entry| entry["namespace"] == json!(room_ns.path()))
                .expect("the room registered itself")
                .clone();
            assert_eq!(
                room_entry["source"],
                json!("automatic"),
                "and says so, so an operator can tell which namespaces it adopted"
            );

            direct.delete_database(&room.db).await?;
            Ok(())
        })
        .await
}

/// `X-Reactor-Max-Staleness` was in the adopter's guide before it was in the
/// code. A caller that cares about freshness in milliseconds has no commit id
/// to name, so the fence cannot serve it.
#[tokio::test]
async fn a_caller_can_refuse_an_answer_older_than_it_tolerates() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_ns_staleness", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;
            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "warm");

            // Let the entry age past the bound we are about to state.
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            let fresh_only = reactor
                .get_with(&path, &[("x-reactor-max-staleness", "100")])
                .await?;
            assert!(
                !fresh_only.is_hit(),
                "an entry older than the stated bound is not a hit: {}",
                fresh_only.cache_status()
            );
            assert_eq!(fresh_only.status, 200, "{}", fresh_only.text());

            // A generous bound still hits.
            let tolerant = reactor
                .get_with(&path, &[("x-reactor-max-staleness", "600000")])
                .await?;
            assert!(tolerant.is_hit(), "{}", tolerant.cache_status());
            Ok(())
        })
        .await
}

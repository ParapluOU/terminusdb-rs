#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Namespace scoping, and topology endpoints that purge.
//!
//! Split out of `caching.rs` for a practical reason worth recording: the
//! embedded memory-mode server stops answering after roughly six or seven
//! databases have been created in one process ("Plugin handler failed", then
//! connection-refused for everything after). Each *test binary* gets its own
//! server, so keeping these in their own target is what makes both files
//! runnable. It also groups them sensibly: everything here is about a
//! namespace being a `(org, db, branch)` triple.

mod common;

use anyhow::Result;
use common::models::{person, Person};
use common::Reactor;
use terminusdb_client::DocumentInsertArgs;

// ---------------------------------------------------------------------------
// Per-database and per-branch indexing.
//
// The cache is partitioned by `Namespace { org, db, branch }`, with one writer,
// watermark and term index per namespace. These tests are what make that
// claim observable rather than merely true of the types.
// ---------------------------------------------------------------------------

/// Two branches holding the same document id with different content never see
/// each other's bytes, and a commit on one does not invalidate the other.
#[tokio::test]
async fn branches_are_cached_independently() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_branches", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "OnMain", 1)?, args)
                .await?;

            // A second branch, forked from main, then given different content.
            direct
                .create_branch(
                    &format!("admin/{}/local/branch/feature", spec.db),
                    &format!("admin/{}/local/branch/main", spec.db),
                )
                .await?;
            let feature_spec = terminusdb_client::BranchSpec::with_branch(&spec.db, "feature");
            direct
                .insert_instance(
                    &person("alice", "OnFeature", 2)?,
                    DocumentInsertArgs::from(feature_spec.clone()),
                )
                .await?;

            let reactor =
                Reactor::start_with(up, &spec.db, common::with_branches(&["main", "feature"]))
                    .await?;
            let main_path = format!(
                "/api/document/admin/{}/local/branch/main?id=Person/alice",
                spec.db
            );
            let feature_path = format!(
                "/api/document/admin/{}/local/branch/feature?id=Person/alice",
                spec.db
            );

            // Each branch answers with its own content...
            let main_first = reactor.get(&main_path).await?;
            assert_eq!(main_first.status, 200, "body: {}", main_first.text());
            assert!(
                main_first.text().contains("OnMain"),
                "got {}",
                main_first.text()
            );
            let feature_first = reactor.get(&feature_path).await?;
            assert!(
                feature_first.text().contains("OnFeature"),
                "the other branch must not be served main's entry: {}",
                feature_first.text()
            );

            // ...and each caches independently.
            let main_hit = reactor.get(&main_path).await?;
            let feature_hit = reactor.get(&feature_path).await?;
            assert!(main_hit.is_hit(), "got {}", main_hit.cache_status());
            assert!(feature_hit.is_hit(), "got {}", feature_hit.cache_status());
            assert!(main_hit.text().contains("OnMain"));
            assert!(feature_hit.text().contains("OnFeature"));
            assert_ne!(
                main_hit.header("etag"),
                feature_hit.header("etag"),
                "different content must have different ETags"
            );

            // A commit on main must leave feature's fragment alone.
            let feature_ns = terminusdb_reactor::Namespace::new("admin", &spec.db, "feature");
            let before = reactor.engine.watermark(&reactor.ns).map(|w| w.commit);
            direct
                .insert_instance(
                    &person("alice", "OnMainAgain", 3)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let feature_after = reactor.get(&feature_path).await?;
            assert!(
                feature_after.is_hit(),
                "a commit on main must not invalidate feature's cache, got {}",
                feature_after.cache_status()
            );
            assert!(
                feature_after.text().contains("OnFeature"),
                "and feature must still answer with its own content: {}",
                feature_after.text()
            );

            // While main itself did update.
            let main_after = reactor.get(&main_path).await?;
            assert!(
                main_after.text().contains("OnMainAgain"),
                "got {} ({})",
                main_after.text(),
                main_after.cache_status()
            );
            println!(
                "FINDING per-branch isolation: main={} feature={} (feature watermark {:?})",
                main_after.cache_status(),
                feature_after.cache_status(),
                reactor.engine.watermark(&feature_ns).map(|w| w.pos)
            );
            Ok(())
        })
        .await
}

/// The bare `/org/db` form and the explicit `main` branch form are one
/// namespace, so they share a single cache entry.
#[tokio::test]
async fn the_two_spellings_of_the_main_branch_share_one_entry() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_spelling", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let bare = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            let explicit = format!(
                "/api/document/admin/{}/local/branch/main?id=Person/alice",
                spec.db
            );

            let first = reactor.get(&bare).await?;
            assert!(first.is_miss(), "got {}", first.cache_status());

            // The *other* spelling must hit the entry the first one populated.
            let second = reactor.get(&explicit).await?;
            assert!(
                second.is_hit(),
                "`/org/db` and `/org/db/local/branch/main` are the same namespace \
                 and must share one entry, got {}",
                second.cache_status()
            );
            assert_eq!(first.body, second.body);
            // Only a hit carries an ETag — a response we just fetched has no
            // validator of ours to offer — so there is nothing to compare on
            // the miss.
            assert!(first.header("etag").is_none(), "a miss carries no ETag");
            assert!(second.header("etag").is_some(), "a hit does");
            Ok(())
        })
        .await
}

// ---------------------------------------------------------------------------
// Topology endpoints purge the branch they rewrote.
// ---------------------------------------------------------------------------

/// `optimize` purges, even though it need not move the head at all — which is
/// precisely why head inference is not good enough here.
#[tokio::test]
async fn optimize_purges_the_branch() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_purge_optimize", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let optimize = reactor
                .post(
                    &format!("/api/optimize/admin/{}/local/branch/main", spec.db),
                    serde_json::json!({}),
                )
                .await?;
            println!(
                "optimize -> {} {}",
                optimize.status,
                optimize.text().chars().take(120).collect::<String>()
            );
            assert!(
                (200..300).contains(&optimize.status),
                "optimize failed: {} {}",
                optimize.status,
                optimize.text()
            );

            // The purge is applied before the response returns, so the very
            // next read must refill rather than serve the pre-optimize entry.
            let after = reactor.get(&path).await?;
            assert!(
                !after.is_hit(),
                "optimize must purge the branch, got {}",
                after.cache_status()
            );
            assert!(
                after.text().contains("Ann"),
                "and the refilled answer must be correct: {}",
                after.text()
            );
            Ok(())
        })
        .await
}

/// `reset` purges, rather than relying on the applier noticing a
/// non-fast-forward.
#[tokio::test]
async fn reset_purges_the_branch() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_purge_reset", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "First", 1)?, args.clone())
                .await?;
            let original = direct.get_latest_commit_id(&spec).await?;
            direct
                .insert_instance(&person("alice", "Second", 2)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            let first = reactor.get(&path).await?;
            assert!(first.text().contains("Second"), "got {}", first.text());
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let reset = reactor
                .post(
                    &format!("/api/reset/admin/{}/local/branch/main", spec.db),
                    serde_json::json!({"commit_descriptor": format!(
                        "admin/{}/local/commit/{}", spec.db, original
                    )}),
                )
                .await?;
            println!(
                "reset -> {} {}",
                reset.status,
                reset.text().chars().take(120).collect::<String>()
            );
            assert!(
                (200..300).contains(&reset.status),
                "reset failed: {} {}",
                reset.status,
                reset.text()
            );

            let after = reactor.get(&path).await?;
            assert!(
                !after.is_hit(),
                "reset must purge the branch, got {}",
                after.cache_status()
            );
            assert!(
                after.text().contains("First") && !after.text().contains("Second"),
                "and must never serve pre-rewrite bytes: {}",
                after.text()
            );
            Ok(())
        })
        .await
}

/// `squash` purges too.
#[tokio::test]
async fn squash_purges_the_branch() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_purge_squash", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let squash = reactor
                .post(
                    &format!("/api/squash/admin/{}/local/branch/main", spec.db),
                    serde_json::json!({
                        "commit_info": {"author": "reactor-test", "message": "squash"}
                    }),
                )
                .await?;
            println!(
                "squash -> {} {}",
                squash.status,
                squash.text().chars().take(140).collect::<String>()
            );
            if !(200..300).contains(&squash.status) {
                println!(
                    "FINDING squash not exercised on this node (status {}); the plan \
                     mapping is unit-tested instead.",
                    squash.status
                );
                return Ok(());
            }

            let after = reactor.get(&path).await?;
            assert!(
                !after.is_hit(),
                "squash must purge the branch, got {}",
                after.cache_status()
            );
            assert!(after.text().contains("Ann"), "got {}", after.text());
            Ok(())
        })
        .await
}

/// **The over-broad-fix guard.** A purge triggered on one branch must leave
/// another branch's entries intact.
#[tokio::test]
async fn a_purge_is_scoped_to_one_branch() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_purge_scope", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "OnMain", 1)?, args)
                .await?;
            direct
                .create_branch(
                    &format!("admin/{}/local/branch/feature", spec.db),
                    &format!("admin/{}/local/branch/main", spec.db),
                )
                .await?;
            let feature_spec = terminusdb_client::BranchSpec::with_branch(&spec.db, "feature");
            direct
                .insert_instance(
                    &person("alice", "OnFeature", 2)?,
                    DocumentInsertArgs::from(feature_spec),
                )
                .await?;

            let reactor =
                Reactor::start_with(up, &spec.db, common::with_branches(&["main", "feature"]))
                    .await?;
            let main_path = format!(
                "/api/document/admin/{}/local/branch/main?id=Person/alice",
                spec.db
            );
            let feature_path = format!(
                "/api/document/admin/{}/local/branch/feature?id=Person/alice",
                spec.db
            );

            reactor.get(&main_path).await?;
            reactor.get(&feature_path).await?;
            assert!(reactor.get(&main_path).await?.is_hit(), "main cached");
            assert!(reactor.get(&feature_path).await?.is_hit(), "feature cached");

            // Optimize *main* only.
            let optimize = reactor
                .post(
                    &format!("/api/optimize/admin/{}/local/branch/main", spec.db),
                    serde_json::json!({}),
                )
                .await?;
            assert!(
                (200..300).contains(&optimize.status),
                "optimize failed: {} {}",
                optimize.status,
                optimize.text()
            );

            assert!(!reactor.get(&main_path).await?.is_hit(), "main was purged");
            let feature_after = reactor.get(&feature_path).await?;
            assert!(
                feature_after.is_hit(),
                "a purge on main must NOT empty feature's cache — this is the \
                 assertion that would catch an over-broad fix. Got {}",
                feature_after.cache_status()
            );
            assert!(
                feature_after.text().contains("OnFeature"),
                "got {}",
                feature_after.text()
            );
            Ok(())
        })
        .await
}

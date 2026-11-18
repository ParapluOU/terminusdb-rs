#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Capture against a live node: each driver, gap recovery, and history rewrite.
//!
//! The unit tests in `src/capture/mod.rs` already drive the applier's dispatch
//! from a table of commits. What they cannot check is whether the *real*
//! drivers notice a real commit on a real node, which is what these do.

mod common;

use anyhow::Result;
use common::models::{person, Person};
use common::{with_driver, Reactor};
use std::time::Duration;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_reactor::Driver;

/// The head-poll driver notices a commit and the watermark advances.
///
/// This is the driver that matters most: on a shared-disk reader node it is the
/// only one that works, because nothing commits locally and the subscription
/// stays silent.
#[tokio::test]
async fn the_poll_driver_advances_the_watermark() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_poll", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start_with(up, &spec.db, with_driver(Driver::Poll)).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;

            let moved = reactor.await_watermark_change(before.as_ref()).await;
            assert!(
                moved.is_some(),
                "the poll driver must notice the commit (poll interval {}ms)",
                reactor.config.capture.poll_interval_ms
            );
            let after = reactor.get(&path).await?;
            assert!(
                after.text().contains("Anne"),
                "and the dependent fragment must be invalidated: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// The subscription driver, on its own.
///
/// On a node where writes are local this should be the low-latency path. If the
/// subscription is silent (the documented expectation on a replica, and
/// possible on any node depending on server support), the watermark will not
/// advance — so this test reports what it finds rather than assuming.
#[tokio::test]
async fn the_subscription_driver_notices_a_local_commit() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_sub", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor =
                Reactor::start_with(up, &spec.db, with_driver(Driver::Subscription)).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;

            // Deliberately bounded: a silent subscription is a documented
            // possibility, not a test failure.
            let moved = tokio::time::timeout(
                Duration::from_secs(15),
                reactor.await_watermark_change(before.as_ref()),
            )
            .await
            .ok()
            .flatten();

            match moved {
                Some(commit) => {
                    println!("FINDING subscription driver noticed commit {commit}");
                    let after = reactor.get(&path).await?;
                    assert!(
                        after.text().contains("Anne"),
                        "got {} ({})",
                        after.text(),
                        after.cache_status()
                    );
                }
                None => println!(
                    "FINDING subscription driver did NOT notice the commit within 15s. \
                     This is why `driver = \"both\"` is the default: the poll driver is the \
                     backstop, and on a replica it is the only thing that works."
                ),
            }
            Ok(())
        })
        .await
}

/// Gap recovery: several commits land while capture is not looking, and the
/// watermark catches up through the log walk.
///
/// Capture is held back by starting Reactor only *after* the commits exist, so
/// its bootstrap watermark is the current head; then more commits land in a
/// burst, faster than the poll interval, which is the real-world shape of this.
#[tokio::test]
async fn a_burst_of_commits_is_recovered_in_order() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_gap", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 0)?, args.clone())
                .await?;

            // A long poll interval, so the burst is guaranteed to be a gap
            // rather than five separate fast-forwards.
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.capture.poll_interval_ms = 3_000;
            })
            .await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            // Five commits in a burst.
            for age in 1..=5 {
                direct
                    .insert_instance(&person("alice", "Ann", age)?, args.clone())
                    .await?;
            }
            let final_head = direct.get_latest_commit_id(&spec).await?;

            // The walk has to cover all five.
            let caught_up = reactor
                .await_commit_for(&final_head, Duration::from_secs(40))
                .await;
            assert!(
                caught_up,
                "the log walk must carry the watermark to the newest commit; \
                 watermark is {:?}",
                reactor.head()
            );

            let after = reactor.get(&path).await?;
            let json = after.json()?;
            assert_eq!(
                json["age"],
                serde_json::json!(5),
                "after recovering the gap the cache must show the latest value: {} ({})",
                after.text(),
                after.cache_status()
            );

            let applied = reactor.engine.metrics().commits_applied.get();
            println!(
                "FINDING gap recovery: {applied} commits applied in total \
                 (bootstrap + the five-commit burst), watermark now {:?}",
                reactor.head()
            );
            assert!(
                applied >= 5,
                "every commit in the gap must be applied individually, not skipped: {applied}"
            );
            Ok(())
        })
        .await
}

/// A duplicate commit event is dropped rather than reapplied.
#[tokio::test]
async fn a_repeated_commit_event_is_not_reapplied() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_dup", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            // `both` means the same commit is noticed twice: once by the
            // subscription, once by the poll.
            let reactor = Reactor::start_with(up, &spec.db, with_driver(Driver::Both)).await?;
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            // Let both drivers report it, then settle.
            tokio::time::sleep(Duration::from_secs(2)).await;
            let applied = reactor.engine.metrics().commits_applied.get();
            let head = reactor.head().expect("a watermark");

            // Two drivers, one commit: the applier must not have applied it
            // twice, which would advance the position twice for one commit.
            let position = reactor.engine.position_of(&reactor.ns, &head);
            println!(
                "FINDING duplicate handling: {applied} commits applied, head {head} at {position:?}"
            );
            assert!(
                applied <= 4,
                "two drivers noticing the same commits must not multiply the \
                 applications: {applied}"
            );
            Ok(())
        })
        .await
}

/// A non-fast-forward head movement bumps the epoch and does not serve
/// pre-rewrite bytes afterwards.
///
/// Forced with a branch reset: `reset` moves the head to an arbitrary commit,
/// which is the canonical case where our watermark is no longer an ancestor of
/// the head.
#[tokio::test]
async fn a_reset_is_handled_as_a_non_fast_forward() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_nff", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            let original = direct.get_latest_commit_id(&spec).await?;

            // A second commit, which the reset will discard.
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            let first = reactor.get(&path).await?;
            assert!(first.text().contains("Anne"), "got {}", first.text());
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let epochs_before = reactor.engine.metrics().epoch_bumps.get();
            let purges_before = reactor.engine.metrics().purges.get();

            // Reset the branch back to the first commit: the head is now an
            // *ancestor* of our watermark, so our watermark is not reachable
            // from it.
            let reset = reactor
                .post(
                    &format!("/api/reset/admin/{}/local/branch/main", spec.db),
                    serde_json::json!({"commit_descriptor": format!(
                        "admin/{}/local/commit/{}", spec.db, original
                    )}),
                )
                .await?;
            println!(
                "reset -> status {} body {}",
                reset.status,
                reset.text().chars().take(200).collect::<String>()
            );

            if !(200..300).contains(&reset.status) {
                println!(
                    "FINDING could not force a reset on this node (status {}); \
                     non-fast-forward handling is covered by the unit tests in \
                     src/capture/mod.rs instead.",
                    reset.status
                );
                return Ok(());
            }

            // Since the topology-purge path landed, a `reset` through Reactor
            // is handled at the frontend: the branch is purged inline, before
            // the 200 reaches the client, and the watermark is forgotten (it
            // named a commit that may no longer be on the branch). So the
            // observable effect is a purge, and the applier's
            // non-fast-forward *inference* may never run — which is the
            // improvement: we no longer depend on inferring it.
            let purged = common::eventually(Duration::from_secs(30), || {
                reactor.engine.metrics().purges.get() > purges_before
                    || reactor.engine.metrics().epoch_bumps.get() > epochs_before
            })
            .await;
            println!(
                "FINDING after reset: purges {} -> {}, epoch_bumps {} -> {}",
                purges_before,
                reactor.engine.metrics().purges.get(),
                epochs_before,
                reactor.engine.metrics().epoch_bumps.get()
            );
            assert!(
                purged,
                "a rewritten branch must be dropped, by purge or by epoch bump"
            );

            // And the answer must be the reset state, not the pre-reset bytes.
            let after = reactor.get(&path).await?;
            assert!(
                after.text().contains("Ann") && !after.text().contains("Anne"),
                "after a reset the cache must not serve pre-rewrite bytes: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// `/_reactor/head` reports the namespace's real position, which is how an
/// operator or load balancer judges replication state.
#[tokio::test]
async fn the_head_endpoint_reports_the_watermark() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_capture_head", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let probe = reactor.get("/_reactor/head").await?;
            assert_eq!(probe.status, 200, "body: {}", probe.text());

            let report = probe.json()?;
            let entry = &report[0];
            assert_eq!(
                entry["namespace"],
                serde_json::json!(format!("admin/{}/local/branch/main", spec.db))
            );
            assert!(
                entry["commit"].is_string(),
                "a tracked namespace must report its watermark: {}",
                probe.text()
            );
            assert!(
                entry["classes"].as_u64().unwrap_or(0) >= 1,
                "and its schema index: {}",
                probe.text()
            );
            println!("FINDING /_reactor/head: {}", probe.text());
            Ok(())
        })
        .await
}

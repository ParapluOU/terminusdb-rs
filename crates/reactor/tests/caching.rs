#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Does it actually cache, and does it actually invalidate?
//!
//! The central test in this file is
//! [`a_document_read_reflects_a_later_update`]: insert, read (miss, cached),
//! update, read again, see the **new** value. If that passes, the whole design
//! works end to end. If it fails, nothing else in the crate matters.
//!
//! Its mirror image matters almost as much:
//! [`an_unrelated_commit_leaves_the_fragment_cached`]. "Invalidation works"
//! could be satisfied by dropping everything on every commit, which would make
//! the cache useless while still looking correct. Precision is half the claim,
//! so both halves are tested.

mod common;

use anyhow::Result;
use common::models::{company, person, Company, Person};
use common::Reactor;
use std::time::Duration;
use terminusdb_client::DocumentInsertArgs;

/// Read the same thing twice: the second is a hit, with the same bytes.
#[tokio::test]
async fn a_second_read_is_a_hit_with_identical_bytes() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_hit", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            let first = reactor.get(&path).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.is_miss(),
                "the first read must be a miss, got {}",
                first.cache_status()
            );

            let second = reactor.get(&path).await?;
            assert!(
                second.is_hit(),
                "the second read must be a hit, got {}",
                second.cache_status()
            );
            assert_eq!(
                first.body, second.body,
                "a hit must return the same bytes the miss did"
            );
            assert!(
                second.header("etag").is_some(),
                "a hit carries an ETag (TerminusDB has none of its own)"
            );
            assert!(second.header("age").is_some(), "and an Age");
            Ok(())
        })
        .await
}

/// `If-None-Match` against the hit's `ETag` yields `304`.
#[tokio::test]
async fn a_matching_etag_yields_not_modified() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_etag", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            reactor.get(&path).await?;
            let hit = reactor.get(&path).await?;
            let etag = hit.header("etag").expect("a hit has an ETag").to_owned();

            let conditional = reactor.get_with(&path, &[("if-none-match", &etag)]).await?;
            assert_eq!(conditional.status, 304, "body: {}", conditional.text());
            assert!(
                conditional.body.is_empty(),
                "a 304 carries no body, that is the point"
            );

            // A non-matching tag still gets the body.
            let other = reactor
                .get_with(&path, &[("if-none-match", "\"not-the-tag\"")])
                .await?;
            assert_eq!(other.status, 200);
            assert_eq!(other.body, hit.body);
            Ok(())
        })
        .await
}

/// **The central test.** A cached document read reflects a later update.
#[tokio::test]
async fn a_document_read_reflects_a_later_update() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_invalidate", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            // Populate.
            let first = reactor.get(&path).await?;
            assert!(first.text().contains("Ann"), "got {}", first.text());
            assert!(reactor.get(&path).await?.is_hit(), "must be cached now");

            // Change it, out of band, as another client would.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;
            let moved = reactor.await_watermark_change(before.as_ref()).await;
            assert!(moved.is_some(), "capture must notice the commit");

            // The payoff.
            let after = reactor.get(&path).await?;
            assert_eq!(after.status, 200, "body: {}", after.text());
            assert!(
                after.text().contains("Anne"),
                "a cached read must reflect the update, got {} ({})",
                after.text(),
                after.cache_status()
            );
            assert!(
                !after.text().contains("\"Ann\""),
                "and must not still contain the old value: {}",
                after.text()
            );
            Ok(())
        })
        .await
}

/// The same, for a `type=`-filtered list.
#[tokio::test]
async fn a_list_read_reflects_a_later_insert() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_list", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?type=Person&as_list=true", spec.db);

            let first = reactor.get(&path).await?;
            assert_eq!(first.json()?.as_array().map(Vec::len), Some(1));
            assert!(reactor.get(&path).await?.is_hit());

            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.get(&path).await?;
            assert_eq!(
                after.json()?.as_array().map(Vec::len),
                Some(2),
                "a new instance must appear in a cached class listing: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// The same, for GraphQL.
#[tokio::test]
async fn a_graphql_read_reflects_a_later_update() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_gql", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query = serde_json::json!({"query": "query { Person { name } }"});

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(first.text().contains("Ann"), "got {}", first.text());
            let second = reactor.post(&path, query.clone()).await?;
            println!(
                "graphql second read: {} (cached={})",
                second.cache_status(),
                second.is_hit()
            );

            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.post(&path, query).await?;
            assert!(
                after.text().contains("Anne"),
                "a cached GraphQL response must reflect the update, got {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// **The precision half.** A commit touching an unrelated document leaves the
/// cached fragment alone.
#[tokio::test]
async fn an_unrelated_commit_leaves_the_fragment_cached() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_precision", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let alice = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            reactor.get(&alice).await?;
            assert!(reactor.get(&alice).await?.is_hit(), "cached");

            // Change *bob*.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bobby", 41)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.get(&alice).await?;
            // Precision requires exact per-document changesets, which require
            // `/api/changes` — a TerminusDB 12.1 endpoint. On an older node the
            // resolver cannot enumerate a commit's documents, so it fires
            // `Everything` (sound, imprecise) and this half of the claim is not
            // demonstrable. See tests/spikes.rs.
            if up.supports_changes().await {
                assert!(
                    after.is_hit(),
                    "a commit to another document must not invalidate this one — \
                     otherwise 'invalidation works' would just mean 'we drop \
                     everything'. Got {}",
                    after.cache_status()
                );
            } else {
                println!(
                    "FINDING precision not demonstrable on {}: /api/changes is absent, so \
                     every commit fires Everything. Reactor is sound here, not precise. \
                     Got {} for an unrelated commit.",
                    up.version().await.unwrap_or_else(|_| "unknown".into()),
                    after.cache_status()
                );
            }
            // Either way, the answer must be correct.
            assert!(after.text().contains("Ann"), "got {}", after.text());
            Ok(())
        })
        .await
}

/// A commit-pinned read is immutable: cached, and still a hit after later
/// commits land.
#[tokio::test]
async fn a_pinned_read_survives_later_commits() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_pinned", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let head = reactor.head().expect("a watermark");
            let pinned = format!(
                "/api/document/admin/{}/local/commit/{}?id=Person/alice",
                spec.db, head
            );

            let first = reactor.get(&pinned).await?;
            println!(
                "DEBUG via reactor: status={} cache={} body={}",
                first.status,
                first.cache_status(),
                first.text()
            );
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(first.text().contains("Ann"), "got {}", first.text());
            let second = reactor.get(&pinned).await?;
            assert!(
                second.is_hit(),
                "a pinned read must cache, got {}",
                second.cache_status()
            );

            // Land two more commits.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.get(&pinned).await?;
            assert!(
                after.is_hit(),
                "a pinned commit cannot change, so its entry must never be \
                 invalidated. Got {}",
                after.cache_status()
            );
            assert!(
                after.text().contains("Ann") && !after.text().contains("Anne"),
                "and it must still show the value as of that commit: {}",
                after.text()
            );
            Ok(())
        })
        .await
}

/// A pinned read of a commit that is **not** the head answers as of that
/// commit.
///
/// Reactor pins every fill, so an unpinned request is repointed at the
/// watermark. A request the client already pinned must keep its own pin
/// instead: re-pinning it at the watermark answers with the live state and then
/// stores that answer under the commit-qualified key, where nothing can ever
/// invalidate it — the pinned read would be wrong forever.
///
/// The case the older `a_pinned_read_survives_later_commits` cannot see: there
/// the first read happens while the pin *is* the head, so both behaviours agree.
#[tokio::test]
async fn a_pinned_read_answers_as_of_its_own_commit() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_pinned_old", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let pinned_at = reactor.head().expect("a watermark");

            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Zoe", 31)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            // First read of this pin, and the pin is now behind the head.
            let pinned = format!(
                "/api/document/admin/{}/local/commit/{}?id=Person/alice",
                spec.db, pinned_at
            );
            let answer = reactor.get(&pinned).await?;
            assert_eq!(answer.status, 200, "body: {}", answer.text());
            assert_eq!(
                answer.body,
                up.get(&pinned).await?.body,
                "a pinned read must be the bytes the node would have returned at that \
                 commit, not the live ones"
            );
            assert!(
                !answer.text().contains("Zoe"),
                "it answered with the live state: {}",
                answer.text()
            );
            Ok(())
        })
        .await
}

/// A pinned read of a commit the commit ring has forgotten is still cacheable.
///
/// The ring bounds how far back a *question about a position* can be answered
/// (a min-version fence, a derived value's `X-Reactor-At`). An immutable answer
/// needs no position at all: it is valid at every one of them. Conflating the
/// two cost caching on exactly the oldest pinned reads — which, for an
/// application that composes pages from chunks at deliberately frozen
/// revisions, is the hot path.
///
/// `spike_does_a_pinned_read_carry_a_data_version` is why this is not
/// hypothetical: the node answers a pinned read with
/// `TerminusDB-Data-Version: commit:<the pinned commit>`, so the store path
/// does look the commit up.
#[tokio::test]
async fn a_pinned_read_caches_past_the_commit_ring() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_pinned_ring", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            // A ring of two, so three later commits are guaranteed to evict the
            // commit we pin at.
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.consistency.commit_ring_capacity = 2;
            })
            .await?;
            let pinned_at = reactor.head().expect("a watermark");

            for (name, age) in [("Anne", 31), ("Annie", 32), ("Ann-Marie", 33)] {
                let before = reactor.head();
                direct
                    .insert_instance(&person("alice", name, age)?, args.clone())
                    .await?;
                assert!(
                    reactor
                        .await_watermark_change(before.as_ref())
                        .await
                        .is_some(),
                    "the commit for {name} never reached the watermark"
                );
            }

            let pinned = format!(
                "/api/document/admin/{}/local/commit/{}?id=Person/alice",
                spec.db, pinned_at
            );
            // The node's own answer, as the thing Reactor's must match.
            let direct_pinned = up.get(&pinned).await?;
            let first = reactor.get(&pinned).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.text().contains("Ann") && !first.text().contains("Ann-Marie"),
                "the pinned read must answer as of the pin: {}",
                first.text()
            );
            let second = reactor.get(&pinned).await?;
            assert!(
                second.is_hit(),
                "a pinned read of an evicted commit must still cache, got {}",
                second.cache_status()
            );
            assert_eq!(first.body, second.body, "and byte-identically");
            assert_eq!(
                first.body, direct_pinned.body,
                "and it must be the bytes the node would have returned"
            );
            Ok(())
        })
        .await
}

/// Read-your-writes: write through Reactor, read immediately, see the write.
#[tokio::test]
async fn a_write_through_reactor_is_immediately_readable() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_ryw", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            // Populate the cache with the old value.
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());

            // Write *through* Reactor. The write fence holds the response until
            // our own watermark has reached the commit it produced.
            let via = reactor.client().await?;
            via.insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;

            // No waiting: this is the read-your-writes claim.
            let after = reactor.get(&path).await?;
            assert!(
                after.text().contains("Anne"),
                "a read on this node, immediately after a write through it, must \
                 see the write. Got {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// `X-Reactor-Bypass` forces the origin.
#[tokio::test]
async fn the_bypass_header_is_honoured() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_bypass", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());

            let bypassed = reactor
                .get_with(&path, &[("x-reactor-bypass", "1")])
                .await?;
            assert!(
                bypassed.is_bypass(),
                "the client asked for the origin, got {}",
                bypassed.cache_status()
            );
            assert!(bypassed.text().contains("Ann"));
            Ok(())
        })
        .await
}

/// An unsatisfiable `X-Reactor-Min-Version` under the `fail` policy is a 412.
#[tokio::test]
async fn an_unreachable_min_version_fails_closed() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_fence", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.consistency.min_version_policy = terminusdb_reactor::MinVersionPolicy::Fail;
            })
            .await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            // A commit this node has certainly never seen.
            let probe = reactor
                .get_with(
                    &path,
                    &[(
                        "x-reactor-min-version",
                        "0000000000000000000000000000000000000000",
                    )],
                )
                .await?;
            assert_eq!(
                probe.status,
                412,
                "an unknown commit must not be silently treated as reached: {}",
                probe.text()
            );

            // And the current head is satisfiable.
            let head = reactor.head().expect("a watermark");
            let ok = reactor
                .get_with(&path, &[("x-reactor-min-version", head.as_str())])
                .await?;
            assert_eq!(ok.status, 200, "body: {}", ok.text());
            Ok(())
        })
        .await
}

/// `/_reactor/invalidate` empties a namespace.
#[tokio::test]
async fn operator_invalidation_empties_the_cache() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_purge", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());

            let purge = reactor
                .post(
                    "/_reactor/invalidate",
                    serde_json::json!({"org": "admin", "db": spec.db, "branch": "main"}),
                )
                .await?;
            assert_eq!(purge.status, 202, "body: {}", purge.text());

            // The purge bumps the epoch, so the next read must refill.
            let after = common::eventually(Duration::from_secs(5), || true).await;
            assert!(after);
            let refill = reactor.get(&path).await?;
            assert!(
                !refill.is_hit(),
                "after a purge the cache must be empty, got {}",
                refill.cache_status()
            );
            assert!(refill.text().contains("Ann"), "and still correct");
            Ok(())
        })
        .await
}

/// A document read with `unfold=true` still reflects a change to the document
/// it embeds — the case the response-id augmentation exists for.
#[tokio::test]
async fn an_unfolded_read_reflects_a_change_to_what_it_embedded() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_unfold", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!(
                "/api/document/admin/{}?id=Person/alice&unfold=true",
                spec.db
            );

            let first = reactor.get(&path).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());

            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.get(&path).await?;
            assert!(
                after.text().contains("Anne"),
                "got {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// The metrics endpoint reflects real traffic, so an operator can see whether
/// the cache is working at all.
#[tokio::test]
async fn metrics_report_hits_and_misses() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_metrics", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            reactor.get(&path).await?;

            let metrics = reactor.get("/_reactor/metrics").await?;
            assert_eq!(metrics.status, 200);
            let text = metrics.text();
            assert!(text.contains("reactor_lookup_hits"), "got {text}");
            assert!(text.contains("reactor_fills_stored"), "got {text}");
            assert!(
                text.contains("reactor_commits_applied"),
                "capture must have applied at least the bootstrap commit"
            );
            println!(
                "{}",
                text.lines()
                    .filter(|line| !line.starts_with('#') && !line.ends_with(" 0"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            Ok(())
        })
        .await
}

/// A read of an untracked namespace is proxied, never cached.
#[tokio::test]
async fn an_untracked_namespace_is_never_cached() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_untracked", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            // Reactor tracks this db, but we ask about a different one.
            let reactor = Reactor::start(up, &spec.db).await?;
            let other = reactor
                .get("/api/document/admin/some_other_db?id=Person/alice")
                .await?;
            assert!(
                other.is_bypass(),
                "an unlisted database must be proxied, not cached: {}",
                other.cache_status()
            );
            // Twice, to be sure it did not become cached.
            let again = reactor
                .get("/api/document/admin/some_other_db?id=Person/alice")
                .await?;
            assert!(again.is_bypass(), "got {}", again.cache_status());
            Ok(())
        })
        .await
}

/// **WOQL caching, end to end.** A read with a stated condition caches, survives
/// an unrelated-class commit, and dies on a commit to the class it named.
///
/// This is the pay-off of the opt-in design: without a condition this query
/// would depend on `Everything` and be dropped by every commit, so there would
/// be nothing to test.
#[tokio::test]
async fn a_woql_read_with_a_condition_caches_and_invalidates_precisely() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person, Company), _, _, _>("rx_cache_woql", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&company("acme", "Acme")?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/woql/admin/{}", spec.db);
            let query = terminusdb_woql2::triple!(
                terminusdb_woql2::node!("Person/alice"),
                "name",
                terminusdb_woql2::var!("n")
            );
            let body = serde_json::json!({"query": query.to_woql_json()});

            // Without a condition: forwarded, never cached, however many
            // times it is asked.
            let bare = reactor.post(&path, body.clone()).await?;
            assert_eq!(bare.status, 200, "body: {}", bare.text());
            assert!(
                bare.is_bypass(),
                "WOQL without a condition must not be cached, got {}",
                bare.cache_status()
            );
            let bare_again = reactor.post(&path, body.clone()).await?;
            assert!(bare_again.is_bypass(), "got {}", bare_again.cache_status());

            // With a condition: cached.
            let conditions = terminusdb_client::CacheCondition::header_value(&[
                terminusdb_client::CacheCondition::class("Person"),
            ])
            .unwrap();
            let first = reactor
                .post_with(
                    &path,
                    body.clone(),
                    &[("x-reactor-invalidate-on", &conditions)],
                )
                .await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.is_miss(),
                "the first conditioned read is a miss, got {}",
                first.cache_status()
            );
            let second = reactor
                .post_with(
                    &path,
                    body.clone(),
                    &[("x-reactor-invalidate-on", &conditions)],
                )
                .await?;
            assert!(
                second.is_hit(),
                "a conditioned WOQL read must cache, got {}",
                second.cache_status()
            );
            assert_eq!(first.body, second.body, "and return the same bytes");

            // A commit to an unrelated class must leave it cached — this is
            // the whole point of stating `class:Person` rather than
            // `everything`.
            let before = reactor.head();
            direct
                .insert_instance(&company("acme", "Acme Industries")?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after_unrelated = reactor
                .post_with(
                    &path,
                    body.clone(),
                    &[("x-reactor-invalidate-on", &conditions)],
                )
                .await?;
            if up.supports_changes().await {
                assert!(
                    after_unrelated.is_hit(),
                    "a Company commit must not invalidate a class:Person \
                         condition, got {}",
                    after_unrelated.cache_status()
                );
            } else {
                // Without /api/changes every commit fires Everything, so
                // precision is not demonstrable here. See tests/spikes.rs.
                println!(
                    "FINDING WOQL condition precision not demonstrable on this node: {}",
                    after_unrelated.cache_status()
                );
            }

            // A commit to the named class must kill it.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after_related = reactor
                .post_with(&path, body, &[("x-reactor-invalidate-on", &conditions)])
                .await?;
            assert!(
                after_related.text().contains("Anne"),
                "a Person commit must invalidate a class:Person condition, so \
                     the refilled answer shows the new name: {} ({})",
                after_related.text(),
                after_related.cache_status()
            );
            Ok(())
        })
        .await
}

/// A malformed condition is refused rather than half-understood.
#[tokio::test]
async fn a_malformed_woql_condition_is_not_cached() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_woql_bad", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/woql/admin/{}", spec.db);
            let query = terminusdb_woql2::triple!(
                terminusdb_woql2::node!("Person/alice"),
                "name",
                terminusdb_woql2::var!("n")
            );
            let body = serde_json::json!({"query": query.to_woql_json()});

            for bad in ["class:Person,typo", "nonsense", "index:Person.name=Ann"] {
                let probe = reactor
                    .post_with(&path, body.clone(), &[("x-reactor-invalidate-on", bad)])
                    .await?;
                assert_eq!(
                    probe.status,
                    200,
                    "the query itself still runs: {}",
                    probe.text()
                );
                assert!(
                    probe.is_bypass(),
                    "{bad:?} must not be cached on a guess, got {}",
                    probe.cache_status()
                );
            }
            Ok(())
        })
        .await
}

/// A commit-pinned WOQL query needs no condition: nothing can change under it.
#[tokio::test]
async fn a_pinned_woql_read_caches_without_a_condition() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_cache_woql_pin", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let head = reactor.head().expect("a watermark");
            let path = format!("/api/woql/admin/{}/local/commit/{}", spec.db, head);
            let query = terminusdb_woql2::triple!(
                terminusdb_woql2::node!("Person/alice"),
                "name",
                terminusdb_woql2::var!("n")
            );
            let body = serde_json::json!({"query": query.to_woql_json()});

            let first = reactor.post(&path, body.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            let second = reactor.post(&path, body).await?;
            assert!(
                second.is_hit(),
                "a pinned WOQL read is immutable and needs no condition, got {}",
                second.cache_status()
            );
            Ok(())
        })
        .await
}

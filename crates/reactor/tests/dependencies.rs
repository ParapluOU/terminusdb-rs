#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! What a cached answer is *allowed* to depend on, against a real node.
//!
//! Two layers of one application pattern are tested here, because the ORM asks
//! TerminusDB two different questions and they must not be conflated:
//!
//! 1. **How many, and which ones?** A count and an id list are statements about
//!    *membership*. Their value cannot change when a field changes, so an entry
//!    that dies on a field change is a bug — and the bug bites hardest exactly
//!    where it hurts most, because paging metadata is re-read on every write.
//! 2. **What is in them?** The documents themselves, batch-loaded by id, do
//!    depend on field values — and, since the list is decomposed, invalidate
//!    one fragment at a time.
//!
//! Every assertion here is about behaviour against a live server: a term set
//! that looks right in a unit test proves nothing about what the origin's diff
//! actually fires.

//! Raw document reads where a test only needs to know whether a document is
//! there; the untyped getter is the right one for that.
#![allow(deprecated)]

mod common;

use anyhow::Result;
use common::models::{person, team, Person, Team};
use common::Reactor;
use terminusdb_client::{DocumentInsertArgs, GetOpts};

/// `{"data":{"_count":N}}` → `N`.
fn count_of(probe: &common::Probe) -> i64 {
    probe
        .json()
        .unwrap_or_default()
        .pointer("/data/_count")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_else(|| panic!("no count in {}", probe.text()))
}

/// A total count depends on how many documents exist, so a field edit must
/// leave it cached — and a create must not.
#[tokio::test]
async fn an_unfiltered_count_survives_a_field_update_and_dies_on_a_create() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_count_members", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query = serde_json::json!({"query": "query { _count(Person: {}) }"});

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert_eq!(count_of(&first), 1);
            let warm = reactor.post(&path, query.clone()).await?;
            assert!(
                warm.is_hit(),
                "the count must be cached before we can test what breaks it, got {}",
                warm.cache_status()
            );

            // A field change to a member. The count is arithmetically unchanged,
            // so the entry must survive it.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after_update = reactor.post(&path, query.clone()).await?;
            assert!(
                after_update.is_hit(),
                "a field update must not discard a total count, got {} ({})",
                after_update.cache_status(),
                after_update.text()
            );
            assert_eq!(count_of(&after_update), 1);

            // A create changes membership, so it must.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after_create = reactor.post(&path, query).await?;
            assert_eq!(
                count_of(&after_create),
                2,
                "a create must be reflected: {} ({})",
                after_create.text(),
                after_create.cache_status()
            );
            Ok(())
        })
        .await
}

/// The other half of membership: a delete.
#[tokio::test]
async fn an_unfiltered_count_dies_on_a_delete() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_count_delete", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query = serde_json::json!({"query": "query { _count(Person: {}) }"});

            assert_eq!(count_of(&reactor.post(&path, query.clone()).await?), 2);
            assert!(reactor.post(&path, query.clone()).await?.is_hit());

            let before = reactor.head();
            direct
                .delete_document(
                    Some("Person/bob"),
                    &spec,
                    "reactor-test",
                    "delete for the count test",
                    "instance",
                    Default::default(),
                )
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.post(&path, query).await?;
            assert_eq!(
                count_of(&after),
                1,
                "a delete must be reflected: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// A filtered count is a value index: a document moving into or out of the
/// bucket must discard it, in both directions.
#[tokio::test]
async fn a_filtered_count_dies_when_the_filtered_property_moves() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_count_filtered", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query =
                serde_json::json!({"query": r#"query { _count(Person: {name: {eq: "Ann"}}) }"#});

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(count_of(&first), 1, "body: {}", first.text());
            assert!(reactor.post(&path, query.clone()).await?.is_hit());

            // *Into* the bucket: Bob becomes an Ann.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Ann", 40)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_in = reactor.post(&path, query.clone()).await?;
            assert_eq!(
                count_of(&after_in),
                2,
                "a document entering the filtered bucket must be reflected: {} ({})",
                after_in.text(),
                after_in.cache_status()
            );

            // And *out* of it again.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bobby", 40)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_out = reactor.post(&path, query).await?;
            assert_eq!(
                count_of(&after_out),
                1,
                "and leaving it: {} ({})",
                after_out.text(),
                after_out.cache_status()
            );
            Ok(())
        })
        .await
}

/// `/api/document?...&counted=true` returns a count, so it gets the same
/// treatment as `_count`.
#[tokio::test]
async fn a_counted_document_read_depends_on_membership_only() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_count_docapi", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!(
                "/api/document/admin/{}?type=Person&counted=true&as_list=true",
                spec.db
            );

            let first = reactor.get(&path).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            let body = first.text();
            assert!(
                body.contains("count"),
                "counted=true must return a count envelope, got {body}"
            );
            let warm = reactor.get(&path).await?;
            assert!(warm.is_hit(), "got {}", warm.cache_status());
            // A count envelope has no `@id`-bearing documents in it, so
            // decomposition declines it and the response is cached whole. That
            // is the harmless path, not an error path.
            assert!(
                reactor.engine.metrics().lists_undecomposable.get() > 0,
                "a count body is expected to report as undecomposable"
            );
            assert_eq!(
                reactor.engine.metrics().compositions_abandoned.get(),
                0,
                "and must not look like a failed composition"
            );

            // A field update leaves it alone.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_update = reactor.get(&path).await?;
            assert!(
                after_update.is_hit(),
                "a field update must not discard a document-API count, got {} ({})",
                after_update.cache_status(),
                after_update.text()
            );
            assert_eq!(after_update.body, first.body);

            // A create does not.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bob", 40)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_create = reactor.get(&path).await?;
            assert_ne!(
                after_create.body,
                first.body,
                "a create must be reflected: {} ({})",
                after_create.text(),
                after_create.cache_status()
            );
            assert_eq!(
                after_create.body,
                up.get(&path).await?.body,
                "and the bytes must still be the origin's"
            );
            Ok(())
        })
        .await
}

/// The ORM's phase one: ids and edges only. A field edit to a returned entity
/// must leave it cached, because no field value is in the response.
#[tokio::test]
async fn an_id_only_graphql_selection_survives_a_field_update() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person, Team), _, _, _>("rx_idonly_survives", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&team("t1", "Alpha", "alice")?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let ids = serde_json::json!({"query": "query { Team { _id lead { _id } } }"});

            let first = reactor.post(&path, ids.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.text().contains("Team/t1") && first.text().contains("Person/alice"),
                "the id list must contain both ends of the edge: {}",
                first.text()
            );
            assert!(reactor.post(&path, ids.clone()).await?.is_hit());

            // A field change to the *root* entity in the list.
            let before = reactor.head();
            direct
                .insert_instance(&team("t1", "Beta", "alice")?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_root = reactor.post(&path, ids.clone()).await?;
            assert!(
                after_root.is_hit(),
                "an id list must survive a field update to a listed document, got {} ({})",
                after_root.cache_status(),
                after_root.text()
            );
            assert_eq!(after_root.body, first.body);

            // And to the *traversed* entity.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Annette", 32)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_target = reactor.post(&path, ids).await?;
            assert!(
                after_target.is_hit(),
                "nor a field update to an edge target, got {} ({})",
                after_target.cache_status(),
                after_target.text()
            );
            assert_eq!(after_target.body, first.body);
            Ok(())
        })
        .await
}

/// The same selection must still die when the *structure* moves: a different
/// edge target, a new member, or a deleted target.
#[tokio::test]
async fn an_id_only_graphql_selection_dies_on_structural_change() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person, Team), _, _, _>("rx_idonly_structure", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for p in [person("alice", "Ann", 30)?, person("bob", "Bob", 40)?] {
                direct.insert_instance(&p, args.clone()).await?;
            }
            direct
                .insert_instance(&team("t1", "Alpha", "alice")?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let ids = serde_json::json!({"query": "query { Team { _id lead { _id } } }"});

            assert!(reactor
                .post(&path, ids.clone())
                .await?
                .text()
                .contains("Person/alice"));
            assert!(reactor.post(&path, ids.clone()).await?.is_hit());

            // Re-point the edge.
            let before = reactor.head();
            direct
                .insert_instance(&team("t1", "Alpha", "bob")?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_edge = reactor.post(&path, ids.clone()).await?;
            assert!(
                after_edge.text().contains("Person/bob"),
                "a re-pointed edge must be reflected: {} ({})",
                after_edge.text(),
                after_edge.cache_status()
            );

            // Add a member.
            let before = reactor.head();
            direct
                .insert_instance(&team("t2", "Gamma", "alice")?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after_members = reactor.post(&path, ids.clone()).await?;
            assert!(
                after_members.text().contains("Team/t2"),
                "a new member must be reflected: {} ({})",
                after_members.text(),
                after_members.cache_status()
            );

            // The whole structure, after both changes: both teams, and the
            // re-pointed edge.
            let cached = reactor.post(&path, ids).await?;
            let text = cached.text();
            for expected in ["Team/t1", "Team/t2", "Person/bob", "Person/alice"] {
                assert!(
                    text.contains(expected),
                    "the refilled id list is missing {expected}: {text}"
                );
            }
            Ok(())
        })
        .await
}

/// One scalar field in the selection puts the field dependence back.
#[tokio::test]
async fn a_mixed_selection_still_dies_on_a_field_update() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person, Team), _, _, _>("rx_idonly_mixed", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&team("t1", "Alpha", "alice")?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let mixed = serde_json::json!({"query": "query { Team { _id name lead { _id } } }"});

            let first = reactor.post(&path, mixed.clone()).await?;
            assert!(first.text().contains("Alpha"), "body: {}", first.text());
            assert!(reactor.post(&path, mixed.clone()).await?.is_hit());

            let before = reactor.head();
            direct
                .insert_instance(&team("t1", "Beta", "alice")?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.post(&path, mixed).await?;
            assert!(
                after.text().contains("Beta"),
                "a selection that returns a field value must reflect its update: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// A cache capped below the working set still answers correctly: eviction costs
/// refills, never bytes.
#[tokio::test]
async fn a_capped_key_index_still_serves_the_origins_bytes() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_key_cap", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for (id, name) in [("alice", "Ann"), ("bob", "Bob"), ("carol", "Carol")] {
                direct
                    .insert_instance(&person(id, name, 30)?, args.clone())
                    .await?;
            }

            // Two keys is below what three decomposed by-id reads need, so
            // every read evicts something.
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.cache.max_tracked_keys = 2;
            })
            .await?;

            let mut expected = Vec::new();
            for id in ["alice", "bob", "carol"] {
                let path = format!("/api/document/admin/{}?id=Person/{id}", spec.db);
                let via = reactor.get(&path).await?;
                assert_eq!(via.status, 200, "body: {}", via.text());
                let origin = up.get(&path).await?.body;
                expected.push((path, origin));
            }

            // Read them all again, in the same order, with the cache thrashing.
            for (path, origin) in &expected {
                let via = reactor.get(path).await?;
                assert_eq!(
                    &via.body,
                    origin,
                    "an evicted key must refill to the origin's bytes ({})",
                    via.cache_status()
                );
            }

            assert!(
                reactor.engine.metrics().keys_evicted.get() > 0,
                "the cap must actually have been hit, or this test proves nothing"
            );
            // And the document itself is still readable through the client.
            let via = reactor.client().await?;
            assert!(via
                .get_document_if_exists("Person/alice", &spec, GetOpts::default())
                .await?
                .is_some());
            Ok(())
        })
        .await
}

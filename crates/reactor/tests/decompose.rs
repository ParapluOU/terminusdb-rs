#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Russian-doll decomposition of document lists, against a live 12.1 node.
//!
//! The test that matters most here is
//! [`composed_lists_are_byte_identical_to_the_origin`]: it composes a list out
//! of cached per-document fragments and compares the result with what the node
//! itself returns, for every framing and projection combination we claim to
//! support. One assertion catches framing slips, ordering changes and separator
//! errors at once, and if it fails nothing else in this file is worth reading.
//!
//! The second is [`one_changed_member_refills_exactly_one_child`], which is the
//! point of the feature rather than its safety: a field change to one member of
//! a list must cost one child refill, not a whole list refetch. It asserts on
//! counters rather than on timing, because a timing assertion would pass on a
//! cache that was quietly doing nothing.

mod common;

use anyhow::Result;
use common::models::{person, Person};
use common::{Reactor, Upstream};
use terminusdb_client::{DeleteOpts, DocumentInsertArgs};

/// The documents every test starts from. Four, so that "a middle one" exists.
const PEOPLE: [(&str, &str, i32); 4] = [
    ("p1", "Ann", 30),
    ("p2", "Bob", 40),
    ("p3", "Cal", 50),
    ("p4", "Dee", 60),
];

/// Query suffixes covering every option combination we claim to compose.
///
/// Measured against the live origin: the default and `minimized=false` are
/// newline-delimited per-document renderings, `as_list=true` wraps them in an
/// array, and `unfold=true` changes the documents but not the joining. Nothing
/// in Reactor hardcodes any of that — the framing is learned from the response —
/// so this list is here to prove the learning, not to enumerate special cases.
const FRAMINGS: [&str; 4] = ["", "&minimized=false", "&as_list=true", "&unfold=true"];

/// Seed the four people.
async fn seed(
    direct: &terminusdb_client::TerminusDBHttpClient,
    args: &DocumentInsertArgs,
) -> Result<()> {
    for (id, name, age) in PEOPLE {
        direct
            .insert_instance(&person(id, name, age)?, args.clone())
            .await?;
    }
    Ok(())
}

/// Read the same path through Reactor and straight from the node, and insist
/// the bytes agree.
async fn assert_matches_origin(reactor: &Reactor, up: &Upstream, path: &str) -> Result<()> {
    let through = reactor.get(path).await?;
    let direct = up.get(path).await?;
    assert_eq!(through.status, direct.status, "{path}: status");
    assert_eq!(
        through.body,
        direct.body,
        "{path}: composed through Reactor ({}) differs from the origin\n\
         reactor: {:?}\n\
         origin:  {:?}",
        through.cache_status(),
        through.text(),
        direct.text(),
    );
    Ok(())
}

/// The `@id`s of a list body, in the order the body gives them.
fn order(body: &bytes::Bytes) -> Vec<String> {
    terminusdb_reactor::decompose(body)
        .expect("a document list")
        .members
        .iter()
        .map(|m| m.id.as_str().to_owned())
        .collect()
}

/// **The test to look at first.** A composed response is the origin's bytes.
#[tokio::test]
async fn composed_lists_are_byte_identical_to_the_origin() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_bytes", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            for framing in FRAMINGS {
                let path = format!("/api/document/admin/{}?type=Person{}", spec.db, framing);

                // The first read relays the origin's own bytes, so this half
                // only checks the proxy. It is the cheap half.
                let first = reactor.get(&path).await?;
                assert!(
                    first.is_miss(),
                    "{path}: first read {}",
                    first.cache_status()
                );
                assert_matches_origin(&reactor, up, &path).await?;

                // The second read is assembled out of per-document fragments.
                // This is the half that can go wrong.
                let composed = reactor.get(&path).await?;
                assert!(
                    composed.is_hit() && composed.cache_status().contains("composed"),
                    "{path} must be composed from children, got {}",
                    composed.cache_status()
                );
                assert_eq!(
                    composed.body,
                    up.get(&path).await?.body,
                    "{path}: a composed body must equal the origin's, byte for byte"
                );
                assert_eq!(
                    order(&composed.body),
                    vec!["Person/p1", "Person/p2", "Person/p3", "Person/p4"],
                    "{path}: and in the origin's order"
                );
            }
            Ok(())
        })
        .await
}

/// A document's bytes are the same whether it came from a list or from a by-id
/// read.
///
/// This is the assumption that lets one child fragment serve both, and
/// therefore the assumption that makes a list read warm a by-id read. It is a
/// claim about TerminusDB, not about Reactor, so it is asserted against the
/// node directly — if the origin ever starts rendering a document differently
/// inside an array, this fails here rather than silently serving the wrong
/// wrapper somewhere else.
#[tokio::test]
async fn the_per_document_rendering_does_not_depend_on_framing() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_framing", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            for framing in FRAMINGS {
                let list = up
                    .get(&format!(
                        "/api/document/admin/{}?type=Person{}",
                        spec.db, framing
                    ))
                    .await?;
                let listed = terminusdb_reactor::decompose(&list.body).expect("a document list");

                for member in &listed.members {
                    let single = up
                        .get(&format!(
                            "/api/document/admin/{}?id={}{}",
                            spec.db,
                            member.id.as_str(),
                            framing
                        ))
                        .await?;
                    let alone =
                        terminusdb_reactor::decompose(&single.body).expect("a single document");
                    assert_eq!(alone.members.len(), 1);
                    assert_eq!(
                        alone.members[0].bytes,
                        member.bytes,
                        "{}{}: a document's bytes must not depend on what it is wrapped in\n\
                         in a list:  {:?}\n\
                         on its own: {:?}",
                        member.id.as_str(),
                        framing,
                        String::from_utf8_lossy(&member.bytes),
                        String::from_utf8_lossy(&alone.members[0].bytes),
                    );
                }
            }
            Ok(())
        })
        .await
}

/// **The point of the feature.** One changed member costs one child refill.
#[tokio::test]
async fn one_changed_member_refills_exactly_one_child() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_one", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?type=Person", spec.db);

            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "cached and decomposed");

            let metrics = reactor.engine.metrics();
            let batches = metrics.child_batches.get();
            let filled = metrics.children_filled.get();
            let abandoned = metrics.compositions_abandoned.get();

            // Change exactly one member — the second of four, so that an
            // ordering mistake has somewhere to show.
            let before = reactor.head();
            direct
                .insert_instance(&person("p2", "Bobby", 41)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let after = reactor.get(&path).await?;
            assert_eq!(
                after.body,
                up.get(&path).await?.body,
                "the recomposed list must equal the origin's ({})",
                after.cache_status()
            );
            assert!(after.text().contains("Bobby"), "got {}", after.text());
            assert_eq!(
                metrics.compositions_abandoned.get(),
                abandoned,
                "no composition should have been given up on"
            );

            if up.supports_changes().await {
                assert_eq!(
                    metrics.children_filled.get() - filled,
                    1,
                    "one member changed, so exactly one child may be refilled — \
                     four would mean the parent was dirtied and the whole list \
                     refetched, which is the cost this feature exists to remove"
                );
                assert_eq!(
                    metrics.child_batches.get() - batches,
                    1,
                    "and in one upstream request"
                );
            } else {
                println!(
                    "FINDING precision not demonstrable on this node: /api/changes is \
                     absent, so every commit fires Everything. {} children refilled.",
                    metrics.children_filled.get() - filled
                );
            }
            Ok(())
        })
        .await
}

/// The origin's list order does not move when a member's field changes.
///
/// The parent fragment depends on membership and nothing else, which is only
/// sound if order is a function of membership. This asserts that against the
/// node, with Reactor out of the picture entirely.
#[tokio::test]
async fn the_origin_keeps_its_order_when_a_middle_document_changes() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_order", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;
            let path = format!("/api/document/admin/{}?type=Person", spec.db);

            let before = order(&up.get(&path).await?.body);
            direct
                .insert_instance(&person("p2", "Bobby", 41)?, args.clone())
                .await?;
            let after = order(&up.get(&path).await?.body);
            assert_eq!(
                before, after,
                "a field update moved the origin's list order. The parent fragment's \
                 membership-only dependency assumes it does not, so this needs a \
                 different parent dependency rather than a workaround."
            );

            // And once more after changing the first and last, since a reordering
            // that only shows at the edges would be the easy thing to miss.
            direct
                .insert_instance(&person("p1", "Annie", 31)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("p4", "Deedee", 61)?, args)
                .await?;
            assert_eq!(before, order(&up.get(&path).await?.body));
            Ok(())
        })
        .await
}

/// A list read and a by-id read share one child fragment, both ways round.
#[tokio::test]
async fn a_list_read_and_a_by_id_read_warm_each_other() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_share", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let list = format!("/api/document/admin/{}?type=Person", spec.db);
            let by_id = |id: &str| format!("/api/document/admin/{}?id=Person/{}", spec.db, id);

            // The list populates a child fragment per member.
            reactor.get(&list).await?;
            // One by-id read, which is what teaches Reactor how this shape
            // wraps a single document. Framing is a property of the request's
            // parameters, so one observation covers every document.
            reactor.get(&by_id("p1")).await?;

            // p3 has never been read on its own, and yet:
            let shared = reactor.get(&by_id("p3")).await?;
            assert!(
                shared.is_hit(),
                "a member warmed by the list read must be a hit on its first by-id \
                 read, got {}",
                shared.cache_status()
            );
            assert_eq!(
                shared.body,
                up.get(&by_id("p3")).await?.body,
                "and byte-identical to the origin ({})",
                shared.cache_status()
            );

            // The reverse: after a change, the ONE batch the list's
            // recomposition issued also answers the by-id read.
            let metrics = reactor.engine.metrics();
            let before = reactor.head();
            direct
                .insert_instance(&person("p3", "Callum", 51)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let refreshed = reactor.get(&list).await?;
            assert_eq!(refreshed.body, up.get(&list).await?.body);
            let batches = metrics.child_batches.get();

            let after = reactor.get(&by_id("p3")).await?;
            assert_eq!(
                after.body,
                up.get(&by_id("p3")).await?.body,
                "the by-id read must see the update too ({})",
                after.cache_status()
            );
            if up.supports_changes().await {
                assert!(after.is_hit(), "got {}", after.cache_status());
                assert_eq!(
                    metrics.child_batches.get(),
                    batches,
                    "the list's refill already fetched this document; the by-id read \
                     must not fetch it again"
                );
            }
            Ok(())
        })
        .await
}

/// Several missing children cost **one** upstream request, not one each.
#[tokio::test]
async fn a_parent_with_many_missing_children_issues_one_request() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_batch", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?type=Person", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());

            let metrics = reactor.engine.metrics();
            let batches = metrics.child_batches.get();
            let filled = metrics.children_filled.get();

            // Three separate commits, so three children go dirty without the
            // parent's membership moving.
            for (id, name, age) in [
                ("p1", "Annie", 31),
                ("p2", "Bobby", 41),
                ("p3", "Cally", 51),
            ] {
                let before = reactor.head();
                direct
                    .insert_instance(&person(id, name, age)?, args.clone())
                    .await?;
                assert!(reactor
                    .await_watermark_change(before.as_ref())
                    .await
                    .is_some());
            }

            // One read, which must resolve all three in one go.
            let after = reactor.get(&path).await?;
            assert_eq!(
                after.body,
                up.get(&path).await?.body,
                "three changed members, still the origin's bytes ({})",
                after.cache_status()
            );
            for name in ["Annie", "Bobby", "Cally", "Dee"] {
                assert!(
                    after.text().contains(name),
                    "missing {name}: {}",
                    after.text()
                );
            }
            if up.supports_changes().await {
                assert_eq!(
                    metrics.child_batches.get() - batches,
                    1,
                    "N missing children must cost ONE upstream request; an N+1 \
                     fan-out against the node would dominate every other cost"
                );
                assert_eq!(metrics.children_filled.get() - filled, 3);
            }
            Ok(())
        })
        .await
}

/// Inserting and deleting a member invalidates the parent, and a delete leaves
/// no dangling child behind.
#[tokio::test]
async fn membership_changes_invalidate_the_parent() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_members", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?type=Person", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());
            let metrics = reactor.engine.metrics();
            let abandoned = metrics.compositions_abandoned.get();

            // Insert.
            let before = reactor.head();
            direct
                .insert_instance(&person("p5", "Eve", 70)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let grown = reactor.get(&path).await?;
            assert_eq!(
                grown.body,
                up.get(&path).await?.body,
                "a new member must appear ({})",
                grown.cache_status()
            );
            assert_eq!(order(&grown.body).len(), 5, "got {}", grown.text());

            // And delete. A parent still holding p5 would either serve stale
            // bytes or fail to compose; neither is allowed.
            assert!(reactor.get(&path).await?.is_hit(), "cached again");
            let before = reactor.head();
            direct
                .delete_instance(&person("p5", "Eve", 70)?, args, DeleteOpts::default())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let shrunk = reactor.get(&path).await?;
            assert_eq!(
                shrunk.body,
                up.get(&path).await?.body,
                "a deleted member must be gone ({})",
                shrunk.cache_status()
            );
            assert_eq!(order(&shrunk.body).len(), 4, "got {}", shrunk.text());
            assert!(
                !shrunk.text().contains("Person/p5"),
                "a deleted member must not survive as a dangling child: {}",
                shrunk.text()
            );
            assert_eq!(
                metrics.compositions_abandoned.get(),
                abandoned,
                "and this must be handled by invalidating the parent, not by \
                 abandoning a half-composed answer"
            );
            Ok(())
        })
        .await
}

/// A refill whose content did not actually move leaves the `ETag` alone.
///
/// Backdating, end to end and visible to a client. The composed response's
/// identity is derived from the children's hashes, so a child refilled back to
/// bytes it has held before inherits its old hash and the whole list's `ETag`
/// returns to what it was.
#[tokio::test]
async fn an_unchanged_refill_restores_the_etag() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_backdate", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?type=Person", spec.db);
            reactor.get(&path).await?;
            let original = reactor.get(&path).await?;
            let etag = original
                .header("etag")
                .expect("a composed hit carries an ETag")
                .to_owned();

            // Change p2, then change it back.
            let mut heads = Vec::new();
            for (name, age) in [("Bobby", 41), ("Bob", 40)] {
                let before = reactor.head();
                direct
                    .insert_instance(&person("p2", name, age)?, args.clone())
                    .await?;
                assert!(reactor
                    .await_watermark_change(before.as_ref())
                    .await
                    .is_some());
                heads.push(reactor.get(&path).await?);
            }

            assert_ne!(
                heads[0].header("etag"),
                Some(etag.as_str()),
                "a real change must move the ETag"
            );
            assert_eq!(
                heads[1].header("etag"),
                Some(etag.as_str()),
                "and reverting it must bring the ETag back: the composed identity \
                 comes from the children's hashes, not from the spliced bytes"
            );
            assert_eq!(heads[1].body, original.body);
            assert_eq!(heads[1].body, up.get(&path).await?.body);

            // Which a client can act on.
            let conditional = reactor.get_with(&path, &[("if-none-match", &etag)]).await?;
            assert_eq!(conditional.status, 304, "body: {}", conditional.text());
            Ok(())
        })
        .await
}

/// With decomposition off, everything still works as whole-response caching.
///
/// The escape hatch has to be real: if byte fidelity ever bites, turning this
/// off must leave a correct cache, not a broken one.
#[tokio::test]
async fn whole_response_caching_still_works_with_decomposition_off() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_off", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;

            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.cache.decompose = false;
            })
            .await?;
            let metrics = reactor.engine.metrics();

            for framing in FRAMINGS {
                let path = format!("/api/document/admin/{}?type=Person{}", spec.db, framing);
                assert!(reactor.get(&path).await?.is_miss(), "{path}");
                let hit = reactor.get(&path).await?;
                assert!(hit.is_hit(), "{path}: {}", hit.cache_status());
                assert!(
                    !hit.cache_status().contains("composed"),
                    "{path} must not be composed: {}",
                    hit.cache_status()
                );
                assert_eq!(hit.body, up.get(&path).await?.body, "{path}");
            }

            // And invalidation still reaches it.
            let path = format!("/api/document/admin/{}?type=Person", spec.db);
            let by_id = format!("/api/document/admin/{}?id=Person/p2", spec.db);
            reactor.get(&by_id).await?;
            assert!(reactor.get(&by_id).await?.is_hit());

            let before = reactor.head();
            direct
                .insert_instance(&person("p2", "Bobby", 41)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            assert_eq!(reactor.get(&path).await?.body, up.get(&path).await?.body);
            assert_eq!(reactor.get(&by_id).await?.body, up.get(&by_id).await?.body);

            assert_eq!(
                metrics.lists_decomposed.get(),
                0,
                "nothing may be decomposed with the switch off"
            );
            assert_eq!(metrics.lists_composed.get(), 0);
            assert_eq!(metrics.child_batches.get(), 0);
            Ok(())
        })
        .await
}

/// The origin wants `ids` as a JSON array, and a comma-separated list is a 400.
///
/// MEASURED, and the reason this test exists: the batched child refill is the
/// whole N+1 defence, so the form it sends has to be the form the server takes.
/// `?ids=Person/p1,Person/p2` is rejected with `api:JSONInvalid` /
/// `illegal_json`; `?ids=["Person/p1","Person/p2"]` returns both documents,
/// newline-delimited exactly as a `?type=` list does.
///
/// It also pins the *parsing* side. `terminusdb-client` never sends ids through
/// the query string at all — `get_instances` POSTs them in a JSON body with
/// `X-HTTP-Method-Override: GET` — so nothing else in this workspace exercises
/// the query form, and an analyzer that mis-parses it produces an entry keyed on
/// a term no commit can fire: cached, never invalidated, stale forever.
#[tokio::test]
async fn the_origin_wants_ids_as_a_json_array() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_dc_idform", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            seed(&direct, &args).await?;
            let ids = vec![
                terminusdb_reactor::DocId::parse("Person/p1"),
                terminusdb_reactor::DocId::parse("Person/p2"),
            ];

            // What Reactor builds is accepted, and returns both documents.
            let query = terminusdb_reactor::analyze::projection_query(&[], &ids);
            let ours = up
                .get(&format!("/api/document/admin/{}?{}", spec.db, query))
                .await?;
            assert_eq!(ours.status, 200, "{query} -> {}", ours.text());
            let parts = terminusdb_reactor::decompose(&ours.body).expect("a document list");
            assert_eq!(parts.ids(), ids);

            // And the form it would be tempting to send is not.
            let naive = up
                .get(&format!(
                    "/api/document/admin/{}?ids=Person/p1,Person/p2",
                    spec.db
                ))
                .await?;
            assert_eq!(
                naive.status,
                400,
                "a comma-separated ids list is rejected, so Reactor must not send one: {}",
                naive.text()
            );

            // The per-document bytes are the same as a class listing's, which is
            // what makes a refilled child spliceable into a cached list.
            let listed = up
                .get(&format!("/api/document/admin/{}?type=Person", spec.db))
                .await?;
            let listed = terminusdb_reactor::decompose(&listed.body).expect("a document list");
            assert_eq!(parts.members[0].bytes, listed.members[0].bytes);
            assert_eq!(parts.members[1].bytes, listed.members[1].bytes);
            Ok(())
        })
        .await
}

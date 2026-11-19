//! `/api/document` analysis.
//!
//! The simplest and highest-volume route, and the one where the parameter
//! whitelist does the real work. Every parameter that can change the response
//! body has to be in the key, so an *unrecognized* parameter cannot be
//! tolerated: if TerminusDB grows a new switch that reshapes the body and we do
//! not know about it, two different responses would collide on one key. The
//! only safe response to an unknown parameter is to stop caching.

use crate::analyze::{
    is_read_method, Augment, CanonicalRequest, Decompose, ListSpec, Plan, ReqView,
};
use crate::ids::{DocId, Namespace};
use crate::schema_index::SchemaIndex;
use crate::term::Term;

use super::AnalyzeConfig;

/// Query parameters we understand on `/api/document`.
///
/// Taken from `GetOpts` and the client's URL builder. `author`, `message`,
/// `full_replace` and `merge_repeats` only appear on writes, but they are
/// listed so that a write carrying them is still classified as a write rather
/// than bailing out as unanalyzable.
pub const RECOGNIZED_DOCUMENT_PARAMS: &[&str] = &[
    "id",
    "ids",
    "type",
    "graph_type",
    "as_list",
    "unfold",
    "skip",
    "count",
    "minimized",
    "raw_json",
    "prefixed",
    "compress_ids",
    "format",
    "identifiers",
    "id_prefix",
    "raw_id",
    "xsd",
    "author",
    "message",
    "full_replace",
    "merge_repeats",
    "counted",
    // Write-side parameters the client's URL builder sends. They never affect a
    // cached read's shape, but listing them keeps a write from looking
    // unanalyzable.
    "create",
    "nuke",
    // History/diff read parameters (`/api/document` history forms).
    "start",
    "verbose",
    "created",
    "updated",
    "before",
    "after",
    "diff",
    "fast",
    "complete",
];

/// Plan a `/api/document` request.
pub(super) fn plan(
    req: &ReqView,
    ns: &Namespace,
    schema: &SchemaIndex,
    _cfg: &AnalyzeConfig,
) -> Plan {
    let params: Vec<(String, String)> = url::form_urlencoded::parse(req.query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    // Writes are classified FIRST, before the parameter whitelist.
    //
    // The whitelist protects *cache keys*: a parameter we do not model might
    // reshape a response in a way the shape hash misses. A write has no cache
    // key, so the whitelist has nothing to protect there — and bypassing a
    // write costs us the capture hint, which is what read-your-writes depends
    // on. Found by `tests/caching.rs`: `insert_instance` sends `create=...`,
    // which was missing from the list, so every insert through Reactor
    // bypassed, no commit hint was sent, no write fence was applied, and the
    // very next read was served from the stale cache.
    //
    // Note that TerminusDB 12.1 also accepts POST for some *reads*;
    // classifying those as writes is wasteful but sound — we forward them and
    // tell capture to look for a commit, which simply finds none.
    if !is_read_method(req.method) {
        return Plan::Write;
    }

    for (name, _) in &params {
        if !RECOGNIZED_DOCUMENT_PARAMS.contains(&name.as_str()) {
            // Not "ignore it": a parameter we do not model may change the body
            // in a way the shape hash misses, which would mean serving one
            // request's bytes for another.
            return Plan::bypass("unrecognized document parameter");
        }
    }

    let get = |name: &str| -> Option<&str> {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };

    let canonical = CanonicalRequest::with_params("document", ns, params.clone(), req.body, None);

    // The schema graph, not the instance graph.
    if get("graph_type") == Some("schema") {
        return Plan::Read {
            key: canonical.request_key(ns, None),
            terms: vec![Term::Schema],
            immutable: false,
            rewrite: None,
            augment: Augment::None,
            // Class definitions are not documents with a `Doc` identity in the
            // instance graph, and a schema change invalidates every shape in
            // the namespace anyway, so there is nothing for a child to absorb.
            decompose: Decompose::No,
        };
    }

    // `unfold=true` embeds linked documents in the response, and which
    // documents those are is not derivable from the request: it depends on the
    // target classes' `@unfoldable` annotation and on the data. So the read set
    // has to be completed from the response ids.
    let unfolded = matches!(get("unfold"), Some("true"));
    let augment = if unfolded {
        Augment::DocIds
    } else {
        Augment::None
    };

    let ids: Vec<DocId> = get("ids")
        .map(crate::analyze::parse_ids)
        .unwrap_or_default();

    // The spine of a decomposed read, built once and used by every selector
    // below that names documents.
    let spine = |named: &[DocId]| {
        Decompose::Documents(ListSpec {
            child_shape: canonical.projection_shape_id(),
            projection: canonical.projection_params(),
            parent_terms: existence_terms(named),
            named: named.to_vec(),
        })
    };

    match (get("id"), get("type")) {
        // One document. Rooted at the document, so two spellings of the same
        // request for the same projection share one entry.
        //
        // Decomposed as a one-member list, which is not pedantry: it is what
        // puts the document's bytes under the *projection* key that a `?type=`
        // list also uses, so a by-id read warms the list and the list warms the
        // by-id read. The framing a by-id read wraps the document in (a
        // trailing newline, or `as_list`'s array) then lives on the parent,
        // where it belongs.
        (Some(id), None) if ids.is_empty() => {
            let id = DocId::parse(id);
            Plan::Read {
                key: canonical.doc_key(ns, id.clone()),
                terms: vec![Term::Doc(id.clone())],
                immutable: false,
                rewrite: None,
                augment,
                decompose: spine(std::slice::from_ref(&id)),
            }
        }
        // A list of named documents.
        (None, None) if !ids.is_empty() => Plan::Read {
            key: canonical.request_key(ns, None),
            terms: ids.iter().cloned().map(Term::Doc).collect(),
            immutable: false,
            rewrite: None,
            augment,
            decompose: spine(&ids),
        },
        // Every instance of a class. `Members` covers creates and deletes,
        // `Class` covers updates to any instance — including instances of
        // subclasses, which the type query also returns.
        (None, Some(class)) if ids.is_empty() => {
            let class = crate::ids::ClassName::new(class);
            let mut terms = vec![Term::Members(class.clone()), Term::Class(class.clone())];
            // The parent of a decomposed list depends on membership ONLY. See
            // `ListSpec::parent_terms`: adding `Class` here would make every
            // field change refetch the list, which is the whole thing this
            // feature exists to stop.
            let mut parent = vec![Term::Members(class.clone())];
            if schema.is_empty() {
                // Without a hierarchy we cannot know which subclasses the
                // query also returns, and `Class`/`Members` only match a class
                // and its known descendants. Fall back to the one term that is
                // unconditionally sound. The children stay precise, so a
                // composed read still costs one `?ids=` refill rather than a
                // full list refetch.
                terms.push(Term::Everything);
                parent.push(Term::Everything);
            }
            Plan::Read {
                key: canonical.request_key(ns, None),
                terms,
                immutable: false,
                rewrite: None,
                augment,
                decompose: Decompose::Documents(ListSpec {
                    child_shape: canonical.projection_shape_id(),
                    projection: canonical.projection_params(),
                    parent_terms: parent,
                    // A class listing's membership is the origin's to state.
                    named: Vec::new(),
                }),
            }
        }
        // Everything in the instance graph. There is no term for "all classes",
        // and enumerating the schema's classes would miss a class added by a
        // later commit, so this is genuinely an `Everything` dependency.
        //
        // Not decomposed: the parent would depend on `Everything` whatever we
        // did, so it is refetched upstream on every commit and a spine over it
        // would cost a split for no saved request.
        (None, None) if ids.is_empty() => Plan::Read {
            key: canonical.request_key(ns, None),
            terms: vec![Term::Everything],
            immutable: false,
            rewrite: None,
            augment,
            decompose: Decompose::No,
        },
        // `id` together with `type`/`ids` is not a shape we model.
        _ => Plan::bypass("ambiguous document selector"),
    }
}

/// The parent dependency for a read that names its documents by id: "do these
/// documents still exist, and is this still the set that comes back".
///
/// `Members(class)` rather than `Doc(id)` on purpose. `Doc(id)` fires on a mere
/// field change, which would dirty the parent and force a full refetch — the
/// exact cost decomposition removes. What the parent's content actually depends
/// on is existence, and only a create or a delete changes that, which is what
/// `Members` fires on. An id with no class prefix has no class whose membership
/// we could name, so it falls back to `Doc(id)`: coarser for that one entry,
/// and still sound.
fn existence_terms(ids: &[DocId]) -> Vec<Term> {
    let mut terms: Vec<Term> = ids
        .iter()
        .map(|id| match id.class() {
            Some(class) => Term::Members(class),
            None => Term::Doc(id.clone()),
        })
        .collect();
    terms.sort();
    terms.dedup();
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::{plan as analyze, Plan, ReqView, Target};
    use crate::engine::RootKey;
    use crate::testutil::{cls, doc, schema_fixture};
    use bytes::Bytes;
    use http::{HeaderMap, Method};

    fn plan_get(query: &str) -> Plan {
        plan_with(&Method::GET, query, &Bytes::new())
    }

    fn plan_with(method: &Method, query: &str, body: &Bytes) -> Plan {
        let headers = HeaderMap::new();
        let req = ReqView::new(method, "/api/document/admin/mydb", query, &headers, body);
        analyze(&req, &schema_fixture(), &AnalyzeConfig::default())
    }

    #[test]
    fn id_depends_on_that_one_document() {
        match plan_get("id=Person/alice") {
            Plan::Read {
                terms,
                key,
                immutable,
                augment,
                ..
            } => {
                assert_eq!(terms, vec![Term::Doc(doc("Person/alice"))]);
                assert_eq!(key.root, RootKey::Doc(doc("Person/alice")));
                assert!(!immutable);
                assert_eq!(augment, Augment::None);
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_full_iri_id_normalizes_to_the_short_form() {
        match plan_get("id=terminusdb%3A%2F%2F%2Fdata%2FPerson%2Falice") {
            Plan::Read { terms, .. } => {
                assert_eq!(terms, vec![Term::Doc(doc("Person/alice"))]);
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn type_depends_on_membership_and_the_class() {
        match plan_get("type=Person") {
            Plan::Read { terms, key, .. } => {
                assert_eq!(
                    terms,
                    vec![Term::Members(cls("Person")), Term::Class(cls("Person"))]
                );
                assert!(matches!(key.root, RootKey::Request(_)));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_type_query_with_no_schema_falls_back_to_everything() {
        let headers = HeaderMap::new();
        let body = Bytes::new();
        let req = ReqView::new(
            &Method::GET,
            "/api/document/admin/mydb",
            "type=Person",
            &headers,
            &body,
        );
        match analyze(&req, &SchemaIndex::default(), &AnalyzeConfig::default()) {
            Plan::Read { terms, .. } => assert!(
                terms.contains(&Term::Everything),
                "without a hierarchy we cannot know which subclasses are returned"
            ),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn graph_type_schema_depends_on_the_schema() {
        match plan_get("graph_type=schema") {
            Plan::Read { terms, .. } => assert_eq!(terms, vec![Term::Schema]),
            other => panic!("expected a read, got {other:?}"),
        }
        // And with a type filter it is still the schema that it reads.
        match plan_get("graph_type=schema&type=Person") {
            Plan::Read { terms, .. } => assert_eq!(terms, vec![Term::Schema]),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn an_unfiltered_list_depends_on_everything() {
        match plan_get("as_list=true") {
            Plan::Read { terms, .. } => assert_eq!(terms, vec![Term::Everything]),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn multiple_ids_depend_on_each_document() {
        // The JSON-array form is the one the server accepts, and therefore the
        // one that must parse. REGRESSION: it used to be split on commas, which
        // produced `Doc("[\"Person/alice\"")` — a term no commit can fire, so
        // the entry was cached and then never invalidated. Stale forever, and
        // invisible.
        for query in [
            "ids=%5B%22Person%2Falice%22%2C%22Person%2Fbob%22%5D",
            "ids=Person/alice,Person/bob",
        ] {
            match plan_get(query) {
                Plan::Read { terms, .. } => assert_eq!(
                    terms,
                    vec![Term::Doc(doc("Person/alice")), Term::Doc(doc("Person/bob"))],
                    "{query}"
                ),
                other => panic!("expected a read for {query}, got {other:?}"),
            }
        }
    }

    #[test]
    fn an_ids_array_round_trips_through_the_query_we_send() {
        // What we build for a child refill must be what we would parse back.
        let ids = vec![doc("Person/alice"), doc("Person/bob")];
        let rendered = crate::analyze::render_ids(&ids);
        assert_eq!(rendered, r#"["Person/alice","Person/bob"]"#);
        assert_eq!(crate::analyze::parse_ids(&rendered), ids);
        // Including ids a comma split would have mangled.
        let awkward = vec![doc("Person/hello, world")];
        assert_eq!(
            crate::analyze::parse_ids(&crate::analyze::render_ids(&awkward)),
            awkward
        );
    }

    #[test]
    fn unfold_requires_response_augmentation() {
        // An unfoldable link embeds another document, whose identity only the
        // response reveals.
        match plan_get("id=Person/alice&unfold=true") {
            Plan::Read { augment, terms, .. } => {
                assert_eq!(augment, Augment::DocIds);
                assert_eq!(terms, vec![Term::Doc(doc("Person/alice"))]);
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn an_unrecognized_parameter_bypasses() {
        for query in [
            "id=Person/alice&surprise=1",
            "future_switch=true",
            "id=Person/alice&unfold=true&experimental=yes",
        ] {
            assert!(
                matches!(plan_get(query), Plan::Bypass { .. }),
                "{query} must bypass"
            );
        }
    }

    #[test]
    fn every_recognized_parameter_is_accepted() {
        for name in RECOGNIZED_DOCUMENT_PARAMS {
            // `ids` and `type` are selectors in their own right and conflict
            // with `id`; they have their own tests.
            let query = match *name {
                "ids" | "type" => format!("{name}=Person/alice"),
                _ => format!("id=Person/alice&{name}=x"),
            };
            assert!(
                !matches!(plan_get(&query), Plan::Bypass { .. }),
                "{name} should be recognized"
            );
        }
    }

    #[test]
    fn writes_are_writes() {
        for method in [Method::POST, Method::PUT, Method::DELETE, Method::PATCH] {
            assert_eq!(
                plan_with(&method, "author=me&message=hi", &Bytes::from_static(b"{}")),
                Plan::Write,
                "{method} must be a write"
            );
        }
    }

    #[test]
    fn a_write_is_a_write_even_with_an_unknown_parameter() {
        // Regression: the whitelist used to run first, so a write carrying a
        // parameter we did not model (`create=true`, which every
        // `insert_instance` sends) was bypassed — losing the capture hint and
        // with it read-your-writes. A write has no cache key, so there is
        // nothing for the whitelist to protect.
        assert_eq!(
            plan_with(&Method::POST, "surprise=1", &Bytes::from_static(b"{}")),
            Plan::Write
        );
    }

    #[test]
    fn the_client_s_own_write_parameters_are_recognized() {
        // Exactly what `insert_instance` sends.
        assert_eq!(
            plan_with(
                &Method::POST,
                "author=me&message=hi&graph_type=instance&create=true",
                &Bytes::from_static(b"{}")
            ),
            Plan::Write
        );
    }

    #[test]
    fn an_ambiguous_selector_bypasses() {
        assert!(matches!(
            plan_get("id=Person/alice&type=Person"),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn shape_differs_by_projection_but_not_by_document() {
        let key_of = |query: &str| match plan_get(query) {
            Plan::Read { key, .. } => key,
            other => panic!("expected a read, got {other:?}"),
        };
        let alice = key_of("id=Person/alice&minimized=true");
        let bob = key_of("id=Person/bob&minimized=true");
        let alice_verbose = key_of("id=Person/alice&minimized=false");

        assert_eq!(alice.shape, bob.shape);
        assert_ne!(alice.root, bob.root);
        assert_ne!(alice.shape, alice_verbose.shape);
    }

    /// The spec of a decomposed read, or a panic saying what we got instead.
    fn spec_of(query: &str) -> ListSpec {
        match plan_get(query) {
            Plan::Read { decompose, .. } => match decompose {
                Decompose::Documents(spec) => spec,
                Decompose::No => panic!("{query} should be decomposed"),
            },
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_class_list_parent_depends_on_membership_but_never_on_the_class() {
        // THE subtlest property in the whole feature. `Class(Person)` fires on
        // every field change to every Person, so a parent holding it would be
        // refetched by exactly the change decomposition exists to absorb — and
        // the cache would look correct while doing nothing. The whole-response
        // read set still carries `Class`, because a whole-response entry really
        // does depend on it; only the spine is narrower.
        let spec = spec_of("type=Person");
        assert_eq!(spec.parent_terms, vec![Term::Members(cls("Person"))]);
        assert!(
            !spec.parent_terms.contains(&Term::Class(cls("Person"))),
            "a membership spine must not depend on the class"
        );
        match plan_get("type=Person") {
            Plan::Read { terms, .. } => assert!(
                terms.contains(&Term::Class(cls("Person"))),
                "the undecomposed fallback still needs Class"
            ),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_parent_that_names_its_documents_depends_on_their_existence() {
        // `Doc(id)` would fire on a field change; what the parent's content
        // actually depends on is whether the document is still there.
        assert_eq!(
            spec_of("ids=Person/alice,Person/bob").parent_terms,
            vec![Term::Members(cls("Person"))],
            "one Members term covers both ids, deduplicated"
        );
        assert_eq!(
            spec_of("id=Person/alice").parent_terms,
            vec![Term::Members(cls("Person"))]
        );
        assert_eq!(
            spec_of("ids=Person/alice,Company/acme").parent_terms,
            vec![Term::Members(cls("Company")), Term::Members(cls("Person"))]
        );
        // An id with no class prefix has no membership to name, so it falls
        // back to the coarser-for-this-entry but sound `Doc`.
        assert_eq!(
            spec_of("id=loose").parent_terms,
            vec![Term::Doc(doc("loose"))]
        );
    }

    #[test]
    fn a_by_id_read_and_a_class_list_share_one_child_shape() {
        // The sharing property the feature is for: a list read warms by-id
        // reads and vice versa, which only happens if both compute the same
        // child key.
        assert_eq!(
            spec_of("id=Person/alice").child_shape,
            spec_of("type=Person").child_shape
        );
        assert_eq!(
            spec_of("ids=Person/alice,Person/bob").child_shape,
            spec_of("type=Person").child_shape
        );
        // Framing is the parent's business, so it must not move the child.
        assert_eq!(
            spec_of("type=Person&as_list=true").child_shape,
            spec_of("type=Person").child_shape
        );
        assert_eq!(
            spec_of("type=Person&skip=10&count=5").child_shape,
            spec_of("type=Person").child_shape
        );
    }

    #[test]
    fn the_child_shape_separates_different_document_renderings() {
        // And the other half: anything that changes how one document renders
        // MUST change the child key, or two projections would collide on one
        // entry and we would serve one request's bytes for another.
        let base = spec_of("type=Person").child_shape;
        for query in [
            "type=Person&minimized=false",
            "type=Person&unfold=true",
            "type=Person&compress_ids=false",
            "type=Person&identifiers=true",
            "type=Person&raw_id=true",
            "type=Person&xsd=true",
            "type=Person&format=markdown",
            "type=Person&prefixed=false",
            "type=Person&raw_json=true",
        ] {
            assert_ne!(spec_of(query).child_shape, base, "{query}");
        }
    }

    #[test]
    fn a_child_shape_never_collides_with_a_whole_response_shape() {
        // The two hashes are tagged differently on purpose: a child fragment
        // and a whole-response entry for the same document would otherwise be
        // one slot holding two different bodies.
        match plan_get("id=Person/alice") {
            Plan::Read { key, decompose, .. } => {
                let spec = decompose.spec().expect("decomposed");
                assert_ne!(key.shape, spec.child_shape);
                assert_eq!(key.root, RootKey::Doc(doc("Person/alice")));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_child_refill_query_carries_only_the_projection() {
        let spec = spec_of("type=Person&as_list=true&minimized=false&skip=3&count=2");
        let query = crate::analyze::projection_query(
            &spec.projection,
            &[doc("Person/alice"), doc("Person/bob")],
        );
        assert_eq!(
            query, "minimized=false&ids=%5B%22Person%2Falice%22%2C%22Person%2Fbob%22%5D",
            "the document API wants a JSON array here, not a comma-separated list"
        );
        assert!(
            !query.contains("as_list") && !query.contains("skip") && !query.contains("count"),
            "framing and windowing would change which documents come back: {query}"
        );
    }

    #[test]
    fn the_shapes_we_do_not_decompose_say_so() {
        // The schema graph is not a sequence of instance documents, and an
        // unfiltered list already depends on `Everything`, so its parent would
        // be refetched on every commit regardless.
        for query in ["graph_type=schema", "as_list=true", ""] {
            match plan_get(query) {
                Plan::Read { decompose, .. } => {
                    assert_eq!(decompose, Decompose::No, "{query:?}")
                }
                other => panic!("expected a read for {query:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_document_endpoint_only_claims_document_paths() {
        // Sanity: the target parser and the route table agree.
        assert_eq!(
            Target::parse("/api/document/admin/mydb").endpoint(),
            "document"
        );
    }
}

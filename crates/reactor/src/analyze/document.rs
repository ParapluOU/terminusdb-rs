//! `/api/document` analysis.
//!
//! The simplest and highest-volume route, and the one where the parameter
//! whitelist does the real work. Every parameter that can change the response
//! body has to be in the key, so an *unrecognized* parameter cannot be
//! tolerated: if TerminusDB grows a new switch that reshapes the body and we do
//! not know about it, two different responses would collide on one key. The
//! only safe response to an unknown parameter is to stop caching.

use crate::analyze::{is_read_method, Augment, CanonicalRequest, Plan, ReqView};
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

    for (name, _) in &params {
        if !RECOGNIZED_DOCUMENT_PARAMS.contains(&name.as_str()) {
            // Not "ignore it": a parameter we do not model may change the body
            // in a way the shape hash misses, which would mean serving one
            // request's bytes for another.
            return Plan::bypass("unrecognized document parameter");
        }
    }

    // Writes: POST (insert), PUT (replace), DELETE. Note that TerminusDB 12.1
    // also accepts POST for some *reads*; classifying those as writes is
    // wasteful but sound — we forward them and tell capture to look for a
    // commit, which simply finds none.
    if !is_read_method(req.method) {
        return Plan::Write;
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
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(DocId::parse)
                .collect()
        })
        .unwrap_or_default();

    match (get("id"), get("type")) {
        // One document. Rooted at the document, so two spellings of the same
        // request for the same projection share one entry.
        (Some(id), None) if ids.is_empty() => {
            let id = DocId::parse(id);
            Plan::Read {
                key: canonical.doc_key(ns, id.clone()),
                terms: vec![Term::Doc(id)],
                immutable: false,
                rewrite: None,
                augment,
            }
        }
        // A list of named documents.
        (None, None) if !ids.is_empty() => Plan::Read {
            key: canonical.request_key(ns, None),
            terms: ids.into_iter().map(Term::Doc).collect(),
            immutable: false,
            rewrite: None,
            augment,
        },
        // Every instance of a class. `Members` covers creates and deletes,
        // `Class` covers updates to any instance — including instances of
        // subclasses, which the type query also returns.
        (None, Some(class)) if ids.is_empty() => {
            let class = crate::ids::ClassName::new(class);
            let mut terms = vec![Term::Members(class.clone()), Term::Class(class.clone())];
            if schema.is_empty() {
                // Without a hierarchy we cannot know which subclasses the
                // query also returns, and `Class` only matches a class and its
                // known descendants. Fall back to the one term that is
                // unconditionally sound.
                terms.push(Term::Everything);
            }
            Plan::Read {
                key: canonical.request_key(ns, None),
                terms,
                immutable: false,
                rewrite: None,
                augment,
            }
        }
        // Everything in the instance graph. There is no term for "all classes",
        // and enumerating the schema's classes would miss a class added by a
        // later commit, so this is genuinely an `Everything` dependency.
        (None, None) if ids.is_empty() => Plan::Read {
            key: canonical.request_key(ns, None),
            terms: vec![Term::Everything],
            immutable: false,
            rewrite: None,
            augment,
        },
        // `id` together with `type`/`ids` is not a shape we model.
        _ => Plan::bypass("ambiguous document selector"),
    }
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
        match plan_get("ids=Person/alice,Person/bob") {
            Plan::Read { terms, .. } => assert_eq!(
                terms,
                vec![Term::Doc(doc("Person/alice")), Term::Doc(doc("Person/bob"))]
            ),
            other => panic!("expected a read, got {other:?}"),
        }
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
    fn a_write_with_an_unknown_parameter_still_bypasses() {
        // The parameter check runs first on purpose: we would rather forward
        // without a capture hint than mis-key anything.
        assert!(matches!(
            plan_with(&Method::POST, "surprise=1", &Bytes::from_static(b"{}")),
            Plan::Bypass { .. }
        ));
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

    #[test]
    fn the_document_endpoint_only_claims_document_paths() {
        // Sanity: the target parser and the route table agree.
        assert_eq!(
            Target::parse("/api/document/admin/mydb").endpoint(),
            "document"
        );
    }
}

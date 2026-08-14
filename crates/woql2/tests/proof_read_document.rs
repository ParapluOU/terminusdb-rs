//! WOQL `ReadDocument` (increment 2b) — end-to-end, adversarial, and reject-boundary tests.
//!
//! Builds a TWO-graph fixture (instance + schema) under a combined `database_root` (copying the
//! IsA/Subsumption two-layer setup + the cons-chain helpers) and proves a whole document — scalars,
//! a link, a set, and an ordered list — then verifies + resolves it against the trusted root. The
//! adversarial cases each MUST fail verify; the reject cases each reject-close at plan/prove with the
//! specific `BgpPlanError`.
//!
//! Proof-heavy paths run on a large stack (deep envelope / list-walk recursion). Fixtures are kept
//! small (a 3-element list) to bound proving time.

mod proof_common;
use proof_common::iri;

use std::sync::Arc;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminus_store::proof::commitment::{database_root, LayerCommitment};
use terminus_store::proof::CanonicalObject;
use terminus_store::{open_sync_memory_store, ValueTriple};

use terminusdb_woql2::document::ReadDocument;
use terminusdb_woql2::proof::{
    plan_read_document, BgpPlanError, CompiledReadDocument, ProvedLinkTarget, QueryProofError,
    ResolvedValue,
};
use terminusdb_woql2::value::{NodeValue, Value};

// ---------------------------------------------------------------------------------------------------
// Well-known IRIs (real RDF/sys/xsd IRIs — NOT `iri()`-wrapped).
// ---------------------------------------------------------------------------------------------------

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
const SYS_CLASS: &str = "http://terminusdb.com/schema/sys#Class";
const SYS_SET: &str = "http://terminusdb.com/schema/sys#Set";
const SYS_LIST: &str = "http://terminusdb.com/schema/sys#List";
const SYS_ARRAY: &str = "http://terminusdb.com/schema/sys#Array";
const SYS_CLASS_PRED: &str = "http://terminusdb.com/schema/sys#class";
const SYS_SUBDOC: &str = "http://terminusdb.com/schema/sys#subdocument";
const SYS_UNFOLDABLE: &str = "http://terminusdb.com/schema/sys#unfoldable";
const SYS_UNFOLD: &str = "http://terminusdb.com/schema/sys#unfold";
const SYS_INHERITS: &str = "http://terminusdb.com/schema/sys#inherits";
const SYS_ONEOF: &str = "http://terminusdb.com/schema/sys#oneOf";
const SYS_MIN_CARD: &str = "http://terminusdb.com/schema/sys#min_cardinality";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

// ---------------------------------------------------------------------------------------------------
// Layer building.
// ---------------------------------------------------------------------------------------------------

/// One triple to seed: a node/node/node edge, or a node/predicate/string-literal.
enum Fact {
    Node(String, String, String),
    Str(String, String, String),
}

fn layer(facts: Vec<Fact>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for fact in &facts {
        match fact {
            Fact::Node(s, p, o) => builder
                .add_value_triple(ValueTriple::new_node(s, p, o))
                .unwrap(),
            Fact::Str(s, p, o) => builder
                .add_value_triple(ValueTriple::new_string_value(s, p, o))
                .unwrap(),
        }
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// The instance-side cons chain `t(subject, predicate, [elements...])` (node elements). Mirrors the
/// increment-2a `chain_triples`: cells `cell0, cell1, …` with `rdf:first`/`rdf:rest`/`rdf:nil`; no
/// `rdf:type rdf:List` needed (the store walk relies on chain topology only).
fn cons_chain(subject: &str, predicate: &str, elements: &[&str], facts: &mut Vec<Fact>) {
    if elements.is_empty() {
        facts.push(Fact::Node(subject.to_string(), predicate.to_string(), RDF_NIL.to_string()));
        return;
    }
    facts.push(Fact::Node(subject.to_string(), predicate.to_string(), iri("cell0")));
    for (i, element) in elements.iter().enumerate() {
        facts.push(Fact::Node(iri(&format!("cell{i}")), RDF_FIRST.to_string(), iri(element)));
        let rest = if i + 1 < elements.len() {
            iri(&format!("cell{}", i + 1))
        } else {
            RDF_NIL.to_string()
        };
        facts.push(Fact::Node(iri(&format!("cell{i}")), RDF_REST.to_string(), rest));
    }
}

/// The FULL happy-path schema: class `C` with a scalar (`name: xsd:string`), a link (`author: Person`),
/// a set (`tags: {xsd:string}`), and a list (`tracks: [Track]`). No inheritance, no subdocuments.
fn full_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("tags"), iri("TagsSet")),
        Fact::Node(iri("TagsSet"), RDF_TYPE.into(), SYS_SET.into()),
        Fact::Node(iri("TagsSet"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("tracks"), iri("TracksList")),
        Fact::Node(iri("TracksList"), RDF_TYPE.into(), SYS_LIST.into()),
        Fact::Node(iri("TracksList"), SYS_CLASS_PRED.into(), iri("Track")),
        Fact::Node(iri("Track"), RDF_TYPE.into(), SYS_CLASS.into()),
    ])
}

/// The FULL happy-path instance: `doc` of class `C` with `name="hello"`, `author=person1`,
/// `tags={"x","y"}`, `tracks=[a,b,a]` (order + duplicate).
fn full_instance() -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("name"), "hello".into()),
        Fact::Node(iri("doc"), iri("author"), iri("person1")),
        Fact::Str(iri("doc"), iri("tags"), "x".into()),
        Fact::Str(iri("doc"), iri("tags"), "y".into()),
    ];
    cons_chain(&iri("doc"), &iri("tracks"), &["a", "b", "a"], &mut facts);
    layer(facts)
}

/// A FLAT schema (scalar + link only, no list) — for the light (no-walk) adversarial cases.
fn flat_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
    ])
}

fn flat_instance() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("name"), "hello".into()),
        Fact::Node(iri("doc"), iri("author"), iri("person1")),
    ])
}

fn compiled_read(id: &str) -> CompiledReadDocument {
    plan_read_document(&ReadDocument {
        identifier: NodeValue::Node(id.to_string()),
        document: Value::Variable("doc".to_string()),
    })
    .unwrap()
}

fn on_big_stack(name: &str, mb: usize, body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(mb * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}

fn as_value_lexical(object: &CanonicalObject) -> Vec<u8> {
    match object {
        CanonicalObject::Value { lexical, .. } => lexical.clone(),
        other => panic!("expected a literal value, got {other:?}"),
    }
}

fn as_node(object: &CanonicalObject) -> Vec<u8> {
    match object {
        CanonicalObject::Node(bytes) => bytes.clone(),
        other => panic!("expected a node, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------
// E2E happy path + adversarial (missing walk, tampered walk) — one full-bundle proof.
// ---------------------------------------------------------------------------------------------------

#[test]
fn read_document_flat_and_list_end_to_end() {
    on_big_stack("read-document-e2e", 1024, || {
        let instance = full_instance();
        let schema = full_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        // Prove the whole document (subject BGP + frame BGP + 4 range BGPs + 1 ordered-list walk).
        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();

        // ---- HAPPY PATH: verify + resolve against the trusted combined root. ----
        let resolved = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();

        assert_eq!(resolved.id, iri("doc").into_bytes());
        assert_eq!(resolved.class, iri("C").into_bytes());

        // Scalar `name` — one string literal, exactly as stored.
        match resolved.get(&iri("name")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_value_lexical(&objects[0]), b"hello".to_vec());
            }
            other => panic!("name should be Direct, got {other:?}"),
        }

        // Link `author` — one node (the link target id, edge only).
        match resolved.get(&iri("author")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_node(&objects[0]), iri("person1").into_bytes());
            }
            other => panic!("author should be Direct, got {other:?}"),
        }

        // Set `tags` — the complete, unordered {"x","y"} multiset from the bound-subject BGP.
        match resolved.get(&iri("tags")).unwrap() {
            ResolvedValue::Direct(objects) => {
                let mut lexicals: Vec<Vec<u8>> =
                    objects.iter().map(as_value_lexical).collect();
                lexicals.sort();
                assert_eq!(lexicals, vec![b"x".to_vec(), b"y".to_vec()]);
            }
            other => panic!("tags should be Direct, got {other:?}"),
        }

        // List `tracks` — the EXACT ordered element sequence `[a, b, a]` (order + duplicate preserved),
        // rendered from the authenticated OrderedListWalk (NOT the raw cons head).
        match resolved.get(&iri("tracks")).unwrap() {
            ResolvedValue::List(elements) => {
                let nodes: Vec<Vec<u8>> = elements.iter().map(as_node).collect();
                assert_eq!(
                    nodes,
                    vec![
                        iri("a").into_bytes(),
                        iri("b").into_bytes(),
                        iri("a").into_bytes()
                    ],
                );
            }
            other => panic!("tracks should be a List, got {other:?}"),
        }

        // ---- ADVERSARIAL 3: tamper the delivered list order / forge the walk envelope. ----
        // Corrupt one committed element id in the list-walk sub-proof: the store walk re-verify (bound
        // to the trusted instance root and the (subject, predicate) digest) rejects the swap.
        bundle.lists[0].walk.result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a tampered list-walk result column must fail verify",
        );

        // ---- ADVERSARIAL 2: deliver the list property WITHOUT its walk (raw cons head only). ----
        // Drop every list-walk sub-proof. The frame authenticates `tracks` as a LIST and D still
        // carries its single head edge, so verify DEMANDS a walk and reject-closes rather than
        // rendering the raw head.
        bundle.lists.clear();
        let err = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap_err();
        assert!(
            matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentMalformedListHead { .. })),
            "a list property with no walk must reject as MalformedListHead, got {err:?}",
        );
    });
}

// ---------------------------------------------------------------------------------------------------
// Adversarial (light, no walk): tampered D, injected instance property.
// ---------------------------------------------------------------------------------------------------

#[test]
fn read_document_tampered_direct_triples_fail_verify() {
    on_big_stack("read-document-tamper-d", 512, || {
        let instance = flat_instance();
        let schema = flat_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();
        // Sanity: the honest flat document verifies.
        bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();

        // Corrupt one committed value of the COMPLETE direct-triple set: a prover cannot deliver a
        // partial/altered document — the bound-subject BGP re-verify rejects it.
        bundle.subject_bgp.result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a tampered direct-triple column must fail verify",
        );
    });
}

#[test]
fn read_document_injected_instance_property_fails_verify() {
    on_big_stack("read-document-injection", 512, || {
        // The instance carries an EXTRA triple `t(doc, ghost, "boom")` whose predicate is not in `C`'s
        // frame (schema non-conformance / injection). The prover CAN assemble a bundle (D is honest,
        // ghost is not a list), but verify computes the complete authenticated D, sees `ghost` is not
        // frame-classified, and reject-closes — the no-injection guarantee.
        let instance = layer(vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Str(iri("doc"), iri("name"), "hello".into()),
            Fact::Str(iri("doc"), iri("ghost"), "boom".into()),
        ]);
        let schema = flat_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        let bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();
        let err = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap_err();
        assert!(
            matches!(
                err,
                QueryProofError::Plan(BgpPlanError::ReadDocumentUnknownInstanceProperty { .. })
            ),
            "an un-framed instance property must reject as UnknownInstanceProperty, got {err:?}",
        );
    });
}

// ---------------------------------------------------------------------------------------------------
// Reject boundaries (plan / prove reject-closed with the specific error).
// ---------------------------------------------------------------------------------------------------

#[test]
fn read_document_rejects_variable_identifier() {
    // Plan-level: no layers needed.
    let read = ReadDocument {
        identifier: NodeValue::Variable("x".to_string()),
        document: Value::Variable("d".to_string()),
    };
    assert_eq!(
        plan_read_document(&read).unwrap_err(),
        BgpPlanError::ReadDocumentUnboundIdentifier,
    );
}

/// Prove `doc` against a schema whose single property has the given range shape, expecting a
/// reject-closed prove with `expected`.
fn assert_prove_rejects(
    name: &'static str,
    schema_facts: Vec<Fact>,
    instance_facts: Vec<Fact>,
    check: impl Fn(&QueryProofError) -> bool + Send + 'static,
) {
    on_big_stack(name, 512, move || {
        let instance = layer(instance_facts);
        let schema = layer(schema_facts);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled.prove_on_layers(&*instance, &*schema).unwrap_err();
        assert!(check(&err), "unexpected error for {name}: {err:?}");
    });
}

/// Like [`assert_prove_rejects`] but for an INHERITING class: the prover proves the reflexive
/// `sys:inherits` closure (an envelope recursion) BEFORE `classify_frame` rejects, so it runs on the
/// larger envelope stack.
fn assert_inherited_prove_rejects(
    name: &'static str,
    schema_facts: Vec<Fact>,
    instance_facts: Vec<Fact>,
    check: impl Fn(&QueryProofError) -> bool + Send + 'static,
) {
    on_big_stack(name, 1024, move || {
        let instance = layer(instance_facts);
        let schema = layer(schema_facts);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled.prove_on_layers(&*instance, &*schema).unwrap_err();
        assert!(check(&err), "unexpected error for {name}: {err:?}");
    });
}

/// The TOP-LEVEL `id`'s OWN class is a subdocument. A subdocument is inlined INTO a parent (increment
/// 3), never read directly by id, so it reject-closes (the repurposed `ReadDocumentSubdocumentDeferred`
/// — the old subdocument-RANGE reject was removed because a subdocument range is now inlined per-child).
#[test]
fn read_document_rejects_top_level_subdocument() {
    assert_prove_rejects(
        "rd-top-level-subdocument",
        vec![
            Fact::Node(iri("Meta"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Meta"), SYS_SUBDOC.into(), RDF_NIL.into()),
            Fact::Node(iri("Meta"), iri("title"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Meta")),
            Fact::Str(iri("doc"), iri("title"), "hi".into()),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentSubdocumentDeferred)),
    );
}

#[test]
fn read_document_rejects_array_range() {
    assert_prove_rejects(
        "rd-array",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("cells"), iri("CellsArray")),
            Fact::Node(iri("CellsArray"), RDF_TYPE.into(), SYS_ARRAY.into()),
            Fact::Node(iri("CellsArray"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("cells"), iri("acell")),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentArrayDeferred)),
    );
}

/// An INHERITED subdocument nature. `Super` bears `sys:subdocument` and `Sub` inherits it; because
/// subdocument-ness is inheritance-closed (`schema_is_subdocument`, schema.pl:654), `Sub` IS a
/// subdocument → reject. (Repurposed from the old `read_document_rejects_inherited_class`: an inheriting
/// class is no longer a blanket reject — see `read_document_inherited_frame_end_to_end`.)
#[test]
fn read_document_rejects_inherited_subdocument() {
    assert_inherited_prove_rejects(
        "rd-inherited-subdoc",
        vec![
            Fact::Node(iri("Super"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Super"), SYS_SUBDOC.into(), RDF_NIL.into()),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_INHERITS.into(), iri("Super")),
            Fact::Node(iri("Sub"), iri("name"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
            Fact::Str(iri("doc"), iri("name"), "hi".into()),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentSubdocumentDeferred)),
    );
}

/// An INHERITED tagged-union nature. `Super` bears `sys:oneOf` and `Sub` inherits it. A tagged-union
/// ancestor's oneOf-CHOICE properties flow into `Sub`'s frame through a `sys:oneOf`-group node the
/// direct frame BGP does not enumerate, so rendering `Sub` without them would omit inherited data — the
/// frame refuses ANY `sys:oneOf`/tagged-union nature anywhere in the chain (fail-closed).
#[test]
fn read_document_rejects_inherited_tagged_union() {
    assert_inherited_prove_rejects(
        "rd-inherited-oneof",
        vec![
            Fact::Node(iri("Super"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Super"), SYS_ONEOF.into(), iri("ChoiceDesc")),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_INHERITS.into(), iri("Super")),
            Fact::Node(iri("Sub"), iri("name"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
            Fact::Str(iri("doc"), iri("name"), "hi".into()),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "tagged-union"
            })
        ),
    );
}

/// A CONFLICTING inherited range: `Super` declares `p` with a scalar range (`xsd:string`) and `Sub`
/// re-declares `p` with a LINK range (`Person`). A validated schema cannot produce this, so the effective
/// frame's cross-closure union reject-closes rather than pick one arbitrarily.
#[test]
fn read_document_rejects_conflicting_inherited_range() {
    assert_inherited_prove_rejects(
        "rd-conflicting-inherited-range",
        vec![
            Fact::Node(iri("Super"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Super"), iri("p"), XSD_STRING.into()),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_INHERITS.into(), iri("Super")),
            Fact::Node(iri("Sub"), iri("p"), iri("Person")),
            Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
            Fact::Node(iri("doc"), iri("p"), iri("person1")),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "conflicting-inherited-range"
            })
        ),
    );
}

#[test]
fn read_document_rejects_cardinality_bearing_set() {
    assert_prove_rejects(
        "rd-cardinality-set",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("tags"), iri("BoundedSet")),
            Fact::Node(iri("BoundedSet"), RDF_TYPE.into(), SYS_SET.into()),
            Fact::Node(iri("BoundedSet"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
            // Presence of a cardinality bound is what distinguishes a `set(Class,N,M)` descriptor.
            Fact::Str(iri("BoundedSet"), SYS_MIN_CARD.into(), "1".into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Str(iri("doc"), iri("tags"), "x".into()),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "cardinality-set"
            })
        ),
    );
}

#[test]
fn read_document_rejects_conflicting_class_and_collection_range() {
    // Defense-in-depth precedence guard. The range node `WeirdRange` is MALFORMED: it claims to be
    // BOTH a system class (`sys:Class`) AND a collection (`sys:List`), and even carries a valid list
    // element class (`sys:class Track`). The server settles the system-class identity FIRST (it would
    // treat this as a class/link); this classifier checks collections first, so WITHOUT the guard it
    // would have accepted it as a `List`. Such a node cannot arise in a validated schema, so v1
    // reject-closes rather than diverge from the server.
    assert_prove_rejects(
        "rd-conflicting-class-collection",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("weird"), iri("WeirdRange")),
            Fact::Node(iri("WeirdRange"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("WeirdRange"), RDF_TYPE.into(), SYS_LIST.into()),
            Fact::Node(iri("WeirdRange"), SYS_CLASS_PRED.into(), iri("Track")),
            Fact::Node(iri("Track"), RDF_TYPE.into(), SYS_CLASS.into()),
        ],
        // `doc` needs >= 2 direct triples so the instance subject BGP satisfies the store's
        // minimum-rows-per-proof constraint and prove reaches frame classification (where the guard
        // fires). The `weird` edge also makes the conflicting property genuinely present.
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("weird"), iri("target1")),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "conflicting-class-and-collection"
            })
        ),
    );
}

#[test]
fn read_document_rejects_one_of_tagged_union() {
    assert_prove_rejects(
        "rd-oneof",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), SYS_ONEOF.into(), iri("ChoiceDesc")),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Str(iri("doc"), iri("name"), "hi".into()),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "tagged-union"
            })
        ),
    );
}

// ---------------------------------------------------------------------------------------------------
// Inheritance: the effective (inherited-union) frame, proved via the reflexive `sys:inherits` closure.
// ---------------------------------------------------------------------------------------------------

/// A TWO-LEVEL chain `Sub -> Mid -> Super`. `Super` declares a scalar (`name`) and a `List` (`tracks`),
/// `Mid` declares a scalar (`rating`), and `Sub` declares a link (`author`). No subdocuments; the
/// link/element target classes (`Person`, `Track`) are plain non-inheriting classes.
fn inherited_schema() -> Arc<LayerCommitment> {
    layer(vec![
        // Super: an inherited scalar + an inherited list.
        Fact::Node(iri("Super"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Super"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("Super"), iri("tracks"), iri("TracksList")),
        Fact::Node(iri("TracksList"), RDF_TYPE.into(), SYS_LIST.into()),
        Fact::Node(iri("TracksList"), SYS_CLASS_PRED.into(), iri("Track")),
        Fact::Node(iri("Track"), RDF_TYPE.into(), SYS_CLASS.into()),
        // Mid: inherits Super, adds a scalar.
        Fact::Node(iri("Mid"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Mid"), SYS_INHERITS.into(), iri("Super")),
        Fact::Node(iri("Mid"), iri("rating"), XSD_STRING.into()),
        // Sub: inherits Mid, adds a link.
        Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Sub"), SYS_INHERITS.into(), iri("Mid")),
        Fact::Node(iri("Sub"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
    ])
}

/// A `doc` of class `Sub` that uses an inherited scalar (`name`, from `Super`), a second inherited
/// scalar (`rating`, from `Mid`), its OWN link (`author`), and an inherited ordered `List`
/// (`tracks=[a,b,a]`, from `Super`).
fn inherited_instance() -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
        Fact::Str(iri("doc"), iri("name"), "hello".into()),
        Fact::Str(iri("doc"), iri("rating"), "five".into()),
        Fact::Node(iri("doc"), iri("author"), iri("person1")),
    ];
    cons_chain(&iri("doc"), &iri("tracks"), &["a", "b", "a"], &mut facts);
    layer(facts)
}

/// A LIGHT single-level inherited schema (no list): `Sub -> Super`, `Super` declares a scalar
/// (`name`), `Sub` declares a link (`author`). Used by the no-walk adversarial cases.
fn flat_inherited_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("Super"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Super"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Sub"), SYS_INHERITS.into(), iri("Super")),
        Fact::Node(iri("Sub"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
    ])
}

fn flat_inherited_instance() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
        Fact::Str(iri("doc"), iri("name"), "hi".into()),
        Fact::Node(iri("doc"), iri("author"), iri("person1")),
    ])
}

/// The headline: a `Sub -> Mid -> Super` document renders inherited scalars, an inherited ordered list
/// (order + duplicate preserved), and an own link — the effective frame is the UNION across the whole
/// super-chain, proved via the AUTHENTICATED reflexive `sys:inherits` closure. Then two adversarial
/// tampers on the same bundle each fail verify.
#[test]
fn read_document_inherited_frame_end_to_end() {
    on_big_stack("read-document-inherited-e2e", 1024, || {
        let instance = inherited_instance();
        let schema = inherited_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();

        // ---- HAPPY PATH: verify + resolve the whole inherited document. ----
        let resolved = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();

        assert_eq!(resolved.id, iri("doc").into_bytes());
        assert_eq!(resolved.class, iri("Sub").into_bytes());

        // Inherited scalar `name` (declared on Super).
        match resolved.get(&iri("name")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_value_lexical(&objects[0]), b"hello".to_vec());
            }
            other => panic!("name should be Direct, got {other:?}"),
        }

        // Inherited scalar `rating` (declared on Mid).
        match resolved.get(&iri("rating")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_value_lexical(&objects[0]), b"five".to_vec());
            }
            other => panic!("rating should be Direct, got {other:?}"),
        }

        // Own link `author` (declared on Sub).
        match resolved.get(&iri("author")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_node(&objects[0]), iri("person1").into_bytes());
            }
            other => panic!("author should be Direct, got {other:?}"),
        }

        // Inherited ordered list `tracks=[a,b,a]` (declared on Super) — order + duplicate preserved,
        // rendered from the authenticated OrderedListWalk.
        match resolved.get(&iri("tracks")).unwrap() {
            ResolvedValue::List(elements) => {
                let nodes: Vec<Vec<u8>> = elements.iter().map(as_node).collect();
                assert_eq!(
                    nodes,
                    vec![
                        iri("a").into_bytes(),
                        iri("b").into_bytes(),
                        iri("a").into_bytes()
                    ],
                );
            }
            other => panic!("tracks should be a List, got {other:?}"),
        }

        // ---- ADVERSARIAL: tamper the INHERITED list order / forge the walk envelope. ----
        bundle.lists[0].walk.result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a tampered inherited list-walk result column must fail verify",
        );

        // ---- ADVERSARIAL: deliver the inherited list property WITHOUT its walk. ----
        bundle.lists.clear();
        let err = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap_err();
        assert!(
            matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentMalformedListHead { .. })),
            "an inherited list property with no walk must reject as MalformedListHead, got {err:?}",
        );
    });
}

/// Dropped/forged ancestor set. The ancestor SET is the proven output of the reflexive `sys:inherits`
/// closure, RE-VERIFIED at verify from the derived class; a prover cannot shorten it. Corrupting the
/// closure's committed class column makes the closure re-verify fail — so `Super` (and its inherited
/// `name`) can never be silently dropped from the frame. (The complementary defence — a genuinely
/// un-framed inherited property tripping the no-injection cross-check — is exercised by
/// `read_document_inherited_injected_instance_property_fails_verify`.)
#[test]
fn read_document_inherited_dropped_ancestor_fails_verify() {
    on_big_stack("read-document-inherited-drop-ancestor", 1024, || {
        let instance = flat_inherited_instance();
        let schema = flat_inherited_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();
        // Sanity: the honest inherited document verifies.
        bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();

        // Corrupt the AUTHENTICATED ancestor set: verify re-builds the closure plan from the derived
        // class and re-verifies it against the schema root — the tampered column no longer opens.
        bundle
            .frame
            .ancestor_closure
            .as_mut()
            .expect("an inheriting class carries an ancestor closure")
            .result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a tampered (dropped/forged) ancestor closure must fail verify",
        );
    });
}

/// No-injection across inheritance. The instance carries `t(doc, ghost, "boom")`, a predicate NO class
/// in the super-chain declares. The prover CAN assemble a bundle (D is honest, ghost is not a list), but
/// verify computes the complete authenticated D, sees `ghost` is not in the effective (inherited) frame,
/// and reject-closes.
#[test]
fn read_document_inherited_injected_instance_property_fails_verify() {
    on_big_stack("read-document-inherited-injection", 1024, || {
        // `name` is an inherited frame property (fine); `ghost` is declared by NO class in the chain.
        let instance = layer(vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("Sub")),
            Fact::Str(iri("doc"), iri("name"), "hi".into()),
            Fact::Str(iri("doc"), iri("ghost"), "boom".into()),
        ]);
        let schema = flat_inherited_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));

        let bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();
        let err = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap_err();
        assert!(
            matches!(
                err,
                QueryProofError::Plan(BgpPlanError::ReadDocumentUnknownInstanceProperty { .. })
            ),
            "an un-framed instance property must reject as UnknownInstanceProperty, got {err:?}",
        );
    });
}

// ---------------------------------------------------------------------------------------------------
// Increment 3 — subdocument inlining (recursive).
//
// The inline-vs-stop decision is keyed off the CHILD's authenticated `rdf:type` + its `sys:inherits`
// closure (any class bearing `sys:subdocument` ⇒ inline), never the schema range. Fixtures are kept
// MINIMAL — every subdocument node is several BGP proofs + (for inheriting classes) a closure.
// ---------------------------------------------------------------------------------------------------

/// A `C` with a single-valued SUBDOCUMENT property `meta: Meta`. `Meta` (a subdocument) has a scalar
/// (`title`), an ordered `List` of plain nodes (`points: [Point]`), and a NESTED subdocument
/// (`inner: Inner`). `Inner` (a subdocument) has a scalar (`label`) — a 2-level nest.
fn subdoc_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("meta"), iri("Meta")),
        Fact::Node(iri("Meta"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Meta"), SYS_SUBDOC.into(), RDF_NIL.into()),
        Fact::Node(iri("Meta"), iri("title"), XSD_STRING.into()),
        Fact::Node(iri("Meta"), iri("points"), iri("PointsList")),
        Fact::Node(iri("Meta"), iri("inner"), iri("Inner")),
        Fact::Node(iri("PointsList"), RDF_TYPE.into(), SYS_LIST.into()),
        Fact::Node(iri("PointsList"), SYS_CLASS_PRED.into(), iri("Point")),
        Fact::Node(iri("Point"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Inner"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Inner"), SYS_SUBDOC.into(), RDF_NIL.into()),
        Fact::Node(iri("Inner"), iri("label"), XSD_STRING.into()),
    ])
}

fn subdoc_instance() -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Node(iri("doc"), iri("meta"), iri("m1")),
        Fact::Node(iri("m1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Str(iri("m1"), iri("title"), "meta-title".into()),
        Fact::Node(iri("m1"), iri("inner"), iri("in1")),
        Fact::Node(iri("in1"), RDF_TYPE.into(), iri("Inner")),
        Fact::Str(iri("in1"), iri("label"), "inner-label".into()),
    ];
    // The ONLY cons chain in this fixture (the helper hard-codes `cell0..` ids).
    cons_chain(&iri("m1"), &iri("points"), &["pa", "pb", "pa"], &mut facts);
    layer(facts)
}

/// Headline e2e: a single-valued subdocument `meta` INLINES recursively — a scalar, an ordered list
/// (order + duplicate preserved), AND a nested subdocument — then two adversarial tampers on the SAME
/// bundle each fail verify (a tampered NESTED list-walk, and a DROPPED inlined child).
#[test]
fn read_document_subdocument_inlining_end_to_end() {
    on_big_stack("read-document-subdoc-e2e", 1024, || {
        let instance = subdoc_instance();
        let schema = subdoc_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));
        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();

        // ---- HAPPY PATH ----
        let resolved = bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();
        assert_eq!(resolved.id, iri("doc").into_bytes());

        // `meta` is INLINED (its class Meta bears sys:subdocument).
        let meta = match resolved.get(&iri("meta")).unwrap() {
            ResolvedValue::Subdocument(nested) => nested,
            other => panic!("meta should be a Subdocument, got {other:?}"),
        };
        assert_eq!(meta.class, iri("Meta").into_bytes());

        // Nested scalar `title` = Direct.
        match meta.get(&iri("title")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_value_lexical(&objects[0]), b"meta-title".to_vec());
            }
            other => panic!("nested title should be Direct, got {other:?}"),
        }

        // Nested ordered `List` `points = [pa, pb, pa]` (order + duplicate preserved).
        match meta.get(&iri("points")).unwrap() {
            ResolvedValue::List(elements) => {
                let nodes: Vec<Vec<u8>> = elements.iter().map(as_node).collect();
                assert_eq!(
                    nodes,
                    vec![
                        iri("pa").into_bytes(),
                        iri("pb").into_bytes(),
                        iri("pa").into_bytes()
                    ],
                );
            }
            other => panic!("nested points should be a List, got {other:?}"),
        }

        // Second-level nested subdocument `inner`.
        let inner = match meta.get(&iri("inner")).unwrap() {
            ResolvedValue::Subdocument(nested) => nested,
            other => panic!("inner should be a Subdocument, got {other:?}"),
        };
        assert_eq!(inner.class, iri("Inner").into_bytes());
        match inner.get(&iri("label")).unwrap() {
            ResolvedValue::Direct(objects) => {
                assert_eq!(objects.len(), 1);
                assert_eq!(as_value_lexical(&objects[0]), b"inner-label".to_vec());
            }
            other => panic!("deeply-nested label should be Direct, got {other:?}"),
        }

        // ---- ADVERSARIAL: tamper the NESTED list-walk order → verify fails. ----
        match &mut bundle.links[0].target {
            ProvedLinkTarget::Inline(m1) => {
                m1.lists[0].walk.result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
            }
            _ => panic!("the meta link should be inlined"),
        }
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a tampered NESTED list-walk must fail verify",
        );

        // ---- ADVERSARIAL: DROP the inlined child sub-bundle → verify DEMANDS it, reject-closes. ----
        bundle.links.clear();
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "a dropped inlined child sub-bundle must fail verify",
        );
    });
}

/// A `C` with TWO subdocument links (`meta`, `note`) to `Meta`. Used to exercise the injected /
/// duplicate link cross-check.
fn two_subdoc_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("meta"), iri("Meta")),
        Fact::Node(iri("C"), iri("note"), iri("Meta")),
        Fact::Node(iri("Meta"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Meta"), SYS_SUBDOC.into(), RDF_NIL.into()),
        Fact::Node(iri("Meta"), iri("title"), XSD_STRING.into()),
    ])
}

fn two_subdoc_instance() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Node(iri("doc"), iri("meta"), iri("m1")),
        Fact::Node(iri("doc"), iri("note"), iri("n1")),
        Fact::Node(iri("m1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Str(iri("m1"), iri("title"), "a".into()),
        Fact::Node(iri("n1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Str(iri("n1"), iri("title"), "b".into()),
    ])
}

/// Inject an un-demanded node sub-bundle: re-key the SECOND inlined child to duplicate the first. The
/// verifier's per-link cross-check rejects the duplicate/injected sub-bundle (present ⇒ reject).
#[test]
fn read_document_subdocument_injected_child_fails_verify() {
    on_big_stack("read-document-subdoc-injection", 1024, || {
        let instance = two_subdoc_instance();
        let schema = two_subdoc_schema();
        let db_root = database_root(
            &instance.state.commitment_root,
            &schema.state.commitment_root,
        );
        let compiled = compiled_read(&iri("doc"));
        let mut bundle = compiled.prove_on_layers(&*instance, &*schema).unwrap();

        // Sanity: the honest two-subdocument document verifies.
        bundle
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();
        assert_eq!(bundle.links.len(), 2);

        // Re-key the second link onto the first's (predicate, child) → an injected/duplicate sub-bundle.
        let predicate = bundle.links[0].predicate.clone();
        let child_id = bundle.links[0].child_id.clone();
        bundle.links[1].predicate = predicate;
        bundle.links[1].child_id = child_id;
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .is_err(),
            "an injected/duplicate link sub-bundle must fail verify",
        );
    });
}

/// A subdocument CYCLE: `m1` (a subdocument) links back to ITSELF via a subdocument-typed property. The
/// prover reject-closes (`ReadDocumentSubdocumentCycle`) rather than looping.
#[test]
fn read_document_subdocument_cycle_rejects() {
    on_big_stack("read-document-subdoc-cycle", 1024, || {
        let schema = layer(vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("meta"), iri("Meta")),
            Fact::Node(iri("Meta"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Meta"), SYS_SUBDOC.into(), RDF_NIL.into()),
            Fact::Node(iri("Meta"), iri("title"), XSD_STRING.into()),
            Fact::Node(iri("Meta"), iri("selfref"), iri("Meta")),
        ]);
        let instance = layer(vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("meta"), iri("m1")),
            Fact::Node(iri("m1"), RDF_TYPE.into(), iri("Meta")),
            Fact::Str(iri("m1"), iri("title"), "x".into()),
            Fact::Node(iri("m1"), iri("selfref"), iri("m1")),
        ]);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled.prove_on_layers(&*instance, &*schema).unwrap_err();
        assert!(
            matches!(
                err,
                QueryProofError::Plan(BgpPlanError::ReadDocumentSubdocumentCycle)
            ),
            "a subdocument cycle must reject (not loop), got {err:?}",
        );
    });
}

/// An over-BUDGET subdocument chain `doc -> n1 -> n2 -> n3 -> n4` (depth 4 > `MAX_SUBDOC_DEPTH`). The
/// prover reject-closes (`ReadDocumentSubdocumentBudgetExceeded`) rather than truncating the tree.
#[test]
fn read_document_subdocument_over_budget_rejects() {
    on_big_stack("read-document-subdoc-budget", 1024, || {
        let schema = layer(vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("first"), iri("Node")),
            Fact::Node(iri("Node"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Node"), SYS_SUBDOC.into(), RDF_NIL.into()),
            Fact::Node(iri("Node"), iri("next"), iri("Node")),
        ]);
        let instance = layer(vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("first"), iri("n1")),
            Fact::Node(iri("n1"), RDF_TYPE.into(), iri("Node")),
            Fact::Node(iri("n1"), iri("next"), iri("n2")),
            Fact::Node(iri("n2"), RDF_TYPE.into(), iri("Node")),
            Fact::Node(iri("n2"), iri("next"), iri("n3")),
            Fact::Node(iri("n3"), RDF_TYPE.into(), iri("Node")),
            Fact::Node(iri("n3"), iri("next"), iri("n4")),
            Fact::Node(iri("n4"), RDF_TYPE.into(), iri("Node")),
        ]);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled.prove_on_layers(&*instance, &*schema).unwrap_err();
        assert!(
            matches!(
                err,
                QueryProofError::Plan(BgpPlanError::ReadDocumentSubdocumentBudgetExceeded)
            ),
            "an over-budget subdocument chain must reject (not truncate), got {err:?}",
        );
    });
}

/// An `@unfoldable` (non-subdocument) LINK target. `get_document(unfold(true))` unfolds it inline;
/// increment 3 inlines only true subdocuments, so rendering it as a bare link would OMIT content — the
/// new `ReadDocumentUnfoldableDeferred` reject.
#[test]
fn read_document_rejects_unfoldable_link_target() {
    assert_prove_rejects(
        "rd-unfoldable-target",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("ref"), iri("Target")),
            Fact::Node(iri("Target"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Target"), SYS_UNFOLDABLE.into(), RDF_NIL.into()),
            Fact::Node(iri("Target"), iri("name"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("ref"), iri("t1")),
            Fact::Node(iri("t1"), RDF_TYPE.into(), iri("Target")),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentUnfoldableDeferred)),
    );
}

/// An `@unfold` PROPERTY (its frame range node bears `sys:unfold`) whose (non-subdocument) target the
/// reference would unfold inline. Same `ReadDocumentUnfoldableDeferred` reject.
#[test]
fn read_document_rejects_unfold_property() {
    assert_prove_rejects(
        "rd-unfold-property",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("ref"), iri("Target")),
            Fact::Node(iri("Target"), RDF_TYPE.into(), SYS_CLASS.into()),
            // The property's frame RANGE node bears `sys:unfold` (schema_property_is_unfold).
            Fact::Node(iri("Target"), SYS_UNFOLD.into(), RDF_NIL.into()),
            Fact::Node(iri("Target"), iri("name"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("ref"), iri("t1")),
            Fact::Node(iri("t1"), RDF_TYPE.into(), iri("Target")),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentUnfoldableDeferred)),
    );
}

/// A `Set`-OF-subdocument element. v1 inlines only single-valued (`Link`) subdocuments; a subdocument
/// collection element stays reject-closed (`check_element_class`).
#[test]
fn read_document_rejects_set_of_subdocument() {
    assert_prove_rejects(
        "rd-set-of-subdocument",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("tags"), iri("SubSet")),
            Fact::Node(iri("SubSet"), RDF_TYPE.into(), SYS_SET.into()),
            Fact::Node(iri("SubSet"), SYS_CLASS_PRED.into(), iri("Sub")),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_SUBDOC.into(), RDF_NIL.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("tags"), iri("s1")),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "subdocument-element"
            })
        ),
    );
}

/// A `List`-OF-subdocument element — same reject as the `Set` case.
#[test]
fn read_document_rejects_list_of_subdocument() {
    assert_prove_rejects(
        "rd-list-of-subdocument",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("items"), iri("SubList")),
            Fact::Node(iri("SubList"), RDF_TYPE.into(), SYS_LIST.into()),
            Fact::Node(iri("SubList"), SYS_CLASS_PRED.into(), iri("Sub")),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_SUBDOC.into(), RDF_NIL.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("items"), iri("i1")),
        ],
        |err| matches!(
            err,
            QueryProofError::Plan(BgpPlanError::ReadDocumentUnsupportedPropertyShape {
                shape: "subdocument-element"
            })
        ),
    );
}

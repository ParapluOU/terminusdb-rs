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
    plan_read_document, BgpPlanError, CompiledReadDocument, QueryProofError, ResolvedValue,
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

#[test]
fn read_document_rejects_subdocument_range() {
    assert_prove_rejects(
        "rd-subdocument",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("child"), iri("Sub")),
            Fact::Node(iri("Sub"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Sub"), SYS_SUBDOC.into(), RDF_NIL.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Node(iri("doc"), iri("child"), iri("kid1")),
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

#[test]
fn read_document_rejects_inherited_class() {
    assert_prove_rejects(
        "rd-inherit",
        vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), SYS_INHERITS.into(), iri("Base")),
            Fact::Node(iri("Base"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
        ],
        vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Str(iri("doc"), iri("name"), "hi".into()),
        ],
        |err| matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentInheritedFrame)),
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

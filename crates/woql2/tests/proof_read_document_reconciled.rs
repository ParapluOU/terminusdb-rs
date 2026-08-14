//! WOQL `ReadDocument` over the DELTA-STACK (reconciled) path — increment 6a.
//!
//! Builds a delta INSTANCE stack (base + 1 delta, head last) + a MATERIALIZED schema, and proves a
//! whole document whose every instance bound-subject read routes to the store's single-atom present
//! FILTER over the reconciled `present`; the schema frame BGPs + `sys:inherits` closure stay on the
//! materialized schema. Asserts:
//!   * PARITY — the delta-stack document EQUALS the materialized-path document for the same final state;
//!   * SOUNDNESS — a corrupted combined root, a tampered reconciled column, and a route mismatch
//!     (verifying the reconciled bundle through the materialized entry) each fail;
//!   * REJECT BOUNDARY (delta path) — a `List` / `Array` property, a subdocument link, and a
//!     delta-head schema each reject-close with the specific `BgpPlanError`.
//!
//! Reconcile is proof-heavy (each bound-subject read reconciles the whole present set), so the delta
//! stack is kept SMALL (base + 1 delta) and each committed group carries >= 2 records (the reconcile
//! >= 2-record-per-group limitation).

mod proof_common;
use proof_common::iri;

use std::sync::Arc;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use tdb_succinct::{LangString, TdbDataType};
use terminus_store::proof::commitment::{database_root, LayerCommitment};
use terminus_store::store::sync::SyncStoreLayerBuilder;
use terminus_store::{open_sync_memory_store, ValueTriple};

use terminusdb_woql2::document::ReadDocument;
use terminusdb_woql2::proof::{
    plan_read_document, BgpPlanError, CompiledReadDocument, InstanceSubjectProof, QueryProofError,
    ResolvedValue,
};
use terminusdb_woql2::value::{NodeValue, Value};

// ---------------------------------------------------------------------------------------------------
// Well-known IRIs (real RDF/sys/xsd IRIs — NOT `iri()`-wrapped).
// ---------------------------------------------------------------------------------------------------

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
const RDF_LANGSTRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";
const SYS_CLASS: &str = "http://terminusdb.com/schema/sys#Class";
const SYS_SET: &str = "http://terminusdb.com/schema/sys#Set";
const SYS_LIST: &str = "http://terminusdb.com/schema/sys#List";
const SYS_ARRAY: &str = "http://terminusdb.com/schema/sys#Array";
const SYS_OPTIONAL: &str = "http://terminusdb.com/schema/sys#Optional";
const SYS_CLASS_PRED: &str = "http://terminusdb.com/schema/sys#class";
const SYS_INHERITS: &str = "http://terminusdb.com/schema/sys#inherits";
const SYS_SUBDOC: &str = "http://terminusdb.com/schema/sys#subdocument";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

// ---------------------------------------------------------------------------------------------------
// Fixture building.
// ---------------------------------------------------------------------------------------------------

/// One triple to seed: a node/node/node edge, a node/predicate/string-literal, or a language-tagged
/// literal (glued store lexical `"lang@text"`, exactly as the materialized suite seeds it).
enum Fact {
    Node(String, String, String),
    Str(String, String, String),
    Lang(String, String, String),
}

fn add_fact(builder: &SyncStoreLayerBuilder, fact: &Fact) {
    match fact {
        Fact::Node(s, p, o) => builder.add_value_triple(ValueTriple::new_node(s, p, o)).unwrap(),
        Fact::Str(s, p, o) => {
            builder.add_value_triple(ValueTriple::new_string_value(s, p, o)).unwrap()
        }
        Fact::Lang(s, p, glued) => builder
            .add_value_triple(ValueTriple::new_value(s, p, LangString::make_entry(glued)))
            .unwrap(),
    }
}

/// A single MATERIALIZED base layer holding `facts` (the schema graph, and the materialized-path
/// instance used for the parity comparison).
fn layer(facts: Vec<Fact>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for fact in &facts {
        add_fact(&builder, fact);
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// A delta INSTANCE stack: a base layer with `base` facts, then a delta layer ADDING `delta` facts
/// (head last). Copies the store reconcile-test chain pattern (`create_base_layer` → `commit` →
/// `open_write` → `commit`). Both committed groups must carry >= 2 records (reconcile limitation).
fn delta_stack(base: Vec<Fact>, delta: Vec<Fact>) -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for fact in &base {
        add_fact(&builder, fact);
    }
    let base_layer = builder.commit().unwrap();
    let base_lc = base_layer.proof_commitment().unwrap();
    let child = base_layer.open_write().unwrap();
    for fact in &delta {
        add_fact(&child, fact);
    }
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

fn refs(layers: &[Arc<LayerCommitment>]) -> Vec<&LayerCommitment> {
    layers.iter().map(|l| l.as_ref()).collect()
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

fn as_node(object: &terminus_store::proof::CanonicalObject) -> Vec<u8> {
    match object {
        terminus_store::proof::CanonicalObject::Node(bytes) => bytes.clone(),
        other => panic!("expected a node, got {other:?}"),
    }
}

fn as_value_lexical(object: &terminus_store::proof::CanonicalObject) -> Vec<u8> {
    match object {
        terminus_store::proof::CanonicalObject::Value { lexical, .. } => lexical.clone(),
        other => panic!("expected a literal value, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------
// The PARITY fixture: an inheriting class `C : Parent` with a flat body — inherited scalar (`name`),
// `rdf:langString` (`title`), `Optional<xsd:string>` (`nick`), `Set<xsd:string>` (`tags`), and a
// NON-subdocument class link (`author` -> `Person`). No list/array/subdocument (those reject on the
// delta path), so this whole document IS provable over the delta stack.
// ---------------------------------------------------------------------------------------------------

/// The materialized schema (used identically by both paths).
fn parity_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("Parent"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Parent"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), SYS_INHERITS.into(), iri("Parent")),
        Fact::Node(iri("C"), iri("title"), RDF_LANGSTRING.into()),
        Fact::Node(iri("C"), iri("nick"), iri("NickOpt")),
        Fact::Node(iri("NickOpt"), RDF_TYPE.into(), SYS_OPTIONAL.into()),
        Fact::Node(iri("NickOpt"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("tags"), iri("TagsSet")),
        Fact::Node(iri("TagsSet"), RDF_TYPE.into(), SYS_SET.into()),
        Fact::Node(iri("TagsSet"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
    ])
}

/// The document's direct triples, split base (6) + delta (2). The full present set is the flat `doc`.
fn parity_base() -> Vec<Fact> {
    vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("name"), "alice".into()), // inherited scalar
        Fact::Lang(iri("doc"), iri("title"), "en@Hello".into()), // langString (glued lexical)
        Fact::Str(iri("doc"), iri("nick"), "ally".into()),  // present Optional scalar
        Fact::Str(iri("doc"), iri("tags"), "x".into()),
        Fact::Str(iri("doc"), iri("tags"), "y".into()),
    ]
}

fn parity_delta() -> Vec<Fact> {
    vec![
        Fact::Node(iri("doc"), iri("author"), iri("person1")), // non-subdocument link edge
        Fact::Node(iri("person1"), RDF_TYPE.into(), iri("Person")), // the link child's type
    ]
}

fn parity_materialized_instance() -> Arc<LayerCommitment> {
    let mut facts = parity_base();
    facts.extend(parity_delta());
    layer(facts)
}

#[test]
fn read_document_reconciled_flat_and_inheritance_parity() {
    on_big_stack("read-document-reconciled-parity", 1024, || {
        let schema = parity_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));

        // ---- Materialized path (the reference answer for the SAME final state). ----
        let mat_instance = parity_materialized_instance();
        let mat_db_root = database_root(&mat_instance.state.commitment_root, &schema_root);
        let mat_bundle = compiled.prove_on_layers(&*mat_instance, &*schema).unwrap();
        let mat_doc = mat_bundle
            .verify_over_graphs_and_resolve(&compiled, &*mat_instance, &*schema, mat_db_root)
            .unwrap();

        // ---- Delta-stack (reconciled) path over the SAME final state. ----
        let layers = delta_stack(parity_base(), parity_delta());
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);

        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();

        // HAPPY: verify + resolve against the trusted combined root (the delta-HEAD anchor).
        let delta_doc = bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        // PARITY: the delta-stack document is byte-for-byte the materialized-path document.
        assert_eq!(delta_doc, mat_doc, "delta-stack document must equal the materialized document");
        assert_eq!(delta_doc.id, iri("doc").into_bytes());
        assert_eq!(delta_doc.class, iri("C").into_bytes());

        // Spot-check the mix of shapes rendered over the delta stack.
        match delta_doc.get(&iri("name")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"alice".to_vec()),
            other => panic!("name should be Direct, got {other:?}"),
        }
        match delta_doc.get(&iri("nick")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"ally".to_vec()),
            other => panic!("nick should be Direct, got {other:?}"),
        }
        match delta_doc.get(&iri("title")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"en@Hello".to_vec()),
            other => panic!("title should be Direct langString, got {other:?}"),
        }
        match delta_doc.get(&iri("author")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_node(&o[0]), iri("person1").into_bytes()),
            other => panic!("author should be Direct link, got {other:?}"),
        }
        match delta_doc.get(&iri("tags")).unwrap() {
            ResolvedValue::Direct(o) => {
                let mut ls: Vec<Vec<u8>> = o.iter().map(as_value_lexical).collect();
                ls.sort();
                assert_eq!(ls, vec![b"x".to_vec(), b"y".to_vec()]);
            }
            other => panic!("tags should be Direct set, got {other:?}"),
        }

        // ---- SOUNDNESS 1: a corrupted combined root fails (the head/schema roots are bound). ----
        let mut bad = db_root;
        bad[0] ^= 0xFF;
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, bad)
                .is_err(),
            "a corrupted combined root must fail verify",
        );

        // ---- SOUNDNESS 2: route mismatch — the reconciled bundle must NOT verify through the
        //      MATERIALIZED entry (its Reconciled leaf disagrees with the entry-chosen source). ----
        let head_layer = lref.last().unwrap();
        assert!(
            bundle
                .verify_over_graphs_and_resolve(&compiled, *head_layer, &*schema, db_root)
                .is_err(),
            "a reconciled bundle must reject through the materialized entry (route mismatch)",
        );

        // ---- SOUNDNESS 3: swapping instance/schema fails. ----
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(
                    &compiled,
                    &[*head_layer],
                    &**lref.first().unwrap(),
                    db_root,
                )
                .is_err(),
            "swapping instance/schema must fail verify",
        );

        // ---- SOUNDNESS 4: tamper a reconciled subject-read column → the single-atom filter re-verify
        //      (bound to the trusted head root) rejects. ----
        if let InstanceSubjectProof::Reconciled { columns, .. } = &mut bundle.subject_bgp {
            columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
        } else {
            panic!("the delta-path root subject read must be a Reconciled leaf");
        }
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "a tampered reconciled subject column must fail verify",
        );
    });
}

// ---------------------------------------------------------------------------------------------------
// Reject boundaries on the DELTA path (prove reject-closed with the specific error).
// ---------------------------------------------------------------------------------------------------

/// A minimal non-inheriting `C` with a `Set<xsd:string>` filler (so each committed delta group can
/// carry >= 2 records) plus one extra property whose descriptor `facts` add.
fn reject_schema(extra: Vec<Fact>) -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("fill"), iri("FillSet")),
        Fact::Node(iri("FillSet"), RDF_TYPE.into(), SYS_SET.into()),
        Fact::Node(iri("FillSet"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ];
    facts.extend(extra);
    layer(facts)
}

/// A delta stack whose base carries `doc rdf:type C`, one `fill` value, and `extra` (the reject
/// property's instance triples), and whose delta adds two more `fill` values (>= 2 records).
fn reject_stack(extra: Vec<Fact>) -> Vec<Arc<LayerCommitment>> {
    let mut base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
    ];
    base.extend(extra);
    let delta = vec![
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
        Fact::Str(iri("doc"), iri("fill"), "c".into()),
    ];
    delta_stack(base, delta)
}

fn assert_reconciled_prove_rejects(
    name: &'static str,
    schema: Arc<LayerCommitment>,
    layers: Vec<Arc<LayerCommitment>>,
    check: impl Fn(&QueryProofError) -> bool + Send + 'static,
) {
    on_big_stack(name, 1024, move || {
        let lref = refs(&layers);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap_err();
        assert!(check(&err), "unexpected error for {name}: {err:?}");
    });
}

#[test]
fn read_document_reconciled_rejects_list() {
    let schema = reject_schema(vec![
        Fact::Node(iri("C"), iri("lst"), iri("ListDesc")),
        Fact::Node(iri("ListDesc"), RDF_TYPE.into(), SYS_LIST.into()),
        Fact::Node(iri("ListDesc"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ]);
    // A single head edge makes the list PRESENT (its ordered walk is not reconciled → reject).
    let layers = reject_stack(vec![Fact::Node(iri("doc"), iri("lst"), iri("head0"))]);
    assert_reconciled_prove_rejects("reconciled-rejects-list", schema, layers, |e| {
        matches!(
            e,
            QueryProofError::Plan(BgpPlanError::ReadDocumentDeltaWalkDeferred { shape: "list" })
        )
    });
}

#[test]
fn read_document_reconciled_rejects_array() {
    let schema = reject_schema(vec![
        Fact::Node(iri("C"), iri("arr"), iri("ArrDesc")),
        Fact::Node(iri("ArrDesc"), RDF_TYPE.into(), SYS_ARRAY.into()),
        Fact::Node(iri("ArrDesc"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ]);
    // One cell edge makes the array PRESENT (its ordered walk is not reconciled → reject).
    let layers = reject_stack(vec![Fact::Node(iri("doc"), iri("arr"), iri("cell0"))]);
    assert_reconciled_prove_rejects("reconciled-rejects-array", schema, layers, |e| {
        matches!(
            e,
            QueryProofError::Plan(BgpPlanError::ReadDocumentDeltaWalkDeferred { shape: "array" })
        )
    });
}

#[test]
fn read_document_reconciled_rejects_subdocument_link() {
    // `author -> Person` where `Person` is a SUBDOCUMENT. The child's subdocument nature is re-derived
    // from `person`'s reconciled `rdf:type` read (SOUND, not silent) → reject rather than omit.
    let schema = reject_schema(vec![
        Fact::Node(iri("C"), iri("author"), iri("Person")),
        Fact::Node(iri("Person"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Person"), SYS_SUBDOC.into(), RDF_NIL.into()),
    ]);
    // The child `person1` is typed in the delta; the author edge lives in the base.
    let base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
        Fact::Node(iri("doc"), iri("author"), iri("person1")),
    ];
    let delta = vec![
        Fact::Node(iri("person1"), RDF_TYPE.into(), iri("Person")),
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
    ];
    let layers = delta_stack(base, delta);
    assert_reconciled_prove_rejects("reconciled-rejects-subdoc", schema, layers, |e| {
        matches!(
            e,
            QueryProofError::Plan(BgpPlanError::ReadDocumentDeltaSubdocumentDeferred)
        )
    });
}

#[test]
fn read_document_reconciled_rejects_delta_head_schema() {
    on_big_stack("reconciled-rejects-delta-schema", 512, || {
        // A SCHEMA supplied as a delta head (parent_state_root = Some) reject-closes BEFORE any read.
        let schema_layers = delta_stack(
            vec![
                Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
                Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
                Fact::Node(iri("X"), RDF_TYPE.into(), SYS_CLASS.into()),
            ],
            vec![
                Fact::Node(iri("Y"), RDF_TYPE.into(), SYS_CLASS.into()),
                Fact::Node(iri("Z"), RDF_TYPE.into(), SYS_CLASS.into()),
            ],
        );
        let schema_head = schema_layers.last().unwrap();
        let instance_layers = delta_stack(
            vec![
                Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
                Fact::Str(iri("doc"), iri("name"), "n".into()),
                Fact::Str(iri("doc"), iri("name"), "m".into()),
            ],
            vec![
                Fact::Str(iri("doc"), iri("name"), "o".into()),
                Fact::Str(iri("doc"), iri("name"), "p".into()),
            ],
        );
        let lref = refs(&instance_layers);
        let compiled = compiled_read(&iri("doc"));
        let err = compiled
            .prove_on_layers_reconciled(&lref, &**schema_head)
            .unwrap_err();
        assert!(
            matches!(err, QueryProofError::Plan(BgpPlanError::ReadDocumentDeltaSchemaHead)),
            "a delta-head schema must reject as ReadDocumentDeltaSchemaHead, got {err:?}",
        );
    });
}

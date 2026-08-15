//! WOQL `ReadDocument` over the DELTA-STACK (reconciled) path — increments 6a + 6b.
//!
//! Builds a delta INSTANCE stack (base + 1 delta, head last) + a MATERIALIZED schema, and proves a
//! whole document whose every instance bound-subject read routes to the store's single-atom present
//! FILTER over the reconciled `present`; the schema frame BGPs + `sys:inherits` closure stay on the
//! materialized schema. Asserts:
//!   * PARITY (6a flat + 6b subdocument) — the delta-stack document EQUALS the materialized-path
//!     document for the same final state, INCLUDING a recursively-inlined FLAT subdocument child;
//!   * SOUNDNESS — a corrupted combined root, a tampered reconciled column, and a route mismatch
//!     (verifying the reconciled bundle through the materialized entry) each fail; AND for 6b: a dropped
//!     inlined child sub-bundle, an injected duplicate child, a `Stop` over a re-derived subdocument
//!     child, and a subdocument cycle each reject;
//!   * LIST / ARRAY PARITY (delta path, 6c-iii / 6d) — a top-level `List` proves over the delta stack
//!     (exact ordered `[a, b, a]`) and a top-level 1-D dense `Array` proves over the delta stack (exact
//!     `sys:index` order `[b, c, a]`); each whole document EQUALS the materialized-path result and a
//!     tampered order reject-closes. A subdocument node bearing an `Array` proves recursively (6d).
//!   * REJECT BOUNDARY (delta path) — a delta-head schema reject-closes with the specific `BgpPlanError`.
//!
//! Increment 6c LIFTS the delta-path `List` reject (an ordered `List` proves over the reconciled
//! `present` via the store's reconciled `OrderedListWalk`); increment 6d LIFTS the delta-path `Array`
//! reject the same way (the reconciled `OrderedArrayWalk`, built on the 6c-i foundation + the audited
//! array gluing gadgets + the combined-range-dict `sys:index` authentication — NO new crypto).
//! Increment 6b INLINES a FLAT subdocument over the delta stack, so a subdocument's `List`/`Array`
//! proves recursively too.
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
    plan_read_document, ArrayWalkProof, BgpPlanError, CompiledReadDocument, InstanceSubjectProof,
    ListWalkProof, ProvedChildTypeWitness, ProvedLink, ProvedLinkTarget, QueryProofError,
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
const RDF_LANGSTRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";
const SYS_CLASS: &str = "http://terminusdb.com/schema/sys#Class";
const SYS_SET: &str = "http://terminusdb.com/schema/sys#Set";
const SYS_LIST: &str = "http://terminusdb.com/schema/sys#List";
const SYS_ARRAY: &str = "http://terminusdb.com/schema/sys#Array";
const SYS_INDEX: &str = "http://terminusdb.com/schema/sys#index";
const SYS_VALUE: &str = "http://terminusdb.com/schema/sys#value";
const SYS_OPTIONAL: &str = "http://terminusdb.com/schema/sys#Optional";
const SYS_CLASS_PRED: &str = "http://terminusdb.com/schema/sys#class";
const SYS_INHERITS: &str = "http://terminusdb.com/schema/sys#inherits";
const SYS_SUBDOC: &str = "http://terminusdb.com/schema/sys#subdocument";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

// ---------------------------------------------------------------------------------------------------
// Fixture building.
// ---------------------------------------------------------------------------------------------------

/// One triple to seed: a node/node/node edge, a node/predicate/string-literal, a language-tagged
/// literal (glued store lexical `"lang@text"`, exactly as the materialized suite seeds it), or a
/// `sys:index` `UInt64` literal (the order-preserving unsigned datatype the array-walk composite
/// authenticates against).
#[derive(Clone)]
enum Fact {
    Node(String, String, String),
    Str(String, String, String),
    Lang(String, String, String),
    Index(String, u64),
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
        Fact::Index(cell, index) => builder
            .add_value_triple(ValueTriple::new_value(cell, SYS_INDEX, u64::make_entry(index)))
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

/// A cons cell IRI `http://ex/cell{i}`.
fn cell(i: usize) -> String {
    iri(&format!("cell{i}"))
}

/// The cons chain `subject -predicate-> [string elements...]` — `t(subject,predicate,cell0)` then per
/// element `t(celli, rdf:first, "e"^^xsd:string)` + `t(celli, rdf:rest, celli+1 | rdf:nil)`.
fn cons_string_facts(subject: &str, predicate: &str, elements: &[&str]) -> Vec<Fact> {
    let mut facts = Vec::new();
    if elements.is_empty() {
        facts.push(Fact::Node(subject.into(), predicate.into(), RDF_NIL.into()));
        return facts;
    }
    facts.push(Fact::Node(subject.into(), predicate.into(), cell(0)));
    for (i, e) in elements.iter().enumerate() {
        facts.push(Fact::Str(cell(i), RDF_FIRST.into(), (*e).into()));
        let rest = if i + 1 < elements.len() { cell(i + 1) } else { RDF_NIL.to_string() };
        facts.push(Fact::Node(cell(i), RDF_REST.into(), rest));
    }
    facts
}

/// The reject-fixture schema plus a `lst: List<xsd:string>` property.
fn list_schema() -> Arc<LayerCommitment> {
    reject_schema(vec![
        Fact::Node(iri("C"), iri("lst"), iri("ListDesc")),
        Fact::Node(iri("ListDesc"), RDF_TYPE.into(), SYS_LIST.into()),
        Fact::Node(iri("ListDesc"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ])
}

/// A delta stack carrying `doc rdf:type C`, a `fill` value, and the cons chain in the BASE, with two
/// more `fill` values in the delta (so each committed group has >= 2 records). The whole list is thus
/// read over the reconciled `present` (base ⊎ delta), not a single materialized layer.
fn list_stack(list_facts: Vec<Fact>) -> Vec<Arc<LayerCommitment>> {
    let mut base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
    ];
    base.extend(list_facts);
    let delta = vec![
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
        Fact::Str(iri("doc"), iri("fill"), "c".into()),
    ];
    delta_stack(base, delta)
}

/// The materialized reference instance holding the SAME final present as [`list_stack`].
fn list_materialized_instance(list_facts: Vec<Fact>) -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
        Fact::Str(iri("doc"), iri("fill"), "c".into()),
    ];
    facts.extend(list_facts);
    layer(facts)
}

/// 6c-iii PARITY: a top-level `List` property proves over the delta stack, the resolved list is the
/// EXACT ordered `[a, b, a]`, the whole document EQUALS the materialized-path result, and a tampered
/// list order reject-closes.
#[test]
fn read_document_reconciled_list_parity() {
    on_big_stack("read-document-reconciled-list-parity", 1024, || {
        let schema = list_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));

        // ---- Materialized path (the reference answer for the SAME final state). ----
        let mat_instance =
            list_materialized_instance(cons_string_facts(&iri("doc"), &iri("lst"), &["a", "b", "a"]));
        let mat_db_root = database_root(&mat_instance.state.commitment_root, &schema_root);
        let mat_bundle = compiled.prove_on_layers(&*mat_instance, &*schema).unwrap();
        let mat_doc = mat_bundle
            .verify_over_graphs_and_resolve(&compiled, &*mat_instance, &*schema, mat_db_root)
            .unwrap();

        // ---- Delta-stack (reconciled) path over the SAME final state. ----
        let layers = list_stack(cons_string_facts(&iri("doc"), &iri("lst"), &["a", "b", "a"]));
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();
        let delta_doc = bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        // PARITY: the delta-stack document is byte-for-byte the materialized-path document.
        assert_eq!(delta_doc, mat_doc, "delta-stack list document must equal the materialized document");

        // The list renders as the EXACT ordered `[a, b, a]` (order + duplicate multiplicity).
        match delta_doc.get(&iri("lst")).unwrap() {
            ResolvedValue::List(elements) => {
                let ls: Vec<Vec<u8>> = elements.iter().map(as_value_lexical).collect();
                assert_eq!(ls, vec![b"a".to_vec(), b"b".to_vec(), b"a".to_vec()]);
            }
            other => panic!("lst should be a List, got {other:?}"),
        }

        // ---- SOUNDNESS: a corrupted combined root fails. ----
        let mut bad = db_root;
        bad[0] ^= 0xFF;
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, bad)
                .is_err(),
            "a corrupted combined root must fail verify",
        );

        // ---- SOUNDNESS: a TAMPERED list order rejects. Reorder the reconciled walk's public element
        //      ids so they no longer open the authenticated `com_elements` (the chain value-resolution
        //      re-commit binding fails closed). ----
        match &mut bundle.lists[0].walk {
            ListWalkProof::Reconciled(walk) => walk.element_ids.swap(0, 1),
            ListWalkProof::Materialized(_) => panic!("delta-path list walk must be Reconciled"),
        }
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "a tampered list order must fail verify",
        );
    });
}

/// The reject-fixture schema plus an `arr: Array<xsd:string>` property.
fn array_schema() -> Arc<LayerCommitment> {
    reject_schema(vec![
        Fact::Node(iri("C"), iri("arr"), iri("ArrDesc")),
        Fact::Node(iri("ArrDesc"), RDF_TYPE.into(), SYS_ARRAY.into()),
        Fact::Node(iri("ArrDesc"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ])
}

/// The dense array `subject -predicate-> {cells}` as facts — per cell an edge `t(subject,predicate,celli)`,
/// `t(celli, rdf:type, sys:Array)`, a `UInt64` `sys:index`, and a `sys:value` string element. `cells` are
/// `(index, value)` in PHYSICAL storage order (cell0, cell1, …), so a layout whose physical order differs
/// from index order genuinely exercises the `sys:index` ordering.
fn array_string_facts(subject: &str, predicate: &str, cells: &[(u64, &str)]) -> Vec<Fact> {
    let mut facts = Vec::new();
    for (k, (index, value)) in cells.iter().enumerate() {
        let c = cell(k);
        facts.push(Fact::Node(subject.into(), predicate.into(), c.clone()));
        facts.push(Fact::Node(c.clone(), RDF_TYPE.into(), SYS_ARRAY.into()));
        facts.push(Fact::Index(c.clone(), *index));
        facts.push(Fact::Str(c.clone(), SYS_VALUE.into(), (*value).into()));
    }
    facts
}

/// A delta stack carrying `doc rdf:type C`, a `fill` value, and the array cells in the BASE, with two
/// more `fill` values in the delta (so each committed group has >= 2 records). The whole array is thus
/// read over the reconciled `present` (base ⊎ delta), not a single materialized layer.
fn array_stack(array_facts: Vec<Fact>) -> Vec<Arc<LayerCommitment>> {
    let mut base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
    ];
    base.extend(array_facts);
    let delta = vec![
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
        Fact::Str(iri("doc"), iri("fill"), "c".into()),
    ];
    delta_stack(base, delta)
}

/// The materialized reference instance holding the SAME final present as [`array_stack`].
fn array_materialized_instance(array_facts: Vec<Fact>) -> Arc<LayerCommitment> {
    let mut facts = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("fill"), "a".into()),
        Fact::Str(iri("doc"), iri("fill"), "b".into()),
        Fact::Str(iri("doc"), iri("fill"), "c".into()),
    ];
    facts.extend(array_facts);
    layer(facts)
}

/// 6d PARITY: a top-level 1-D dense `Array` property proves over the delta stack, the resolved array is
/// the EXACT `sys:index`-ordered `[b, c, a]` (physical order `[idx2, idx0, idx1]`), the whole document
/// EQUALS the materialized-path result, and a tampered array order reject-closes. (Repurposed from the
/// old `read_document_reconciled_rejects_array` — arrays no longer reject on the delta path.)
#[test]
fn read_document_reconciled_array_parity() {
    on_big_stack("read-document-reconciled-array-parity", 1024, || {
        let schema = array_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));
        // Physical storage order != index order, so the ordering is genuinely exercised: stored
        // [idx2->a, idx0->b, idx1->c] => the ordered array is [b, c, a].
        let cells = [(2u64, "a"), (0, "b"), (1, "c")];

        // ---- Materialized path (the reference answer for the SAME final state). ----
        let mat_instance =
            array_materialized_instance(array_string_facts(&iri("doc"), &iri("arr"), &cells));
        let mat_db_root = database_root(&mat_instance.state.commitment_root, &schema_root);
        let mat_bundle = compiled.prove_on_layers(&*mat_instance, &*schema).unwrap();
        let mat_doc = mat_bundle
            .verify_over_graphs_and_resolve(&compiled, &*mat_instance, &*schema, mat_db_root)
            .unwrap();

        // ---- Delta-stack (reconciled) path over the SAME final state. ----
        let layers = array_stack(array_string_facts(&iri("doc"), &iri("arr"), &cells));
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();
        let delta_doc = bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        // PARITY: the delta-stack document is byte-for-byte the materialized-path document.
        assert_eq!(delta_doc, mat_doc, "delta-stack array document must equal the materialized document");

        // The array renders as the EXACT `sys:index`-ordered `[b, c, a]`.
        match delta_doc.get(&iri("arr")).unwrap() {
            ResolvedValue::Array(elements) => {
                let vs: Vec<Vec<u8>> = elements.iter().map(as_value_lexical).collect();
                assert_eq!(vs, vec![b"b".to_vec(), b"c".to_vec(), b"a".to_vec()]);
            }
            other => panic!("arr should be an Array, got {other:?}"),
        }

        // ---- SOUNDNESS: a corrupted combined root fails. ----
        let mut bad = db_root;
        bad[0] ^= 0xFF;
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, bad)
                .is_err(),
            "a corrupted combined root must fail verify",
        );

        // ---- SOUNDNESS: a TAMPERED array order rejects. Reorder the reconciled walk's public value ids
        //      so they no longer open the authenticated `com_out` (the chain value-resolution re-commit
        //      binding fails closed). ----
        match &mut bundle.arrays[0].walk {
            ArrayWalkProof::Reconciled(walk) => walk.value_ids.swap(0, 1),
            ArrayWalkProof::Materialized(_) => panic!("delta-path array walk must be Reconciled"),
        }
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "a tampered array order must fail verify",
        );
    });
}

/// 6d RECURSIVE: a subdocument node that itself bears a 1-D dense `Array` now PROVES over the delta
/// stack — the reconciled `OrderedArrayWalk` runs from INSIDE the subdocument recursion, exactly as 6c
/// lifted a nested `List`. (Repurposed from `read_document_reconciled_subdocument_with_array_rejects`:
/// a nested array no longer reject-closes on the delta path.) Parity with the materialized-path result
/// for the same final state; the nested array renders in the exact `sys:index` order `[p, q]`.
#[test]
fn read_document_reconciled_subdocument_with_array_parity() {
    on_big_stack("read-document-reconciled-subdoc-array", 1024, || {
        let schema = layer(vec![
            Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
            Fact::Node(iri("C"), iri("addr"), iri("Address")),
            Fact::Node(iri("Address"), RDF_TYPE.into(), SYS_CLASS.into()),
            Fact::Node(iri("Address"), SYS_SUBDOC.into(), RDF_NIL.into()),
            Fact::Node(iri("Address"), iri("street"), XSD_STRING.into()),
            Fact::Node(iri("Address"), iri("city"), XSD_STRING.into()),
            Fact::Node(iri("Address"), iri("items"), iri("ItemsArray")),
            Fact::Node(iri("ItemsArray"), RDF_TYPE.into(), SYS_ARRAY.into()),
            Fact::Node(iri("ItemsArray"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
        ]);
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));

        // The child `addr1` bears a well-formed 2-element array (physical `[idx1->q, idx0->p]`) => the
        // `sys:index`-ordered array is `[p, q]`. The array cells live in the BASE; the delta carries the
        // child's `street`/`city` scalars (2 records — satisfying the >= 2-per-group reconcile floor).
        let mut base = vec![
            Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
            Fact::Str(iri("doc"), iri("name"), "alice".into()),
            Fact::Node(iri("doc"), iri("addr"), iri("addr1")),
            Fact::Node(iri("addr1"), RDF_TYPE.into(), iri("Address")),
        ];
        base.extend(array_string_facts(&iri("addr1"), &iri("items"), &[(1, "q"), (0, "p")]));
        let delta = vec![
            Fact::Str(iri("addr1"), iri("street"), "Main St".into()),
            Fact::Str(iri("addr1"), iri("city"), "Springfield".into()),
        ];

        // ---- Materialized reference over the SAME final state. ----
        let mut mat_facts = base.clone();
        mat_facts.extend(delta.clone());
        let mat_instance = layer(mat_facts);
        let mat_db_root = database_root(&mat_instance.state.commitment_root, &schema_root);
        let mat_bundle = compiled.prove_on_layers(&*mat_instance, &*schema).unwrap();
        let mat_doc = mat_bundle
            .verify_over_graphs_and_resolve(&compiled, &*mat_instance, &*schema, mat_db_root)
            .unwrap();

        // ---- Delta-stack (reconciled) path — the nested array proves recursively. ----
        let layers = delta_stack(base, delta);
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();
        let delta_doc = bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        assert_eq!(
            delta_doc, mat_doc,
            "delta-stack subdocument-with-array must equal the materialized document",
        );
        // The nested subdocument's array renders in the exact `sys:index` order `[p, q]`.
        let addr = match delta_doc.get(&iri("addr")).unwrap() {
            ResolvedValue::Subdocument(nested) => nested,
            other => panic!("addr should be an inlined Subdocument, got {other:?}"),
        };
        match addr.get(&iri("items")).unwrap() {
            ResolvedValue::Array(elements) => {
                let vs: Vec<Vec<u8>> = elements.iter().map(as_value_lexical).collect();
                assert_eq!(vs, vec![b"p".to_vec(), b"q".to_vec()]);
            }
            other => panic!("nested items should be an Array, got {other:?}"),
        }
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

// ===================================================================================================
// Increment 6b: FLAT subdocument inlining over the delta stack.
//
// `C` (a plain class) has a top-level scalar (`name`) and a single-valued SUBDOCUMENT link
// (`addr -> Address`). `Address` bears `sys:subdocument` and has a FLAT body — two scalars (`street`,
// `city`) and a `Set<xsd:string>` (`tags`) — so it INLINES recursively over the delta stack: the inc-3
// recursion runs with the reconciled instance source threaded through, so the child `addr1`'s own `D`
// routes over `present` exactly like the root's. The multi-object `tags` set lives in the BASE (its
// element order matches the merged materialized layer, keeping the byte-for-byte parity robust); the
// delta carries the child's single-valued scalars (≥ 2 records per committed group either way).
// ===================================================================================================

fn subdoc_schema() -> Arc<LayerCommitment> {
    layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("name"), XSD_STRING.into()),
        Fact::Node(iri("C"), iri("addr"), iri("Address")),
        Fact::Node(iri("Address"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Address"), SYS_SUBDOC.into(), RDF_NIL.into()),
        Fact::Node(iri("Address"), iri("street"), XSD_STRING.into()),
        Fact::Node(iri("Address"), iri("city"), XSD_STRING.into()),
        Fact::Node(iri("Address"), iri("tags"), iri("TagsSet")),
        Fact::Node(iri("TagsSet"), RDF_TYPE.into(), SYS_SET.into()),
        Fact::Node(iri("TagsSet"), SYS_CLASS_PRED.into(), XSD_STRING.into()),
    ])
}

fn subdoc_base() -> Vec<Fact> {
    vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Str(iri("doc"), iri("name"), "alice".into()),
        Fact::Node(iri("doc"), iri("addr"), iri("addr1")),
        Fact::Node(iri("addr1"), RDF_TYPE.into(), iri("Address")),
        Fact::Str(iri("addr1"), iri("tags"), "home".into()),
        Fact::Str(iri("addr1"), iri("tags"), "work".into()),
    ]
}

fn subdoc_delta() -> Vec<Fact> {
    vec![
        Fact::Str(iri("addr1"), iri("street"), "Main St".into()),
        Fact::Str(iri("addr1"), iri("city"), "Springfield".into()),
    ]
}

fn subdoc_materialized_instance() -> Arc<LayerCommitment> {
    let mut facts = subdoc_base();
    facts.extend(subdoc_delta());
    layer(facts)
}

/// PARITY e2e: a document with a FLAT subdocument property proves over the delta stack, and the resolved
/// nested `ResolvedDocument` equals the materialized-path result for the SAME final state (a 1-level
/// nest — a 2-level nest re-reconciles one more child per node, kept out to bound proof cost). Plus two
/// adversarial checks: a corrupted combined root and a DROPPED inlined child sub-bundle both fail verify.
#[test]
fn read_document_reconciled_subdocument_parity() {
    on_big_stack("read-document-reconciled-subdoc-parity", 1024, || {
        let schema = subdoc_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));

        // ---- Materialized path (the reference answer for the SAME final state). ----
        let mat_instance = subdoc_materialized_instance();
        let mat_db_root = database_root(&mat_instance.state.commitment_root, &schema_root);
        let mat_bundle = compiled.prove_on_layers(&*mat_instance, &*schema).unwrap();
        let mat_doc = mat_bundle
            .verify_over_graphs_and_resolve(&compiled, &*mat_instance, &*schema, mat_db_root)
            .unwrap();

        // ---- Delta-stack (reconciled) path over the SAME final state. ----
        let layers = delta_stack(subdoc_base(), subdoc_delta());
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();
        let delta_doc = bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        // PARITY: the delta-stack document (INCLUDING the inlined subdocument) is byte-for-byte the
        // materialized-path document.
        assert_eq!(
            delta_doc, mat_doc,
            "delta-stack subdocument must equal the materialized document",
        );
        assert_eq!(delta_doc.id, iri("doc").into_bytes());
        assert_eq!(delta_doc.class, iri("C").into_bytes());

        // Top-level scalar.
        match delta_doc.get(&iri("name")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"alice".to_vec()),
            other => panic!("name should be Direct, got {other:?}"),
        }
        // `addr` INLINED as a nested subdocument (Address bears sys:subdocument, re-derived over `present`).
        let addr = match delta_doc.get(&iri("addr")).unwrap() {
            ResolvedValue::Subdocument(nested) => nested,
            other => panic!("addr should be an inlined Subdocument, got {other:?}"),
        };
        assert_eq!(addr.id, iri("addr1").into_bytes());
        assert_eq!(addr.class, iri("Address").into_bytes());
        match addr.get(&iri("street")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"Main St".to_vec()),
            other => panic!("nested street should be Direct, got {other:?}"),
        }
        match addr.get(&iri("city")).unwrap() {
            ResolvedValue::Direct(o) => assert_eq!(as_value_lexical(&o[0]), b"Springfield".to_vec()),
            other => panic!("nested city should be Direct, got {other:?}"),
        }
        match addr.get(&iri("tags")).unwrap() {
            ResolvedValue::Direct(o) => {
                let mut ls: Vec<Vec<u8>> = o.iter().map(as_value_lexical).collect();
                ls.sort();
                assert_eq!(ls, vec![b"home".to_vec(), b"work".to_vec()]);
            }
            other => panic!("nested tags should be Direct set, got {other:?}"),
        }

        // ---- SOUNDNESS: a corrupted combined root fails. ----
        let mut bad = db_root;
        bad[0] ^= 0xFF;
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, bad)
                .is_err(),
            "a corrupted combined root must fail verify",
        );

        // ---- ADVERSARIAL: DROP the inlined child sub-bundle → verify DEMANDS the link, reject-closes. ----
        bundle.links.clear();
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "a dropped inlined child sub-bundle must fail verify",
        );
    });
}

/// ADVERSARIAL: a `Stop` link whose child is actually a SUBDOCUMENT reject-closes over the delta stack.
/// The honest delta prover INLINES a subdocument child (6b), so a forged `Stop` is an OMISSION — verify
/// RE-DERIVES the child's subdocument nature over `present` (the child's own bound-subject read + its
/// schema closure) and refuses. (This is the 6b lift of the 6a `SOUND-not-silent` reject: instead of
/// deferring, we now inline the honest case and still reject the dishonest `Stop`.)
#[test]
fn read_document_reconciled_stop_over_subdocument_rejects() {
    on_big_stack("read-document-reconciled-stop-over-subdoc", 1024, || {
        let schema = subdoc_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));
        let layers = delta_stack(subdoc_base(), subdoc_delta());
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();

        // Sanity: the honest FLAT subdocument INLINES + verifies.
        bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();

        // FORGE a STOP over the subdocument child: rebuild the `addr` link as a plain-link `Stop` witness
        // carrying the child's OWN reconciled type read + schema (Address does not inherit ⇒ no strict
        // ancestors, no closure). Verify re-derives Address's `sys:subdocument` nature and reject-closes.
        let idx = bundle
            .links
            .iter()
            .position(|l| l.child_id == iri("addr1").into_bytes())
            .expect("the honest delta bundle inlines the addr subdocument child");
        let link = bundle.links.remove(idx);
        let predicate = link.predicate.clone();
        let child_id = link.child_id.clone();
        let child = match link.target {
            ProvedLinkTarget::Inline(child) => *child,
            _ => panic!("the addr link should be inlined on the honest delta bundle"),
        };
        bundle.links.push(ProvedLink {
            predicate,
            child_id,
            target: ProvedLinkTarget::Stop(ProvedChildTypeWitness {
                subject_bgp: child.subject_bgp,
                class_schema: Some(child.frame.class_schema),
                ancestor_closure: child.frame.ancestor_closure,
                ancestor_reads: Vec::new(),
            }),
        });
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "a Stop over a re-derived subdocument child must fail verify (missing inlined body)",
        );
    });
}

/// A `C` with TWO subdocument links (`meta`, `note`) to the same subdocument class `Meta` — the fixture
/// for the injected/duplicate-child cross-check.
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

fn two_subdoc_stack() -> Vec<Arc<LayerCommitment>> {
    let base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Node(iri("doc"), iri("meta"), iri("m1")),
        Fact::Node(iri("doc"), iri("note"), iri("n1")),
        Fact::Node(iri("m1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Str(iri("m1"), iri("title"), "a".into()),
    ];
    let delta = vec![
        Fact::Node(iri("n1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Str(iri("n1"), iri("title"), "b".into()),
    ];
    delta_stack(base, delta)
}

/// ADVERSARIAL: an INJECTED (un-demanded) inlined child sub-bundle reject-closes over the delta stack.
/// Re-key the SECOND inlined child onto the FIRST's `(predicate, child)` → the verifier's per-link
/// cross-check sees a duplicate/injected sub-bundle (present ⇒ reject). The cross-check is source-
/// agnostic, so it fires on the delta path exactly as on the materialized path.
#[test]
fn read_document_reconciled_injected_child_rejects() {
    on_big_stack("read-document-reconciled-subdoc-injection", 1024, || {
        let schema = two_subdoc_schema();
        let schema_root = schema.state.commitment_root;
        let compiled = compiled_read(&iri("doc"));
        let layers = two_subdoc_stack();
        let lref = refs(&layers);
        let instance_head_root = lref.last().unwrap().state.commitment_root;
        let db_root = database_root(&instance_head_root, &schema_root);
        let mut bundle = compiled.prove_on_layers_reconciled(&lref, &*schema).unwrap();

        // Sanity: the honest two-subdocument document INLINES both + verifies.
        bundle
            .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
            .unwrap();
        assert_eq!(bundle.links.len(), 2);

        // Re-key the second link onto the first's (predicate, child) → an injected/duplicate sub-bundle.
        let predicate = bundle.links[0].predicate.clone();
        let child_id = bundle.links[0].child_id.clone();
        bundle.links[1].predicate = predicate;
        bundle.links[1].child_id = child_id;
        assert!(
            bundle
                .verify_over_graphs_and_resolve_reconciled(&compiled, &lref, &*schema, db_root)
                .is_err(),
            "an injected/duplicate inlined child sub-bundle must fail verify",
        );
    });
}

/// ADVERSARIAL: a subdocument CYCLE (`m1 -> m1` via a subdoc-typed property) reject-closes rather than
/// loops. The recursion's `visited` set — enforced IDENTICALLY over the reconciled source — catches the
/// repeat and raises `ReadDocumentSubdocumentCycle` at PROVE time (before any unbounded recursion).
#[test]
fn read_document_reconciled_subdocument_cycle_rejects() {
    let schema = layer(vec![
        Fact::Node(iri("C"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("C"), iri("meta"), iri("Meta")),
        Fact::Node(iri("Meta"), RDF_TYPE.into(), SYS_CLASS.into()),
        Fact::Node(iri("Meta"), SYS_SUBDOC.into(), RDF_NIL.into()),
        Fact::Node(iri("Meta"), iri("title"), XSD_STRING.into()),
        Fact::Node(iri("Meta"), iri("note"), XSD_STRING.into()),
        Fact::Node(iri("Meta"), iri("selfref"), iri("Meta")),
    ]);
    let base = vec![
        Fact::Node(iri("doc"), RDF_TYPE.into(), iri("C")),
        Fact::Node(iri("doc"), iri("meta"), iri("m1")),
        Fact::Node(iri("m1"), RDF_TYPE.into(), iri("Meta")),
        Fact::Node(iri("m1"), iri("selfref"), iri("m1")),
    ];
    let delta = vec![
        Fact::Str(iri("m1"), iri("title"), "x".into()),
        Fact::Str(iri("m1"), iri("note"), "y".into()),
    ];
    let layers = delta_stack(base, delta);
    assert_reconciled_prove_rejects("reconciled-subdoc-cycle", schema, layers, |e| {
        matches!(e, QueryProofError::Plan(BgpPlanError::ReadDocumentSubdocumentCycle))
    });
}

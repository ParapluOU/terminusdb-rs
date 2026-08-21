//! CAPSTONE — end-to-end reconciled delta-stack read: compile a REAL woql2 query, PROVE it over
//! a base+deltas layer chain via the reconcile + present-BGP path, VERIFY + RESOLVE without a
//! re-run, and check the resolved canonical answer equals the query over the MATERIALIZED set.
//!
//! The lowering is GENERAL (matching the base path's BGP coverage over a delta stack): n-atom
//! joins, constants in any position, repeated-variable same-row equality, and numeric ranges.

mod proof_common;
use proof_common::*;

use std::sync::Arc;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::{open_sync_memory_store, ValueTriple};

use tdb_succinct::TdbDataType;

use terminusdb_woql2::query::Query;
use terminusdb_woql2::reconciled::{
    is_reconciled_read, prove_reconciled_read, verify_and_resolve_reconciled_read,
};
use terminusdb_woql2::triple::{AddedTriple, DeletedTriple};
use terminusdb_woql2::value::{NodeValue, Value};

/// `AddedTriple(?subj, pred, ?obj)` — reads THIS commit's additions file.
fn added_triple(subj: &str, pred: &str, obj: &str) -> Query {
    Query::AddedTriple(AddedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

/// `DeletedTriple(?subj, pred, ?obj)` — reads THIS commit's removals file.
fn deleted_triple(subj: &str, pred: &str, obj: &str) -> Query {
    Query::DeletedTriple(DeletedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

fn node(s: &str) -> CanonicalObject {
    CanonicalObject::Node(iri(s).into_bytes())
}

/// The canonical `xsd:unsignedInt` value object as the reconciled resolver renders it.
fn uint(v: u32) -> CanonicalObject {
    CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical: v.to_be_bytes().to_vec() }
}

fn refs(layers: &[Arc<LayerCommitment>]) -> Vec<&LayerCommitment> {
    layers.iter().map(|a| a.as_ref()).collect()
}

/// Transpose per-column resolved objects into rows.
fn rows_of(cols: &[Vec<CanonicalObject>]) -> Vec<Vec<CanonicalObject>> {
    if cols.is_empty() {
        return vec![];
    }
    let n = cols[0].len();
    (0..n).map(|i| cols.iter().map(|c| c[i].clone()).collect()).collect()
}

fn bag_eq(mut a: Vec<Vec<CanonicalObject>>, mut b: Vec<Vec<CanonicalObject>>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let key = |r: &Vec<CanonicalObject>| format!("{r:?}");
    a.sort_by_key(&key);
    b.sort_by_key(&key);
    a == b
}

/// Base {a1..a4 p1 b1..b4}; delta removes {a1,a2} and adds {a3->b5, a5->b6}.
/// present p1 = {a3->b3, a4->b4, a3->b5, a5->b6}.
fn filter_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, o) in [("a1", "b1"), ("a2", "b2"), ("a3", "b3"), ("a4", "b4")] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    for (s, o) in [("a1", "b1"), ("a2", "b2")] {
        child.remove_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(o))).unwrap();
    }
    for (s, o) in [("a3", "b5"), ("a5", "b6")] {
        child.add_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(o))).unwrap();
    }
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// present p1 = {x1->y1, x2->y1, x5->y1}, p2 = {y1->z1, y1->z2, y1->z5}.
fn join_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, p, o) in [
        ("x1", "p1", "y1"), ("x2", "p1", "y1"), ("y1", "p2", "z1"),
        ("y1", "p2", "z2"), ("x3", "p1", "y3"), ("y4", "p2", "z4"),
    ] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("x3"), &iri("p1"), &iri("y3"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("y4"), &iri("p2"), &iri("z4"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("x5"), &iri("p1"), &iri("y1"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("y1"), &iri("p2"), &iri("z5"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// A base+delta stack whose present supports a 3-atom chain
/// Triple(?x, p1, ?y) & Triple(?y, p2, ?z) & Triple(?z, p3, ?w):
///   present p1 = {x1->y1, x2->y1, x4->y1}   (base x3->y3 removed; delta adds x4->y1)
///   present p2 = {y1->z1}                    (base y1->z9 removed)
///   present p3 = {z1->w1, z1->w2, z1->w3}    (delta adds z1->w3)
/// Join on ?y=y1 (3 x) and ?z=z1 (3 w) => 3 x-bindings x 3 w-bindings = 9 solutions.
fn bgp_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, p, o) in [
        ("x1", "p1", "y1"), ("x2", "p1", "y1"), ("y1", "p2", "z1"),
        ("z1", "p3", "w1"), ("z1", "p3", "w2"),
        ("x3", "p1", "y3"), ("y1", "p2", "z9"),
    ] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("x3"), &iri("p1"), &iri("y3"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("y1"), &iri("p2"), &iri("z9"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("x4"), &iri("p1"), &iri("y1"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("z1"), &iri("p3"), &iri("w3"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// A stack whose present has self-loops (subject==object) and non-loops:
/// present pself = {a1->a1, a2->a2, a3->b3, a5->a5, a6->b6}. Triple(?s, pself, ?s) => {a1,a2,a5}.
fn self_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, o) in [("a1", "a1"), ("a2", "a2"), ("a3", "b3"), ("a4", "a4"), ("a7", "a7")] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri("pself"), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("a4"), &iri("pself"), &iri("a4"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("a7"), &iri("pself"), &iri("a7"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("a5"), &iri("pself"), &iri("a5"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("a6"), &iri("pself"), &iri("b6"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// A cyclic 2-atom stack: present p1 = {x1->y1, x2->y2, x3->y3}, p2 = {y1->x1, y2->x9, y3->x7}.
/// Triple(?x, p1, ?y) & Triple(?y, p2, ?x) is a composite (x AND y) self-join that keeps only the
/// round-trip x1<->y1 => {(x1,y1)}. The delta ADDS two records and REMOVES two records so each
/// committed group satisfies the reconcile pass's ">= 2 records per non-empty group" rule.
fn cyclic_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, p, o) in [
        ("x1", "p1", "y1"), ("x2", "p1", "y2"), ("y1", "p2", "x1"),
        ("y2", "p2", "x9"), ("x8", "p1", "y8"), ("y8", "p2", "x8"),
    ] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    // Remove the base round-trip x8<->y8 so present's only p1&p2 cycle is x1<->y1, and add a
    // non-cycling pair x3->y3 (p1) / y3->x7 (p2) — x7 != x3, so no new cycle.
    child.remove_value_triple(ValueTriple::new_node(&iri("x8"), &iri("p1"), &iri("y8"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("y8"), &iri("p2"), &iri("x8"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("x3"), &iri("p1"), &iri("y3"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("y3"), &iri("p2"), &iri("x7"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// present ages: alice=30, bob=15, carol=18, dave=40, erin=18, grace=25 (frank=99 removed,
/// grace=25 added). Each person also has a `name` node edge.
fn age_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (person, age) in [
        ("alice", 30u32), ("bob", 15), ("carol", 18), ("dave", 40), ("erin", 18), ("frank", 99),
    ] {
        b.add_value_triple(ValueTriple::new_value(&iri(person), &iri("age"), u32::make_entry(&age)))
            .unwrap();
        b.add_value_triple(ValueTriple::new_node(&iri(person), &iri("name"), &iri(&format!("{person}_n"))))
            .unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_value(&iri("frank"), &iri("age"), u32::make_entry(&99u32))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("frank"), &iri("name"), &iri("frank_n"))).unwrap();
    child.add_value_triple(ValueTriple::new_value(&iri("grace"), &iri("age"), u32::make_entry(&25u32))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("grace"), &iri("name"), &iri("grace_n"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// Prove + verify + resolve one query over `layers`, asserting the resolved answer bag-equals
/// `expected` AND a wrong chain anchor (the base root) rejects.
fn prove_resolve_expect(
    layers: &[Arc<LayerCommitment>],
    query: &Query,
    expected: Vec<Vec<CanonicalObject>>,
) -> Vec<Vec<CanonicalObject>> {
    let lref = refs(layers);
    assert!(is_reconciled_read(query), "query must route to the reconciled path");
    let proof = prove_reconciled_read(query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(query, &lref, head_root, &proof).unwrap();
    assert!(bag_eq(rows_of(&resolved), expected), "resolved must equal materialized");

    // Soundness: a wrong chain anchor (the base root, not the head) rejects.
    let base_root = lref[0].resolution_root();
    assert!(verify_and_resolve_reconciled_read(query, &lref, base_root, &proof).is_err());
    resolved
}

#[test]
fn woql_single_atom_filter_over_delta_stack() {
    let layers = filter_stack();
    let query = tv("s", "p1", "o"); // Triple(?s, p1, ?o)
    // The resolved answer equals the query over the materialized present set (real IRIs).
    let expected = vec![
        vec![node("a3"), node("b3")],
        vec![node("a4"), node("b4")],
        vec![node("a3"), node("b5")],
        vec![node("a5"), node("b6")],
    ];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_two_atom_join_over_delta_stack() {
    let layers = join_stack();
    // Triple(?x, p1, ?y) & Triple(?y, p2, ?z)
    let query = and_of(vec![tv("x", "p1", "y"), tv("y", "p2", "z")]);
    let mut expected = Vec::new();
    for x in ["x1", "x2", "x5"] {
        for z in ["z1", "z2", "z5"] {
            expected.push(vec![node(x), node("y1"), node(z)]);
        }
    }
    let resolved = prove_resolve_expect(&layers, &query, expected);
    assert_eq!(resolved.len(), 3, "join delivers ?x, ?y, ?z");
}

#[test]
fn woql_three_atom_bgp_over_delta_stack() {
    // GENERAL n-atom BGP: Triple(?x, p1, ?y) & Triple(?y, p2, ?z) & Triple(?z, p3, ?w) — 9 rows.
    let layers = bgp_stack();
    let query = and_of(vec![
        tv("x", "p1", "y"),
        tv("y", "p2", "z"),
        tv("z", "p3", "w"),
    ]);
    let mut expected = Vec::new();
    for x in ["x1", "x2", "x4"] {
        for w in ["w1", "w2", "w3"] {
            expected.push(vec![node(x), node("y1"), node("z1"), node(w)]);
        }
    }
    let resolved = prove_resolve_expect(&layers, &query, expected);
    assert_eq!(resolved.len(), 4, "3-chain delivers ?x, ?y, ?z, ?w");
    assert_eq!(resolved[0].len(), 9, "3-atom chain over the delta stack has 9 solutions");
}

#[test]
fn woql_subject_constant_join_over_delta_stack() {
    // Constant in the subject position: Triple(x1, p1, ?y) & Triple(?y, p2, ?z) pins ?y=y1.
    let layers = bgp_stack();
    let query = and_of(vec![tv_subj_const("x1", "p1", "y"), tv("y", "p2", "z")]);
    let expected = vec![vec![node("y1"), node("z1")]];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_range_filter_over_delta_stack() {
    // Numeric object RANGE over the delta stack: Triple(?x, age, ?v) & ?v >= 18.
    // present ages >= 18: alice=30, carol=18, dave=40, erin=18, grace=25.
    let layers = age_stack();
    let query = and_of(vec![tv("x", "age", "v"), gte(dvar("v"), duint(18))]);
    let expected = vec![
        vec![node("alice"), uint(30)],
        vec![node("carol"), uint(18)],
        vec![node("dave"), uint(40)],
        vec![node("erin"), uint(18)],
        vec![node("grace"), uint(25)],
    ];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_range_join_over_delta_stack() {
    // Range + join: Triple(?x, age, ?v)[?v >= 18] & Triple(?x, name, ?n). Left-deep chain output
    // order is [v, x, n] (r_other=[v] after dropping the acc key x, then key x, then s_other=[n]).
    let layers = age_stack();
    let query = and_of(vec![
        tv("x", "age", "v"),
        gte(dvar("v"), duint(18)),
        tv("x", "name", "n"),
    ]);
    let expected = vec![
        vec![uint(30), node("alice"), node("alice_n")],
        vec![uint(18), node("carol"), node("carol_n")],
        vec![uint(40), node("dave"), node("dave_n")],
        vec![uint(18), node("erin"), node("erin_n")],
        vec![uint(25), node("grace"), node("grace_n")],
    ];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_repeated_var_single_atom_over_delta_stack() {
    // Triple(?s, pself, ?s) — the S==O same-row equality is now SUPPORTED (self-join). present
    // self-loops = {a1,a2,a5}.
    let layers = self_stack();
    let query = tv("s", "pself", "s");
    let expected = vec![vec![node("a1")], vec![node("a2")], vec![node("a5")]];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_cyclic_join_over_delta_stack() {
    // Triple(?x, p1, ?y) & Triple(?y, p2, ?x) — the ?x reuse (atom2.object == atom1.subject) is
    // now SUPPORTED (cyclic join). present round-trips = {(x1, y1)}.
    let layers = cyclic_stack();
    let query = and_of(vec![tv("x", "p1", "y"), tv("y", "p2", "x")]);
    let expected = vec![vec![node("x1"), node("y1")]];
    prove_resolve_expect(&layers, &query, expected);
}

/// Base {a1..a3 p1, a4 p2}; delta removes {a1,a2 p1} and adds {a5,a6 p1}. The head commit's
/// ADDITIONS file == {a5->b5, a6->b6}; REMOVALS file == {a1->b1, a2->b2} (ANCESTOR/base terms);
/// p2 (a4->c4) is inherited-only. Both delta groups have >= 2 rows (the group-domain floor).
fn delta_read_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (s, p, o) in [("a1", "p1", "b1"), ("a2", "p1", "b2"), ("a3", "p1", "b3"), ("a4", "p2", "c4")] {
        b.add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("a1"), &iri("p1"), &iri("b1"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("a2"), &iri("p1"), &iri("b2"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("a5"), &iri("p1"), &iri("b5"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("a6"), &iri("p1"), &iri("b6"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

#[test]
fn woql_added_triple_over_delta_stack() {
    // WOQL `AddedTriple(?s, p1, ?o)` proves EXACTLY this commit's p1 additions over the chain,
    // routed through the restored delta-read path (predicate p1 is an ANCESTOR-defined constant,
    // resolved via the combined chain dictionaries).
    let layers = delta_read_stack();
    let query = added_triple("s", "p1", "o");
    let expected = vec![vec![node("a5"), node("b5")], vec![node("a6"), node("b6")]];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_deleted_triple_over_delta_stack() {
    // WOQL `DeletedTriple(?s, p1, ?o)` proves EXACTLY this commit's p1 removals — both reference
    // ancestor (base-defined) terms, authenticated through the chain dictionaries.
    let layers = delta_read_stack();
    let query = deleted_triple("s", "p1", "o");
    let expected = vec![vec![node("a1"), node("b1")], vec![node("a2"), node("b2")]];
    prove_resolve_expect(&layers, &query, expected);
}

#[test]
fn woql_added_triple_inherited_predicate_is_empty() {
    // p2 appears only on the inherited base row a4->c4 (no addition this commit), so
    // `AddedTriple(?s, p2, ?o)` is empty: an inherited/materialized row is NOT an AddedTriple.
    let layers = delta_read_stack();
    let lref = refs(&layers);
    let query = added_triple("s", "p2", "o");
    assert!(is_reconciled_read(&query));
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    assert!(rows_of(&resolved).is_empty(), "an inherited row is not provable as an AddedTriple");
}

#[test]
fn woql_delta_read_source_substitution_rejects() {
    // Anti-substitution across the ROUTE: an AddedTriple proof (same BGP shape) must not verify
    // as a DeletedTriple or a materialized Triple query — the source is part of the binding.
    let layers = delta_read_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().resolution_root();
    let added = added_triple("s", "p1", "o");
    let proof = prove_reconciled_read(&added, &lref).unwrap();
    let deleted = deleted_triple("s", "p1", "o");
    let materialized = tv("s", "p1", "o");
    assert!(
        verify_and_resolve_reconciled_read(&deleted, &lref, head_root, &proof).is_err(),
        "an AddedTriple proof must not verify as a DeletedTriple"
    );
    assert!(
        verify_and_resolve_reconciled_read(&materialized, &lref, head_root, &proof).is_err(),
        "an AddedTriple proof must not verify as a materialized Triple"
    );
    // Sanity: it verifies against its own query.
    assert!(verify_and_resolve_reconciled_read(&added, &lref, head_root, &proof).is_ok());
}

#[test]
fn woql_mixed_source_conjunction_reject_closes() {
    // A CROSS-SOURCE conjunction (AddedTriple ⋈ materialized Triple) is not routable: each store
    // read path reads a SINGLE committed group, so a mixed-source BGP reject-closes.
    let layers = delta_read_stack();
    let lref = refs(&layers);
    let mixed = and_of(vec![added_triple("s", "p1", "o"), tv("s", "p1", "o")]);
    assert!(!is_reconciled_read(&mixed));
    assert!(prove_reconciled_read(&mixed, &lref).is_err());
}

#[test]
fn woql_tampered_result_rejects() {
    let layers = filter_stack();
    let lref = refs(&layers);
    let query = tv("s", "p1", "o");
    let mut proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    // A fabricated result binding fails the re-commitment bind to the authenticated result.
    proof.result.columns[0][0] = ArkFr::from_u64(123_456_789);
    assert!(verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).is_err());
}

#[test]
fn woql_unsupported_shape_reject_closes() {
    let layers = filter_stack();
    let lref = refs(&layers);

    // A predicate-VARIABLE single atom reject-closes (present authenticates a constant predicate).
    let pred_var = tv_pred_var("s", "p", "o");
    assert!(!is_reconciled_read(&pred_var));
    assert!(prove_reconciled_read(&pred_var, &lref).is_err());

    // A fully-ground atom keeps no variable on the present path — reject-close.
    let ground = tv_ground("a3", "p1", "b3");
    assert!(!is_reconciled_read(&ground));

    // A non-BGP construct (`Or`) does not compile as a BGP — reject-close via plan_bgp.
    let non_bgp = terminusdb_woql2::query::Query::Or(terminusdb_woql2::query::Or {
        or: vec![tv("s", "p1", "o"), tv("s", "p2", "o")],
    });
    assert!(!is_reconciled_read(&non_bgp));
    assert!(prove_reconciled_read(&non_bgp, &lref).is_err());

    // A schema-graph query reject-closes (only the instance graph is proven).
    let schema = terminusdb_woql2::query::Query::Triple(terminusdb_woql2::triple::Triple {
        subject: terminusdb_woql2::value::NodeValue::Variable("s".into()),
        predicate: terminusdb_woql2::value::NodeValue::Node(iri("p1")),
        object: terminusdb_woql2::value::Value::Variable("o".into()),
        graph: Some(terminusdb_schema::GraphType::Schema),
    });
    assert!(!is_reconciled_read(&schema));
}

#[test]
fn woql_verify_rejects_proof_for_a_different_query() {
    let layers = filter_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().resolution_root();

    // A valid proof for Triple(?s, p1, ?o) ...
    let proved_query = tv("s", "p1", "o");
    let proof = prove_reconciled_read(&proved_query, &lref).unwrap();
    // ... must NOT verify in the name of a DIFFERENT supported query (different predicate constant).
    let other_query = tv("s", "p2", "o");
    assert!(is_reconciled_read(&other_query));
    assert!(
        verify_and_resolve_reconciled_read(&other_query, &lref, head_root, &proof).is_err(),
        "a proof for p1 must not verify as the answer to the p2 query"
    );
    // Anti-substitution also holds for a different SHAPE (a 2-atom join proof vs a single atom).
    let join_layers = join_stack();
    let jref = refs(&join_layers);
    let join_query = and_of(vec![tv("x", "p1", "y"), tv("y", "p2", "z")]);
    let join_proof = prove_reconciled_read(&join_query, &jref).unwrap();
    let jhead = jref.last().unwrap().resolution_root();
    assert!(
        verify_and_resolve_reconciled_read(&proved_query, &jref, jhead, &join_proof).is_err(),
        "a 2-atom join proof must not verify as the answer to a single-atom query"
    );
    // Sanity: each DOES verify against its own query.
    assert!(verify_and_resolve_reconciled_read(&proved_query, &lref, head_root, &proof).is_ok());
    assert!(verify_and_resolve_reconciled_read(&join_query, &jref, jhead, &join_proof).is_ok());
}

/// C4-slice-2: the reconciled MATERIALIZED-present byte-in wrapper
/// (`decode_and_verify_reconciled_read_bgp`) must REJECT a delta-source (`AddedTriple` /
/// `DeletedTriple`) query as a shape mismatch — a delta read ships a `DeltaReadProof` and goes
/// through `decode_and_verify_reconciled_delta_read` instead. The guard fires on the re-lowered
/// route BEFORE the proof bytes are touched, so no real proof is needed here (the store test
/// `c4_reconcile_read_byte_in_matches_in_memory_and_rejects_substitution` covers the positive E2E).
#[test]
fn woql_reconciled_read_bgp_byte_in_rejects_delta_source_query() {
    use terminusdb_woql2::reconciled::{decode_and_verify_reconciled_read_bgp, ReconciledReadError};
    let layers = filter_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().resolution_root();

    let delta_query = added_triple("s", "p1", "o");
    let err = decode_and_verify_reconciled_read_bgp(&delta_query, &[], &[], head_root)
        .expect_err("a delta-source query must be rejected by the materialized-present byte-in path");
    assert!(
        matches!(err, ReconciledReadError::UnsupportedShape(_)),
        "delta-source rejection must be an UnsupportedShape shape mismatch, got {err:?}"
    );

    // A materialized (`Triple`) query passes the route guard — it fails LATER (empty commitment
    // chain), NOT with the shape-mismatch UnsupportedShape, confirming the guard is route-specific.
    let mat_query = tv("s", "p1", "o");
    let err2 = decode_and_verify_reconciled_read_bgp(&mat_query, &[], &[], head_root)
        .expect_err("an empty commitment chain must still error");
    assert!(
        !matches!(err2, ReconciledReadError::UnsupportedShape(_)),
        "a materialized query must pass the route guard (fail later, not as a shape mismatch), got {err2:?}"
    );
}

// ============================================================================================
// ORDER — single-column, full-key, numeric OrderBy over the reconciled materialized present.
// ============================================================================================

use terminusdb_woql2::order::{Order, OrderBy, OrderTemplate};

/// A base+delta stack where subject `item` has many `score` (UInt32) values, so
/// `Triple(<item>, score, ?score)` binds ONE numeric object column with several rows.
/// present item scores = {30, 10, 20, 25, 40}  (base 5, 50 removed; 25, 40 added).
fn order_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for score in [30u32, 10, 20, 50, 5] {
        b.add_value_triple(ValueTriple::new_value(&iri("item"), &iri("score"), u32::make_entry(&score)))
            .unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_value(&iri("item"), &iri("score"), u32::make_entry(&5u32))).unwrap();
    child.remove_value_triple(ValueTriple::new_value(&iri("item"), &iri("score"), u32::make_entry(&50u32))).unwrap();
    child.add_value_triple(ValueTriple::new_value(&iri("item"), &iri("score"), u32::make_entry(&25u32))).unwrap();
    child.add_value_triple(ValueTriple::new_value(&iri("item"), &iri("score"), u32::make_entry(&40u32))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// `OrderBy([score <order>], Triple(<item>, score, ?score) & ?score >= 0)` — the `>= 0` range pins
/// `?score`'s datatype to UInt32 (a full-key single-column numeric order the reconciled path proves).
fn order_by_item_score(order: Order) -> Query {
    Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "score".into(), order }],
        query: Box::new(and_of(vec![
            tv_subj_const("item", "score", "score"),
            gte(dvar("score"), duint(0)),
        ])),
    })
}

#[test]
fn woql_order_by_ascending_over_delta_stack() {
    let layers = order_stack();
    let lref = refs(&layers);
    let query = order_by_item_score(Order::Asc);
    assert!(is_reconciled_read(&query), "single-column numeric OrderBy must route to the reconciled path");
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    // The ordered answer is ONE column, ASCENDING by numeric value (NOT raw dictionary-id order).
    assert_eq!(
        rows_of(&resolved),
        vec![vec![uint(10)], vec![uint(20)], vec![uint(25)], vec![uint(30)], vec![uint(40)]],
        "resolved scores must come back in ascending numeric order"
    );
    // Soundness: a wrong chain anchor (the base root, not the head) rejects.
    let base_root = lref[0].resolution_root();
    assert!(verify_and_resolve_reconciled_read(&query, &lref, base_root, &proof).is_err());
}

#[test]
fn woql_order_by_descending_over_delta_stack() {
    let layers = order_stack();
    let lref = refs(&layers);
    let query = order_by_item_score(Order::Desc);
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    assert_eq!(
        rows_of(&resolved),
        vec![vec![uint(40)], vec![uint(30)], vec![uint(25)], vec![uint(20)], vec![uint(10)]],
        "resolved scores must come back in descending numeric order"
    );
    // Anti-substitution: the descending proof must NOT verify in the ascending query's name.
    let ascending = order_by_item_score(Order::Asc);
    assert!(
        verify_and_resolve_reconciled_read(&ascending, &lref, head_root, &proof).is_err(),
        "a descending order proof must not verify as an ascending order"
    );
}

#[test]
fn woql_order_by_tampered_result_rejects() {
    // A REORDERED claimed result (swap two rows) fails the resolution re-commitment bind to the
    // authenticated ordered output — a permuted/wrong-order answer is rejected.
    let layers = order_stack();
    let lref = refs(&layers);
    let query = order_by_item_score(Order::Asc);
    let mut proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    proof.result.columns[0].swap(0, 1);
    assert!(verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).is_err());
}

#[test]
fn woql_order_by_unsupported_shapes_reject_close() {
    let layers = age_stack();
    let lref = refs(&layers);

    // SUBSET-key: a free-subject two-column BGP `[x, v]` sorted by `v` alone has an unprovable
    // invented tie-break over the unnamed `x` column, so it reject-closes (still UnsupportedShape).
    let subset = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "v".into(), order: Order::Asc }],
        query: Box::new(and_of(vec![tv("x", "age", "v"), gte(dvar("v"), duint(0))])),
    });
    assert!(!is_reconciled_read(&subset), "subset-key order must reject-close");
    assert!(prove_reconciled_read(&subset, &lref).is_err());

    // NON-NUMERIC: ordering by a NODE column (`name`, no order-preserving numeric datatype)
    // reject-closes.
    let non_numeric = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "n".into(), order: Order::Asc }],
        query: Box::new(tv_subj_const("alice", "name", "n")),
    });
    assert!(!is_reconciled_read(&non_numeric), "non-numeric order must reject-close");
    assert!(prove_reconciled_read(&non_numeric, &lref).is_err());

    // MULTI-KEY: a two-key order over the two-column `[x, v]` BGP is out of this single-column
    // slice (a multi-key `ExactMultiOrder`, or here rejected earlier because `x` is a non-numeric
    // node key) — either way the reconciled path does not accept it.
    let multi_key = Query::OrderBy(OrderBy {
        ordering: vec![
            OrderTemplate { variable: "v".into(), order: Order::Asc },
            OrderTemplate { variable: "x".into(), order: Order::Asc },
        ],
        query: Box::new(and_of(vec![tv("x", "age", "v"), gte(dvar("v"), duint(0))])),
    });
    assert!(!is_reconciled_read(&multi_key), "multi-key order is out of the single-column slice");
}

// ===========================================================================
// FIDELITY: numeric range / order datatype-HOMOGENEITY guard over the DELTA stack.
//
// The present-BGP range path (`reconcile/present_bgp.rs`) and the D1a present-order path
// (`reconcile/present_order.rs`) must reject-close a compared column that is not
// datatype-homogeneous over the predicate-matched rows — TerminusDB compares rational-family
// numerics across subtypes by value (and errors on rational-vs-float), so the exact-datatype
// mask would silently under-report. See `proof_prove.rs` for the model.
// ===========================================================================

/// Like `age_stack`, but alice's age is `xsd:int` (Int32) while every other age is
/// `xsd:unsignedInt` (UInt32) — a MIXED rational compared column under one predicate `age`.
fn mixed_rational_age_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    b.add_value_triple(ValueTriple::new_value(&iri("alice"), &iri("age"), i32::make_entry(&30i32)))
        .unwrap();
    b.add_value_triple(ValueTriple::new_node(&iri("alice"), &iri("name"), &iri("alice_n")))
        .unwrap();
    for (person, age) in [("bob", 15u32), ("carol", 18), ("dave", 40), ("erin", 18), ("frank", 99)] {
        b.add_value_triple(ValueTriple::new_value(&iri(person), &iri("age"), u32::make_entry(&age)))
            .unwrap();
        b.add_value_triple(ValueTriple::new_node(&iri(person), &iri("name"), &iri(&format!("{person}_n"))))
            .unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_value(&iri("frank"), &iri("age"), u32::make_entry(&99u32))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("frank"), &iri("name"), &iri("frank_n"))).unwrap();
    child.add_value_triple(ValueTriple::new_value(&iri("grace"), &iri("age"), u32::make_entry(&25u32))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("grace"), &iri("name"), &iri("grace_n"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

#[test]
fn woql_mixed_rational_range_over_delta_stack_reject_closes() {
    // present-BGP RANGE path: Triple(?x, age, ?v) & ?v >= 18 over a MIXED rational column
    // (alice=30^^xsd:int, the rest ^^xsd:unsignedInt). The predicate-matched (age) rows are
    // not datatype-homogeneous → reject-close (the exact-datatype mask would drop alice).
    let layers = mixed_rational_age_stack();
    let lref = refs(&layers);
    let query = and_of(vec![tv("x", "age", "v"), gte(dvar("v"), duint(18))]);
    assert!(is_reconciled_read(&query));
    let err = match prove_reconciled_read(&query, &lref) {
        Ok(_) => panic!("expected a datatype-homogeneity reject-close, but prove succeeded"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("RangeDatatypeNotHomogeneous"),
        "expected a datatype-homogeneity reject-close over the delta stack, got: {err}"
    );
}

#[test]
fn woql_mixed_rational_order_over_delta_stack_reject_closes() {
    // D1a present-ORDER path: OrderBy([v ASC], Triple(metric, reading, ?v)) over a MIXED
    // rational multi-valued column reject-closes (present_order's completeness guard).
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for v in [10u32, 30u32] {
        b.add_value_triple(ValueTriple::new_value(&iri("metric"), &iri("reading"), u32::make_entry(&v))).unwrap();
    }
    b.add_value_triple(ValueTriple::new_value(&iri("metric"), &iri("reading"), i32::make_entry(&20i32))).unwrap();
    b.add_value_triple(ValueTriple::new_node(&iri("metric"), &iri("kind"), &iri("sensor"))).unwrap();
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.add_value_triple(ValueTriple::new_value(&iri("metric"), &iri("reading"), u32::make_entry(&40u32))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("other"), &iri("kind"), &iri("sensor"))).unwrap();
    let head = child.commit().unwrap();
    let layers = vec![base_lc, head.proof_commitment().unwrap()];
    let lref = refs(&layers);
    // The `>= 0` range pins the numeric order datatype; over the MIXED delta column the
    // datatype-homogeneity guard reject-closes the D1a ordered read end to end.
    let query = Query::OrderBy(terminusdb_woql2::order::OrderBy {
        ordering: vec![terminusdb_woql2::order::OrderTemplate {
            variable: "v".into(),
            order: terminusdb_woql2::order::Order::Asc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("metric", "reading", "v"),
            gte(dvar("v"), duint(0)),
        ])),
    });
    assert!(is_reconciled_read(&query), "OrderBy over the delta stack must route to reconciled");
    let err = match prove_reconciled_read(&query, &lref) {
        Ok(_) => panic!("expected an order datatype reject-close, but prove succeeded"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("datatype"),
        "expected an order datatype reject-close over the delta stack, got: {err}"
    );
}

// ============================================================================================
// COUNT — `Count(BGP)` over the reconciled materialized present. The cardinality is the present-BGP
// result-relation row count (`result_len`), already chain-authenticated (the join-count identity
// inside the BGP chain, resp. the single-atom filter count); no value-authentication and no new
// gadget is required. The delivered answer is a single `UInt64` count cell.
// ============================================================================================

use terminusdb_woql2::misc::Count;
use terminusdb_woql2::value::DataValue;

/// The reconciled `Count`'s single delivered answer cell: a `UInt64` rendering of the cardinality.
fn ucount(v: u64) -> CanonicalObject {
    CanonicalObject::Value { datatype: CanonicalDatatype::UInt64, lexical: v.to_be_bytes().to_vec() }
}

/// `Count(inner, ?n)`.
fn count_of(inner: Query) -> Query {
    Query::Count(Count { query: Box::new(inner), count: DataValue::Variable("n".into()) })
}

#[test]
fn woql_count_over_delta_stack() {
    // present age triples = {alice30, bob15, carol18, dave40, erin18, grace25} (frank removed) => 6.
    let layers = age_stack();
    let lref = refs(&layers);
    let query = count_of(tv("x", "age", "v"));
    assert!(is_reconciled_read(&query), "Count(BGP) must route to the reconciled path");
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    assert_eq!(rows_of(&resolved), vec![vec![ucount(6)]], "Count over the present age relation is 6");
    // Soundness: a wrong chain anchor (the base root, not the head) rejects.
    let base_root = lref[0].resolution_root();
    assert!(verify_and_resolve_reconciled_read(&query, &lref, base_root, &proof).is_err());
}

#[test]
fn woql_count_with_range_filter_over_delta_stack() {
    // Count(Triple(?x, age, ?v) & ?v >= 18) => {alice30, carol18, dave40, erin18, grace25} = 5.
    let layers = age_stack();
    let lref = refs(&layers);
    let query = count_of(and_of(vec![tv("x", "age", "v"), gte(dvar("v"), duint(18))]));
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    assert_eq!(rows_of(&resolved), vec![vec![ucount(5)]], "Count with >= 18 filter is 5");
}

#[test]
fn woql_count_tampered_result_rejects() {
    // A tampered delivered count cell (claim 7 vs the authenticated 6) fails the bind to the
    // chain-authenticated cardinality.
    let layers = age_stack();
    let lref = refs(&layers);
    let query = count_of(tv("x", "age", "v"));
    let mut proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    proof.result.columns[0][0] = ArkFr::from_u64(7);
    assert!(verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).is_err());
}

#[test]
fn woql_count_anti_substitution_rejects_cross_proof() {
    // The Count flag is part of the anti-substitution binding: a plain read proof must NOT verify
    // in a Count query's name, and a Count proof must NOT verify in a plain read's name.
    let layers = age_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().resolution_root();
    let plain = tv("x", "age", "v");
    let counted = count_of(tv("x", "age", "v"));
    let plain_proof = prove_reconciled_read(&plain, &lref).unwrap();
    let count_proof = prove_reconciled_read(&counted, &lref).unwrap();
    assert!(verify_and_resolve_reconciled_read(&counted, &lref, head_root, &plain_proof).is_err());
    assert!(verify_and_resolve_reconciled_read(&plain, &lref, head_root, &count_proof).is_err());
    // Each verifies in its OWN name.
    assert!(verify_and_resolve_reconciled_read(&plain, &lref, head_root, &plain_proof).is_ok());
    assert!(verify_and_resolve_reconciled_read(&counted, &lref, head_root, &count_proof).is_ok());
}

#[test]
fn woql_count_over_delta_source_reject_closes() {
    // Count over a delta source (AddedTriple/DeletedTriple) is out of scope this slice: reject-close.
    let layers = delta_read_stack();
    let lref = refs(&layers);
    let query = count_of(added_triple("s", "p1", "o"));
    assert!(!is_reconciled_read(&query), "Count over a delta source must reject-close");
    assert!(prove_reconciled_read(&query, &lref).is_err());
}

// ============================================================================================
// GROUP-COUNT — `GroupBy(keys, ..) + Length` over the reconciled materialized present via
// VERIFIER-RECOMPUTE. The child rows are authenticated + resolved by the store, then grouped by
// the authenticated key IRIs and counted per group. The grouping is bound to the authenticated
// key column, so a tampered/fabricated child row rejects at resolution; the aggregate itself is
// recomputed (unforgeable). Group keys must be Node IRIs (the base `plan_group_count_form`
// restricts keys to the Node domain — a subject variable).
// ============================================================================================

use terminusdb_woql2::collection::Length;
use terminusdb_woql2::order::GroupBy;

/// present `member` = {d1->a1, d1->a2, d1->a6, d2->a3, d2->a4, d2->a7} (base d3->a5, d3->a8 removed;
/// delta adds d1->a6, d2->a7). Group by the SUBJECT ?dept (Node domain) => d1:3, d2:3. Each committed
/// delta group has >= 2 records (the reconcile pass requires it).
fn group_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let b = store.create_base_layer().unwrap();
    for (dept, person) in [("d1", "a1"), ("d1", "a2"), ("d2", "a3"), ("d2", "a4"), ("d3", "a5"), ("d3", "a8")] {
        b.add_value_triple(ValueTriple::new_node(&iri(dept), &iri("member"), &iri(person))).unwrap();
    }
    let base = b.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("d3"), &iri("member"), &iri("a5"))).unwrap();
    child.remove_value_triple(ValueTriple::new_node(&iri("d3"), &iri("member"), &iri("a8"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("d1"), &iri("member"), &iri("a6"))).unwrap();
    child.add_value_triple(ValueTriple::new_node(&iri("d2"), &iri("member"), &iri("a7"))).unwrap();
    let head = child.commit().unwrap();
    vec![base_lc, head.proof_commitment().unwrap()]
}

/// `GroupBy([dept], count members) over Triple(?dept, member, ?person)` — group by the subject.
fn group_count_by_dept() -> Query {
    and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("person".into()),
            group_by: vec!["dept".into()],
            grouped_value: Value::Variable("people".into()),
            query: Box::new(tv("dept", "member", "person")),
        }),
        Query::Length(Length {
            list: DataValue::Variable("people".into()),
            length: DataValue::Variable("count".into()),
        }),
    ])
}

#[test]
fn woql_group_count_over_delta_stack() {
    let layers = group_stack();
    let lref = refs(&layers);
    let query = group_count_by_dept();
    assert!(is_reconciled_read(&query), "GroupCount must route to the reconciled path");
    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();
    // Each group row is [dept, count]; d1 and d2 each have 3 present members.
    assert!(
        bag_eq(
            rows_of(&resolved),
            vec![vec![node("d1"), ucount(3)], vec![node("d2"), ucount(3)]],
        ),
        "group-count must be d1:3, d2:3, got {:?}",
        rows_of(&resolved)
    );
    // Soundness: a wrong chain anchor (the base root, not the head) rejects.
    let base_root = lref[0].resolution_root();
    assert!(verify_and_resolve_reconciled_read(&query, &lref, base_root, &proof).is_err());
}

#[test]
fn woql_group_count_tampered_result_rejects() {
    // Tampering a delivered child row fails the resolution re-commitment inside the store
    // recompute — a mis-grouped/forged count is not producible.
    let layers = group_stack();
    let lref = refs(&layers);
    let query = group_count_by_dept();
    let mut proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().resolution_root();
    proof.result.columns[1][0] = ArkFr::from_u64(987_654_321);
    assert!(verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).is_err());
}

#[test]
fn woql_group_count_anti_substitution_rejects_cross_proof() {
    // The GroupBy spec is part of the anti-substitution binding: a GroupCount proof must NOT verify
    // in a plain read's name, and a plain read proof must NOT verify in the GroupCount's name.
    let layers = group_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().resolution_root();
    let plain = tv("dept", "member", "person");
    let grouped = group_count_by_dept();
    let plain_proof = prove_reconciled_read(&plain, &lref).unwrap();
    let group_proof = prove_reconciled_read(&grouped, &lref).unwrap();
    assert!(verify_and_resolve_reconciled_read(&grouped, &lref, head_root, &plain_proof).is_err());
    assert!(verify_and_resolve_reconciled_read(&plain, &lref, head_root, &group_proof).is_err());
    // Each verifies in its OWN name.
    assert!(verify_and_resolve_reconciled_read(&plain, &lref, head_root, &plain_proof).is_ok());
    assert!(verify_and_resolve_reconciled_read(&grouped, &lref, head_root, &group_proof).is_ok());
}

#[test]
fn woql_group_count_value_key_reject_closes() {
    // Grouping by a VALUE key (a non-node object variable) reject-closes: the base
    // `plan_group_count_form` restricts group keys to the Node domain, so a value-keyed grouping
    // never compiles to a reconciled read.
    let layers = age_stack();
    let lref = refs(&layers);
    let value_keyed = and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("x".into()),
            group_by: vec!["v".into()], // ?v is a value object (age), not a node
            grouped_value: Value::Variable("xs".into()),
            query: Box::new(tv("x", "age", "v")),
        }),
        Query::Length(Length {
            list: DataValue::Variable("xs".into()),
            length: DataValue::Variable("count".into()),
        }),
    ]);
    assert!(!is_reconciled_read(&value_keyed), "value-keyed grouping must reject-close");
    assert!(prove_reconciled_read(&value_keyed, &lref).is_err());
}

//! CAPSTONE — end-to-end reconciled delta-stack read: compile a REAL woql2 query, PROVE it over
//! a base+deltas layer chain via the reconcile + present-BGP path, VERIFY + RESOLVE without a
//! re-run, and check the resolved canonical answer equals the query over the MATERIALIZED set.

mod proof_common;
use proof_common::*;

use std::sync::Arc;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::CanonicalObject;
use terminus_store::{open_sync_memory_store, ValueTriple};

use terminusdb_woql2::reconciled::{
    is_reconciled_read, prove_reconciled_read, verify_and_resolve_reconciled_read,
    ReconciledReadProof,
};

fn node(s: &str) -> CanonicalObject {
    CanonicalObject::Node(iri(s).into_bytes())
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

#[test]
fn woql_single_atom_filter_over_delta_stack() {
    let layers = filter_stack();
    let lref = refs(&layers);
    let query = tv("s", "p1", "o"); // Triple(?s, p1, ?o)
    assert!(is_reconciled_read(&query));

    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().state.commitment_root;
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();

    // The resolved answer equals the query over the materialized present set (real IRIs).
    let expected = vec![
        vec![node("a3"), node("b3")],
        vec![node("a4"), node("b4")],
        vec![node("a3"), node("b5")],
        vec![node("a5"), node("b6")],
    ];
    assert!(bag_eq(rows_of(&resolved), expected), "resolved filter must equal materialized");

    // Soundness: a wrong chain anchor (the base root, not the head) rejects.
    let base_root = lref[0].state.commitment_root;
    assert!(verify_and_resolve_reconciled_read(&query, &lref, base_root, &proof).is_err());
}

#[test]
fn woql_two_atom_join_over_delta_stack() {
    let layers = join_stack();
    let lref = refs(&layers);
    // Triple(?x, p1, ?y) & Triple(?y, p2, ?z)
    let query = and_of(vec![tv("x", "p1", "y"), tv("y", "p2", "z")]);
    assert!(is_reconciled_read(&query));

    let proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().state.commitment_root;
    let resolved = verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).unwrap();

    let mut expected = Vec::new();
    for x in ["x1", "x2", "x5"] {
        for z in ["z1", "z2", "z5"] {
            expected.push(vec![node(x), node("y1"), node(z)]);
        }
    }
    assert_eq!(resolved.len(), 3, "join delivers ?x, ?y, ?z");
    assert!(bag_eq(rows_of(&resolved), expected), "resolved join must equal materialized");

    // Soundness: a wrong chain anchor rejects.
    let base_root = lref[0].state.commitment_root;
    assert!(verify_and_resolve_reconciled_read(&query, &lref, base_root, &proof).is_err());
}

#[test]
fn woql_tampered_result_rejects() {
    let layers = filter_stack();
    let lref = refs(&layers);
    let query = tv("s", "p1", "o");
    let mut proof = prove_reconciled_read(&query, &lref).unwrap();
    let head_root = lref.last().unwrap().state.commitment_root;
    // A fabricated result binding fails the re-commitment bind to the authenticated result.
    if let ReconciledReadProof::SingleAtom { result, .. } = &mut proof {
        result.columns[0][0] = ArkFr::from_u64(123_456_789);
    }
    assert!(verify_and_resolve_reconciled_read(&query, &lref, head_root, &proof).is_err());
}

#[test]
fn woql_unsupported_shape_reject_closes() {
    // A 3-atom BGP is out of scope for the reconciled path (deferred, general BGP).
    let three = and_of(vec![
        tv("x", "p1", "y"),
        tv("y", "p2", "z"),
        tv("z", "p3", "w"),
    ]);
    assert!(!is_reconciled_read(&three));
    let layers = filter_stack();
    let lref = refs(&layers);
    assert!(prove_reconciled_read(&three, &lref).is_err());

    // An all-variable single atom (no constant) also reject-closes on the reconciled path.
    let all_vars = tv_pred_var("s", "p", "o");
    assert!(!is_reconciled_read(&all_vars));
}

// --- Audit HOLE #1 regressions: repeated variables must reject-close (not over-answer) ---

#[test]
fn woql_repeated_var_single_atom_reject_closes() {
    // Triple(?s, p1, ?s) — the S==O equality is NOT enforced by a plain predicate filter, so
    // lowering it as a filter would return an over-complete answer. Must reject-close.
    let repeated = tv("s", "p1", "s");
    assert!(!is_reconciled_read(&repeated));
    let layers = filter_stack();
    let lref = refs(&layers);
    assert!(prove_reconciled_read(&repeated, &lref).is_err());
}

#[test]
fn woql_cyclic_join_reject_closes() {
    // Triple(?x, p1, ?y) & Triple(?y, p2, ?x) — the ?x reuse (atom2.object == atom1.subject) is
    // NOT enforced by the 2-atom join, so it would return a strict superset. Must reject-close.
    let cyclic = and_of(vec![tv("x", "p1", "y"), tv("y", "p2", "x")]);
    assert!(!is_reconciled_read(&cyclic));
    let layers = join_stack();
    let lref = refs(&layers);
    assert!(prove_reconciled_read(&cyclic, &lref).is_err());
}

// --- Audit HOLE #2 regression: verify is bound to the verifier's OWN query (anti-substitution) ---

#[test]
fn woql_verify_rejects_proof_for_a_different_query() {
    let layers = filter_stack();
    let lref = refs(&layers);
    let head_root = lref.last().unwrap().state.commitment_root;

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
    // Sanity: it DOES verify against its own query.
    assert!(verify_and_resolve_reconciled_read(&proved_query, &lref, head_root, &proof).is_ok());
}

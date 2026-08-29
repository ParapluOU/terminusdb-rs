//! Merkle-dict I2b S5 — term-less base/rollup verify + resolve through the woql2 path.
//!
//! The prover ships a TERM-LESS `VerifierCommitment` (no clear catalogs — the O(1)-artifact /
//! 256 MiB win) and carries the matched-constant + resolved-result Merkle openings INSIDE the
//! proof (`ProvedBgp::attach_termless_openings`). The verifier consumes them for the S4 belt-hash
//! (`verify_layer_bgp`) and the S3 resolution (`verify_and_resolve` /
//! `resolve_*_with_openings`). A term-less proof WITHOUT its openings, and any range/order/group
//! query on a term-less artifact, fail closed.

mod proof_common;
use proof_common::*;

use terminus_store::proof::commitment::{LayerCommitment, VerifierCommitment};
use terminus_store::{open_sync_memory_store, ValueTriple};
use terminusdb_woql_proof::proof::plan_bgp;

use tdb_succinct::TdbDataType;

fn node_layer(triples: &[(&str, &str, &str)]) -> LayerCommitment {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, p, o) in triples {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o)))
            .unwrap();
    }
    (*builder.commit().unwrap().proof_commitment().unwrap()).clone()
}

fn value_layer(scores: &[u32]) -> LayerCommitment {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in scores {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"),
                &iri("score"),
                u32::make_entry(score),
            ))
            .unwrap();
    }
    (*builder.commit().unwrap().proof_commitment().unwrap()).clone()
}

/// N-live (the key test): a constant-lookup BGP and a 2-atom join, proven over a TERM-LESS artifact
/// + shipped openings, VERIFY + RESOLVE to the SAME result as the term-bearing artifact — through
/// the real woql2 compile -> prove -> attach openings -> verify+resolve pipeline. Also the
/// fail-close: the same term-less proof WITHOUT its openings is rejected.
#[test]
fn s5_termless_instance_bgp_and_join_verify_and_resolve_through_woql2() {
    let lc = node_layer(&[("a", "knows", "b"), ("b", "knows", "c"), ("a", "likes", "c")]);
    let bearing = lc.verifier_commitment().unwrap();
    let termless = lc.verifier_commitment_termless().unwrap();
    assert!(bearing.terms_present && !termless.terms_present);
    // `resolution_root` (and `commitment_root`) byte-identical — only the framing differs.
    assert_eq!(bearing.resolution_root(), termless.resolution_root());
    assert_eq!(bearing.state.commitment_root, termless.state.commitment_root);

    // (1) single-atom OBJECT-constant lookup: ?s knows <b> (matched constant + resolved ?s).
    let compiled = plan_bgp(&tv_obj_node("s", "knows", "b")).unwrap();
    let mut proved = compiled.prove(&lc).unwrap();
    let reference = proved
        .verify_and_resolve(&compiled, &bearing, bearing.resolution_root())
        .unwrap();
    assert!(!reference.is_empty(), "?s knows <b> must resolve to <a>");
    // FAIL-CLOSE: term-less WITHOUT openings is rejected (residual S3/S4 guard).
    assert!(
        proved
            .verify_and_resolve(&compiled, &termless, termless.resolution_root())
            .is_err(),
        "term-less proof with no openings must fail closed"
    );
    // Attach the openings, then the SAME proof verifies + resolves against the TERM-LESS artifact
    // to the SAME objects (the win), and still against the term-bearing one.
    proved.attach_termless_openings(&compiled, &lc).unwrap();
    let termless_objects = proved
        .verify_and_resolve(&compiled, &termless, termless.resolution_root())
        .unwrap();
    assert_eq!(termless_objects, reference);
    assert_eq!(
        proved
            .verify_and_resolve(&compiled, &bearing, bearing.resolution_root())
            .unwrap(),
        reference
    );

    // (2) 2-atom predicate-constant JOIN: ?x knows ?y . ?x likes ?z (join on ?x).
    let compiled2 = plan_bgp(&and_of(vec![tv("x", "knows", "y"), tv("x", "likes", "z")])).unwrap();
    let mut proved2 = compiled2.prove(&lc).unwrap();
    let reference2 = proved2
        .verify_and_resolve(&compiled2, &bearing, bearing.resolution_root())
        .unwrap();
    proved2.attach_termless_openings(&compiled2, &lc).unwrap();
    let termless2 = proved2
        .verify_and_resolve(&compiled2, &termless, termless.resolution_root())
        .unwrap();
    assert_eq!(termless2, reference2);
    assert_eq!(reference2[0].len(), 1, "x=a joins a-knows-b with a-likes-c");
}

/// S5 fence: a RANGE query on a term-less artifact fails closed EVEN WITH the constant/resolution
/// openings attached — S5 relaxed only the constant-match; range/order/group still read
/// value/datatype tied to real terms ONLY by the dropped catalog re-MSM (`require_terms_present`).
#[test]
fn s5_termless_range_query_fails_closed_through_woql2() {
    let vlc = value_layer(&[10, 20, 30, 40]);
    let bearing = vlc.verifier_commitment().unwrap();
    let termless = vlc.verifier_commitment_termless().unwrap();

    // ?m score ?v . ?v > 15
    let range_q = and_of(vec![tv("m", "score", "v"), greater(dvar("v"), duint(15))]);
    let compiled = plan_bgp(&range_q).unwrap();
    let mut proved = compiled.prove(&vlc).unwrap();
    // Term-bearing verifies the range.
    proved
        .verify_and_resolve_values(&compiled, &bearing, bearing.resolution_root())
        .unwrap();
    // Attach openings anyway — the range fence is independent of the belt/resolution.
    proved.attach_termless_openings(&compiled, &vlc).unwrap();
    assert!(
        proved
            .verify_and_resolve_values(&compiled, &termless, termless.resolution_root())
            .is_err(),
        "a range query on a term-less artifact must fail closed"
    );
}

/// A base + one delta layer; returns the (base, delta) `LayerCommitment`s.
fn base_and_delta_node_layers() -> (LayerCommitment, LayerCommitment) {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("b")))
        .unwrap();
    // A second row so `compiled.prove(&base_lc)` succeeds (the BGP prover needs >= 2 rows);
    // the E1 guard under test fires on the delta VIEW regardless of the base proof's shape.
    builder
        .add_value_triple(ValueTriple::new_node(&iri("e"), &iri("knows"), &iri("f")))
        .unwrap();
    let base = builder.commit().unwrap();
    let base_lc = (*base.proof_commitment().unwrap()).clone();
    let child = base.open_write().unwrap();
    child
        .add_value_triple(ValueTriple::new_node(&iri("c"), &iri("knows"), &iri("d")))
        .unwrap();
    let head = child.commit().unwrap();
    let delta_lc = (*head.proof_commitment().unwrap()).clone();
    (base_lc, delta_lc)
}

/// E1 CROSS-PATH GUARD (the soundness requirement of E1): a TERM-LESS DELTA artifact is now
/// constructible / decodable / validatable (previously rejected at construction), but it MUST NOT
/// verify on the single-layer base envelope path — a delta is an INCREMENT, not the full resolved
/// state. A real (base-layer) instance BGP proof, fed the term-less delta view with the delta's own
/// `resolution_root` (so the resolution gate passes and control reaches the guard), is rejected by
/// the E1 guard at the top of `verify_relation`, before any envelope/proof-vs-root work. The base
/// e2e above proves a term-less BASE artifact still verifies, so the guard is scoped to delta only.
#[test]
fn e1_termless_delta_constructible_but_rejected_on_single_layer_base_verify() {
    let (base_lc, delta_lc) = base_and_delta_node_layers();
    assert!(base_lc.state.parent_state_root.is_none());
    assert!(delta_lc.state.parent_state_root.is_some(), "constructed a delta layer");

    // E1: the term-less delta artifact BUILDS, validates, and round-trips through encode/decode.
    let delta_termless = delta_lc.verifier_commitment_termless().unwrap();
    assert!(!delta_termless.terms_present);
    delta_termless.validate().unwrap();
    let decoded = VerifierCommitment::decode(&delta_termless.encode().unwrap()).unwrap();
    assert!(!decoded.terms_present && decoded.state.parent_state_root.is_some());
    decoded.validate().unwrap();

    // Verifying a real instance BGP proof against the term-less DELTA view is rejected by the E1
    // cross-path guard on the single-layer envelope path.
    let compiled = plan_bgp(&tv("s", "knows", "o")).unwrap();
    let proved = compiled.prove(&base_lc).unwrap();
    let err = proved
        .verify(&compiled, &delta_termless, delta_termless.resolution_root())
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("term-less delta layer cannot be verified on the single-layer"),
        "expected the E1 cross-path guard error, got: {err}"
    );
}

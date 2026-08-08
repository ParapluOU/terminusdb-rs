//! CP3 — end-to-end faithfulness: compile a real woql2 BGP, PROVE it against a
//! committed terminus-store layer, VERIFY without rerunning, and check the proof's
//! result columns equal an INDEPENDENTLY recomputed natural join (the TRUE answer,
//! not a superset). Plus the audit's key risk: a demonstration that the shape guard
//! is exactly what prevents a wrong-but-verifying superset.

mod common;
use common::*;

use std::sync::Arc;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminus_store::proof::argument::commit_column;
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::join2::{binary_join_domain, compute_binary_join};
use terminus_store::proof::query::{
    prove_layer_bgp, verify_layer_bgp, AtomTerm, LayerBgpAtom, LayerBgpPlan,
};
use terminus_store::proof::range::RangeOp;
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::{open_sync_memory_store, ValueTriple};

use woql_proof::plan_bgp;

// ----------------------------------------------------------------------------
// Store + independent-join helpers (mirror the store's own query.rs tests).
// ----------------------------------------------------------------------------

/// Build a materialized base layer from (subject, predicate, object) short names.
fn build_layer(triples: &[(&str, &str, &str)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, p, o) in triples {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o)))
            .unwrap();
    }
    let (layer, _state) = builder.commit_with_proof().unwrap();
    layer.proof_commitment().unwrap().unwrap()
}

/// Build a materialized base layer from arbitrary `ValueTriple`s (node OR value objects).
fn build_layer_triples(triples: Vec<ValueTriple>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for t in triples {
        builder.add_value_triple(t).unwrap();
    }
    let (layer, _state) = builder.commit_with_proof().unwrap();
    layer.proof_commitment().unwrap().unwrap()
}

fn padded_len(real_rows: usize) -> usize {
    real_rows.checked_next_power_of_two().unwrap().max(2)
}

fn pad(col: &[ArkFr], domain: usize) -> Vec<ArkFr> {
    let mut v = col.to_vec();
    v.resize(domain, ArkFr::zero());
    v
}

/// One atom's `[S, O]` columns, filtered from the layer's additions in stored order —
/// the exact order the proof's per-atom filter preserves.
fn atom_relation(lc: &LayerCommitment, pred_id: u64) -> Vec<Vec<ArkFr>> {
    let mut s = Vec::new();
    let mut o = Vec::new();
    for rec in &lc.additions {
        if rec.predicate_id == pred_id {
            s.push(ArkFr::from_u64(rec.subject_id));
            o.push(ArkFr::from_u64(rec.object_id));
        }
    }
    vec![s, o]
}

/// Independently recompute the COMPLETE BGP result columns (real length) and the final
/// join domain by folding the plaintext binary join over the plan's steps — the
/// faithfulness oracle. Same structure as the store's own `expected_layer_bgp`.
fn expected_bgp(
    lc: &LayerCommitment,
    ids: &[u64],
    joins: &[JoinStep],
) -> (Vec<Vec<ArkFr>>, usize) {
    let split = |cols: &[Vec<ArkFr>], key: usize| -> (Vec<ArkFr>, Vec<Vec<ArkFr>>) {
        let k = cols[key].clone();
        let other = cols
            .iter()
            .enumerate()
            .filter(|(c, _)| *c != key)
            .map(|(_, c)| c.clone())
            .collect();
        (k, other)
    };
    let mut acc = atom_relation(lc, ids[0]);
    let mut acc_len = acc[0].len();
    let mut domain = padded_len(lc.additions.len());
    for (j, step) in joins.iter().enumerate() {
        let rel = atom_relation(lc, ids[j + 1]);
        let rel_len = rel[0].len();
        let (r_key, r_other) = split(&acc, step.acc_key);
        let (s_key, s_other) = split(&rel, step.rel_key);
        let r_refs: Vec<&[ArkFr]> = r_other.iter().map(|c| c.as_slice()).collect();
        let s_refs: Vec<&[ArkFr]> = s_other.iter().map(|c| c.as_slice()).collect();
        let w = compute_binary_join(&r_key, &r_refs, &s_key, &s_refs);
        domain = binary_join_domain(acc_len, rel_len, w.m_j, w.n_u).unwrap();
        let mut new_acc = w.r_other_out.clone();
        new_acc.push(w.key.clone());
        new_acc.extend(w.s_other_out.clone());
        acc = new_acc;
        acc_len = w.m_j;
    }
    (acc, domain)
}

/// Compile -> prove -> verify -> assert the proof's committed result columns equal the
/// independently recomputed natural join projected to `schema`.
fn assert_true_join_end_to_end(lc: &LayerCommitment, q: &terminusdb_woql2::query::Query) {
    let compiled = plan_bgp(q).unwrap();

    let (proof, result_coms, m, ids) = prove_layer_bgp(lc, &compiled.plan).unwrap();
    // The verifier sees ONLY the IRIs (inside the plan) + revealed ids + result
    // commitments + the external root — never reruns the BGP.
    verify_layer_bgp(
        lc,
        lc.state.commitment_root,
        &compiled.plan,
        &ids,
        &result_coms,
        m,
        &proof,
    )
    .unwrap();

    // Independent oracle: the TRUE natural join.
    let (exp_cols, domain) = expected_bgp(lc, &ids, &compiled.plan.joins);
    assert_eq!(
        result_coms.len(),
        compiled.schema.len(),
        "one commitment per schema column"
    );
    assert_eq!(exp_cols.len(), compiled.schema.len());
    assert_eq!(m, exp_cols[0].len(), "proven row count == true join row count");
    for (c, col) in exp_cols.iter().enumerate() {
        assert_eq!(
            commit_column(&pad(col, domain)).unwrap(),
            result_coms[c],
            "result column {c} ({}) is EXACTLY the true join column",
            compiled.schema[c]
        );
    }
}

// ----------------------------------------------------------------------------
// ACCEPT + CORRECT plan, proven end-to-end, equal to the TRUE join.
// ----------------------------------------------------------------------------

#[test]
fn path_three_atoms_proves_true_join() {
    // knows/likes/owns chain; BGP t(?x,knows,?y), t(?y,likes,?z), t(?z,owns,?w).
    // Paths (x,y,z,w): (a,b,g,k), (c,b,g,k), (e,f,h,m).
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("c", "knows", "b"),
        ("e", "knows", "f"),
        ("b", "likes", "g"),
        ("f", "likes", "h"),
        ("g", "owns", "k"),
        ("h", "owns", "m"),
    ]);
    let q = and_of(vec![
        tv("x", "knows", "y"),
        tv("y", "likes", "z"),
        tv("z", "owns", "w"),
    ]);
    // Sanity: plan shape is what we expect before proving.
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, &c.plan).unwrap();
    assert_eq!(m, 3);

    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn star_three_atoms_proves_true_join() {
    // Star on ?c: t(?c,knows,?a), t(?c,likes,?b), t(?c,owns,?d).
    // c1 has all three; c2 is missing owns -> only c1 contributes.
    let lc = build_layer(&[
        ("c1", "knows", "a1"),
        ("c1", "likes", "b1"),
        ("c1", "owns", "d1"),
        ("c2", "knows", "a2"),
        ("c2", "likes", "b2"),
    ]);
    let q = and_of(vec![
        tv("c", "knows", "a"),
        tv("c", "likes", "b"),
        tv("c", "owns", "d"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["a", "b", "c", "d"]);
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, &c.plan).unwrap();
    assert_eq!(m, 1); // only c1 has all three edges

    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn variable_in_three_atoms_mixed_positions_proves_true_join() {
    // x appears as object then twice as subject: t(?a,knows,?x), t(?x,likes,?b), t(?x,owns,?c).
    let lc = build_layer(&[
        ("a1", "knows", "x1"),
        ("a2", "knows", "x1"),
        ("x1", "likes", "b1"),
        ("x1", "owns", "c1"),
        ("x1", "owns", "c2"),
    ]);
    let q = and_of(vec![
        tv("a", "knows", "x"),
        tv("x", "likes", "b"),
        tv("x", "owns", "c"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["a", "b", "x", "c"]);
    // (a in {a1,a2}) x (b1) x (c in {c1,c2}) = 2 * 1 * 2 = 4 rows.
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, &c.plan).unwrap();
    assert_eq!(m, 4);

    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn dsl_macro_bgp_proves_true_join() {
    // Real-AST compatibility: the same path BGP built with woql2's DSL macros.
    use terminusdb_woql2::{and, t, triple, var};
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("b", "likes", "g"),
        ("g", "owns", "k"),
    ]);
    let knows = iri("knows");
    let likes = iri("likes");
    let owns = iri("owns");
    let q = and!(
        t!(var!(x), knows.as_str(), var!(y)),
        t!(var!(y), likes.as_str(), var!(z)),
        t!(var!(z), owns.as_str(), var!(w)),
    );
    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn select_projection_reads_true_columns() {
    // Select ?x, ?w from the path BGP; projection indices must read the true columns.
    use terminusdb_woql2::control::Select;
    use terminusdb_woql2::query::Query;

    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("b", "likes", "g"),
        ("g", "owns", "k"),
    ]);
    let bgp = and_of(vec![
        tv("x", "knows", "y"),
        tv("y", "likes", "z"),
        tv("z", "owns", "w"),
    ]);
    let q = Query::Select(Select {
        variables: vec!["x".into(), "w".into()],
        query: Box::new(bgp),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.projection, Some(vec![0, 3]));

    let (proof, result_coms, m, ids) = prove_layer_bgp(&lc, &compiled.plan).unwrap();
    verify_layer_bgp(
        &lc,
        lc.state.commitment_root,
        &compiled.plan,
        &ids,
        &result_coms,
        m,
        &proof,
    )
    .unwrap();

    // Projected columns are exactly the true-join columns at the selected indices.
    let (exp_cols, domain) = expected_bgp(&lc, &ids, &compiled.plan.joins);
    for &idx in compiled.projection.as_ref().unwrap() {
        assert_eq!(
            commit_column(&pad(&exp_cols[idx], domain)).unwrap(),
            result_coms[idx]
        );
    }
}

// ----------------------------------------------------------------------------
// GUARD MITIGATION — the audit's key risk made explicit.
//
// A composite-key BGP joined on a SINGLE key yields a wrong-but-VERIFYING superset.
// We (a) hand-build that bogus single-key plan, prove+verify it, and show its result
// STRICTLY contains rows the true BGP excludes; then (b) show plan_bgp REFUSES to emit
// such a plan — the guard is the whole mitigation.
// ----------------------------------------------------------------------------

/// The TRUE composite-key result count: pairs where BOTH subject and object agree
/// across the two predicates. Hand-rolled (no join gadget), fully independent.
fn true_composite_count(lc: &LayerCommitment, p_id: u64, q_id: u64) -> usize {
    let p = atom_relation(lc, p_id);
    let q = atom_relation(lc, q_id);
    let mut n = 0;
    for a in 0..p[0].len() {
        for b in 0..q[0].len() {
            if p[0][a] == q[0][b] && p[1][a] == q[1][b] {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn composite_key_single_key_plan_is_a_verifying_superset_but_guard_refuses_it() {
    // p: x1->y1, x1->y2 ;  q: x1->y1, x1->y3.
    // True BGP t(?x,p,?y) AND t(?x,q,?y) needs x AND y equal -> exactly {(x1,y1)} = 1 row.
    let lc = build_layer(&[
        ("x1", "p", "y1"),
        ("x1", "p", "y2"),
        ("x1", "q", "y1"),
        ("x1", "q", "y3"),
    ]);

    // (a) BOGUS single-key plan: join only on the subject x (acc_key=0, rel_key=0),
    // silently DROPPING the y equality the composite key requires.
    let bogus = LayerBgpPlan {
        atoms: vec![
            LayerBgpAtom::all_variables(iri("p").into_bytes()),
            LayerBgpAtom::all_variables(iri("q").into_bytes()),
        ],
        joins: vec![JoinStep { acc_key: 0, rel_key: 0 }],
    };
    let (proof, result_coms, m_bogus, ids) = prove_layer_bgp(&lc, &bogus).unwrap();
    // It VERIFIES — the crypto is faithful to the (wrong) plan.
    verify_layer_bgp(
        &lc,
        lc.state.commitment_root,
        &bogus,
        &ids,
        &result_coms,
        m_bogus,
        &proof,
    )
    .unwrap();

    // The bogus x-only join is the cross product on x1: (y1,y2) x (y1,y3) = 4 rows.
    assert_eq!(m_bogus, 4);
    // The TRUE composite BGP has just 1 row -> the single-key plan is a strict SUPERSET.
    let true_count = true_composite_count(&lc, ids[0], ids[1]);
    assert_eq!(true_count, 1);
    assert!(
        m_bogus > true_count,
        "single-key plan must over-produce vs the true composite join"
    );

    // (b) The guard is the mitigation: plan_bgp REFUSES to emit any single-key plan
    // for the composite-key BGP, so this wrong-but-verifying result is unreachable
    // through the compiler.
    let q = and_of(vec![tv("x", "p", "y"), tv("x", "q", "y")]);
    assert_eq!(
        plan_bgp(&q).unwrap_err(),
        woql_proof::BgpPlanError::CompositeOrCyclicKey
    );
}

#[test]
fn cycle_triangle_would_superset_but_guard_refuses_it() {
    // Triangle t(?x,p,?y), t(?y,q,?z), t(?z,r,?x). A left-deep single-key chain can
    // only place two of the three joins (x-y, then y-z); the closing z==x edge cannot
    // be a single-key step (?z and ?x are both already bound) -> it would be dropped,
    // yielding a superset. The guard refuses before any such plan exists.
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        tv("z", "r", "x"),
    ]);
    assert_eq!(
        plan_bgp(&q).unwrap_err(),
        woql_proof::BgpPlanError::CompositeOrCyclicKey
    );
}

// ----------------------------------------------------------------------------
// CP2/CP3 — CONSTANT-bearing atoms (subject/object node constants + object typed
// literals), compiled from woql2, proven end-to-end, and checked EXACTLY equal to an
// independently recomputed filtered+joined result (the true answer, not a superset).
//
// The independent oracle mirrors the store's own `expected_const_bgp`: it applies each
// atom's predicate id + revealed subject/object constant id to filter the layer's
// additions into that atom's dense KEPT (variable) columns, then folds the audited
// binary join over the plan's steps. A misaligned value canonicalization (CP3) would
// resolve to a wrong/absent id and select the wrong rows — this catches it.
// ----------------------------------------------------------------------------

/// One atom's dense KEPT (variable) columns from the layer additions in stored order,
/// applying the predicate + any subject/object constant ids. Mirrors what the store's
/// `prove_atom_filter` compacts (kept in S,O order); one column per variable.
fn kept_relation(
    lc: &LayerCommitment,
    atom: &LayerBgpAtom,
    predicate_id: u64,
    subject_id: Option<u64>,
    object_id: Option<u64>,
) -> Vec<Vec<ArkFr>> {
    let keep_s = matches!(atom.subject, AtomTerm::Variable);
    let keep_o = matches!(atom.object, AtomTerm::Variable);
    let mut cols: Vec<Vec<ArkFr>> = Vec::new();
    if keep_s {
        cols.push(Vec::new());
    }
    if keep_o {
        cols.push(Vec::new());
    }
    for rec in &lc.additions {
        if rec.predicate_id == predicate_id
            && subject_id.map_or(true, |s| rec.subject_id == s)
            && object_id.map_or(true, |o| rec.object_id == o)
        {
            let mut idx = 0;
            if keep_s {
                cols[idx].push(ArkFr::from_u64(rec.subject_id));
                idx += 1;
            }
            if keep_o {
                cols[idx].push(ArkFr::from_u64(rec.object_id));
            }
        }
    }
    cols
}

/// Independently recompute the complete BGP result columns (real length) and final join
/// domain over per-atom KEPT relations, folding the audited `compute_binary_join` over the
/// plan's join steps (the variable-arity analogue of `expected_bgp`).
fn expected_const_bgp(
    lc: &LayerCommitment,
    atoms: &[LayerBgpAtom],
    predicate_ids: &[u64],
    subject_ids: &[Option<u64>],
    object_ids: &[Option<u64>],
    joins: &[JoinStep],
) -> (Vec<Vec<ArkFr>>, usize) {
    let split = |cols: &[Vec<ArkFr>], key: usize| -> (Vec<ArkFr>, Vec<Vec<ArkFr>>) {
        let k = cols[key].clone();
        let other = cols
            .iter()
            .enumerate()
            .filter(|(c, _)| *c != key)
            .map(|(_, c)| c.clone())
            .collect();
        (k, other)
    };
    let rel =
        |i: usize| kept_relation(lc, &atoms[i], predicate_ids[i], subject_ids[i], object_ids[i]);
    let mut acc = rel(0);
    let mut acc_len = acc[0].len();
    let mut domain = padded_len(lc.additions.len());
    for (j, step) in joins.iter().enumerate() {
        let r = rel(j + 1);
        let rel_len = r[0].len();
        let (r_key, r_other) = split(&acc, step.acc_key);
        let (s_key, s_other) = split(&r, step.rel_key);
        let r_refs: Vec<&[ArkFr]> = r_other.iter().map(|c| c.as_slice()).collect();
        let s_refs: Vec<&[ArkFr]> = s_other.iter().map(|c| c.as_slice()).collect();
        let w = compute_binary_join(&r_key, &r_refs, &s_key, &s_refs);
        domain = binary_join_domain(acc_len, rel_len, w.m_j, w.n_u).unwrap();
        let mut new_acc = w.r_other_out.clone();
        new_acc.push(w.key.clone());
        new_acc.extend(w.s_other_out.clone());
        acc = new_acc;
        acc_len = w.m_j;
    }
    (acc, domain)
}

/// Compile -> prove -> verify a constant-bearing BGP, then assert the proof's committed
/// result columns equal the independently recomputed filtered+joined result (count +
/// contents), using the proof's OWN revealed constant ids as the oracle's filter keys.
fn assert_const_join_end_to_end(
    lc: &LayerCommitment,
    q: &terminusdb_woql2::query::Query,
    expected_m: usize,
) {
    let compiled = plan_bgp(q).unwrap();
    let (proof, coms, m, ids) = prove_layer_bgp(lc, &compiled.plan).unwrap();
    verify_layer_bgp(
        lc,
        lc.state.commitment_root,
        &compiled.plan,
        &ids,
        &coms,
        m,
        &proof,
    )
    .unwrap();
    assert_eq!(m, expected_m, "proven row count");

    let subject_ids: Vec<Option<u64>> = proof.atoms.iter().map(|a| a.subject_id).collect();
    let object_ids: Vec<Option<u64>> = proof.atoms.iter().map(|a| a.object_id).collect();
    let (exp_cols, domain) = expected_const_bgp(
        lc,
        &compiled.plan.atoms,
        &ids,
        &subject_ids,
        &object_ids,
        &compiled.plan.joins,
    );
    assert_eq!(
        coms.len(),
        compiled.schema.len(),
        "one commitment per schema column"
    );
    assert_eq!(exp_cols.len(), compiled.schema.len());
    assert_eq!(m, exp_cols[0].len(), "proven count == true filtered-join count");
    for (c, col) in exp_cols.iter().enumerate() {
        assert_eq!(
            commit_column(&pad(col, domain)).unwrap(),
            coms[c],
            "result column {c} ({}) is EXACTLY the true filtered join",
            compiled.schema[c]
        );
    }
}

#[test]
fn object_node_constant_proves_true_join() {
    // t(?x, likes, ?z), t(?x, knows, <b>): x likes z AND x knows <b>.
    // likes: a->m,a->o,c->n,d->p ; knows: a->b,c->b,d->q. x knows <b> => {a,c}.
    // Join on x -> (a,m),(a,o),(c,n) = 3 rows. The const atom is the RIGHT relation
    // (arity 1) with an empty s_other.
    let lc = build_layer(&[
        ("a", "likes", "m"),
        ("a", "likes", "o"),
        ("c", "likes", "n"),
        ("d", "likes", "p"),
        ("a", "knows", "b"),
        ("c", "knows", "b"),
        ("d", "knows", "q"),
    ]);
    let q = and_of(vec![tv("x", "likes", "z"), tv_obj_node("x", "knows", "b")]);
    // Plan sanity: the const atom keeps only S (=x); schema is [z, x].
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["z", "x"]);
    assert!(matches!(c.plan.atoms[1].object, AtomTerm::NodeConstant(_)));
    assert_const_join_end_to_end(&lc, &q, 3);
}

#[test]
fn subject_node_constant_proves_true_join() {
    // t(<a>, knows, ?y), t(?y, likes, ?z): a knows y AND y likes z. The subject-const
    // atom is the LEFT relation (arity 1, keeps O = y), seeding the accumulator.
    // knows: a->b,a->c,d->e ; likes: b->m,b->o,c->n. a knows y => {b,c}.
    // Join on y -> (b,m),(b,o),(c,n) = 3 rows.
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("a", "knows", "c"),
        ("d", "knows", "e"),
        ("b", "likes", "m"),
        ("b", "likes", "o"),
        ("c", "likes", "n"),
    ]);
    let q = and_of(vec![tv_subj_const("a", "knows", "y"), tv("y", "likes", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["y", "z"]);
    assert!(matches!(c.plan.atoms[0].subject, AtomTerm::NodeConstant(_)));
    assert_const_join_end_to_end(&lc, &q, 3);
}

#[test]
fn object_constant_joined_on_object_variable_proves_true_join() {
    // t(?x, knows, ?y), t(?y, likes, <g>): x knows y AND y likes <g>. Joins on the OBJECT
    // variable y (acc_key = 1) of the all-variable atom against the object-const atom.
    // knows: a->b,c->b,a->e ; likes: b->g,e->h,f->g. y likes <g> => {b,f}.
    // -> (a,b),(c,b) = 2 rows.
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("c", "knows", "b"),
        ("a", "knows", "e"),
        ("b", "likes", "g"),
        ("e", "likes", "h"),
        ("f", "likes", "g"),
    ]);
    let q = and_of(vec![tv("x", "knows", "y"), tv_obj_node("y", "likes", "g")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y"]);
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (1, 0));
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn absent_node_constant_fails_closed() {
    // The object constant <absent> compiles fine, but proving fails closed because it is
    // not in the layer's node/value dictionary (non-membership proofs are deferred).
    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "c")]);
    let q = and_of(vec![tv("x", "knows", "y"), tv_obj_node("y", "likes", "absent")]);
    let compiled = plan_bgp(&q).unwrap();
    let err = prove_layer_bgp(&lc, &compiled.plan).unwrap_err();
    assert!(err
        .to_string()
        .contains("constant not in layer node/value dictionary"));
}

// ----------------------------------------------------------------------------
// CP3 — xsd:string OBJECT literal alignment (routed through the store's own
// `new_string_value` canonicalization). Each test STORES a string-value triple via the
// store API, then compiles a woql2 query whose object is the MATCHING `Value::Data(String)`
// literal and proves it selects EXACTLY the rows with that literal. A misaligned
// `(CanonicalDatatype, lexical)` would resolve to a wrong/absent dictionary id: proving
// would fail closed, or the result would differ from the independent oracle — either way a
// test failure. This is the round-trip alignment guarantee.
// ----------------------------------------------------------------------------

#[test]
fn object_string_literal_proves_true_join() {
    // t(?x, name, "hi"), t(?x, knows, ?z): x named "hi" AND x knows z.
    // name: a->"hi", c->"hi", d->"bye" ; knows: a->m, a->o, c->n. x named "hi" => {a,c}.
    // Join on x -> (a,m),(a,o),(c,n) = 3 rows. The value-literal atom is the LEFT relation
    // (arity 1) with an empty r_other.
    let name = iri("name");
    let lc = build_layer_triples(vec![
        ValueTriple::new_string_value(&iri("a"), &name, "hi"),
        ValueTriple::new_string_value(&iri("c"), &name, "hi"),
        ValueTriple::new_string_value(&iri("d"), &name, "bye"),
        ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("m")),
        ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("o")),
        ValueTriple::new_node(&iri("c"), &iri("knows"), &iri("n")),
    ]);
    let q = and_of(vec![tv_obj_string("x", "name", "hi"), tv("x", "knows", "z")]);
    // Plan sanity: the object is an aligned value constant; schema is [x, z].
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(c.plan.atoms[0].object, AtomTerm::ValueConstant(_, _)));
    // The compiled value constant is exactly the store's canonicalization of "hi".
    match &c.plan.atoms[0].object {
        AtomTerm::ValueConstant(dt, lex) => {
            match terminus_store::proof::CanonicalObject::from_store_object(
                &ValueTriple::new_string_value("", "", "hi").object,
            ) {
                terminus_store::proof::CanonicalObject::Value { datatype, lexical } => {
                    assert_eq!(*dt, datatype);
                    assert_eq!(*lex, lexical);
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
    assert_const_join_end_to_end(&lc, &q, 3);
}

#[test]
fn object_string_literal_discriminates_by_content() {
    // Same layer, but query "bye": it must select EXACTLY d (the only "bye" row), proving
    // the value constant discriminates by literal CONTENT (not "any string").
    let name = iri("name");
    let lc = build_layer_triples(vec![
        ValueTriple::new_string_value(&iri("a"), &name, "hi"),
        ValueTriple::new_string_value(&iri("c"), &name, "hi"),
        ValueTriple::new_string_value(&iri("d"), &name, "bye"),
        ValueTriple::new_node(&iri("d"), &iri("knows"), &iri("z1")),
        ValueTriple::new_node(&iri("d"), &iri("knows"), &iri("z2")),
        ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("m")),
    ]);
    let q = and_of(vec![tv_obj_string("x", "name", "bye"), tv("x", "knows", "z")]);
    // d named "bye"; d knows {z1,z2} -> 2 rows (a named "hi", excluded).
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn absent_string_literal_fails_closed() {
    // A string literal not present in the layer's value dictionary fails closed at prove
    // time (the compiled value constant is well-formed, but resolves to no dictionary id).
    let name = iri("name");
    let lc = build_layer_triples(vec![
        ValueTriple::new_string_value(&iri("a"), &name, "hi"),
        ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("m")),
    ]);
    let q = and_of(vec![tv_obj_string("x", "name", "nope"), tv("x", "knows", "z")]);
    let compiled = plan_bgp(&q).unwrap();
    let err = prove_layer_bgp(&lc, &compiled.plan).unwrap_err();
    assert!(err
        .to_string()
        .contains("constant not in layer node/value dictionary"));
}

// ----------------------------------------------------------------------------
// Phase 7.4 — numeric object-value RANGE filters, compiled from woql2 comparison
// conjuncts, proven end-to-end against a committed layer, and checked EXACTLY equal to an
// independently recomputed range-filtered natural join. Also asserts the compiled range
// constant `c` matches the store's OWN committed dict OP encoding of the same literal
// (the round-trip alignment guarantee), and exercises the age==k boundary.
// ----------------------------------------------------------------------------

/// A base layer of `t(person, age, <UInt32>)` + `t(person, name, <node>)`. Mirrors the
/// store's own `build_age_name_layer`: ages stored as `xsd:unsignedInt` (UInt32) values.
fn build_age_name_layer(people: &[(&str, u32)]) -> Arc<LayerCommitment> {
    use tdb_succinct::TdbDataType;
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    let age = iri("age");
    let name = iri("name");
    for (person, a) in people {
        builder
            .add_value_triple(ValueTriple::new_value(&iri(person), &age, u32::make_entry(a)))
            .unwrap();
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(person),
                &name,
                &iri(&format!("{person}_n")),
            ))
            .unwrap();
    }
    let (layer, _state) = builder.commit_with_proof().unwrap();
    layer.proof_commitment().unwrap().unwrap()
}

/// The UInt32 value behind a node/value dict id (its OP encoding is the big-endian u32),
/// or `None` for a non-UInt32 object. Mirrors the store's `uint32_value_of`.
fn uint32_value_of(lc: &LayerCommitment, object_id: u64) -> Option<u64> {
    use std::convert::TryInto;
    lc.node_value_catalog
        .iter()
        .find(|r| r.object_id == object_id)
        .and_then(|r| match &r.triple.object {
            CanonicalObject::Value {
                datatype: CanonicalDatatype::UInt32,
                lexical,
            } => Some(u32::from_be_bytes(lexical.as_slice().try_into().unwrap()) as u64),
            _ => None,
        })
}

fn cmp_op(op: RangeOp, a: u64, b: u64) -> bool {
    match op {
        RangeOp::Lt => a < b,
        RangeOp::Le => a <= b,
        RangeOp::Gt => a > b,
        RangeOp::Ge => a >= b,
    }
}

/// The store's committed dict OP value for a `xsd:unsignedInt` literal `k`, recomputed
/// INDEPENDENTLY by routing the SAME literal through the store's own value canonicalization
/// (`u32::make_entry` -> `from_store_object` -> big-endian read of `to_lexical`). The
/// compiler's emitted `c` must equal this byte-for-byte.
fn store_committed_op_value(k: usize) -> ArkFr {
    use std::convert::TryInto;
    use tdb_succinct::TdbDataType;
    let v = k as u32;
    let triple = ValueTriple::new_value("", "", u32::make_entry(&v));
    match CanonicalObject::from_store_object(&triple.object) {
        CanonicalObject::Value {
            datatype: CanonicalDatatype::UInt32,
            lexical,
        } => {
            let bytes: [u8; 4] = lexical.as_slice().try_into().unwrap();
            ArkFr::from_u64(u32::from_be_bytes(bytes) as u64)
        }
        _ => unreachable!("u32 literal canonicalizes to a UInt32 value"),
    }
}

/// Independently recompute `t(?x,age,?a)[?a op k], t(?x,name,?n)`: the range-filtered
/// age `[S,O]` pairs (stored order) joined with the name `[S,O]` pairs on the shared x.
/// Returns the result columns `[a, x, n]` and the final join domain.
fn expected_range_join(
    lc: &LayerCommitment,
    age_id: u64,
    name_id: u64,
    op: RangeOp,
    k: u64,
) -> (Vec<Vec<ArkFr>>, usize) {
    let (mut age_s, mut age_o) = (Vec::new(), Vec::new());
    for rec in &lc.additions {
        if rec.predicate_id == age_id {
            if let Some(v) = uint32_value_of(lc, rec.object_id) {
                if cmp_op(op, v, k) {
                    age_s.push(ArkFr::from_u64(rec.subject_id));
                    age_o.push(ArkFr::from_u64(rec.object_id));
                }
            }
        }
    }
    let (mut name_s, mut name_o) = (Vec::new(), Vec::new());
    for rec in &lc.additions {
        if rec.predicate_id == name_id {
            name_s.push(ArkFr::from_u64(rec.subject_id));
            name_o.push(ArkFr::from_u64(rec.object_id));
        }
    }
    let w = compute_binary_join(&age_s, &[age_o.as_slice()], &name_s, &[name_o.as_slice()]);
    let domain = binary_join_domain(age_s.len(), name_s.len(), w.m_j, w.n_u).unwrap();
    let mut cols = w.r_other_out.clone(); // [a]
    cols.push(w.key.clone()); // x
    cols.extend(w.s_other_out.clone()); // [n]
    (cols, domain)
}

/// Compile `select ?x ?n where { t(?x,age,?a). t(?x,name,?n). ?a op k }`, prove+verify it,
/// assert the committed result columns equal the independently recomputed range-filtered
/// join, and assert the compiled `c` matches the store's committed OP encoding of `k`.
fn check_range_end_to_end(lc: &LayerCommitment, op: RangeOp, k: usize, expected_m: usize) {
    use terminusdb_woql2::control::Select;
    use terminusdb_woql2::query::Query;

    let cmp = match op {
        RangeOp::Gt => greater(dvar("a"), duint(k)),
        RangeOp::Lt => less(dvar("a"), duint(k)),
        RangeOp::Ge => gte(dvar("a"), duint(k)),
        RangeOp::Le => lte(dvar("a"), duint(k)),
    };
    let bgp = and_of(vec![tv("x", "age", "a"), tv("x", "name", "n"), cmp]);
    let q = Query::Select(Select {
        variables: vec!["x".into(), "n".into()],
        query: Box::new(bgp),
    });

    let compiled = plan_bgp(&q).unwrap();

    // Round-trip alignment: the compiled range spec matches the store's OWN encoding.
    let age_atom = compiled
        .plan
        .atoms
        .iter()
        .find(|a| a.object_range.is_some())
        .expect("the age atom carries the range");
    let spec = age_atom.object_range.as_ref().unwrap();
    assert_eq!(spec.op, op, "operator");
    assert_eq!(
        spec.datatype,
        ArkFr::from_u64(CanonicalDatatype::UInt32 as u64),
        "datatype tag"
    );
    assert_eq!(spec.c, ArkFr::from_u64(k as u64), "constant c");
    assert_eq!(
        spec.c,
        store_committed_op_value(k),
        "compiled c == store's committed dict OP encoding of k"
    );

    let (proof, coms, m, ids) = prove_layer_bgp(lc, &compiled.plan).unwrap();
    verify_layer_bgp(
        lc,
        lc.state.commitment_root,
        &compiled.plan,
        &ids,
        &coms,
        m,
        &proof,
    )
    .unwrap();
    assert_eq!(m, expected_m, "proven row count (op {:?}, k {})", op, k);

    // Independent oracle: the TRUE range-filtered join. `ids` are predicate ids in plan
    // atom order; the ranged atom is the age atom.
    let age_idx = compiled
        .plan
        .atoms
        .iter()
        .position(|a| a.object_range.is_some())
        .unwrap();
    let name_idx = 1 - age_idx;
    let (exp_cols, domain) = expected_range_join(lc, ids[age_idx], ids[name_idx], op, k as u64);
    assert_eq!(coms.len(), compiled.schema.len());
    assert_eq!(exp_cols.len(), compiled.schema.len());
    assert_eq!(m, exp_cols[0].len(), "proven count == true filtered-join count");
    for (c, col) in exp_cols.iter().enumerate() {
        assert_eq!(
            commit_column(&pad(col, domain)).unwrap(),
            coms[c],
            "result column {c} ({}) is EXACTLY the true range-filtered join",
            compiled.schema[c]
        );
    }

    // schema is [a, x, n]; the Select reads ?x, ?n -> indices [1, 2].
    assert_eq!(compiled.schema, vec!["a", "x", "n"]);
    assert_eq!(compiled.projection, Some(vec![1, 2]));
}

// ages: alice=30, bob=15, carol=18, dave=40, erin=18 (each with one name). k=18 straddles
// them and hits the boundary (carol, erin == 18) — Ge/Le include it, Gt/Lt exclude it.
fn age_name_fixture() -> Arc<LayerCommitment> {
    build_age_name_layer(&[("alice", 30), ("bob", 15), ("carol", 18), ("dave", 40), ("erin", 18)])
}

#[test]
fn object_range_gt_proves_true_filtered_join() {
    // ?a > 18 -> {alice(30), dave(40)} = 2 (boundary 18 EXCLUDED).
    check_range_end_to_end(&age_name_fixture(), RangeOp::Gt, 18, 2);
}

#[test]
fn object_range_ge_boundary_proves_true_filtered_join() {
    // ?a >= 18 -> {alice, carol, dave, erin} = 4 (boundary 18 INCLUDED).
    check_range_end_to_end(&age_name_fixture(), RangeOp::Ge, 18, 4);
}

#[test]
fn object_range_lt_proves_true_filtered_join() {
    // ?a < 18 -> {bob(15)} = 1.
    check_range_end_to_end(&age_name_fixture(), RangeOp::Lt, 18, 1);
}

#[test]
fn object_range_le_boundary_proves_true_filtered_join() {
    // ?a <= 18 -> {bob, carol, erin} = 3 (boundary 18 INCLUDED).
    check_range_end_to_end(&age_name_fixture(), RangeOp::Le, 18, 3);
}

#[test]
fn object_range_excludes_everything_m_zero() {
    // Nothing satisfies ?a > 1000 -> the ranged relation is empty, so the join is too.
    check_range_end_to_end(&age_name_fixture(), RangeOp::Gt, 1000, 0);
}

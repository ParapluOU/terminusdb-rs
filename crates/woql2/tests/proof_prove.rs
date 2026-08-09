//! CP3 — end-to-end faithfulness: compile a real woql2 BGP, PROVE it against a
//! committed terminus-store layer, VERIFY without rerunning, and check the proof's
//! result columns equal an INDEPENDENTLY recomputed natural join (the TRUE answer,
//! not a superset). Plus the audit's key risk: a demonstration that the shape guard
//! is exactly what prevents a wrong-but-verifying superset.

mod proof_common;
use proof_common::*;

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
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::{open_sync_memory_store, ValueTriple};

use tdb_succinct::{TdbDataType, TypedDictEntry};

use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::control::Distinct;
use terminusdb_woql2::proof::{decode_and_verify_envelope, plan_bgp};
use terminusdb_woql2::{
    misc::{Count, InRange},
    query::{Not, Or, Query},
};

#[test]
fn correlated_not_proves_exact_left_anti_join_with_duplicate_matches() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("e", "p", "b"),
        ("b", "q", "u"),
        ("b", "q", "v"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("y", "q", "z")),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(compiled.schema, vec!["x", "y"]);
    assert_eq!(proved.result_len, 1, "only c-p-d has no correlated q match");
    proved
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
            .unwrap();
    assert_eq!(verified.proved.result_len, 1);
    let mut corrupt = envelope;
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &layer, layer.state.commitment_root,)
            .is_err()
    );

    let original_len = proved.result_len;
    let mut forged = proved;
    forged.result_len = 0;
    assert!(forged
        .verify(&compiled, &layer, layer.state.commitment_root)
        .is_err());
    forged.result_len = original_len;
    forged.result_columns[0][0] = ArkFr::from_u64(999_999);
    assert!(forged
        .verify(&compiled, &layer, layer.state.commitment_root)
        .is_err());
}

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
    let layer = builder.commit().unwrap();
    layer.proof_commitment().unwrap()
}

/// Build a materialized base layer from arbitrary `ValueTriple`s (node OR value objects).
fn build_layer_triples(triples: Vec<ValueTriple>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for t in triples {
        builder.add_value_triple(t).unwrap();
    }
    let layer = builder.commit().unwrap();
    layer.proof_commitment().unwrap()
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
fn expected_bgp(lc: &LayerCommitment, ids: &[u64], joins: &[JoinStep]) -> (Vec<Vec<ArkFr>>, usize) {
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
    assert_eq!(
        m,
        exp_cols[0].len(),
        "proven row count == true join row count"
    );
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
fn single_atom_proves_the_complete_filtered_relation() {
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("b", "knows", "c"),
        ("x", "likes", "y"),
    ]);
    let q = and_of(vec![tv("s", "knows", "o")]);
    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn full_schema_distinct_is_proof_transparent_end_to_end() {
    let lc = build_layer(&[("a", "p", "b"), ("a", "p", "c"), ("d", "p", "c")]);
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(tv("x", "p", "y")),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert!(compiled.full_row_distinct);
    assert_true_join_end_to_end(&lc, &q);
}

#[test]
fn repeated_variable_atom_proves_same_row_equality() {
    let lc = build_layer(&[
        ("a", "knows", "a"),
        ("a", "knows", "b"),
        ("c", "knows", "c"),
        ("x", "likes", "x"),
    ]);
    let q = and_of(vec![tv("x", "knows", "x")]);
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.schema, vec!["x"]);
    assert!(compiled.plan.atoms[0].subject_object_equal);
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 2);
    proved
        .verify(&compiled, &lc, lc.state.commitment_root)
        .unwrap();

    let predicate_id = proved.as_single().unwrap().1[0];
    let expected: Vec<ArkFr> = lc
        .additions
        .iter()
        .filter(|row| row.predicate_id == predicate_id && row.subject_id == row.object_id)
        .map(|row| ArkFr::from_u64(row.subject_id))
        .collect();
    assert_eq!(proved.result_columns, vec![expected.clone()]);
    assert_eq!(proved.projected_columns(&compiled).count(), 1);
    assert_eq!(
        proved.result_commitments[0],
        commit_column(&pad(&expected, padded_len(lc.additions.len()))).unwrap()
    );

    let mut tampered = proved.result_columns.clone();
    tampered[0][0] = tampered[0][0] + ArkFr::one();
    assert!(proved.verify_columns(&tampered).is_err());

    let objects = proved
        .verify_and_resolve(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    assert_eq!(
        objects,
        vec![vec![
            CanonicalObject::Node(iri("a").into_bytes()),
            CanonicalObject::Node(iri("c").into_bytes()),
        ]]
    );
    let mut tampered_objects = objects.clone();
    tampered_objects[0][0] = CanonicalObject::Node(iri("b").into_bytes());
    assert!(proved
        .verify_objects(&compiled, &lc, lc.state.commitment_root, &tampered_objects,)
        .is_err());

    let encoded = proved
        .encode_envelope(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    let decoded =
        decode_and_verify_envelope(&encoded, &compiled, &lc, lc.state.commitment_root).unwrap();
    assert_eq!(decoded.proved.result_len, proved.result_len);
    assert_eq!(decoded.result_objects, objects);

    let mut corrupt = encoded.clone();
    corrupt[20] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &lc, lc.state.commitment_root,).is_err()
    );

    let id = |name: &str| {
        lc.node_value_catalog
            .iter()
            .find(|record| record.triple.object == CanonicalObject::Node(iri(name).into_bytes()))
            .unwrap()
            .object_id
    };
    // The ordinary executor is free to enumerate the same relation in another order.
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["x".into()],
            &[vec![id("c")], vec![id("a")]],
        )
        .unwrap();
    // Sorting normalizes enumeration order, but it must not collapse duplicates.
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["x".into()],
            &[vec![id("a")], vec![id("a")]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["x".into()],
            &[vec![id("a")], vec![id("b")]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["wrong".into()],
            &[vec![id("a")], vec![id("c")]],
        )
        .is_err());

    let mut renamed_schema = compiled.clone();
    renamed_schema.schema[0] = "not_x".into();
    assert!(
        decode_and_verify_envelope(&encoded, &renamed_schema, &lc, lc.state.commitment_root,)
            .is_err()
    );
}

#[test]
fn typed_literal_results_resolve_through_the_authenticated_dictionary() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_value(
            &iri("a"),
            &iri("age"),
            u32::make_entry(&42),
        ))
        .unwrap();
    builder
        .add_value_triple(ValueTriple::new_value(
            &iri("b"),
            &iri("age"),
            u32::make_entry(&7),
        ))
        .unwrap();
    let layer = builder.commit().unwrap();
    let lc = layer.proof_commitment().unwrap();
    let q = and_of(vec![tv("person", "age", "age")]);
    let compiled = plan_bgp(&q).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    let objects = proved
        .verify_and_resolve(&compiled, &lc, lc.state.commitment_root)
        .unwrap();

    assert_eq!(objects.len(), 2);
    assert!(objects[1].iter().all(|object| matches!(
        object,
        CanonicalObject::Value {
            datatype: CanonicalDatatype::UInt32,
            ..
        }
    )));

    let mut tampered = objects.clone();
    tampered[1][0] = CanonicalObject::Value {
        datatype: CanonicalDatatype::String,
        lexical: b"42".to_vec(),
    };
    assert!(proved
        .verify_objects(&compiled, &lc, lc.state.commitment_root, &tampered)
        .is_err());
}

#[test]
fn count_binds_the_proven_relation_cardinality() {
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("b", "knows", "c"),
        ("x", "likes", "y"),
    ]);
    let q = Query::Count(Count {
        query: Box::new(and_of(vec![tv("s", "knows", "o")])),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&q).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    assert_eq!(proved.count_binding(&compiled), Some(("count", 2)));
    assert_eq!(proved.projected_commitments(&compiled).count(), 0);
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["count".into()],
            &[vec![2]],
        )
        .unwrap();
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["wrong".into()],
            &[vec![2]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["count".into()],
            &[vec![1]],
        )
        .is_err());
}

#[test]
fn ground_triple_authenticates_presence_absence_and_zero_arity_rows() {
    // (a,likes,b) exercises row nonmembership with dictionary-present terms;
    // (a,likes,missing) exercises the root-authenticated dictionary-absence shortcut.
    let lc = build_layer(&[("a", "knows", "b"), ("a", "likes", "c")]);

    for (query, expected_len) in [
        (tv_ground("a", "knows", "b"), 1usize),
        (tv_ground("a", "likes", "b"), 0usize),
        (tv_ground("a", "likes", "missing"), 0usize),
    ] {
        let compiled = plan_bgp(&query).unwrap();
        let proved = compiled.prove(&lc).unwrap();
        proved
            .verify(&compiled, &lc, lc.state.commitment_root)
            .unwrap();
        assert_eq!(proved.result_len, expected_len);
        assert_eq!(proved.result_commitments.len(), 1); // hidden UNIT
        assert_eq!(proved.projected_commitments(&compiled).count(), 0);
        assert!(proved
            .verify_and_resolve(&compiled, &lc, lc.state.commitment_root)
            .unwrap()
            .is_empty());

        let rows = vec![Vec::new(); expected_len];
        proved
            .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &[], &rows)
            .unwrap();
        let wrong_rows = vec![Vec::new(); 1 - expected_len];
        assert!(proved
            .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &[], &wrong_rows)
            .is_err());

        let encoded = proved
            .encode_envelope(&compiled, &lc, lc.state.commitment_root)
            .unwrap();
        let decoded =
            decode_and_verify_envelope(&encoded, &compiled, &lc, lc.state.commitment_root).unwrap();
        assert!(decoded.result_objects.is_empty());

        let count_query = Query::Count(Count {
            query: Box::new(query),
            count: dvar("count"),
        });
        let count_compiled = plan_bgp(&count_query).unwrap();
        proved
            .verify_executed_rows(
                &count_compiled,
                &lc,
                lc.state.commitment_root,
                &["count".into()],
                &[vec![expected_len as u64]],
            )
            .unwrap();
    }
}

#[test]
fn all_ground_conjunction_authenticates_boolean_product() {
    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "c")]);
    for (query, expected_len) in [
        (
            and_of(vec![
                tv_ground("a", "knows", "b"),
                tv_ground("b", "likes", "c"),
            ]),
            1usize,
        ),
        (
            and_of(vec![
                tv_ground("a", "knows", "b"),
                tv_ground("a", "likes", "c"),
            ]),
            0usize,
        ),
    ] {
        let compiled = plan_bgp(&query).unwrap();
        let proved = compiled.prove(&lc).unwrap();
        assert_eq!(proved.result_len, expected_len);
        let rows = vec![Vec::new(); expected_len];
        proved
            .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &[], &rows)
            .unwrap();
        let envelope = proved
            .encode_envelope(&compiled, &lc, lc.state.commitment_root)
            .unwrap();
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.state.commitment_root).unwrap();
    }
}

#[test]
fn ground_atoms_authentically_gate_variable_results() {
    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "c")]);
    let id = |name: &str| {
        lc.node_value_catalog
            .iter()
            .find(|record| record.triple.object == CanonicalObject::Node(iri(name).into_bytes()))
            .unwrap()
            .object_id
    };
    for (ground, expected_rows) in [
        (tv_ground("a", "knows", "b"), vec![vec![id("b"), id("c")]]),
        (tv_ground("a", "likes", "c"), Vec::new()),
    ] {
        let query = and_of(vec![ground, tv("s", "likes", "o")]);
        let compiled = plan_bgp(&query).unwrap();
        let mut proved = compiled.prove(&lc).unwrap();
        proved
            .verify_executed_rows(
                &compiled,
                &lc,
                lc.state.commitment_root,
                &["s".into(), "o".into()],
                &expected_rows,
            )
            .unwrap();
        let swapped_rows = if expected_rows.is_empty() {
            vec![vec![id("b"), id("c")]]
        } else {
            Vec::new()
        };
        assert!(proved
            .verify_executed_rows(
                &compiled,
                &lc,
                lc.state.commitment_root,
                &["s".into(), "o".into()],
                &swapped_rows,
            )
            .is_err());
        let envelope = proved
            .encode_envelope(&compiled, &lc, lc.state.commitment_root)
            .unwrap();
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.state.commitment_root).unwrap();
        proved.result_log_domain += 1;
        assert!(proved
            .verify(&compiled, &lc, lc.state.commitment_root)
            .is_err());
    }
}

#[test]
fn disconnected_components_prove_complete_cartesian_results() {
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("e", "q", "f"),
        ("g", "q", "h"),
        ("i", "r", "j"),
        ("z", "q", "marker"),
    ]);
    let id = |name: &str| {
        lc.node_value_catalog
            .iter()
            .find(|record| record.triple.object == CanonicalObject::Node(iri(name).into_bytes()))
            .unwrap()
            .object_id
    };

    let product = and_of(vec![tv("x", "p", "y"), tv("a", "q", "b")]);
    let expected = vec![
        vec![id("a"), id("b"), id("e"), id("f")],
        vec![id("a"), id("b"), id("g"), id("h")],
        vec![id("a"), id("b"), id("z"), id("marker")],
        vec![id("c"), id("d"), id("e"), id("f")],
        vec![id("c"), id("d"), id("g"), id("h")],
        vec![id("c"), id("d"), id("z"), id("marker")],
    ];
    let present = and_of(vec![product.clone(), tv_ground("a", "p", "b")]);
    let compiled = plan_bgp(&present).unwrap();
    assert!(compiled.plan.joins[0].is_cross_product());
    let proved = compiled.prove(&lc).unwrap();
    let variables = ["x".into(), "y".into(), "a".into(), "b".into()];
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &variables,
            &expected,
        )
        .unwrap();
    let mut dropped = expected.clone();
    dropped.pop();
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &variables,
            &dropped,
        )
        .is_err());
    let mut duplicated = expected.clone();
    duplicated.push(expected[0].clone());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &variables,
            &duplicated,
        )
        .is_err());

    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    let reordered = and_of(vec![tv("a", "q", "b"), tv("x", "p", "y")]);
    let reordered = plan_bgp(&reordered).unwrap();
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &lc, lc.state.commitment_root,).is_err()
    );

    // A false authenticated ground gate suppresses the complete product without
    // changing its schema, while an empty relational component does the same through
    // the Cartesian join's pinned zero multiplicity.
    let absent = and_of(vec![product, tv_ground("a", "q", "b")]);
    let compiled = plan_bgp(&absent).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &variables, &[])
        .unwrap();

    let empty_component = and_of(vec![tv("x", "p", "y"), tv_subj_const("i", "p", "w")]);
    let compiled = plan_bgp(&empty_component).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["x".into(), "y".into(), "w".into()],
            &[],
        )
        .unwrap();
}

#[test]
fn or_proves_exact_bag_union_with_overlap() {
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("c", "q", "d"),
        ("e", "q", "f"),
    ]);
    let id = |name: &str| {
        lc.node_value_catalog
            .iter()
            .find(|record| record.triple.object == CanonicalObject::Node(iri(name).into_bytes()))
            .unwrap()
            .object_id
    };
    let query = Query::Or(Or {
        or: vec![tv("s", "p", "o"), tv("s", "q", "o")],
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    let variables = ["s".into(), "o".into()];
    let rows = vec![
        vec![id("a"), id("b")],
        vec![id("c"), id("d")],
        vec![id("c"), id("d")],
        vec![id("e"), id("f")],
    ];
    proved
        .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &variables, &rows)
        .unwrap();
    let mut duplicated = rows.clone();
    duplicated.push(rows[0].clone());
    for forged in [
        rows[..3].to_vec(),
        duplicated,
        vec![
            vec![id("a"), id("d")],
            vec![id("c"), id("b")],
            vec![id("c"), id("d")],
            vec![id("e"), id("f")],
        ],
    ] {
        assert!(proved
            .verify_executed_rows(
                &compiled,
                &lc,
                lc.state.commitment_root,
                &variables,
                &forged,
            )
            .is_err());
    }

    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    let decoded =
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.state.commitment_root).unwrap();
    assert!(decoded.proved.as_bag_union().is_some());
    assert!(decoded.proved.as_single().is_none());
    let mut corrupt = envelope.clone();
    corrupt[20] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &lc, lc.state.commitment_root).is_err()
    );

    let single = plan_bgp(&tv("s", "p", "o")).unwrap();
    let single_proved = single.prove(&lc).unwrap();
    assert!(single_proved.as_single().is_some());
    assert!(single_proved.as_bag_union().is_none());
    let single_envelope = single_proved
        .encode_envelope(&single, &lc, lc.state.commitment_root)
        .unwrap();
    assert!(decode_and_verify_envelope(&envelope, &single, &lc, lc.state.commitment_root).is_err());
    assert!(
        decode_and_verify_envelope(&single_envelope, &compiled, &lc, lc.state.commitment_root,)
            .is_err()
    );
    let reordered = plan_bgp(&Query::Or(Or {
        or: vec![tv("s", "q", "o"), tv("s", "p", "o")],
    }))
    .unwrap();
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &lc, lc.state.commitment_root,).is_err()
    );
}

#[test]
fn or_supports_empty_and_ground_branches() {
    use terminusdb_woql2::control::Select;

    let lc = build_layer(&[("a", "p", "a"), ("b", "q", "c")]);
    let query = Query::Or(Or {
        or: vec![tv("x", "p", "x"), tv("x", "q", "x")],
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    let a = lc
        .node_value_catalog
        .iter()
        .find(|record| record.triple.object == CanonicalObject::Node(iri("a").into_bytes()))
        .unwrap()
        .object_id;
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["x".into()],
            &[vec![a]],
        )
        .unwrap();

    let counted_projection = Query::Count(Count {
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(query.clone()),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&counted_projection).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.state.commitment_root,
            &["count".into()],
            &[vec![1]],
        )
        .unwrap();

    let ground = Query::Or(Or {
        or: vec![tv_ground("a", "p", "a"), tv_ground("a", "q", "c")],
    });
    let compiled = plan_bgp(&ground).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(&compiled, &lc, lc.state.commitment_root, &[], &[vec![]])
        .unwrap();
}

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
    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "g"), ("g", "owns", "k")]);
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

    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "g"), ("g", "owns", "k")]);
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
// COMPOSITE/CYCLIC KEYS — the audit's key risk made explicit.
//
// A composite-key BGP joined on a SINGLE key yields a wrong-but-VERIFYING superset.
// We (a) hand-build that bogus single-key plan, prove+verify it, and show its result
// STRICTLY contains rows the true BGP excludes; then (b) show plan_bgp REFUSES to emit
// such a plan; the compiler now emits the authenticated two-column key.
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
fn composite_key_compiler_proves_exact_result_not_single_key_superset() {
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
        joins: vec![JoinStep {
            acc_key: 0,
            rel_key: 0,
            additional_keys: Vec::new(),
        }],
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

    // (b) The compiler retains both shared variables in the public join key.
    let q = and_of(vec![tv("x", "p", "y"), tv("x", "q", "y")]);
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.plan.joins[0].additional_keys, vec![(1, 1)]);
    let (exact_proof, exact_coms, exact_m, exact_ids) =
        prove_layer_bgp(&lc, &compiled.plan).unwrap();
    verify_layer_bgp(
        &lc,
        lc.state.commitment_root,
        &compiled.plan,
        &exact_ids,
        &exact_coms,
        exact_m,
        &exact_proof,
    )
    .unwrap();
    assert_eq!(exact_m, true_count);
    assert_eq!(exact_coms.len(), 2);

    let p_rows = atom_relation(&lc, exact_ids[0]);
    let q_rows = atom_relation(&lc, exact_ids[1]);
    let mut x = Vec::new();
    let mut y = Vec::new();
    for a in 0..p_rows[0].len() {
        if (0..q_rows[0].len())
            .any(|b| p_rows[0][a] == q_rows[0][b] && p_rows[1][a] == q_rows[1][b])
        {
            x.push(p_rows[0][a]);
            y.push(p_rows[1][a]);
        }
    }
    let domain = 4;
    assert_eq!(exact_coms[0], commit_column(&pad(&x, domain)).unwrap());
    assert_eq!(exact_coms[1], commit_column(&pad(&y, domain)).unwrap());
}

#[test]
fn cycle_triangle_proves_authenticated_closing_edge() {
    // The closing atom shares z AND x with the accumulator. Both column pairs must be
    // present in the public final join step or the plan would prove a superset.
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        tv("z", "r", "x"),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.schema, vec!["x", "y", "z"]);
    assert_eq!(compiled.plan.joins[1].additional_keys, vec![(0, 1)]);

    let lc = build_layer(&[
        ("a", "p", "b"),
        ("a", "p", "c"),
        ("b", "q", "d"),
        ("c", "q", "d"),
        ("d", "r", "a"),
        ("d", "r", "e"),
    ]);
    let (proof, commitments, result_len, ids) = prove_layer_bgp(&lc, &compiled.plan).unwrap();
    verify_layer_bgp(
        &lc,
        lc.state.commitment_root,
        &compiled.plan,
        &ids,
        &commitments,
        result_len,
        &proof,
    )
    .unwrap();

    let p = atom_relation(&lc, ids[0]);
    let q_rel = atom_relation(&lc, ids[1]);
    let r = atom_relation(&lc, ids[2]);
    let (mut x, mut y, mut z) = (Vec::new(), Vec::new(), Vec::new());
    for a in 0..p[0].len() {
        for b in 0..q_rel[0].len() {
            for c in 0..r[0].len() {
                if p[1][a] == q_rel[0][b] && q_rel[1][b] == r[0][c] && p[0][a] == r[1][c] {
                    x.push(p[0][a]);
                    y.push(p[1][a]);
                    z.push(q_rel[1][b]);
                }
            }
        }
    }
    assert_eq!(result_len, 2);
    assert_eq!(commitments[0], commit_column(&pad(&x, 2)).unwrap());
    assert_eq!(commitments[1], commit_column(&pad(&y, 2)).unwrap());
    assert_eq!(commitments[2], commit_column(&pad(&z, 2)).unwrap());
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
    let rel = |i: usize| {
        kept_relation(
            lc,
            &atoms[i],
            predicate_ids[i],
            subject_ids[i],
            object_ids[i],
        )
    };
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
    assert_eq!(
        m,
        exp_cols[0].len(),
        "proven count == true filtered-join count"
    );
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
    let q = and_of(vec![
        tv_subj_const("a", "knows", "y"),
        tv("y", "likes", "z"),
    ]);
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
    let q = and_of(vec![
        tv("x", "knows", "y"),
        tv_obj_node("y", "likes", "absent"),
    ]);
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
    let q = and_of(vec![
        tv_obj_string("x", "name", "hi"),
        tv("x", "knows", "z"),
    ]);
    // Plan sanity: the object is an aligned value constant; schema is [x, z].
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(
        c.plan.atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
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
    let q = and_of(vec![
        tv_obj_string("x", "name", "bye"),
        tv("x", "knows", "z"),
    ]);
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
    let q = and_of(vec![
        tv_obj_string("x", "name", "nope"),
        tv("x", "knows", "z"),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    let err = prove_layer_bgp(&lc, &compiled.plan).unwrap_err();
    assert!(err
        .to_string()
        .contains("constant not in layer node/value dictionary"));
}

// ----------------------------------------------------------------------------
// Wave A — EQUALITY over more object-literal datatypes (boolean / integer(Int64) /
// float(Float64) / unsignedInt(UInt32)), each routed through the store's OWN typed-value
// construction. Each test STORES value triples of that datatype via the store API, compiles
// a woql2 equality query whose object is the MATCHING `Value::Data(..)` literal, and proves
// it selects EXACTLY the rows with that literal. A misaligned `(datatype, lexical)` would
// resolve to a wrong/absent id: proving fails closed or the result differs from the oracle.
// ----------------------------------------------------------------------------

/// A base layer of `t(person, <pred>, <typed value>)` + `t(person, knows, <node>)` edges.
fn build_val_knows_layer(
    pred: &str,
    vals: Vec<(&str, TypedDictEntry)>,
    knows: Vec<(&str, &str)>,
) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, entry) in vals {
        builder
            .add_value_triple(ValueTriple::new_value(&iri(s), &iri(pred), entry))
            .unwrap();
    }
    for (s, o) in knows {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri("knows"), &iri(o)))
            .unwrap();
    }
    let layer = builder.commit().unwrap();
    layer.proof_commitment().unwrap()
}

/// Assert `term` is a `ValueConstant` byte-identical to the store's own canonicalization of
/// `entry` (the round-trip alignment guarantee at the compile boundary).
#[track_caller]
fn assert_value_constant_matches(term: &AtomTerm, entry: TypedDictEntry) {
    let (edt, elex) =
        match CanonicalObject::from_store_object(&ValueTriple::new_value("", "", entry).object) {
            CanonicalObject::Value { datatype, lexical } => (datatype, lexical),
            CanonicalObject::Node(_) => unreachable!(),
        };
    match term {
        AtomTerm::ValueConstant(dt, lex) => {
            assert_eq!(*dt, edt, "value-constant datatype tag");
            assert_eq!(*lex, elex, "value-constant lexical bytes");
        }
        other => panic!("expected a ValueConstant, got {other:?}"),
    }
}

#[test]
fn object_decimal_literal_proves_true_join() {
    let wanted: decimal_rs::Decimal = "123.450".parse().unwrap();
    let other: decimal_rs::Decimal = "98.5".parse().unwrap();
    let store_decimal = |value: &decimal_rs::Decimal| {
        let value = tdb_succinct::Decimal::new(value.to_string()).unwrap();
        tdb_succinct::Decimal::make_entry(&value)
    };
    let lc = build_val_knows_layer(
        "balance",
        vec![
            ("a", store_decimal(&wanted)),
            ("b", store_decimal(&wanted)),
            ("c", store_decimal(&other)),
        ],
        vec![("a", "m"), ("b", "n"), ("c", "o")],
    );
    let q = and_of(vec![
        Query::Triple(terminusdb_woql2::triple::Triple {
            subject: terminusdb_woql2::value::NodeValue::Variable("x".into()),
            predicate: terminusdb_woql2::value::NodeValue::Node(iri("balance")),
            object: terminusdb_woql2::value::Value::Data(XSDAnySimpleType::Decimal(wanted.clone())),
            graph: None,
        }),
        tv("x", "knows", "z"),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    assert_value_constant_matches(&compiled.plan.atoms[0].object, store_decimal(&wanted));
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn object_hexbinary_literal_proves_true_join() {
    let entry = |bytes: Vec<u8>| {
        let value = tdb_succinct::HexBinary(bytes);
        tdb_succinct::HexBinary::make_entry(&value)
    };
    let lc = build_val_knows_layer(
        "digest",
        vec![
            ("a", entry(vec![0xde, 0xad])),
            ("b", entry(vec![0xde, 0xad])),
            ("c", entry(vec![0xbe, 0xef])),
        ],
        vec![("a", "m"), ("b", "n"), ("c", "o")],
    );
    let q = and_of(vec![
        Query::Triple(terminusdb_woql2::triple::Triple {
            subject: terminusdb_woql2::value::NodeValue::Variable("x".into()),
            predicate: terminusdb_woql2::value::NodeValue::Node(iri("digest")),
            object: terminusdb_woql2::value::Value::Data(XSDAnySimpleType::HexBinary(
                "dead".into(),
            )),
            graph: None,
        }),
        tv("x", "knows", "z"),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    assert_value_constant_matches(&compiled.plan.atoms[0].object, entry(vec![0xde, 0xad]));
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn object_boolean_literal_proves_true_join() {
    // t(?x, active, true), t(?x, knows, ?z): active flag true AND x knows z.
    // active: a->true, c->true, d->false ; knows: a->m, a->o, c->n. true => {a,c}.
    // Join on x -> (a,m),(a,o),(c,n) = 3 rows.
    let lc = build_val_knows_layer(
        "active",
        vec![
            ("a", bool::make_entry(&true)),
            ("c", bool::make_entry(&true)),
            ("d", bool::make_entry(&false)),
        ],
        vec![("a", "m"), ("a", "o"), ("c", "n")],
    );
    let q = and_of(vec![
        tv_obj_bool("x", "active", true),
        tv("x", "knows", "z"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.plan.atoms[0].object, bool::make_entry(&true));
    assert_const_join_end_to_end(&lc, &q, 3);
}

#[test]
fn object_boolean_literal_discriminates_by_value() {
    // Same layer, but query `false`: it must select EXACTLY d (the only false row).
    let lc = build_val_knows_layer(
        "active",
        vec![
            ("a", bool::make_entry(&true)),
            ("c", bool::make_entry(&true)),
            ("d", bool::make_entry(&false)),
        ],
        vec![("d", "z1"), ("d", "z2"), ("a", "m")],
    );
    let q = and_of(vec![
        tv_obj_bool("x", "active", false),
        tv("x", "knows", "z"),
    ]);
    // d active=false; d knows {z1,z2} -> 2 rows (a active=true, excluded).
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn object_integer_literal_proves_true_join() {
    // t(?x, count, 42), t(?x, knows, ?z): count==42 AND x knows z. Uses Int64 (incl. a
    // negative value in the layer to exercise the offset-binary encoding of siblings).
    let lc = build_val_knows_layer(
        "count",
        vec![
            ("a", i64::make_entry(&42i64)),
            ("c", i64::make_entry(&42i64)),
            ("d", i64::make_entry(&-7i64)),
        ],
        vec![("a", "m"), ("a", "o"), ("c", "n")],
    );
    let q = and_of(vec![
        tv_obj_integer("x", "count", 42),
        tv("x", "knows", "z"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.plan.atoms[0].object, i64::make_entry(&42i64));
    assert_const_join_end_to_end(&lc, &q, 3);
}

#[test]
fn object_integer_negative_literal_proves_true_join() {
    // Querying a negative Int64 selects exactly the matching negative-valued rows.
    let lc = build_val_knows_layer(
        "count",
        vec![
            ("a", i64::make_entry(&42i64)),
            ("d", i64::make_entry(&-7i64)),
        ],
        vec![("d", "z1"), ("a", "m")],
    );
    let q = and_of(vec![
        tv_obj_integer("x", "count", -7),
        tv("x", "knows", "z"),
    ]);
    assert_const_join_end_to_end(&lc, &q, 1);
}

#[test]
fn object_float_literal_proves_true_join() {
    // t(?x, score, 3.5), t(?x, knows, ?z): score==3.5 AND x knows z. Float64.
    let lc = build_val_knows_layer(
        "score",
        vec![
            ("a", f64::make_entry(&3.5f64)),
            ("c", f64::make_entry(&3.5f64)),
            ("d", f64::make_entry(&-1.25f64)),
        ],
        vec![("a", "m"), ("a", "o"), ("c", "n")],
    );
    let q = and_of(vec![tv_obj_float("x", "score", 3.5), tv("x", "knows", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.plan.atoms[0].object, f64::make_entry(&3.5f64));
    assert_const_join_end_to_end(&lc, &q, 3);
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
            .add_value_triple(ValueTriple::new_value(
                &iri(person),
                &age,
                u32::make_entry(a),
            ))
            .unwrap();
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(person),
                &name,
                &iri(&format!("{person}_n")),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap();
    layer.proof_commitment().unwrap()
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
    assert_eq!(
        m,
        exp_cols[0].len(),
        "proven count == true filtered-join count"
    );
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
    build_age_name_layer(&[
        ("alice", 30),
        ("bob", 15),
        ("carol", 18),
        ("dave", 40),
        ("erin", 18),
    ])
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

#[test]
fn woql_in_range_proves_half_open_filtered_join() {
    // [18, 40) includes both 18-boundary rows and 30, but excludes 15 and the exclusive
    // 40 boundary. This exercises compilation plus both authenticated masks.
    let lc = age_name_fixture();
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        Query::InRange(InRange {
            value: dvar("a"),
            start: duint(18),
            end: duint(40),
        }),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    let range = compiled.plan.atoms[0].object_range.as_ref().unwrap();
    assert_eq!(range.op, RangeOp::Ge);
    assert_eq!(range.second.as_ref().unwrap().op, RangeOp::Lt);

    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 3);
    proved
        .verify(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
}

// ----------------------------------------------------------------------------
// Wave A — numeric object-value RANGE filters over MORE order-preserving fixed-width
// datatypes (Boolean / Int64 / Float64, including NEGATIVE Int64/Float64 to exercise the
// store's offset-binary / IEEE total-order encodings). Each test builds a homogeneous
// `t(person, <pred>, <typed>)` + `t(person, name, <node>)` layer, compiles a range query,
// proves+verifies it, checks the committed columns against an independent range-filtered
// join oracle, and asserts the compiled `c`/datatype tag match the store's committed dict
// OP encoding. Range routes through the SAME store construction the equality path uses.
// ----------------------------------------------------------------------------

/// A base layer of `t(person, <pred>, <typed value>)` + `t(person, name, <node>)`.
fn build_val_name_layer(pred: &str, people: Vec<(&str, TypedDictEntry)>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    let name = iri("name");
    for (person, entry) in people {
        builder
            .add_value_triple(ValueTriple::new_value(&iri(person), &iri(pred), entry))
            .unwrap();
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(person),
                &name,
                &iri(&format!("{person}_n")),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap();
    layer.proof_commitment().unwrap()
}

/// The committed `(datatype, order-preserving u64)` behind a node/value dict id, or `None`
/// for a `Node` object. The OP value is the big-endian read of the fixed-width lexical —
/// the SAME encoding the store's range gadget compares.
fn op_value_of(lc: &LayerCommitment, object_id: u64) -> Option<(CanonicalDatatype, u64)> {
    lc.node_value_catalog
        .iter()
        .find(|r| r.object_id == object_id)
        .and_then(|r| match &r.triple.object {
            CanonicalObject::Value { datatype, lexical } => {
                let mut buf = [0u8; 8];
                buf[8 - lexical.len()..].copy_from_slice(lexical);
                Some((*datatype, u64::from_be_bytes(buf)))
            }
            CanonicalObject::Node(_) => None,
        })
}

/// The store's committed OP encoding of a typed dict entry as a `u64` (big-endian read of
/// its fixed-width `to_lexical`). The compiler's emitted `c` must equal this.
fn store_op_value(entry: TypedDictEntry) -> u64 {
    match CanonicalObject::from_store_object(&ValueTriple::new_value("", "", entry).object) {
        CanonicalObject::Value { lexical, .. } => {
            let mut buf = [0u8; 8];
            buf[8 - lexical.len()..].copy_from_slice(&lexical);
            u64::from_be_bytes(buf)
        }
        CanonicalObject::Node(_) => unreachable!(),
    }
}

/// Independently recompute `t(?x,<val>,?a)[?a op c], t(?x,name,?n)`: the range-filtered
/// value `[S,O]` pairs (stored order, masked to `datatype` and `op c` on the OP value) joined
/// with the name pairs on the shared x. Returns result columns `[a, x, n]` and the domain.
fn expected_range_join_generic(
    lc: &LayerCommitment,
    val_id: u64,
    name_id: u64,
    op: RangeOp,
    c_op: u64,
    datatype: CanonicalDatatype,
) -> (Vec<Vec<ArkFr>>, usize) {
    let (mut vs, mut vo) = (Vec::new(), Vec::new());
    for rec in &lc.additions {
        if rec.predicate_id == val_id {
            if let Some((dt, v)) = op_value_of(lc, rec.object_id) {
                if dt == datatype && cmp_op(op, v, c_op) {
                    vs.push(ArkFr::from_u64(rec.subject_id));
                    vo.push(ArkFr::from_u64(rec.object_id));
                }
            }
        }
    }
    let (mut ns, mut no) = (Vec::new(), Vec::new());
    for rec in &lc.additions {
        if rec.predicate_id == name_id {
            ns.push(ArkFr::from_u64(rec.subject_id));
            no.push(ArkFr::from_u64(rec.object_id));
        }
    }
    let w = compute_binary_join(&vs, &[vo.as_slice()], &ns, &[no.as_slice()]);
    let domain = binary_join_domain(vs.len(), ns.len(), w.m_j, w.n_u).unwrap();
    let mut cols = w.r_other_out.clone(); // [a]
    cols.push(w.key.clone()); // x
    cols.extend(w.s_other_out.clone()); // [n]
    (cols, domain)
}

/// Compile `select ?x ?n where { t(?x,<val_pred>,?a). t(?x,name,?n). cmp }`, prove+verify,
/// assert the committed columns equal the independent range-filtered join, and assert the
/// compiled `(op, datatype tag, c, b_bits)` match the store's committed OP encoding.
fn check_range_generic(
    lc: &LayerCommitment,
    val_pred: &str,
    cmp: terminusdb_woql2::query::Query,
    op: RangeOp,
    datatype: CanonicalDatatype,
    c_op: u64,
    expected_m: usize,
) {
    use terminusdb_woql2::control::Select;
    use terminusdb_woql2::query::Query;

    let bgp = and_of(vec![tv("x", val_pred, "a"), tv("x", "name", "n"), cmp]);
    let q = Query::Select(Select {
        variables: vec!["x".into(), "n".into()],
        query: Box::new(bgp),
    });
    let compiled = plan_bgp(&q).unwrap();

    // Round-trip alignment: the compiled range spec matches the store's OWN encoding.
    let val_atom = compiled
        .plan
        .atoms
        .iter()
        .find(|a| a.object_range.is_some())
        .expect("the value atom carries the range");
    let spec = val_atom.object_range.as_ref().unwrap();
    assert_eq!(spec.op, op, "operator");
    assert_eq!(
        spec.datatype,
        ArkFr::from_u64(datatype as u64),
        "datatype tag"
    );
    assert_eq!(spec.c, ArkFr::from_u64(c_op), "constant c");
    assert_eq!(spec.b_bits, OBJECT_RANGE_B_BITS, "bit-width");

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
    assert_eq!(m, expected_m, "proven row count (op {:?})", op);

    let val_idx = compiled
        .plan
        .atoms
        .iter()
        .position(|a| a.object_range.is_some())
        .unwrap();
    let name_idx = 1 - val_idx;
    let (exp_cols, domain) =
        expected_range_join_generic(lc, ids[val_idx], ids[name_idx], op, c_op, datatype);
    assert_eq!(coms.len(), compiled.schema.len());
    assert_eq!(exp_cols.len(), compiled.schema.len());
    assert_eq!(
        m,
        exp_cols[0].len(),
        "proven count == true filtered-join count"
    );
    for (c, col) in exp_cols.iter().enumerate() {
        assert_eq!(
            commit_column(&pad(col, domain)).unwrap(),
            coms[c],
            "result column {c} ({}) is EXACTLY the true range-filtered join",
            compiled.schema[c]
        );
    }
    assert_eq!(compiled.schema, vec!["a", "x", "n"]);
    assert_eq!(compiled.projection, Some(vec![1, 2]));
}

#[test]
fn object_range_boolean_gt_false_selects_true_rows() {
    // active: alice=true, bob=false, carol=true, dave=false. ?a > false -> {alice, carol}.
    let lc = build_val_name_layer(
        "active",
        vec![
            ("alice", bool::make_entry(&true)),
            ("bob", bool::make_entry(&false)),
            ("carol", bool::make_entry(&true)),
            ("dave", bool::make_entry(&false)),
        ],
    );
    check_range_generic(
        &lc,
        "active",
        greater(dvar("a"), dbool(false)),
        RangeOp::Gt,
        CanonicalDatatype::Boolean,
        store_op_value(bool::make_entry(&false)),
        2,
    );
}

#[test]
fn object_range_boolean_lt_true_selects_false_rows() {
    // ?a < true -> the false rows {bob, dave}.
    let lc = build_val_name_layer(
        "active",
        vec![
            ("alice", bool::make_entry(&true)),
            ("bob", bool::make_entry(&false)),
            ("carol", bool::make_entry(&true)),
            ("dave", bool::make_entry(&false)),
        ],
    );
    check_range_generic(
        &lc,
        "active",
        less(dvar("a"), dbool(true)),
        RangeOp::Lt,
        CanonicalDatatype::Boolean,
        store_op_value(bool::make_entry(&true)),
        2,
    );
}

/// ages as Int64, including negatives: alice=30, bob=-5, carol=0, dave=100, erin=18.
fn int64_fixture() -> Arc<LayerCommitment> {
    build_val_name_layer(
        "count",
        vec![
            ("alice", i64::make_entry(&30i64)),
            ("bob", i64::make_entry(&-5i64)),
            ("carol", i64::make_entry(&0i64)),
            ("dave", i64::make_entry(&100i64)),
            ("erin", i64::make_entry(&18i64)),
        ],
    )
}

#[test]
fn object_range_int64_gt_zero_excludes_negatives_and_zero() {
    // ?a > 0 -> {alice(30), dave(100), erin(18)} = 3 (bob=-5, carol=0 excluded).
    check_range_generic(
        &int64_fixture(),
        "count",
        greater(dvar("a"), dint(0)),
        RangeOp::Gt,
        CanonicalDatatype::Int64,
        store_op_value(i64::make_entry(&0i64)),
        3,
    );
}

#[test]
fn object_range_int64_ge_negative_includes_boundary() {
    // ?a >= -5 -> everyone (min is bob=-5) = 5.
    check_range_generic(
        &int64_fixture(),
        "count",
        gte(dvar("a"), dint(-5)),
        RangeOp::Ge,
        CanonicalDatatype::Int64,
        store_op_value(i64::make_entry(&-5i64)),
        5,
    );
}

#[test]
fn object_range_int64_lt_zero_selects_only_negatives() {
    // ?a < 0 -> {bob(-5)} = 1.
    check_range_generic(
        &int64_fixture(),
        "count",
        less(dvar("a"), dint(0)),
        RangeOp::Lt,
        CanonicalDatatype::Int64,
        store_op_value(i64::make_entry(&0i64)),
        1,
    );
}

/// scores as Float64, including negatives: alice=1.5, bob=-2.5, carol=0.0, dave=3.0.
fn float64_fixture() -> Arc<LayerCommitment> {
    build_val_name_layer(
        "score",
        vec![
            ("alice", f64::make_entry(&1.5f64)),
            ("bob", f64::make_entry(&-2.5f64)),
            ("carol", f64::make_entry(&0.0f64)),
            ("dave", f64::make_entry(&3.0f64)),
        ],
    )
}

#[test]
fn object_range_float64_gt_zero_excludes_negatives_and_zero() {
    // ?a > 0.0 -> {alice(1.5), dave(3.0)} = 2.
    check_range_generic(
        &float64_fixture(),
        "score",
        greater(dvar("a"), dfloat(0.0)),
        RangeOp::Gt,
        CanonicalDatatype::Float64,
        store_op_value(f64::make_entry(&0.0f64)),
        2,
    );
}

#[test]
fn object_range_float64_le_boundary_includes_equal() {
    // ?a <= 1.5 -> {alice(1.5), bob(-2.5), carol(0.0)} = 3 (boundary 1.5 INCLUDED).
    check_range_generic(
        &float64_fixture(),
        "score",
        lte(dvar("a"), dfloat(1.5)),
        RangeOp::Le,
        CanonicalDatatype::Float64,
        store_op_value(f64::make_entry(&1.5f64)),
        3,
    );
}

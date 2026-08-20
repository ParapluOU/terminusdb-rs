//! CP3 — end-to-end faithfulness: compile a real woql2 BGP, PROVE it against a
//! committed terminus-store layer, VERIFY without rerunning, and check the proof's
//! result columns equal an INDEPENDENTLY recomputed natural join (the TRUE answer,
//! not a superset). Plus the audit's key risk: a demonstration that the shape guard
//! is exactly what prevents a wrong-but-verifying superset.

mod proof_common;
use proof_common::*;

use std::{convert::TryInto, sync::Arc};

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminus_store::proof::argument::{commit_column, StoreProverContext};
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::commitment::{database_root, LayerCommitment};
use terminus_store::proof::envelope::RecursiveRelationEnvelope;
use terminus_store::proof::join2::{binary_join_domain, compute_binary_join};
use terminus_store::proof::query::{
    prove_layer_bgp, verify_layer_bgp, generated_decimal_i64_field, AtomTerm, CanonicalResultValue, ColumnSource, LayerBgpAtom, LayerBgpPlan,
    ObjectKindFilter, ResultIdNamespace,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::{open_sync_memory_store, ValueTriple};

use tdb_succinct::{Decimal, TdbDataType, TypedDictEntry};

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::expression::{
    ArithmeticExpression, ArithmeticValue, Div, Divide, Exp, Floor, Minus, Plus, Times,
};
use terminusdb_woql2::compare::{Subsumption, Typecast};
use terminusdb_woql2::interval::{
    DayAfter, DayBefore, IsoWeek, MonthEndDate, MonthStartDate, Weekday, WeekdaySundayStart,
};
use terminusdb_woql2::query::Eval;
use terminusdb_woql2::control::{When, WoqlOptional};
use terminusdb_woql2::control::{Distinct, Select, Using};
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::proof::{
    compiled_query_digest, decode_and_verify_envelope,
    decode_verifier_commitment_and_verify_envelope, plan_bgp,
    BgpPlanError, CompiledBgp, CompiledRelation, CompiledRelationStatement, ExecutedResultValue,
    ProofGraph, ProvedQueryStatement,
};
use terminusdb_woql2::triple::{
    AddedTriple, Data, DeletedTriple, Link, Triple, TripleNext, TriplePrevious, TripleSlice,
    TripleSliceRev,
};
use terminusdb_woql2::reconciled::{prove_reconciled_read, verify_and_resolve_reconciled_read};
use terminusdb_woql2::{
    misc::{Collect, Count, HashKey, InRange, Limit, RangeMax, RangeMin, Start, TripleCount},
    order::{GroupBy, Order, OrderBy, OrderTemplate},
    path::{
        InversePathPredicate, PathOr, PathPattern, PathPlus, PathPredicate, PathSequence, PathStar,
        PathTimes,
    },
    query::{Not, Or, Path, Query},
    string::{Concatenate, Join, Pad, Substring},
    value::{DataValue, ListOrVariable, NodeValue, Value},
};

fn ordered_triple_value_layer() -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in [10u32, 20, 30, 40] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"),
                &iri("score"),
                u32::make_entry(&score),
            ))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn resolved_u32_column(values: &[Vec<CanonicalResultValue>]) -> Vec<u32> {
    values[0]
        .iter()
        .map(|value| match value {
            CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::UInt32,
                lexical,
            }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
            other => panic!("unexpected ordered triple value {other:?}"),
        })
        .collect()
}

fn triple_count_query(resource: &str) -> Query {
    Query::TripleCount(TripleCount {
        resource: resource.into(),
        count: DataValue::Variable("Count".into()),
    })
}

fn triple_count_decimal(value: &str) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Decimal(value.parse().unwrap()))
}

fn counted_comparison(comparison: Query) -> Query {
    and_of(vec![
        triple_count_query("admin/db/local/branch/main/instance"),
        comparison,
    ])
}

fn added_triple_q(subj: &str, pred: &str, obj: &str) -> Query {
    Query::AddedTriple(AddedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

fn deleted_triple_q(subj: &str, pred: &str, obj: &str) -> Query {
    Query::DeletedTriple(DeletedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

/// Base p1 { a1..a4 -> b1..b4 }; child ADDS { a5,a6 } and REMOVES { a1,a2 } (all p1). Returns the
/// full chain [base, child]: the commit's addition file == {a5,a6}, removal file == {a1,a2},
/// materialized head == {a3..a6}. Both delta groups have >= 2 rows (the group-domain floor), so
/// the reconciled delta-read path proves them. (After the Wave 0 diet a delta layer is no longer
/// directly query-provable, so delta-reads route over the chain, not a single committed layer.)
fn delta_stack() -> Vec<Arc<LayerCommitment>> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for s in ["a1", "a2", "a3", "a4"] {
        let o = format!("b{}", &s[1..]);
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(&o)))
            .unwrap();
    }
    let base = builder.commit().unwrap();
    let base_lc = base.proof_commitment().unwrap();
    let child = base.open_write().unwrap();
    for s in ["a1", "a2"] {
        let o = format!("b{}", &s[1..]);
        child
            .remove_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(&o)))
            .unwrap();
    }
    for s in ["a5", "a6"] {
        let o = format!("b{}", &s[1..]);
        child
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri("p1"), &iri(&o)))
            .unwrap();
    }
    let child_lc = child.commit().unwrap().proof_commitment().unwrap();
    vec![base_lc, child_lc]
}

/// Column 0 of a resolved reconciled result, as sorted node IRIs.
fn subject_nodes(cols: &[Vec<CanonicalObject>]) -> Vec<String> {
    let mut subjects: Vec<String> = cols
        .first()
        .map(|col| {
            col.iter()
                .map(|o| match o {
                    CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone()).unwrap(),
                    other => panic!("subject must be a node: {other:?}"),
                })
                .collect()
        })
        .unwrap_or_default();
    subjects.sort();
    subjects
}

/// Resolve a column-major node result into sorted rows (one `Vec<String>` per result row).
fn join_rows(values: &[Vec<CanonicalResultValue>]) -> Vec<Vec<String>> {
    let nrows = values.first().map_or(0, |col| col.len());
    let mut rows: Vec<Vec<String>> = (0..nrows)
        .map(|r| {
            values
                .iter()
                .map(|col| match &col[r] {
                    CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => {
                        String::from_utf8(bytes.clone()).unwrap()
                    }
                    other => panic!("result cell must be a node: {:?}", other),
                })
                .collect()
        })
        .collect();
    rows.sort();
    rows
}

#[test]
#[ignore = "delta head; cross-source (additions join materialized) BGP not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn cross_source_additions_join_materialized_proves_end_to_end() {
    // Base pmat edges y_i -> z_i (inherited into materialized); child ADDS padd edges
    // x_i -> y_i (into the additions delta AND the head). The conjunction
    //   AddedTriple(?x, padd, ?y) & Triple(?y, pmat, ?z)
    // is a CROSS-SOURCE join (additions ⋈ materialized) over the shared ?y — one BGP whose
    // two atoms read DIFFERENT committed column sets.
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for i in ["1", "2"] {
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(&format!("y{i}")),
                &iri("pmat"),
                &iri(&format!("z{i}")),
            ))
            .unwrap();
    }
    let base = builder.commit().unwrap();
    let child = base.open_write().unwrap();
    for i in ["1", "2"] {
        child
            .add_value_triple(ValueTriple::new_node(
                &iri(&format!("x{i}")),
                &iri("padd"),
                &iri(&format!("y{i}")),
            ))
            .unwrap();
    }
    let layer = child.commit().unwrap().proof_commitment().unwrap();
    let root = layer.resolution_root();
    assert_eq!(layer.state.addition_count, 2);
    assert_eq!(layer.state.materialized_count, 4);

    let query = and_of(vec![
        added_triple_q("x", "padd", "y"),
        tv("y", "pmat", "z"),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    // The compiled plan threads per-atom sources: atom0 additions, atom1 materialized.
    assert_eq!(compiled.bgp_plan().atoms.len(), 2);
    assert_eq!(compiled.bgp_plan().atoms[0].source, ColumnSource::Additions);
    assert_eq!(compiled.bgp_plan().atoms[1].source, ColumnSource::Materialized);
    assert_eq!(compiled.relation.schema, ["x", "y", "z"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 2);
    let values = proof
        .verify_and_resolve_values(&compiled, &layer, root)
        .unwrap();
    assert_eq!(
        join_rows(&values),
        vec![
            vec![iri("x1"), iri("y1"), iri("z1")],
            vec![iri("x2"), iri("y2"), iri("z2")],
        ]
    );

    // Envelope round-trip verifies; and re-verifying this CROSS-SOURCE envelope against an
    // all-materialized compiled query fails — the reconstructed plan fixes each atom's source,
    // so the plan digest (and the opened columns) diverge (per-atom source anti-substitution).
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
    let all_materialized =
        plan_bgp(&and_of(vec![tv("x", "padd", "y"), tv("y", "pmat", "z")])).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &all_materialized, &layer, root).is_err());
}

#[test]
fn added_and_deleted_triples_prove_exact_delta_rows_end_to_end() {
    // After the Wave 0 commitment diet a delta layer is no longer directly query-provable
    // (`require_base_layer_for_query`); delta-reads route through the reconciled delta-stack path
    // over the FULL chain (base..head), authenticating ancestor constants via the chain dicts.
    let layers = delta_stack();
    let lref: Vec<&LayerCommitment> = layers.iter().map(|a| a.as_ref()).collect();
    let head = *lref.last().unwrap();
    let head_root = head.resolution_root();
    assert!(head.state.parent_state_root.is_some());
    assert_eq!(head.state.addition_count, 2);
    assert_eq!(head.state.removal_count, 2);
    // After the Wave 0 diet a delta head carries NO materialized view (its root no longer
    // binds materialized columns); the current state is reconciled from the chain at proof time.
    assert_eq!(head.state.materialized_count, 0);

    // AddedTriple(?s, p1, ?o) proves EXACTLY the child's added rows {a5, a6}.
    let added = added_triple_q("s", "p1", "o");
    let added_proof = prove_reconciled_read(&added, &lref).unwrap();
    let added_rows =
        verify_and_resolve_reconciled_read(&added, &lref, head_root, &added_proof).unwrap();
    assert_eq!(subject_nodes(&added_rows), vec![iri("a5"), iri("a6")]);

    // DeletedTriple(?s, p1, ?o) proves EXACTLY the child's removed rows {a1, a2} — both reference
    // ancestor (base-defined) terms, authenticated through the combined chain dictionaries.
    let deleted = deleted_triple_q("s", "p1", "o");
    let deleted_proof = prove_reconciled_read(&deleted, &lref).unwrap();
    let deleted_rows =
        verify_and_resolve_reconciled_read(&deleted, &lref, head_root, &deleted_proof).unwrap();
    assert_eq!(subject_nodes(&deleted_rows), vec![iri("a1"), iri("a2")]);

    // NEGATIVE: an inherited row (a3, still live in `materialized`) was neither added nor removed
    // by this child, so it is provable as neither an AddedTriple nor a DeletedTriple.
    assert!(!subject_nodes(&added_rows).contains(&iri("a3")));
    assert!(!subject_nodes(&deleted_rows).contains(&iri("a3")));

    // SOURCE-SUBSTITUTION (anti-substitution across the route): the AddedTriple proof must not
    // verify as a DeletedTriple or a materialized Triple query — the source is part of the
    // binding, so its lowered spec / opened columns diverge.
    let materialized = tv("s", "p1", "o");
    assert!(verify_and_resolve_reconciled_read(&deleted, &lref, head_root, &added_proof).is_err());
    assert!(
        verify_and_resolve_reconciled_read(&materialized, &lref, head_root, &added_proof).is_err()
    );
    // Sanity: it verifies against its own query.
    assert!(verify_and_resolve_reconciled_read(&added, &lref, head_root, &added_proof).is_ok());
}

#[test]
#[ignore = "delta head; TripleCount/Count aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn explicit_instance_triple_count_proves_generated_decimal_across_layer_stack() {
    let store = open_sync_memory_store();
    let base_builder = store.create_base_layer().unwrap();
    base_builder
        .add_value_triple(ValueTriple::new_node(&iri("a"), &iri("p"), &iri("b")))
        .unwrap();
    let base = base_builder.commit().unwrap();
    let base_commitment = base.proof_commitment().unwrap();
    let child_builder = base.open_write().unwrap();
    child_builder
        .add_value_triple(ValueTriple::new_node(&iri("c"), &iri("p"), &iri("d")))
        .unwrap();
    child_builder
        .add_value_triple(ValueTriple::new_node(&iri("e"), &iri("q"), &iri("f")))
        .unwrap();
    let child = child_builder.commit().unwrap();
    let layer = child.proof_commitment().unwrap();

    let compiled = plan_bgp(&triple_count_query(
        "admin/db/local/branch/main/instance",
    ))
    .unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 1);
    assert_eq!(
        proved
            .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(3)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["Count".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(3)]],
        )
        .unwrap();
    for wrong in [
        ExecutedResultValue::GeneratedDecimalU64(4),
        ExecutedResultValue::Id(3),
        ExecutedResultValue::Null,
    ] {
        assert!(proved
            .verify_executed_values(
                &compiled,
                &layer,
                layer.resolution_root(),
                &["Count".into()],
                &[vec![wrong]],
            )
            .is_err());
    }

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let decoded = decode_and_verify_envelope(
        &envelope,
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();
    assert!(decoded.result_objects.is_empty());
    assert_eq!(
        decoded.result_values,
        Some(vec![vec![CanonicalResultValue::GeneratedDecimalU64(3)]])
    );

    // Both the selected resource and trusted layer root are part of the statement.
    let other_resource = plan_bgp(&triple_count_query(
        "admin/other/local/branch/main/instance",
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &other_resource,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    assert!(decode_and_verify_envelope(
        &envelope,
        &compiled,
        &base_commitment,
        base_commitment.resolution_root(),
    )
    .is_err());

    // Recommitting a forged result does not bypass the root-derived count check.
    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(4);
    tampered.result_commitments[0] = commit_column(&pad(
        &tampered.result_columns[0],
        1usize << tampered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn explicit_instance_triple_count_proves_zero_as_a_value_not_null() {
    let store = open_sync_memory_store();
    let layer = store
        .create_base_layer()
        .unwrap()
        .commit()
        .unwrap()
        .proof_commitment()
        .unwrap();
    let compiled = plan_bgp(&triple_count_query(
        "admin/db/local/branch/main/instance",
    ))
    .unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(
        proved
            .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(0)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["Count".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(0)]],
        )
        .unwrap();
}

#[test]
fn generated_triple_count_comparisons_prove_exact_true_and_false_relations() {
    let layer = build_layer(&[("a", "p", "b"), ("c", "p", "d"), ("e", "q", "f")]);
    let cases = [
        (equals(
            Value::Variable("Count".into()),
            Value::Data(XSDAnySimpleType::Decimal("3".parse().unwrap())),
        ), 1),
        (equals(
            Value::Variable("Count".into()),
            Value::Data(XSDAnySimpleType::Decimal("2".parse().unwrap())),
        ), 0),
        (greater(dvar("Count"), triple_count_decimal("0")), 1),
        (gte(dvar("Count"), triple_count_decimal("3")), 1),
        (less(dvar("Count"), triple_count_decimal("3")), 0),
        (lte(triple_count_decimal("3"), dvar("Count")), 1),
        (less(dvar("Count"), triple_count_decimal("18446744073709551615")), 1),
    ];

    for (query, expected_len) in cases {
        let compiled = plan_bgp(&counted_comparison(query)).unwrap();
        let proved = compiled.prove(&layer).unwrap();
        assert_eq!(proved.result_len, expected_len);
        let values = proved
            .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
            .unwrap();
        assert_eq!(
            values,
            vec![if expected_len == 1 {
                vec![CanonicalResultValue::GeneratedDecimalU64(3)]
            } else {
                Vec::new()
            }]
        );
    }
}

#[test]
fn generated_triple_count_comparison_envelope_binds_operator_operand_and_constant() {
    let layer = build_layer(&[("a", "p", "b"), ("c", "p", "d"), ("e", "q", "f")]);
    let query = counted_comparison(gte(dvar("Count"), triple_count_decimal("3")));
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    for substituted in [
        counted_comparison(greater(dvar("Count"), triple_count_decimal("3"))),
        counted_comparison(gte(triple_count_decimal("3"), dvar("Count"))),
        counted_comparison(gte(dvar("Count"), triple_count_decimal("2"))),
    ] {
        let substituted = plan_bgp(&substituted).unwrap();
        assert!(decode_and_verify_envelope(
            &envelope,
            &substituted,
            &layer,
            layer.resolution_root(),
        )
        .is_err());
    }

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(counted_comparison(less(
            dvar("Count"),
            triple_count_decimal("3"),
        ))),
        count: DataValue::Variable("Rows".into()),
    }))
    .unwrap();
    let counted_proof = counted.prove(&layer).unwrap();
    assert_eq!(counted_proof.count_binding(&counted), Some(("Rows", 0)));
    counted_proof
        .verify_executed_rows(
            &counted,
            &layer,
            layer.resolution_root(),
            &["Rows".into()],
            &[vec![0]],
        )
        .unwrap();
}

fn triple_slice_query(reverse: bool) -> Query {
    let subject = NodeValue::Node(iri("metric"));
    let predicate = NodeValue::Node(iri("score"));
    let object = Value::Variable("score".into());
    let low = Value::Data(XSDAnySimpleType::UnsignedInt(15));
    let high = Value::Data(XSDAnySimpleType::UnsignedInt(40));
    if reverse {
        Query::TripleSliceRev(TripleSliceRev { subject, predicate, object, low, high, graph: None })
    } else {
        Query::TripleSlice(TripleSlice { subject, predicate, object, low, high, graph: None })
    }
}

#[test]
fn triple_slice_forward_and_reverse_prove_exact_order_and_reject_direction_substitution() {
    let layer = ordered_triple_value_layer();
    let forward = plan_bgp(&triple_slice_query(false)).unwrap();
    let forward_proof = forward.prove(&layer).unwrap();
    let forward_values = forward_proof
        .verify_and_resolve_values(&forward, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(resolved_u32_column(&forward_values), [20, 30]);

    let reverse = plan_bgp(&triple_slice_query(true)).unwrap();
    let reverse_proof = reverse.prove(&layer).unwrap();
    let reverse_values = reverse_proof
        .verify_and_resolve_values(&reverse, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(resolved_u32_column(&reverse_values), [30, 20]);

    let envelope = forward_proof
        .encode_envelope(&forward, &layer, layer.resolution_root())
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &reverse,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn triple_next_previous_and_end_cursor_prove_nearest_strict_value() {
    let layer = ordered_triple_value_layer();
    let subject = NodeValue::Node(iri("metric"));
    let predicate = NodeValue::Node(iri("score"));
    let reference = |score| Value::Data(XSDAnySimpleType::UnsignedInt(score));
    let variable = || Value::Variable("neighbor".into());
    let queries = [
        Query::TripleNext(TripleNext {
            subject: subject.clone(), predicate: predicate.clone(),
            object: reference(20), next: variable(), graph: None,
        }),
        Query::TriplePrevious(TriplePrevious {
            subject: subject.clone(), predicate: predicate.clone(),
            object: reference(30), previous: variable(), graph: None,
        }),
        Query::TripleNext(TripleNext {
            subject, predicate, object: reference(40), next: variable(), graph: None,
        }),
    ];
    let expected = [vec![30], vec![20], vec![]];

    for (query, expected) in queries.into_iter().zip(expected) {
        let compiled = plan_bgp(&query).unwrap();
        let proved = compiled.prove(&layer).unwrap();
        let values = proved
            .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
            .unwrap();
        assert_eq!(resolved_u32_column(&values), expected);
    }
}

fn ground_triple_slice_query(object: u32, reverse: bool) -> Query {
    let subject = NodeValue::Node(iri("metric"));
    let predicate = NodeValue::Node(iri("score"));
    let object = Value::Data(XSDAnySimpleType::UnsignedInt(object as usize));
    let low = Value::Data(XSDAnySimpleType::UnsignedInt(15));
    let high = Value::Data(XSDAnySimpleType::UnsignedInt(40));
    if reverse {
        Query::TripleSliceRev(TripleSliceRev { subject, predicate, object, low, high, graph: None })
    } else {
        Query::TripleSlice(TripleSlice { subject, predicate, object, low, high, graph: None })
    }
}

fn assert_ground_navigation_result(
    layer: &Arc<LayerCommitment>,
    query: &Query,
    expected_len: usize,
) -> (CompiledBgp, terminusdb_woql2::proof::ProvedBgp) {
    let compiled = plan_bgp(query).unwrap();
    assert!(compiled.relation.schema.is_empty());
    let proved = compiled.prove(layer).unwrap();
    proved
        .verify(&compiled, layer, layer.resolution_root())
        .unwrap();
    assert_eq!(proved.result_len, expected_len);
    assert_eq!(proved.result_columns.len(), 1);
    assert_eq!(proved.result_columns[0], vec![ArkFr::one(); expected_len]);
    (compiled, proved)
}

#[test]
fn ground_triple_slice_proves_membership_boundaries_absence_and_direction_identity() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in [10u32, 20, 40] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"), &iri("score"), u32::make_entry(&score),
            ))
            .unwrap();
    }
    // Present in the dictionary but not for the selected S/P.
    builder
        .add_value_triple(ValueTriple::new_value(
            &iri("other"), &iri("score"), u32::make_entry(&35),
        ))
        .unwrap();
    let layer = builder.commit().unwrap().proof_commitment().unwrap();

    let (forward, proof) = assert_ground_navigation_result(
        &layer, &ground_triple_slice_query(20, false), 1,
    );
    // Existing triple at the exclusive high boundary is outside [15, 40).
    assert_ground_navigation_result(&layer, &ground_triple_slice_query(40, false), 0);
    // Dictionary-present but missing selected S/P triple.
    assert_ground_navigation_result(&layer, &ground_triple_slice_query(35, false), 0);
    // Dictionary-absent in-range constant remains an authenticated empty result.
    assert_ground_navigation_result(&layer, &ground_triple_slice_query(25, false), 0);

    let reverse = plan_bgp(&ground_triple_slice_query(20, true)).unwrap();
    let envelope = proof
        .encode_envelope(&forward, &layer, layer.resolution_root())
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &reverse, &layer, layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn fully_ground_cursor_proves_immediate_adjacency_and_rejects_non_nearest() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in [10u32, 20, 30, 40] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"), &iri("score"), u32::make_entry(&score),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let next = |candidate| Query::TripleNext(TripleNext {
        subject: NodeValue::Node(iri("metric")),
        predicate: NodeValue::Node(iri("score")),
        object: Value::Data(XSDAnySimpleType::UnsignedInt(20)),
        next: Value::Data(XSDAnySimpleType::UnsignedInt(candidate)),
        graph: None,
    });

    let (compiled, proof) = assert_ground_navigation_result(&layer, &next(30), 1);
    // 40 is greater but not the immediate successor of 20.
    assert_ground_navigation_result(&layer, &next(40), 0);

    let previous = plan_bgp(&Query::TriplePrevious(TriplePrevious {
        subject: NodeValue::Node(iri("metric")),
        predicate: NodeValue::Node(iri("score")),
        object: Value::Data(XSDAnySimpleType::UnsignedInt(20)),
        previous: Value::Data(XSDAnySimpleType::UnsignedInt(30)),
        graph: None,
    }))
    .unwrap();
    let envelope = proof
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &previous, &layer, layer.resolution_root(),
    )
    .is_err());
}

fn build_grouped_score_layer() -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (category, score) in [("c1", 30u32), ("c1", 10), ("c2", 5), ("c2", 20)] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(category),
                &iri("score"),
                u32::make_entry(&score),
            ))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn exact_scalar_order_query(minimum: usize) -> Query {
    Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "score".into(),
            order: Order::Asc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("metric", "score", "score"),
            gte(dvar("score"), duint(minimum)),
        ])),
    })
}

fn exact_scalar_desc_order_query(minimum: usize) -> Query {
    let Query::OrderBy(mut order) = exact_scalar_order_query(minimum) else { unreachable!() };
    order.ordering[0].order = Order::Desc;
    Query::OrderBy(order)
}

#[test]
fn exact_scalar_order_proves_nested_routing_envelope_and_rejects_substitution() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in [30u32, 10, 20] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"),
                &iri("score"),
                u32::make_entry(&score),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = Query::Using(Using {
        collection: "admin/db/local/branch/main".into(),
        query: Box::new(exact_scalar_order_query(0)),
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let scores = values[0]
        .iter()
        .map(|value| match value {
            CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::UInt32,
                lexical,
            }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
            other => panic!("unexpected ordered value {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(scores, [10, 20, 30]);

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();

    let mut wrong_type = compiled.clone();
    match &mut wrong_type.relation.statement {
        CompiledRelationStatement::ExactScalarOrder { datatype, .. } => {
            *datatype = CanonicalDatatype::Int32;
        }
        other => panic!("unexpected statement {other:?}"),
    }
    assert!(decode_and_verify_envelope(
        &envelope,
        &wrong_type,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let different_range = plan_bgp(&Query::Using(Using {
        collection: "admin/db/local/branch/main".into(),
        query: Box::new(exact_scalar_order_query(11)),
    }))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &different_range,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut reordered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    reordered.result_columns[0].swap(0, 2);
    reordered.result_commitments[0] = commit_column(&pad(
        &reordered.result_columns[0],
        1usize << reordered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &reordered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

fn assert_desc_ordered_start_limit_slice_proves_and_rejects_tamper() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for score in [30u32, 10, 20] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("metric"),
                &iri("score"),
                u32::make_entry(&score),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = Query::Limit(Limit {
        limit: 2,
        query: Box::new(Query::Start(Start {
            start: 1,
            query: Box::new(exact_scalar_desc_order_query(0)),
        })),
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let scores = values[0]
        .iter()
        .map(|value| match value {
            CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::UInt32,
                lexical,
            }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
            other => panic!("unexpected sliced value {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(scores, [20, 10]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();

    let ascending = plan_bgp(&Query::Limit(Limit {
        limit: 2,
        query: Box::new(Query::Start(Start {
            start: 1,
            query: Box::new(exact_scalar_order_query(0)),
        })),
    }))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &ascending, &layer, layer.resolution_root(),
    ).is_err());

    // The outer Limit(0) has the same already-authenticated Start child and must verify
    // only the canonical empty contiguous slice; no second expensive child proof is needed.
    let zero_compiled = plan_bgp(&Query::Limit(Limit {
        limit: 0,
        query: Box::new(Query::Start(Start {
            start: 1,
            query: Box::new(exact_scalar_desc_order_query(0)),
        })),
    }))
    .unwrap();
    let mut zero = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    zero.query_digest = compiled_query_digest(&zero_compiled).unwrap();
    zero.result_len = 0;
    zero.result_log_domain = 1;
    zero.result_columns[0].clear();
    zero.result_values[0].clear();
    zero.result_commitments[0] = commit_column(&[ArkFr::zero(), ArkFr::zero()]).unwrap();
    decode_and_verify_envelope(
        &zero.encode().unwrap(),
        &zero_compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();

    let substituted = plan_bgp(&Query::Limit(Limit {
        limit: 1,
        query: Box::new(Query::Start(Start {
            start: 1,
            query: Box::new(exact_scalar_desc_order_query(0)),
        })),
    }))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut reordered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    reordered.result_columns[0].swap(0, 1);
    reordered.result_commitments[0] = commit_column(&pad(
        &reordered.result_columns[0],
        1usize << reordered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &reordered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn desc_ordered_start_limit_slice_proves_and_rejects_tamper() {
    std::thread::Builder::new()
        .name("woql-ordered-slice-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_desc_ordered_start_limit_slice_proves_and_rejects_tamper)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_multikey_order_nested_limit_e2e() {
    let store = open_sync_memory_store(); let builder = store.create_base_layer().unwrap();
    for value in [1u32,2] { builder.add_value_triple(ValueTriple::new_value(&iri("left"), &iri("x"), u32::make_entry(&value))).unwrap(); }
    for value in [3u32,4] { builder.add_value_triple(ValueTriple::new_value(&iri("right"), &iri("y"), u32::make_entry(&value))).unwrap(); }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let duplicate_child = || {
        let branch = and_of(vec![tv_subj_const("left","x","x"), gte(dvar("x"),duint(0)), tv_subj_const("right","y","y"), gte(dvar("y"),duint(0))]);
        Query::Or(Or { or: vec![branch.clone(), branch] })
    };
    let ordered = |second_order| Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "y".into(), order: second_order }, OrderTemplate { variable: "x".into(), order: Order::Asc }],
        query: Box::new(duplicate_child()),
    });
    let query = Query::Limit(Limit { limit: 2, query: Box::new(ordered(Order::Desc)) });
    let compiled = plan_bgp(&query).unwrap(); let proved = compiled.prove(&layer).unwrap();
    let values = proved.verify_and_resolve_values(&compiled, &layer, layer.resolution_root()).unwrap();
    let number = |value: &CanonicalResultValue| match value { CanonicalResultValue::Object(CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()), other => panic!("unexpected {other:?}") };
    assert_eq!((number(&values[0][0]),number(&values[1][0])), (1,4));
    assert_eq!((number(&values[0][1]),number(&values[1][1])), (1,4));
    let envelope = proved.encode_envelope(&compiled, &layer, layer.resolution_root()).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();
    let ascending = plan_bgp(&Query::Limit(Limit { limit: 2, query: Box::new(ordered(Order::Asc)) })).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &ascending, &layer, layer.resolution_root()).is_err());
    let schema_order = plan_bgp(&Query::Limit(Limit { limit: 2, query: Box::new(Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "x".into(), order: Order::Asc }, OrderTemplate { variable: "y".into(), order: Order::Desc }],
        query: Box::new(duplicate_child()),
    })) })).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &schema_order, &layer, layer.resolution_root()).is_err());
    let mut changed = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    let replacement = layer.node_value_catalog.iter()
        .find(|record| matches!(record.triple.object, CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, .. })
            && ArkFr::from_u64(record.object_id) != changed.result_columns[0][1])
        .map(|record| ArkFr::from_u64(record.object_id)).unwrap();
    changed.result_columns[0][1] = replacement;
    for (commitment,column) in changed.result_commitments.iter_mut().zip(&changed.result_columns) { *commitment = commit_column(&pad(column, 1usize << changed.result_log_domain)).unwrap(); }
    assert!(decode_and_verify_envelope(&changed.encode().unwrap(), &compiled, &layer, layer.resolution_root()).is_err());
}

#[test]
fn multikey_order_nested_limit_e2e() {
    std::thread::Builder::new().name("woql-multikey-order".into()).stack_size(256 * 1024 * 1024)
        .spawn(assert_multikey_order_nested_limit_e2e).unwrap().join().unwrap();
}

fn assert_order_gadget_tamper_negatives_and_subset_reject_e2e() {
    use terminus_store::proof::order::OrderDirection;
    // Base layer: cross product of {x=1, x=2} and {y=3, y=4} — inner BGP schema [x, y], four rows.
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for value in [1u32, 2] {
        builder.add_value_triple(ValueTriple::new_value(&iri("left"), &iri("x"), u32::make_entry(&value))).unwrap();
    }
    for value in [3u32, 4] {
        builder.add_value_triple(ValueTriple::new_value(&iri("right"), &iri("y"), u32::make_entry(&value))).unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let root = layer.resolution_root();

    // Inner BGP schema [x, y], four rows (cross product). Both x values repeat, so within each x
    // group the y order is load-bearing — which is exactly why a SUBSET-key order (naming only x)
    // is reject-closed, and a FULL-key order (naming x AND y) is required to attest the row order.
    let child = || and_of(vec![
        tv_subj_const("left", "x", "x"), gte(dvar("x"), duint(0)),
        tv_subj_const("right", "y", "y"), gte(dvar("y"), duint(0)),
    ]);
    let ascending = |var: &str| Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: var.into(), order: Order::Asc }],
        query: Box::new(child()),
    });
    // MAXIMUM ORDER ATTESTATION: a SUBSET-key order (names only x over the two-column [x, y] row)
    // reject-closes — its ascending tie-break on y is invented and cannot be attested against
    // TerminusDB's stable order. Only the FULL-key order below is provable.
    assert!(matches!(
        plan_bgp(&ascending("x")),
        Err(BgpPlanError::OrderBySubsetKeyUnsupported)
    ));

    // FULL-key order over [x, y]: both columns named, a deterministic total order. It proves and
    // exercises the full-row order gadget's tamper negatives below. `[x ASC, y ASC]` yields exactly
    // the same rows the folded subset order used to, so every negative below is unchanged.
    let full_key = |y_dir: Order| Query::OrderBy(OrderBy {
        ordering: vec![
            OrderTemplate { variable: "x".into(), order: Order::Asc },
            OrderTemplate { variable: "y".into(), order: y_dir },
        ],
        query: Box::new(child()),
    });
    let compiled = plan_bgp(&full_key(Order::Asc)).unwrap();
    assert!(matches!(compiled.relation.statement,
        CompiledRelationStatement::ExactMultiOrder { ref key_indices, ref directions, .. }
            if key_indices == &[0, 1] && directions == &[OrderDirection::Ascending; 2]),
        "full-key [x ASC, y ASC] compiles to a complete-row order over [x, y]");

    let proved = compiled.prove(&layer).unwrap();
    let values = proved.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    let number = |value: &CanonicalResultValue| match value {
        CanonicalResultValue::Object(CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical }) =>
            u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
        other => panic!("unexpected ordered value {other:?}"),
    };
    let rows = (0..values[0].len())
        .map(|r| (number(&values[0][r]), number(&values[1][r])))
        .collect::<Vec<_>>();
    // Sorted by x ascending, ties on x broken by y ascending — a bound total order.
    assert_eq!(rows, vec![(1, 3), (1, 4), (2, 3), (2, 4)]);

    let envelope = proved.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();

    // ANTI-SUBSTITUTION: the same envelope must not verify under a different FULL-key template — the
    // key direction is folded into both the query digest and the bound sort key. `[x ASC, y DESC]`
    // differs from the proven `[x ASC, y ASC]` only in the tie-break direction.
    let descending = plan_bgp(&full_key(Order::Desc)).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &descending, &layer, root).is_err());

    // NEGATIVE — TIE-BREAK SWAP: exchange the two x=1 rows (indices 0 and 1). The output stays a
    // permutation of the input AND stays sorted on x (1,1,2,2), but y within the x=1 group becomes
    // descending (4 then 3), so the proven lexicographic-nondecreasing chain over [x, y] breaks.
    let mut tie_swapped = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    for column in tie_swapped.result_columns.iter_mut() { column.swap(0, 1); }
    for (commitment, column) in tie_swapped.result_commitments.iter_mut().zip(&tie_swapped.result_columns) {
        *commitment = commit_column(&pad(column, 1usize << tie_swapped.result_log_domain)).unwrap();
    }
    assert!(decode_and_verify_envelope(&tie_swapped.encode().unwrap(), &compiled, &layer, root).is_err());

    // NEGATIVE — REORDER across x groups: swap rows 0 (1,3) and 3 (2,4) — no longer sorted on x.
    let mut reordered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    let last = reordered.result_len - 1;
    for column in reordered.result_columns.iter_mut() { column.swap(0, last); }
    for (commitment, column) in reordered.result_commitments.iter_mut().zip(&reordered.result_columns) {
        *commitment = commit_column(&pad(column, 1usize << reordered.result_log_domain)).unwrap();
    }
    assert!(decode_and_verify_envelope(&reordered.encode().unwrap(), &compiled, &layer, root).is_err());

    // NEGATIVE — TRUNCATION: dropping the final row is no longer a full-row permutation of the input.
    let mut truncated = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    let drop = truncated.result_len - 1;
    for column in truncated.result_columns.iter_mut() { column[drop] = ArkFr::zero(); }
    truncated.result_len = drop;
    for (commitment, column) in truncated.result_commitments.iter_mut().zip(&truncated.result_columns) {
        *commitment = commit_column(&pad(column, 1usize << truncated.result_log_domain)).unwrap();
    }
    assert!(decode_and_verify_envelope(&truncated.encode().unwrap(), &compiled, &layer, root).is_err());

    // NEGATIVE — ROW SUBSTITUTION (add/drop a row): replace a committed key with a different catalog
    // id, so the output multiset no longer equals the input relation.
    let mut substituted = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    let replacement = layer.node_value_catalog.iter()
        .find(|record| matches!(record.triple.object, CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, .. })
            && ArkFr::from_u64(record.object_id) != substituted.result_columns[1][0])
        .map(|record| ArkFr::from_u64(record.object_id)).unwrap();
    substituted.result_columns[1][0] = replacement;
    for (commitment, column) in substituted.result_commitments.iter_mut().zip(&substituted.result_columns) {
        *commitment = commit_column(&pad(column, 1usize << substituted.result_log_domain)).unwrap();
    }
    assert!(decode_and_verify_envelope(&substituted.encode().unwrap(), &compiled, &layer, root).is_err());
}

#[test]
fn order_gadget_tamper_negatives_and_subset_reject_e2e() {
    std::thread::Builder::new().name("woql-order-gadget-tamper".into()).stack_size(256 * 1024 * 1024)
        .spawn(assert_order_gadget_tamper_negatives_and_subset_reject_e2e).unwrap().join().unwrap();
}

// PART B — MAXIMUM ORDER ATTESTATION at the ordinary-executor boundary. For an ORDER-attesting
// relation the executor must deliver rows in the PROVEN order: `verify_executed_rows` compares
// positionally, so a permutation is rejected. For an UNORDERED (bag) relation the SAME permutation
// is accepted (multiset comparison). Previously both sides sorted before comparing, so a permuted
// delivery of an ordered result was wrongly accepted.
fn assert_ordered_executor_delivery_is_positional() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for value in [5u32, 3] {
        builder
            .add_value_triple(ValueTriple::new_value(&iri("m"), &iri("score"), u32::make_entry(&value)))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let root = layer.resolution_root();
    // Dictionary id of a committed UInt32 value object.
    let vid = |v: u32| {
        layer
            .node_value_catalog
            .iter()
            .find(|record| matches!(&record.triple.object,
                CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical }
                    if u32::from_be_bytes(lexical.as_slice().try_into().unwrap()) == v))
            .map(|record| record.object_id)
            .unwrap()
    };
    let bgp = || and_of(vec![tv_subj_const("m", "score", "score"), gte(dvar("score"), duint(0))]);

    // ORDERED: OrderBy([score ASC]) over the single-column [score] relation is a FULL-key order, so
    // it attests the exact ascending order [3, 5].
    let ordered = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "score".into(), order: Order::Asc }],
        query: Box::new(bgp()),
    }))
    .unwrap();
    let proved = ordered.prove(&layer).unwrap();
    // The correct ascending delivery is accepted.
    proved
        .verify_executed_rows(&ordered, &layer, root, &["score".into()], &[vec![vid(3)], vec![vid(5)]])
        .unwrap();
    // PART B: a permuted delivery of the ordered relation is now REJECTED (was accepted pre-fix).
    assert!(proved
        .verify_executed_rows(&ordered, &layer, root, &["score".into()], &[vec![vid(5)], vec![vid(3)]])
        .is_err());

    // UNORDERED: the same BGP without an OrderBy is a bag — the executor may deliver either order,
    // so BOTH permutations are accepted (multiset comparison).
    let bag = plan_bgp(&bgp()).unwrap();
    let bag_proved = bag.prove(&layer).unwrap();
    bag_proved
        .verify_executed_rows(&bag, &layer, root, &["score".into()], &[vec![vid(3)], vec![vid(5)]])
        .unwrap();
    bag_proved
        .verify_executed_rows(&bag, &layer, root, &["score".into()], &[vec![vid(5)], vec![vid(3)]])
        .unwrap();
}

#[test]
fn ordered_executor_delivery_is_positional() {
    std::thread::Builder::new().name("woql-ordered-executor-positional".into()).stack_size(256 * 1024 * 1024)
        .spawn(assert_ordered_executor_delivery_is_positional).unwrap().join().unwrap();
}

// SOUNDNESS: an order-attesting relation wrapped by a POSITION-PRESERVING unary wrapper (here an
// `Eval` per-row map over an `OrderBy`) still fixes the exact row order, so the executor must deliver
// rows POSITIONALLY. Before the recursive `relation_attests_order` fix the top statement (`Eval`) was
// not an exact-order variant, so both sides were sorted (multiset), and a PERMUTED delivery of the
// ordered result verified — a soundness hole. This test pins that the correct order is accepted and
// the reversed delivery is rejected. Uses the EVAL wrapper (Eval adds a generated value column, so
// `verify_executed_values` is the right boundary).
fn assert_ordered_under_position_preserving_wrapper_is_positional() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for value in [5u32, 3] {
        builder
            .add_value_triple(ValueTriple::new_value(&iri("m"), &iri("score"), u32::make_entry(&value)))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let root = layer.resolution_root();
    // Dictionary id of a committed UInt32 value object.
    let vid = |v: u32| {
        layer
            .node_value_catalog
            .iter()
            .find(|record| matches!(&record.triple.object,
                CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical }
                    if u32::from_be_bytes(lexical.as_slice().try_into().unwrap()) == v))
            .map(|record| record.object_id)
            .unwrap()
    };
    let bgp = || and_of(vec![tv_subj_const("m", "score", "score"), gte(dvar("score"), duint(0))]);

    // `And(OrderBy([score ASC], BGP[score]), Eval(score + 0, r))`. `plan_eval_form` extracts the Eval
    // and compiles the remaining `OrderBy` as the operand-binding child, so the compiled top statement
    // is `Eval` wrapping an `ExactScalarOrder` — a position-preserving map over an exact order.
    let query = and_of(vec![
        Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate { variable: "score".into(), order: Order::Asc }],
            query: Box::new(bgp()),
        }),
        Query::Eval(Eval {
            expression: ea_plus(ea_var("score"), ea_lit(XSDAnySimpleType::Int(0))),
            result_value: ArithmeticValue::Variable("r".into()),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    // Top statement is `Eval` over `ExactScalarOrder`.
    match &compiled.relation.statement {
        CompiledRelationStatement::Eval { input, .. } => assert!(matches!(
            input.statement,
            CompiledRelationStatement::ExactScalarOrder { .. }
        )),
        other => panic!("expected Eval over ExactScalarOrder, got {other:?}"),
    }

    let proved = compiled.prove(&layer).unwrap();
    // Proven schema is [score, r]; ordered ascending by score the rows are (3, 3) then (5, 5), where
    // r = score + 0 is the generated integer column.
    let ordered_delivery = &[
        vec![ExecutedResultValue::Id(vid(3)), ExecutedResultValue::GeneratedDecimalI64(3)],
        vec![ExecutedResultValue::Id(vid(5)), ExecutedResultValue::GeneratedDecimalI64(5)],
    ];
    proved
        .verify_executed_values(&compiled, &layer, root, &["score".into(), "r".into()], ordered_delivery)
        .unwrap();
    // PART B: the PERMUTED (reversed) delivery is REJECTED now that the wrapper-over-order is
    // classified as order-attesting (was accepted pre-fix via multiset comparison).
    let reversed_delivery = &[
        vec![ExecutedResultValue::Id(vid(5)), ExecutedResultValue::GeneratedDecimalI64(5)],
        vec![ExecutedResultValue::Id(vid(3)), ExecutedResultValue::GeneratedDecimalI64(3)],
    ];
    assert!(proved
        .verify_executed_values(&compiled, &layer, root, &["score".into(), "r".into()], reversed_delivery)
        .is_err());
}

#[test]
fn ordered_under_position_preserving_wrapper_is_positional() {
    std::thread::Builder::new().name("woql-ordered-wrapper-positional".into()).stack_size(256 * 1024 * 1024)
        .spawn(assert_ordered_under_position_preserving_wrapper_is_positional).unwrap().join().unwrap();
}

fn assert_bounded_when_success_plus_true_e2e() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, predicate, object) in [
        ("a", "p", "b"), ("c", "p", "d"),
        ("a", "q", "b"), ("a", "r", "b"),
    ] {
        builder.add_value_triple(ValueTriple::new_node(&iri(subject), &iri(predicate), &iri(object))).unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = |consequent: &str| and_of(vec![
        tv("x", "p", "y"),
        Query::If(When {
            query: Box::new(tv("x", "q", "y")),
            consequent: Box::new(tv("x", consequent, "y")),
        }),
    ]);
    let compiled = plan_bgp(&query("r")).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved.verify_and_resolve_values(&compiled, &layer, layer.resolution_root()).unwrap();
    let node = |value: &CanonicalResultValue| match value {
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("unexpected {other:?}"),
    };
    let rows = (0..proved.result_len).map(|row| (node(&values[0][row]), node(&values[1][row]))).collect::<Vec<_>>();
    assert_eq!(rows, vec![(iri("a"), iri("b")), (iri("a"), iri("b")), (iri("c"), iri("d"))]);
    let envelope = proved.encode_envelope(&compiled, &layer, layer.resolution_root()).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();
    let substituted = plan_bgp(&query("s")).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &substituted, &layer, layer.resolution_root()).is_err());
}

#[test]
fn bounded_when_success_plus_true_e2e() {
    std::thread::Builder::new().name("woql-when-proof".into()).stack_size(256 * 1024 * 1024)
        .spawn(assert_bounded_when_success_plus_true_e2e).unwrap().join().unwrap();
}

fn assert_when_existential_local_projection_preserves_success_multiplicity() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, predicate, object) in [
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("a", "q", "z1"),
        ("a", "q", "z2"),
        ("z1", "r", "b"),
        ("z2", "r", "b"),
    ] {
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(subject),
                &iri(predicate),
                &iri(object),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = |consequent: &str| Query::Select(Select {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(and_of(vec![
            tv("x", "p", "y"),
            Query::If(When {
                query: Box::new(tv("x", "q", "z")),
                consequent: Box::new(tv("z", consequent, "y")),
            }),
        ])),
    });
    let compiled = plan_bgp(&query("r")).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "y"]);
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let node = |value: &CanonicalResultValue| match value {
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => {
            String::from_utf8(bytes.clone()).unwrap()
        }
        other => panic!("unexpected {other:?}"),
    };
    let rows = (0..proved.result_len)
        .map(|row| (node(&values[0][row]), node(&values[1][row])))
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            (iri("a"), iri("b")),
            (iri("a"), iri("b")),
            (iri("a"), iri("b")),
            (iri("c"), iri("d")),
        ]
    );
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(
        &envelope,
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();
    let substituted = plan_bgp(&query("s")).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn when_existential_local_projection_preserves_success_multiplicity() {
    std::thread::Builder::new()
        .name("woql-when-existential-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_when_existential_local_projection_preserves_success_multiplicity)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn explicit_cpu_context_preserves_recursive_envelope_bytes() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("a"), &iri("knows"), &iri("b")))
        .unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("b"), &iri("knows"), &iri("c")))
        .unwrap();
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&tv("subject", "knows", "object")).unwrap();

    let convenience = compiled.prove(&layer).unwrap();
    let explicit = compiled
        .prove_with_context(&StoreProverContext::cpu(), &layer)
        .unwrap();
    assert_eq!(
        convenience
            .encode_envelope(&compiled, &layer, layer.resolution_root())
            .unwrap(),
        explicit
            .encode_envelope(&compiled, &layer, layer.resolution_root())
            .unwrap(),
    );
}

fn grouped_score_extrema(kind: RangeOp) -> Query {
    let child = and_of(vec![
        tv("category", "score", "score"),
        gte(dvar("score"), duint(0)),
        // A tautological exact equality makes the aggregate child a genuinely
        // recursive relation node rather than only an internal multi-atom BGP.
        equals(value_var("score"), value_var("score")),
    ]);
    let group = Query::GroupBy(GroupBy {
        template: terminusdb_woql2::value::Value::Variable("score".into()),
        group_by: vec!["category".into()],
        grouped_value: terminusdb_woql2::value::Value::Variable("scores".into()),
        query: Box::new(child),
    });
    let extrema = match kind {
        RangeOp::Ge => Query::RangeMin(RangeMin {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extremum".into()),
        }),
        RangeOp::Le => Query::RangeMax(RangeMax {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extremum".into()),
        }),
        _ => unreachable!(),
    };
    Query::Select(Select {
        variables: vec!["category".into(), "extremum".into()],
        query: Box::new(and_of(vec![group, extrema])),
    })
}

fn two_key_grouped_score_extrema(max: bool) -> Query {
    let child = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("owner".into()),
            predicate: NodeValue::Node(iri("link")),
            object: NodeValue::Variable("category".into()),
            graph: GraphType::Instance,
        }),
        tv("category", "score", "score"),
        gte(dvar("score"), duint(0)),
    ]);
    let group = Query::GroupBy(GroupBy {
        template: Value::Variable("score".into()),
        group_by: vec!["category".into(), "owner".into()],
        grouped_value: Value::Variable("scores".into()),
        query: Box::new(child),
    });
    let extrema = if max {
        Query::RangeMax(RangeMax {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extremum".into()),
        })
    } else {
        Query::RangeMin(RangeMin {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extremum".into()),
        })
    };
    and_of(vec![group, extrema])
}

fn grouped_u32_sum_query() -> Query {
    let child = and_of(vec![
        tv_pred_var("category", "predicate", "score"),
        gte(dvar("score"), duint(0)),
    ]);
    let group = Query::GroupBy(GroupBy {
        template: Value::Variable("score".into()),
        group_by: vec!["category".into()],
        grouped_value: Value::Variable("scores".into()),
        query: Box::new(child),
    });
    let sum = Query::Sum(Sum {
        list: DataValue::Variable("scores".into()),
        result: DataValue::Variable("total".into()),
    });
    and_of(vec![group, sum])
}

fn global_u32_sum_query() -> Query {
    let Query::And(mut query) = grouped_u32_sum_query() else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut query.and[0] else {
        unreachable!()
    };
    group.group_by.clear();
    Query::And(query)
}

fn global_u64_sum_query() -> Query {
    let child = and_of(vec![
        tv_pred_var("category", "predicate", "score"),
        gte(
            dvar("score"),
            DataValue::Data(XSDAnySimpleType::UnsignedLong(0)),
        ),
    ]);
    and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("score".into()),
            group_by: Vec::new(),
            grouped_value: Value::Variable("scores".into()),
            query: Box::new(child),
        }),
        Query::Sum(Sum {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
    ])
}

fn grouped_i64_sum_query() -> Query {
    let child = and_of(vec![
        tv_pred_var("category", "predicate", "score"),
        gte(
            dvar("score"),
            DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
        ),
    ]);
    and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("score".into()),
            group_by: vec!["category".into()],
            grouped_value: Value::Variable("scores".into()),
            query: Box::new(child),
        }),
        Query::Sum(Sum {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
    ])
}

fn two_key_i64_sum_query() -> Query {
    let child = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("left".into()),
            predicate: NodeValue::Node(iri("link")),
            object: NodeValue::Variable("right".into()),
            graph: GraphType::Instance,
        }),
        tv("right", "score", "score"),
        gte(
            dvar("score"),
            DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
        ),
    ]);
    and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("score".into()),
            group_by: vec!["right".into(), "left".into()],
            grouped_value: Value::Variable("scores".into()),
            query: Box::new(child),
        }),
        Query::Sum(Sum {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
        less(
            dvar("total"),
            DataValue::Data(XSDAnySimpleType::Decimal("0".parse().unwrap())),
        ),
    ])
}

fn two_key_group_count_query() -> Query {
    let child = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("left".into()),
            predicate: NodeValue::Node(iri("link")),
            object: NodeValue::Variable("right".into()),
            graph: GraphType::Instance,
        }),
        tv("right", "score", "score"),
        gte(dvar("score"), duint(0)),
    ]);
    and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("score".into()),
            group_by: vec!["right".into(), "left".into()],
            grouped_value: Value::Variable("scores".into()),
            query: Box::new(child),
        }),
        Query::Length(Length {
            list: DataValue::Variable("scores".into()),
            length: DataValue::Variable("cardinality".into()),
        }),
        gte(
            dvar("cardinality"),
            DataValue::Data(XSDAnySimpleType::Decimal("3".parse().unwrap())),
        ),
    ])
}

fn collect_length_for_predicate(predicate: &str, minimum: Option<&str>) -> Query {
    let mut queries = vec![
        Query::Collect(Collect {
            // A ground template deliberately makes every collected element identical;
            // Length must still count every child solution.
            template: Value::Node(iri("same")),
            into: Value::Variable("items".into()),
            query: Box::new(Query::Link(Link {
                subject: NodeValue::Variable("subject".into()),
                predicate: NodeValue::Node(iri(predicate)),
                object: NodeValue::Variable("object".into()),
                graph: GraphType::Instance,
            })),
        }),
        Query::Length(Length {
            list: DataValue::Variable("items".into()),
            length: DataValue::Variable("cardinality".into()),
        }),
    ];
    if let Some(minimum) = minimum {
        queries.push(gte(
            dvar("cardinality"),
            DataValue::Data(XSDAnySimpleType::Decimal(minimum.parse().unwrap())),
        ));
    }
    and_of(queries)
}

fn collect_signed_sum_for_predicate(predicate: &str, compare_negative: bool) -> Query {
    let mut queries = vec![
        Query::Collect(Collect {
            template: Value::Variable("score".into()),
            into: Value::Variable("scores".into()),
            query: Box::new(and_of(vec![
                tv("subject", predicate, "score"),
                gte(
                    dvar("score"),
                    DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
                ),
            ])),
        }),
        Query::Sum(Sum {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
    ];
    if compare_negative {
        queries.push(less(
            dvar("total"),
            DataValue::Data(XSDAnySimpleType::Decimal("1".parse().unwrap())),
        ));
    }
    and_of(queries)
}

fn collect_signed_extrema_for_predicate(predicate: &str, max: bool) -> Query {
    let collect = Query::Collect(Collect {
        template: Value::Variable("score".into()),
        into: Value::Variable("scores".into()),
        query: Box::new(and_of(vec![
            tv("subject", predicate, "score"),
            gte(
                dvar("score"),
                DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
            ),
        ])),
    });
    let range = if max {
        Query::RangeMax(RangeMax {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extreme".into()),
        })
    } else {
        Query::RangeMin(RangeMin {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("extreme".into()),
        })
    };
    and_of(vec![collect, range])
}

fn assert_collect_sum_proves_cancellation_empty_and_substitution() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for value in [-3i64, 1, 2] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri("score"),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = collect_signed_sum_for_predicate("score", true);
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(proved.result_len, 1);
    assert_eq!(
        values,
        vec![vec![CanonicalResultValue::GeneratedDecimalI64(0)]]
    );
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    let Query::And(collect_parts) = collect_signed_sum_for_predicate("score", true) else {
        unreachable!()
    };
    let Query::Collect(collect) = collect_parts.and[0].clone() else {
        unreachable!()
    };
    let group_query = and_of(vec![
        Query::GroupBy(GroupBy {
            template: collect.template,
            group_by: Vec::new(),
            grouped_value: collect.into,
            query: collect.query,
        }),
        collect_parts.and[1].clone(),
        collect_parts.and[2].clone(),
    ]);
    let group = plan_bgp(&group_query).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &group,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let Query::Collect(collect) = collect_parts.and[0].clone() else {
        unreachable!()
    };
    let length_query = and_of(vec![
        Query::Collect(collect),
        Query::Length(Length {
            list: DataValue::Variable("scores".into()),
            length: DataValue::Variable("total".into()),
        }),
    ]);
    let length = plan_bgp(&length_query).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &length,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[0][0] = generated_decimal_i64_field(1);
    tampered.result_values[0][0] = CanonicalResultValue::GeneratedDecimalI64(1);
    tampered.result_commitments[0] =
        commit_column(&[generated_decimal_i64_field(1), ArkFr::zero()]).unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let empty_query = collect_signed_sum_for_predicate("missing", false);
    let empty_compiled = plan_bgp(&empty_query).unwrap();
    let empty = empty_compiled.prove(&layer).unwrap();
    let empty_values = empty
        .verify_and_resolve_values(
            &empty_compiled,
            &layer,
            layer.resolution_root(),
        )
        .unwrap();
    assert_eq!(empty.result_len, 1);
    assert_eq!(
        empty_values,
        vec![vec![CanonicalResultValue::GeneratedDecimalI64(0)]]
    );
}

#[test]
fn collect_sum_proves_cancellation_empty_and_substitution() {
    std::thread::Builder::new()
        .name("woql-collect-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_collect_sum_proves_cancellation_empty_and_substitution)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_collect_extrema_preserves_store_id_empty_and_query_identity() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, value) in [("a", i64::MIN), ("b", -3), ("c", -3), ("d", i64::MAX)] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(subject),
                &iri("score"),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let min_query = collect_signed_extrema_for_predicate("score", false);
    let compiled = plan_bgp(&min_query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let expected = CanonicalResultValue::Object(CanonicalObject::from_store_object(
        &ValueTriple::new_value("", "", i64::make_entry(&i64::MIN)).object,
    ));
    assert_eq!(proved.result_len, 1);
    assert_eq!(values, vec![vec![expected]]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    let max = plan_bgp(&collect_signed_extrema_for_predicate("score", true)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &max,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let other_predicate =
        plan_bgp(&collect_signed_extrema_for_predicate("other", false)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &other_predicate,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let empty_compiled =
        plan_bgp(&collect_signed_extrema_for_predicate("missing", false)).unwrap();
    let empty = empty_compiled.prove(&layer).unwrap();
    assert_eq!(empty.result_len, 0);
    assert_eq!(
        empty
            .verify_and_resolve_values(
                &empty_compiled,
                &layer,
                layer.resolution_root(),
            )
            .unwrap(),
        vec![Vec::new()]
    );
}

#[test]
fn collect_extrema_preserves_store_id_empty_and_query_identity() {
    std::thread::Builder::new()
        .name("woql-collect-extrema-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_collect_extrema_preserves_store_id_empty_and_query_identity)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_collect_length_proves_duplicates_empty_policy_and_substitution() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for object in ["x", "y", "z"] {
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri("a"),
                &iri("p"),
                &iri(object),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = collect_length_for_predicate("p", Some("3"));
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(proved.result_len, 1);
    assert_eq!(
        values,
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(3)]]
    );
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    let group_query = and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("object".into()),
            group_by: Vec::new(),
            grouped_value: Value::Variable("items".into()),
            query: Box::new(Query::Link(Link {
                subject: NodeValue::Variable("subject".into()),
                predicate: NodeValue::Node(iri("p")),
                object: NodeValue::Variable("object".into()),
                graph: GraphType::Instance,
            })),
        }),
        Query::Length(Length {
            list: DataValue::Variable("items".into()),
            length: DataValue::Variable("cardinality".into()),
        }),
        gte(
            dvar("cardinality"),
            DataValue::Data(XSDAnySimpleType::Decimal("3".parse().unwrap())),
        ),
    ]);
    let group = plan_bgp(&group_query).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &group,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(4);
    tampered.result_values[0][0] = CanonicalResultValue::GeneratedDecimalU64(4);
    tampered.result_commitments[0] =
        commit_column(&[ArkFr::from_u64(4), ArkFr::zero()]).unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let empty_query = collect_length_for_predicate("missing", None);
    let empty_compiled = plan_bgp(&empty_query).unwrap();
    let empty = empty_compiled.prove(&layer).unwrap();
    let empty_values = empty
        .verify_and_resolve_values(
            &empty_compiled,
            &layer,
            layer.resolution_root(),
        )
        .unwrap();
    assert_eq!(empty.result_len, 1);
    assert_eq!(
        empty_values,
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(0)]]
    );
}

#[test]
fn collect_length_proves_duplicates_empty_policy_and_substitution() {
    std::thread::Builder::new()
        .name("woql-collect-length-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_collect_length_proves_duplicates_empty_policy_and_substitution)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_two_key_group_count_proves_duplicate_cardinality_and_comparison() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("z"), &iri("link"), &iri("z")))
        .unwrap();
    for value in [1u32, 2] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("z"),
                &iri("score"),
                u32::make_entry(&value),
            ))
            .unwrap();
    }
    let base = builder.commit().unwrap();
    let builder = base.open_write().unwrap();
    for (left, right) in [("a", "a"), ("a", "z"), ("z", "a")] {
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(left),
                &iri("link"),
                &iri(right),
            ))
            .unwrap();
    }
    for value in [3u32, 4, 5] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri("score"),
                u32::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = two_key_group_count_query();
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let node = |name: &str| {
        CanonicalResultValue::Object(CanonicalObject::Node(iri(name).into_bytes()))
    };
    assert_eq!(proved.result_len, 2);
    assert_eq!(values[0], vec![node("a"), node("a")]);
    assert_eq!(values[1], vec![node("a"), node("z")]);
    assert_eq!(
        values[2],
        vec![
            CanonicalResultValue::GeneratedDecimalU64(3),
            CanonicalResultValue::GeneratedDecimalU64(3),
        ]
    );
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    let Query::And(mut swapped) = query else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut swapped.and[0] else {
        unreachable!()
    };
    group.group_by.swap(0, 1);
    let swapped = plan_bgp(&Query::And(swapped)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &swapped,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[2][0] = ArkFr::from_u64(4);
    tampered.result_values[2][0] = CanonicalResultValue::GeneratedDecimalU64(4);
    tampered.result_commitments[2] = commit_column(&pad(
        &tampered.result_columns[2],
        1usize << tampered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
#[ignore = "delta head; GroupBy/Count aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn two_key_group_count_proves_duplicate_cardinality_and_comparison() {
    std::thread::Builder::new()
        .name("woql-two-key-group-count-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_two_key_group_count_proves_duplicate_cardinality_and_comparison)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_two_key_signed_sum_proves_declared_tuple_order_and_comparison() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("z"), &iri("link"), &iri("z")))
        .unwrap();
    for value in [-1i64, 1] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("z"),
                &iri("score"),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let base = builder.commit().unwrap();
    let builder = base.open_write().unwrap();
    for (left, right) in [("a", "a"), ("a", "z"), ("z", "a")] {
        builder
            .add_value_triple(ValueTriple::new_node(
                &iri(left),
                &iri("link"),
                &iri(right),
            ))
            .unwrap();
    }
    for value in [-3i64, 1] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri("score"),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let query = two_key_i64_sum_query();
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(compiled.relation.schema, ["right", "left", "total"]);
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let node = |name: &str| {
        CanonicalResultValue::Object(CanonicalObject::Node(iri(name).into_bytes()))
    };
    assert_eq!(proved.result_len, 2);
    assert_eq!(values[0], vec![node("a"), node("a")]);
    assert_eq!(values[1], vec![node("a"), node("z")]);
    assert_eq!(
        values[2],
        vec![
            CanonicalResultValue::GeneratedDecimalI64(-2),
            CanonicalResultValue::GeneratedDecimalI64(-2),
        ]
    );
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();

    let Query::And(mut swapped) = query else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut swapped.and[0] else {
        unreachable!()
    };
    group.group_by.swap(0, 1);
    let swapped = plan_bgp(&Query::And(swapped)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &swapped,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[2][0] = generated_decimal_i64_field(-1);
    tampered.result_values[2][0] = CanonicalResultValue::GeneratedDecimalI64(-1);
    tampered.result_commitments[2] = commit_column(&pad(
        &tampered.result_columns[2],
        1usize << tampered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
#[ignore = "delta head; GroupBy/Sum aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn two_key_signed_sum_proves_declared_tuple_order_and_comparison() {
    std::thread::Builder::new()
        .name("woql-two-key-signed-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_two_key_signed_sum_proves_declared_tuple_order_and_comparison)
        .unwrap()
        .join()
        .unwrap();
}

fn global_i64_sum_query() -> Query {
    let Query::And(mut query) = grouped_i64_sum_query() else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut query.and[0] else {
        unreachable!()
    };
    group.group_by.clear();
    Query::And(query)
}

fn assert_global_u32_sum_proves_one_generated_decimal_row() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, value) in [("a", 2u32), ("b", 3u32)] {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(subject),
                &iri("p"),
                u32::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&global_u32_sum_query()).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 1);
    assert_eq!(
        proved
            .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(5)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["total".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(5)]],
        )
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();
    let keyed = plan_bgp(&grouped_u32_sum_query()).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &keyed,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(6);
    tampered.result_commitments[0] =
        commit_column(&[ArkFr::from_u64(6), ArkFr::zero()]).unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn global_u32_sum_proves_one_generated_decimal_row() {
    std::thread::Builder::new()
        .name("woql-global-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_global_u32_sum_proves_one_generated_decimal_row)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_global_u64_sum_accepts_exact_max_and_rejects_actual_overflow() {
    let layer_with = |values: &[(&str, u64)]| {
        let store = open_sync_memory_store();
        let builder = store.create_base_layer().unwrap();
        for (predicate, value) in values {
            builder
                .add_value_triple(ValueTriple::new_value(
                    &iri("a"),
                    &iri(predicate),
                    u64::make_entry(value),
                ))
                .unwrap();
        }
        builder.commit().unwrap().proof_commitment().unwrap()
    };
    let compiled = plan_bgp(&global_u64_sum_query()).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GroupSum {
            ref key_columns,
            datatype: CanonicalDatatype::UInt64,
            ..
        } if key_columns.is_empty()
    ));

    let exact = layer_with(&[("p", u64::MAX - 5), ("q", 5)]);
    let proved = compiled.prove(&exact).unwrap();
    assert_eq!(
        proved
            .verify_and_resolve_values(&compiled, &exact, exact.resolution_root())
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(u64::MAX)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &exact,
            exact.resolution_root(),
            &["total".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(u64::MAX)]],
        )
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &exact, exact.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &exact, exact.resolution_root())
        .unwrap();

    let overflow = layer_with(&[("p", u64::MAX), ("q", 1)]);
    assert!(compiled.prove(&overflow).is_err());
}

#[test]
fn global_u64_sum_accepts_exact_max_and_rejects_actual_overflow() {
    std::thread::Builder::new()
        .name("woql-global-u64-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_global_u64_sum_accepts_exact_max_and_rejects_actual_overflow)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_grouped_i64_sum_proves_signed_values_and_rejects_overflow() {
    let store = open_sync_memory_store();
    let base_builder = store.create_base_layer().unwrap();
    for (predicate, value) in [("p", i64::MIN), ("q", i64::MAX)] {
        base_builder
            .add_value_triple(ValueTriple::new_value(
                &iri("b"),
                &iri(predicate),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let base = base_builder.commit().unwrap();
    let child_builder = base.open_write().unwrap();
    for (predicate, value) in [("p", -2i64), ("q", -2), ("r", 5), ("z", 0)] {
        child_builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri(predicate),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let layer = child_builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&grouped_i64_sum_query()).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GroupSum { .. }
    ));

    let Query::And(mut filtered_query) = grouped_i64_sum_query() else {
        unreachable!()
    };
    filtered_query.and.push(less(
        dvar("total"),
        DataValue::Data(XSDAnySimpleType::Decimal("0".parse().unwrap())),
    ));
    let filtered_compiled = plan_bgp(&Query::And(filtered_query.clone())).unwrap();
    assert!(matches!(
        filtered_compiled.relation.statement,
        CompiledRelationStatement::GeneratedI64Filter { .. }
    ));
    let filtered = filtered_compiled.prove(&layer).unwrap();
    let filtered_values = filtered
        .verify_and_resolve_values(
            &filtered_compiled,
            &layer,
            layer.resolution_root(),
        )
        .unwrap();
    assert_eq!(filtered.result_len, 1);
    assert_eq!(
        filtered_values[0],
        vec![CanonicalResultValue::Object(CanonicalObject::Node(
            iri("b").into_bytes(),
        ))]
    );
    assert_eq!(
        filtered_values[1],
        vec![CanonicalResultValue::GeneratedDecimalI64(-1)]
    );
    let key_id = layer
        .node_value_catalog
        .iter()
        .find(|record| ArkFr::from_u64(record.object_id) == filtered.result_columns[0][0])
        .unwrap()
        .object_id;
    filtered
        .verify_executed_values(
            &filtered_compiled,
            &layer,
            layer.resolution_root(),
            &["category".into(), "total".into()],
            &[vec![
                ExecutedResultValue::Id(key_id),
                ExecutedResultValue::GeneratedDecimalI64(-1),
            ]],
        )
        .unwrap();
    let filtered_envelope = filtered
        .encode_envelope(
            &filtered_compiled,
            &layer,
            layer.resolution_root(),
        )
        .unwrap();
    decode_and_verify_envelope(
        &filtered_envelope,
        &filtered_compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();

    filtered_query.and.pop();
    filtered_query.and.push(greater(
        DataValue::Data(XSDAnySimpleType::Decimal("0".parse().unwrap())),
        dvar("total"),
    ));
    let reversed = plan_bgp(&Query::And(filtered_query)).unwrap();
    assert!(decode_and_verify_envelope(
        &filtered_envelope,
        &reversed,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let mut filtered_tamper = RecursiveRelationEnvelope::decode(&filtered_envelope).unwrap();
    filtered_tamper.result_columns[1][0] = generated_decimal_i64_field(1);
    filtered_tamper.result_values[1][0] = CanonicalResultValue::GeneratedDecimalI64(1);
    filtered_tamper.result_commitments[1] = commit_column(&pad(
        &filtered_tamper.result_columns[1],
        1usize << filtered_tamper.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &filtered_tamper.encode().unwrap(),
        &filtered_compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let mut wrong_domain = RecursiveRelationEnvelope::decode(&filtered_envelope).unwrap();
    wrong_domain.result_descriptors[1].namespace = ResultIdNamespace::GeneratedDecimalU64;
    assert!(decode_and_verify_envelope(
        &wrong_domain.encode().unwrap(),
        &filtered_compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let overflow_store = open_sync_memory_store();
    let overflow_builder = overflow_store.create_base_layer().unwrap();
    for (predicate, value) in [("p", i64::MAX), ("q", 1)] {
        overflow_builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri(predicate),
                i64::make_entry(&value),
            ))
            .unwrap();
    }
    let overflow = overflow_builder.commit().unwrap().proof_commitment().unwrap();
    let global = plan_bgp(&global_i64_sum_query()).unwrap();
    assert!(global.prove(&overflow).is_err());
}

#[test]
#[ignore = "delta head; GroupBy/Sum aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn grouped_i64_sum_proves_signed_values_and_rejects_overflow() {
    std::thread::Builder::new()
        .name("woql-grouped-i64-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_grouped_i64_sum_proves_signed_values_and_rejects_overflow)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_grouped_u32_sum_proves_duplicates_safe_max_and_canonical_child_stack_key_order() {
    let store = open_sync_memory_store();
    let base_builder = store.create_base_layer().unwrap();
    for (predicate, value) in [("p", 0u32), ("q", u32::MAX)] {
        base_builder
            .add_value_triple(ValueTriple::new_value(
                &iri("b"),
                &iri(predicate),
                u32::make_entry(&value),
            ))
            .unwrap();
    }
    let base = base_builder.commit().unwrap();
    let child_builder = base.open_write().unwrap();
    for predicate in ["p", "q"] {
        child_builder
            .add_value_triple(ValueTriple::new_value(
                &iri("a"),
                &iri(predicate),
                u32::make_entry(&2),
            ))
            .unwrap();
    }
    let layer = child_builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&grouped_u32_sum_query()).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(proved.result_len, 2);
    assert_eq!(
        values[0],
        vec![
            CanonicalResultValue::Object(CanonicalObject::Node(iri("a").into_bytes())),
            CanonicalResultValue::Object(CanonicalObject::Node(iri("b").into_bytes())),
        ]
    );
    assert_eq!(
        values[1],
        vec![
            CanonicalResultValue::GeneratedDecimalU64(4),
            CanonicalResultValue::GeneratedDecimalU64(u64::from(u32::MAX)),
        ]
    );
    let executed = (0..proved.result_len)
        .map(|row| {
            let key_id = layer
                .node_value_catalog
                .iter()
                .find(|record| ArkFr::from_u64(record.object_id) == proved.result_columns[0][row])
                .unwrap()
                .object_id;
            let sum = match values[1][row] {
                CanonicalResultValue::GeneratedDecimalU64(sum) => sum,
                _ => unreachable!(),
            };
            vec![
                ExecutedResultValue::Id(key_id),
                ExecutedResultValue::GeneratedDecimalU64(sum),
            ]
        })
        .collect::<Vec<_>>();
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["category".into(), "total".into()],
            &executed,
        )
        .unwrap();

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
        .unwrap();
    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[1][0] = ArkFr::from_u64(5);
    tampered.result_commitments[1] = commit_column(&pad(
        &tampered.result_columns[1],
        1usize << tampered.result_log_domain,
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &tampered.encode().unwrap(),
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let Query::And(parts) = grouped_u32_sum_query() else {
        unreachable!()
    };
    let Query::GroupBy(group) = parts.and[0].clone() else {
        unreachable!()
    };
    let substituted = plan_bgp(&and_of(vec![
        Query::GroupBy(group),
        Query::RangeMax(RangeMax {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
#[ignore = "delta head; GroupBy/Sum aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn grouped_u32_sum_proves_duplicates_safe_max_and_canonical_child_stack_key_order() {
    std::thread::Builder::new()
        .name("woql-group-sum-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_grouped_u32_sum_proves_duplicates_safe_max_and_canonical_child_stack_key_order)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_grouped_u32_sum_empty_and_outer_count_are_exact() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("x"), &iri("p"), &iri("y")))
        .unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("u"), &iri("q"), &iri("v")))
        .unwrap();
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&Query::Count(Count {
        query: Box::new(global_u32_sum_query()),
        count: DataValue::Variable("groups".into()),
    }))
    .unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.count_binding(&compiled), Some(("groups", 0)));
    proved
        .verify_executed_rows(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["groups".into()],
            &[vec![0]],
        )
        .unwrap();
}

#[test]
fn grouped_u32_sum_empty_and_outer_count_are_exact() {
    std::thread::Builder::new()
        .name("woql-group-sum-empty-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_grouped_u32_sum_empty_and_outer_count_are_exact)
        .unwrap()
        .join()
        .unwrap();
}

fn resolved_group_extrema(
    proved: &terminusdb_woql2::proof::ProvedBgp,
    compiled: &terminusdb_woql2::proof::CompiledBgp,
    layer: &LayerCommitment,
) -> Vec<(String, u32)> {
    let columns = proved
        .verify_and_resolve_values(compiled, layer, layer.resolution_root())
        .unwrap();
    let mut rows = (0..proved.result_len)
        .map(|row| {
            let category = match &columns[0][row] {
                CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => {
                    String::from_utf8(bytes.clone()).unwrap()
                }
                other => panic!("unexpected category {other:?}"),
            };
            let score = match &columns[1][row] {
                CanonicalResultValue::Object(CanonicalObject::Value {
                    datatype: CanonicalDatatype::UInt32,
                    lexical,
                }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
                other => panic!("unexpected score {other:?}"),
            };
            (category, score)
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn assert_grouped_min_and_max_prove_recursive_envelopes_and_compact_artifacts() {
    let layer = build_grouped_score_layer();
    let mut min_envelope = None;
    for (kind, expected) in [
        (RangeOp::Ge, vec![(iri("c1"), 10), (iri("c2"), 5)]),
        (RangeOp::Le, vec![(iri("c1"), 30), (iri("c2"), 20)]),
    ] {
        let query = grouped_score_extrema(kind);
        let compiled = plan_bgp(&query).unwrap();
        let proved = compiled.prove(&layer).unwrap();
        assert_eq!(resolved_group_extrema(&proved, &compiled, &layer), expected);
        let envelope = proved
            .encode_envelope(&compiled, &layer, layer.resolution_root())
            .unwrap();
        let decoded =
            decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
                .unwrap();
        assert_eq!(decoded.proved.result_len, 2);
        let compact = layer.verifier_commitment().unwrap().encode().unwrap();
        decode_verifier_commitment_and_verify_envelope(
            &envelope,
            &compiled,
            &compact,
            layer.resolution_root(),
        )
        .unwrap();
        if kind == RangeOp::Ge {
            min_envelope = Some((envelope, compiled));
        }
    }

    let (envelope, min_compiled) = min_envelope.unwrap();
    // The same proof cannot be substituted for RangeMax: kind is in both relation
    // digest and group transcript.
    let max_compiled = plan_bgp(&grouped_score_extrema(RangeOp::Le)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &max_compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    // Wrong extremum and group reassignment both alter committed public columns.
    let mut wrong = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    wrong.result_columns[1].swap(0, 1);
    wrong.result_commitments[1] = commit_column(&pad(
        &wrong.result_columns[1],
        1usize << wrong.result_log_domain,
    ))
    .unwrap();
    let wrong_extremum = wrong.encode().unwrap();
    assert!(decode_and_verify_envelope(
        &wrong_extremum,
        &min_compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let mut reassigned = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    reassigned.result_columns[0][1] = reassigned.result_columns[0][0];
    reassigned.result_commitments[0] = commit_column(&pad(
        &reassigned.result_columns[0],
        1usize << reassigned.result_log_domain,
    ))
    .unwrap();
    let reassigned = reassigned.encode().unwrap();
    assert!(decode_and_verify_envelope(
        &reassigned,
        &min_compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn grouped_min_and_max_prove_recursive_envelopes_and_compact_artifacts() {
    std::thread::Builder::new()
        .name("woql-group-extrema-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_grouped_min_and_max_prove_recursive_envelopes_and_compact_artifacts)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_two_key_grouped_extrema_preserves_visible_declared_tuple_order() {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder.add_value_triple(ValueTriple::new_node(
        &iri("z"), &iri("link"), &iri("a"),
    )).unwrap();
    for score in [10u32, 30] {
        builder.add_value_triple(ValueTriple::new_value(
            &iri("a"), &iri("score"), u32::make_entry(&score),
        )).unwrap();
    }
    let base = builder.commit().unwrap();
    let builder = base.open_write().unwrap();
    builder.add_value_triple(ValueTriple::new_node(
        &iri("a"), &iri("link"), &iri("z"),
    )).unwrap();
    for score in [5u32, 20] {
        builder.add_value_triple(ValueTriple::new_value(
            &iri("z"), &iri("score"), u32::make_entry(&score),
        )).unwrap();
    }
    let layer = builder.commit().unwrap().proof_commitment().unwrap();
    let compiled = plan_bgp(&two_key_grouped_score_extrema(false)).unwrap();
    assert_eq!(compiled.relation.schema, ["category", "owner", "extremum"]);
    let proved = compiled.prove(&layer).unwrap();
    let values = proved.verify_and_resolve_values(
        &compiled, &layer, layer.resolution_root(),
    ).unwrap();
    let node = |column: usize, row: usize| match &values[column][row] {
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) =>
            String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("unexpected grouped-extrema key {other:?}"),
    };
    let score = |row: usize| match &values[2][row] {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::UInt32,
            lexical,
        }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
        other => panic!("unexpected grouped-extrema value {other:?}"),
    };
    assert_eq!(proved.result_len, 2);
    assert_eq!(
        vec![(node(0, 0), node(1, 0), score(0)), (node(0, 1), node(1, 1), score(1))],
        vec![(iri("a"), iri("z"), 10), (iri("z"), iri("a"), 5)],
    );
    let envelope = proved.encode_envelope(
        &compiled, &layer, layer.resolution_root(),
    ).unwrap();
    decode_and_verify_envelope(
        &envelope, &compiled, &layer, layer.resolution_root(),
    ).unwrap();

    let max = plan_bgp(&two_key_grouped_score_extrema(true)).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &max, &layer, layer.resolution_root(),
    ).is_err());
    let mut reordered = compiled.clone();
    let CompiledRelationStatement::GroupExtrema { key_columns, .. } =
        &mut reordered.relation.statement else { unreachable!() };
    key_columns.swap(0, 1);
    assert!(decode_and_verify_envelope(
        &envelope, &reordered, &layer, layer.resolution_root(),
    ).is_err());
}

#[test]
#[ignore = "delta head; GroupBy/Min-Max aggregate not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn two_key_grouped_extrema_preserves_visible_declared_tuple_order() {
    std::thread::Builder::new()
        .name("woql-two-key-group-extrema-proof".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(assert_two_key_grouped_extrema_preserves_visible_declared_tuple_order)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn grouped_extrema_count_wrapper_binds_group_cardinality() {
    let layer = build_grouped_score_layer();
    let query = Query::Count(Count {
        query: Box::new(grouped_score_extrema(RangeOp::Ge)),
        count: DataValue::Variable("count".into()),
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.count_binding(&compiled), Some(("count", 2)));
    proved
        .verify_executed_rows(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["count".into()],
            &[vec![2]],
        )
        .unwrap();
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["count".into()],
            &[vec![1]],
        )
        .is_err());
}

#[test]
fn correlated_optional_proves_all_matches_and_one_null_extended_row() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "u"),
        ("b", "q", "v"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(proved.result_len, 3);
    assert_eq!(proved.result_columns[2][2], ArkFr::zero());
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(values[2][2], CanonicalResultValue::Null);
    assert!(values[2][..2]
        .iter()
        .all(|value| matches!(value, CanonicalResultValue::Object(_))));
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(
        verified.result_values.as_ref().unwrap()[2][2],
        CanonicalResultValue::Null
    );
    let non_optional = plan_bgp(&tv("x", "p", "y")).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &non_optional,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let rows: Vec<Vec<ExecutedResultValue>> = (0..proved.result_len)
        .map(|row| {
            proved
                .result_columns
                .iter()
                .map(|column| {
                    if column[row].is_zero() {
                        ExecutedResultValue::Null
                    } else {
                        ExecutedResultValue::Id(
                            layer
                                .node_value_catalog
                                .iter()
                                .find(|record| ArkFr::from_u64(record.object_id) == column[row])
                                .unwrap()
                                .object_id,
                        )
                    }
                })
                .collect()
        })
        .collect();
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["x".into(), "y".into(), "z".into()],
            &rows,
        )
        .unwrap();
    let mut null_in_shared = rows.clone();
    null_in_shared[0][1] = ExecutedResultValue::Null;
    assert!(proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &["x".into(), "y".into(), "z".into()],
            &null_in_shared,
        )
        .is_err());

    let mut fake_null = proved;
    fake_null.result_columns[2][0] = ArkFr::zero();
    assert!(fake_null
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn link_form_compiles_at_woql2_ast_level() {
    // `Link` is now implemented in the canonical woql2 proof AST (a typed-triple
    // BGP with an explicit node-kind object filter). The leading TerminusDB JSON
    // parser not yet exposing the `Link` variant is a separate node-integration
    // concern, not an AST-compiler rejection.
    let query = Query::Link(Link {
        subject: terminusdb_woql2::value::NodeValue::Variable("x".into()),
        predicate: terminusdb_woql2::value::NodeValue::Node(iri("p")),
        object: terminusdb_woql2::value::NodeValue::Variable("y".into()),
        graph: GraphType::Instance,
    });
    assert!(plan_bgp(&query).is_ok());
}

#[test]
fn exact_equals_filters_authenticated_relations_and_binds_operands() {
    let layer = build_layer(&[
        ("a", "p", "a"),
        ("a", "p", "b"),
        ("c", "p", "c"),
        ("a", "q", "u"),
        ("b", "q", "v"),
        ("c", "q", "w"),
    ]);
    let constant_query = and_of(vec![
        tv("x", "p", "y"),
        equals(value_var("x"), value_node("a")),
    ]);
    let constant_compiled = plan_bgp(&constant_query).unwrap();
    let constant_proved = constant_compiled.prove(&layer).unwrap();
    assert_eq!(constant_proved.result_len, 2);
    let constant_envelope = constant_proved
        .encode_envelope(
            &constant_compiled,
            &layer,
            layer.resolution_root(),
        )
        .unwrap();
    decode_and_verify_envelope(
        &constant_envelope,
        &constant_compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();

    let wrong_constant = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        equals(value_var("x"), value_node("c")),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &constant_envelope,
        &wrong_constant,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let reversed_operands = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        equals(value_node("a"), value_var("x")),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &constant_envelope,
        &reversed_operands,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut forged_row = constant_proved.result_columns.clone();
    forged_row[1][0] = layer
        .node_value_catalog
        .iter()
        .find(|record| record.triple.object == CanonicalObject::Node(iri("c").into_bytes()))
        .map(|record| ArkFr::from_u64(record.object_id))
        .unwrap();
    assert!(constant_proved.verify_columns(&forged_row).is_err());
    let mut spliced_column = constant_proved.result_columns.clone();
    spliced_column[1] = spliced_column[0].clone();
    assert!(constant_proved.verify_columns(&spliced_column).is_err());

    let other_layer = build_layer(&[("a", "p", "a"), ("a", "p", "b"), ("d", "p", "d")]);
    assert!(decode_and_verify_envelope(
        &constant_envelope,
        &constant_compiled,
        &other_layer,
        other_layer.resolution_root(),
    )
    .is_err());

    let columns_query = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        equals(value_var("x"), value_var("y")),
    ]);
    let columns_compiled = plan_bgp(&columns_query).unwrap();
    let columns_proved = columns_compiled.prove(&layer).unwrap();
    assert_eq!(columns_proved.result_len, 2);
    let columns_envelope = columns_proved
        .encode_envelope(&columns_compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(
        &columns_envelope,
        &columns_compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();
    let reversed_alias = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        equals(value_var("y"), value_var("x")),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &columns_envelope,
        &reversed_alias,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
    let mut reordered_schema = columns_compiled.clone();
    reordered_schema.relation.schema.swap(0, 1);
    reordered_schema.relation.domains.swap(0, 1);
    assert!(decode_and_verify_envelope(
        &columns_envelope,
        &reordered_schema,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let false_gate = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        equals(value_node("a"), value_node("missing")),
    ]))
    .unwrap();
    let false_proved = false_gate.prove(&layer).unwrap();
    assert_eq!(false_proved.result_len, 0);
    let false_envelope = false_proved
        .encode_envelope(&false_gate, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(
        &false_envelope,
        &false_gate,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();
}

#[test]
fn exact_typed_equals_composes_with_select_and_count() {
    let layer = build_age_name_layer(&[("a", 7), ("b", 8), ("c", 7)]);
    let relation = and_of(vec![
        tv("x", "age", "age"),
        greater(dvar("age"), duint(0)),
        equals(value_var("age"), value_uint(7)),
    ]);
    let query = Query::Count(Count {
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into(), "age".into()],
            query: Box::new(relation),
        })),
        count: DataValue::Variable("count".into()),
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 2);
    assert_eq!(proved.count_binding(&compiled), Some(("count", 2)));
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(verified.proved.result_len, 2);
}

#[test]
fn exact_equals_filters_after_recursive_optional_and_not() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "u"),
        ("a", "r", "blocked"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::Not(Not {
            query: Box::new(tv("x", "r", "reason")),
        }),
        equals(value_var("x"), value_node("c")),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let (input, _) = compiled.relation.as_equality().unwrap();
    assert!(input.as_left_anti().unwrap().0.as_left_outer().is_some());
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 1);
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(values[2], [CanonicalResultValue::Null]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();

    // Simulate archive/reopen, derive the compact public view, then perform remote-style
    // decode + verification without retaining any native/materialized witness records.
    let reopened = LayerCommitment::decode(&layer.encode().unwrap()).unwrap();
    let compact = reopened.verifier_commitment().unwrap().encode().unwrap();
    let verified = decode_verifier_commitment_and_verify_envelope(
        &envelope,
        &compiled,
        &compact,
        layer.resolution_root(),
    )
    .unwrap();
    assert_eq!(verified.proved.result_len, 1);
}

#[test]
fn or_of_optional_with_optional_right_or_preserves_bag_and_nulls() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "u"),
        ("b", "r", "v"),
        ("d", "s", "w"), // keeps every queried predicate in the catalog
    ]);
    let optional_right_or = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(Query::Or(Or {
                or: vec![tv("y", "q", "z"), tv("y", "r", "z")],
            })),
        }),
    ]);
    let optional_plain = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
    ]);
    let query = Query::Or(Or {
        or: vec![optional_right_or, optional_plain],
    });
    let compiled = plan_bgp(&query).unwrap();
    let (branches, _) = compiled.relation.as_bag_union().unwrap();
    assert!(branches[0]
        .as_left_outer()
        .unwrap()
        .1
        .as_bag_union()
        .is_some());
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 5);
    assert_eq!(
        proved.result_columns[2]
            .iter()
            .filter(|value| value.is_zero())
            .count(),
        2
    );
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(verified.proved.result_len, 5);
    assert_eq!(
        verified.result_values.as_ref().unwrap()[2]
            .iter()
            .filter(|value| **value == CanonicalResultValue::Null)
            .count(),
        2
    );
    let u = CanonicalResultValue::Object(CanonicalObject::Node(iri("u").into_bytes()));
    assert_eq!(
        verified.result_values.as_ref().unwrap()[2]
            .iter()
            .filter(|value| **value == u)
            .count(),
        2,
        "overlapping Optional branches retain bag multiplicity",
    );
    let compact = layer.verifier_commitment().unwrap().encode().unwrap();
    let compact_verified = decode_verifier_commitment_and_verify_envelope(
        &envelope,
        &compiled,
        &compact,
        layer.resolution_root(),
    )
    .unwrap();
    assert_eq!(compact_verified.proved.result_len, 5);

    let mut wrong_nullability = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    wrong_nullability.result_descriptors[2].nullable = false;
    let wrong_nullability = wrong_nullability.encode().unwrap();
    assert!(decode_and_verify_envelope(
        &wrong_nullability,
        &compiled,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut reordered = compiled.clone();
    if let CompiledRelationStatement::BagUnion { branches, mappings } =
        &mut reordered.relation.statement
    {
        branches.swap(0, 1);
        mappings.swap(0, 1);
    }
    assert!(decode_and_verify_envelope(
        &envelope,
        &reordered,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let substituted = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "s", "z")),
        }),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn sequential_optional_then_not_filters_on_nonnullable_key() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "u"),
        ("a", "r", "blocked"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::Not(Not {
            query: Box::new(tv("x", "r", "reason")),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let (left, _, keys) = compiled.relation.as_left_anti().unwrap();
    assert!(left.as_left_outer().is_some());
    assert_eq!(keys, [(0, 0)]);
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 1);
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert_eq!(values[2], [CanonicalResultValue::Null]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(
        verified.result_values.as_ref().unwrap()[2],
        [CanonicalResultValue::Null]
    );
}

#[test]
fn dictionary_absence_composes_through_sequential_not_and_optional() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("anchor", "q", "value"),
        ("anchor", "r", "value"),
    ]);
    let branch = |predicate: &str, optional_subject: &str| {
        and_of(vec![
            tv("x", predicate, "y"),
            Query::Not(Not {
                query: Box::new(tv_obj_node("y", "q", "missing")),
            }),
            Query::WoqlOptional(WoqlOptional {
                query: Box::new(tv_subj_const(optional_subject, "r", "z")),
            }),
        ])
    };
    let query = branch("p", "missing");
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 2);
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert!(values[2]
        .iter()
        .all(|value| *value == CanonicalResultValue::Null));

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();

    // The absence claim is part of the recursive query statement. Replacing one absent
    // subject with a dictionary-present subject changes both semantics and the digest.
    let substituted = plan_bgp(&branch("p", "anchor")).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn dictionary_absence_is_an_exact_empty_or_branch() {
    let layer = build_layer(&[("a", "p", "b"), ("c", "p", "d")]);
    let query = Query::Or(Or {
        or: vec![tv("x", "p", "y"), tv("x", "absent-predicate", "y")],
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 2);
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();
}

#[test]
fn composite_key_optional_requires_both_keys_and_preserves_right_duplicates() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("e", "p", "f"),
        ("a", "q", "b"),
        ("c", "q", "d"),
        ("a", "q", "c"), // partial-key match only for the first left row
        ("b", "r", "u"),
        ("b", "r", "v"),
        ("c", "r", "partial"),
        ("d", "r", "w"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(and_of(vec![tv("x", "q", "y"), tv("y", "r", "z")])),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(
        compiled.relation.as_left_outer().unwrap().2,
        vec![(0, 0), (1, 1)]
    );
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 4);
    assert_eq!(
        proved.result_columns[2]
            .iter()
            .take(proved.result_len)
            .filter(|value| value.is_zero())
            .count(),
        1
    );
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut swapped = compiled.clone();
    swapped.relation.as_left_outer_mut().unwrap().swap(0, 1);
    assert!(proved
        .verify(&swapped, &layer, layer.resolution_root())
        .is_err());
    let mut fake_null = proved;
    let matched = fake_null.result_columns[2]
        .iter()
        .position(|value| !value.is_zero())
        .unwrap();
    fake_null.result_columns[2][matched] = ArkFr::zero();
    assert!(fake_null
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn global_optional_empty_right_nulls_every_right_namespace_exactly_once() {
    let layer = build_layer(&[("a", "p", "b"), ("c", "p", "d")]);
    // The first right atom exposes a predicate variable. Joining its object to the
    // subject of a p-edge makes the authenticated right relation empty in this layer.
    let right = and_of(vec![tv_pred_var("r", "pred", "o"), tv("o", "p", "z")]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(right),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    assert!(compiled.relation.as_left_outer().unwrap().2.is_empty());
    assert_eq!(compiled.relation.schema, vec!["x", "y", "r", "pred", "o", "z"]);
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 2);
    assert!(proved.result_columns[2..]
        .iter()
        .all(|column| column[..proved.result_len].iter().all(Field::is_zero)));
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert!(values[2..]
        .iter()
        .all(|column| column.iter().all(|value| *value == CanonicalResultValue::Null)));

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root()).unwrap();

    let rows: Vec<Vec<ExecutedResultValue>> = (0..proved.result_len)
        .map(|row| {
            let node_id = |column: usize| {
                layer
                    .node_value_catalog
                    .iter()
                    .find(|record| {
                        ArkFr::from_u64(record.object_id) == proved.result_columns[column][row]
                    })
                    .unwrap()
                    .object_id
            };
            vec![
                ExecutedResultValue::Id(node_id(0)),
                ExecutedResultValue::Id(node_id(1)),
                ExecutedResultValue::Null,
                ExecutedResultValue::Null,
                ExecutedResultValue::Null,
                ExecutedResultValue::Null,
            ]
        })
        .collect();
    let variables = ["x", "y", "r", "pred", "o", "z"].map(str::to_string);
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &variables,
            &rows,
        )
        .unwrap();
    let mut missing = rows.clone();
    missing.pop();
    assert!(proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &variables,
            &missing,
        )
        .is_err());
    let mut extra = rows.clone();
    extra.push(rows[0].clone());
    assert!(proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &variables,
            &extra,
        )
        .is_err());
    let mut forged_predicate = rows;
    let object_id = match forged_predicate[0][0] {
        ExecutedResultValue::Id(id) => id,
        ExecutedResultValue::GeneratedDecimalU64(_)
        | ExecutedResultValue::GeneratedDecimalI64(_)
        | ExecutedResultValue::Null => unreachable!(),
    };
    forged_predicate[0][3] = ExecutedResultValue::Id(object_id);
    assert!(proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.resolution_root(),
            &variables,
            &forged_predicate,
        )
        .is_err());
}

#[test]
fn predicate_variable_proves_and_resolves_only_through_predicate_catalog() {
    let layer = build_layer(&[
        ("a", "knows", "b"),
        ("a", "likes", "c"),
        ("d", "knows", "e"),
    ]);
    let query = tv_pred_var("s", "p", "o");
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(compiled.relation.schema, vec!["s", "p", "o"]);
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 3);
    let objects = proved
        .verify_and_resolve(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let predicates: Vec<_> = objects[1]
        .iter()
        .map(|object| match object {
            CanonicalObject::Node(iri) => iri.clone(),
            CanonicalObject::Value { .. } => panic!("predicate resolved as a value"),
        })
        .collect();
    assert!(predicates.contains(&iri("knows").into_bytes()));
    assert!(predicates.contains(&iri("likes").into_bytes()));

    let mut forged_objects = objects.clone();
    forged_objects[1][0] = CanonicalObject::Node(iri("forged").into_bytes());
    assert!(proved
        .verify_objects(
            &compiled,
            &layer,
            layer.resolution_root(),
            &forged_objects,
        )
        .is_err());

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(verified.result_objects, objects);
    let mut reordered_schema = compiled.clone();
    reordered_schema.relation.schema.swap(0, 1);
    assert!(decode_and_verify_envelope(
        &envelope,
        &reordered_schema,
        &layer,
        layer.resolution_root(),
    )
    .is_err());

    let mut forged = proved;
    forged.result_columns[1][0] = ArkFr::from_u64(999_999);
    assert!(forged
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn predicate_variable_composite_join_preserves_predicate_namespace() {
    let layer = build_layer(&[
        ("a", "knows", "b"),
        ("b", "knows", "c"),
        ("b", "likes", "d"),
        ("d", "likes", "e"),
    ]);
    let query = and_of(vec![tv_pred_var("x", "p", "y"), tv_pred_var("y", "p", "z")]);
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 2);
    let objects = proved
        .verify_and_resolve(&compiled, &layer, layer.resolution_root())
        .unwrap();
    assert!(objects[1].iter().all(|predicate| matches!(
        predicate,
        CanonicalObject::Node(iri) if iri == &crate::proof_common::iri("knows").into_bytes()
            || iri == &crate::proof_common::iri("likes").into_bytes()
    )));
}

#[test]
#[ignore = "delta head; predicate-variable BGP not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn predicate_variable_resolves_inherited_and_new_predicates_at_child_root() {
    let store = open_sync_memory_store();
    let base_builder = store.create_base_layer().unwrap();
    base_builder
        .add_value_triple(ValueTriple::new_node(
            &iri("a"),
            &iri("inherited"),
            &iri("b"),
        ))
        .unwrap();
    let base = base_builder.commit().unwrap();
    let base_commitment = base.proof_commitment().unwrap();
    let child_builder = base.open_write().unwrap();
    child_builder
        .add_value_triple(ValueTriple::new_node(&iri("c"), &iri("new"), &iri("d")))
        .unwrap();
    let child = child_builder.commit().unwrap();
    let child_commitment = child.proof_commitment().unwrap();

    let compiled = plan_bgp(&tv_pred_var("s", "p", "o")).unwrap();
    let proved = compiled.prove(&child_commitment).unwrap();
    let objects = proved
        .verify_and_resolve(
            &compiled,
            &child_commitment,
            child_commitment.resolution_root(),
        )
        .unwrap();
    assert_eq!(proved.result_len, 2);
    assert!(objects[1].contains(&CanonicalObject::Node(iri("inherited").into_bytes())));
    assert!(objects[1].contains(&CanonicalObject::Node(iri("new").into_bytes())));
    assert!(proved
        .verify(
            &compiled,
            &child_commitment,
            base_commitment.resolution_root()
        )
        .is_err());
}

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
    assert_eq!(compiled.relation.schema, vec!["x", "y"]);
    assert_eq!(proved.result_len, 1, "only c-p-d has no correlated q match");
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(verified.proved.result_len, 1);
    let mut corrupt = envelope;
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &layer, layer.resolution_root(),)
            .is_err()
    );

    let original_len = proved.result_len;
    let mut forged = proved;
    forged.result_len = 0;
    assert!(forged
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
    forged.result_len = original_len;
    forged.result_columns[0][0] = ArkFr::from_u64(999_999);
    assert!(forged
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn not_over_or_proves_and_roundtrips_recursive_statement_tree() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "u"),
        ("b", "r", "v"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(Query::Or(Or {
                or: vec![tv("y", "q", "z"), tv("y", "r", "z")],
            })),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    assert!(compiled.relation.as_left_anti().unwrap().1.as_bag_union().is_some());
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(proved.result_len, 1);
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.resolution_root())
            .unwrap();
    assert_eq!(verified.proved.result_len, 1);

    let mut reordered = compiled.clone();
    // Changing the right branch order changes the child and parent digests.
    let right = match &mut reordered.relation.statement {
        CompiledRelationStatement::LeftAnti { right, .. } => right,
        _ => unreachable!(),
    };
    if let CompiledRelationStatement::BagUnion { branches, mappings } = &mut right.statement {
        branches.swap(0, 1);
        mappings.swap(0, 1);
    }
    assert!(decode_and_verify_envelope(
        &envelope,
        &reordered,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
fn composite_not_binds_swapped_shared_schema_and_requires_both_keys() {
    let layer = build_layer(&[
        ("a", "p", "b"),
        ("a", "p", "c"),
        ("d", "p", "e"),
        ("b", "q", "a"),
        ("z", "q", "a"),
        ("e", "q", "d"),
    ]);
    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            // Reversed variable order makes the mapping [(x,1),(y,0)].
            query: Box::new(tv("y", "q", "x")),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(
        compiled.relation.as_left_anti().unwrap().2,
        vec![(0, 1), (1, 0)]
    );
    let proved = compiled.prove(&layer).unwrap();
    assert_eq!(
        proved.result_len, 1,
        "a-p-c only partially matches the right keys"
    );
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let mut reordered = compiled.clone();
    reordered.relation.as_left_anti_mut().unwrap().swap(0, 1);
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &layer, layer.resolution_root(),)
            .is_err()
    );
}

#[test]
fn global_not_gates_complete_outer_relation_under_select_and_count() {
    let layer = build_layer(&[("a", "p", "b"), ("c", "p", "d"), ("u", "q", "v")]);
    let retained = Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(and_of(vec![
            tv("x", "p", "y"),
            Query::Not(Not {
                query: Box::new(tv_subj_const("a", "q", "b")),
            }),
        ])),
    });
    let retained_compiled = plan_bgp(&retained).unwrap();
    assert!(retained_compiled.relation.as_left_anti().unwrap().2
        .is_empty());
    let retained_proved = retained_compiled.prove(&layer).unwrap();
    assert_eq!(retained_proved.result_len, 2);
    assert_eq!(
        retained_proved
            .projected_columns(&retained_compiled)
            .count(),
        1
    );
    retained_proved
        .verify(&retained_compiled, &layer, layer.resolution_root())
        .unwrap();

    let suppressed = Query::Count(Count {
        query: Box::new(and_of(vec![
            tv("x", "p", "y"),
            Query::Not(Not {
                query: Box::new(tv("a", "q", "b")),
            }),
        ])),
        count: terminusdb_woql2::value::DataValue::Variable("count".into()),
    });
    let suppressed_compiled = plan_bgp(&suppressed).unwrap();
    let suppressed_proved = suppressed_compiled.prove(&layer).unwrap();
    assert_eq!(
        suppressed_proved.count_binding(&suppressed_compiled),
        Some(("count", 0))
    );
    suppressed_proved
        .verify(&suppressed_compiled, &layer, layer.resolution_root())
        .unwrap();
    assert!(suppressed_proved
        .verify_columns(&retained_proved.result_columns)
        .is_err());
    let envelope = suppressed_proved
        .encode_envelope(&suppressed_compiled, &layer, layer.resolution_root())
        .unwrap();
    decode_and_verify_envelope(
        &envelope,
        &suppressed_compiled,
        &layer,
        layer.resolution_root(),
    )
    .unwrap();
    let correlated_variant = plan_bgp(&and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("x", "q", "y")),
        }),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &correlated_variant,
        &layer,
        layer.resolution_root(),
    )
    .is_err());
}

#[test]
#[ignore = "delta head; Not/anti-join not in the reconciled/delta-read fragment (Wave 0 diet)"]
fn global_not_tracks_authenticated_nonemptiness_across_layers() {
    let store = open_sync_memory_store();
    let base_builder = store.create_base_layer().unwrap();
    base_builder
        .add_value_triple(ValueTriple::new_node(&iri("a"), &iri("p"), &iri("b")))
        .unwrap();
    base_builder
        .add_value_triple(ValueTriple::new_node(&iri("u"), &iri("q"), &iri("v")))
        .unwrap();
    let base = base_builder.commit().unwrap();
    let base_commitment = base.proof_commitment().unwrap();
    let child_builder = base.open_write().unwrap();
    child_builder
        .add_value_triple(ValueTriple::new_node(&iri("a"), &iri("q"), &iri("hit")))
        .unwrap();
    let child = child_builder.commit().unwrap();
    let child_commitment = child.proof_commitment().unwrap();

    let query = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv_subj_const("a", "q", "z")),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let base_proved = compiled.prove(&base_commitment).unwrap();
    assert_eq!(base_proved.result_len, 1);
    base_proved
        .verify(
            &compiled,
            &base_commitment,
            base_commitment.resolution_root(),
        )
        .unwrap();
    let child_proved = compiled.prove(&child_commitment).unwrap();
    assert_eq!(child_proved.result_len, 0);
    child_proved
        .verify(
            &compiled,
            &child_commitment,
            child_commitment.resolution_root(),
        )
        .unwrap();
    assert!(child_proved
        .verify(
            &compiled,
            &child_commitment,
            base_commitment.resolution_root(),
        )
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

    let (proof, result_coms, m, ids) = prove_layer_bgp(lc, compiled.bgp_plan()).unwrap();
    // The verifier sees ONLY the IRIs (inside the plan) + revealed ids + result
    // commitments + the external root — never reruns the BGP.
    verify_layer_bgp(
        lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
        &ids,
        &result_coms,
        m,
        &proof,
    )
    .unwrap();

    // Independent oracle: the TRUE natural join.
    let (exp_cols, domain) = expected_bgp(lc, &ids, &compiled.bgp_plan().joins);
    assert_eq!(
        result_coms.len(),
        compiled.relation.schema.len(),
        "one commitment per schema column"
    );
    assert_eq!(exp_cols.len(), compiled.relation.schema.len());
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
            compiled.relation.schema[c]
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
fn count_projected_distinct_proves_duplicate_classes_and_rejects_tamper() {
    let lc = build_layer(&[
        ("a", "p", "x"),
        ("a", "p", "y"),
        ("a", "p", "z"),
        ("b", "p", "x"),
        ("b", "p", "y"),
    ]);
    let q = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["subject".into()],
            query: Box::new(tv("subject", "p", "object")),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&q).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 2);
    assert_eq!(proved.count_binding(&compiled), Some(("count", 2)));
    proved.verify(&compiled, &lc, lc.resolution_root()).unwrap();

    let bytes = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, lc.resolution_root()).unwrap();

    let mut duplicate = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    duplicate.result_columns[0][1] = duplicate.result_columns[0][0];
    duplicate.result_values[0][1] = duplicate.result_values[0][0].clone();
    duplicate.result_commitments[0] = commit_column(&duplicate.result_columns[0]).unwrap();
    let duplicate_bytes = duplicate.encode().unwrap();
    assert!(decode_and_verify_envelope(
        &duplicate_bytes,
        &compiled,
        &lc,
        lc.resolution_root(),
    )
    .is_err());

    let substituted = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["predicate".into()],
            query: Box::new(tv_pred_var("subject", "predicate", "object")),
        })),
        count: dvar("count"),
    }))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &bytes,
        &substituted,
        &lc,
        lc.resolution_root(),
    )
    .is_err());
}

#[test]
fn count_projected_distinct_empty_input_proves_zero() {
    let lc = build_layer(&[("a", "other", "x"), ("b", "other", "y")]);
    let query = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["subject".into()],
            query: Box::new(tv("subject", "missing", "object")),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 0);
    assert_eq!(proved.count_binding(&compiled), Some(("count", 0)));
    proved.verify(&compiled, &lc, lc.resolution_root()).unwrap();
}

#[test]
fn visible_projected_distinct_proves_the_exact_set_and_rejects_tamper() {
    // A duplicate-producing base layer: subjects `a` (×3) and `b` (×2) each recur under predicate
    // `p`. One extra `q` edge (a distinct predicate, disjoint subject `c`) exists ONLY so the
    // anti-substitution control below can aim the `p` proof at a valid, present-in-the-layer `q`
    // plan — it never matches the `p` query, so the distinct-`?subject` result stays exactly {a, b}.
    let lc = build_layer(&[
        ("a", "p", "x"),
        ("a", "p", "y"),
        ("a", "p", "z"),
        ("b", "p", "x"),
        ("b", "p", "y"),
        ("c", "q", "z"),
    ]);
    // `SELECT DISTINCT ?subject WHERE { ?subject p ?object }` — a VISIBLE top-level projected
    // Distinct with NO enclosing Count, routed through the audited store distinct gadget (real
    // bag dedup), NOT the transparent full-row no-op.
    let q = Query::Distinct(Distinct {
        variables: vec!["subject".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["subject".into()],
            query: Box::new(tv("subject", "p", "object")),
        })),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert!(!compiled.full_row_distinct, "real gadget dedup, not the full-row no-op");
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Distinct { .. }
    ));
    assert_eq!(compiled.count_variable, None);
    assert_eq!(compiled.projection, Some(vec![0]));

    let proved = compiled.prove(&lc).unwrap();
    // The exact distinct set is {a, b}: two output tuples collapsing the five input rows.
    assert_eq!(proved.result_len, 2);
    proved.verify(&compiled, &lc, lc.resolution_root()).unwrap();

    let bytes = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, lc.resolution_root()).unwrap();

    // NEGATIVE 1 — a DUPLICATED output tuple: re-present {a, a}. Re-commit so the tamper is not
    // caught merely by a commitment mismatch; the distinct gadget's own uniqueness check rejects.
    let mut duplicated = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    duplicated.result_columns[0][1] = duplicated.result_columns[0][0];
    duplicated.result_values[0][1] = duplicated.result_values[0][0].clone();
    duplicated.result_commitments[0] = commit_column(&duplicated.result_columns[0]).unwrap();
    assert!(decode_and_verify_envelope(
        &duplicated.encode().unwrap(),
        &compiled,
        &lc,
        lc.resolution_root(),
    )
    .is_err());

    // NEGATIVE 2 — a DROPPED distinct value: present only {a}. The equality classes no longer
    // cover every input row, so the gadget rejects (class-coverage != input cardinality).
    let mut dropped = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    dropped.result_len = 1;
    dropped.result_log_domain = 1;
    for column in &mut dropped.result_columns {
        column.truncate(1);
        column.resize(2, ArkFr::zero());
    }
    for values in &mut dropped.result_values {
        values.truncate(1);
    }
    for (column, commitment) in dropped
        .result_columns
        .iter()
        .zip(&mut dropped.result_commitments)
    {
        *commitment = commit_column(column).unwrap();
    }
    assert!(decode_and_verify_envelope(
        &dropped.encode().unwrap(),
        &compiled,
        &lc,
        lc.resolution_root(),
    )
    .is_err());

    // ANTI-SUBSTITUTION — the `DISTINCT ?subject` proof must not verify as a DIFFERENT query's
    // plan. The whole query (predicate included) is folded into the digest, so re-aiming this
    // envelope at `DISTINCT ?subject WHERE { ?subject q ?object }` (predicate `q`, present in the
    // layer above) recomputes a different Fiat-Shamir transcript and the distinct proof rejects.
    //
    // Note we cannot use `DISTINCT ?object` as the control: a bare object is a mixed node/literal
    // `Value` position with no committed single-namespace domain, so it rejects at COMPILE time
    // (`DistinctUnsupportedDomain`) — see `visible_projected_distinct_over_mixed_domain_object_rejects`
    // in proof_plan.rs. Only a node-position variable (subject here) is a valid distinct key.
    let other = plan_bgp(&Query::Distinct(Distinct {
        variables: vec!["subject".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["subject".into()],
            query: Box::new(tv("subject", "q", "object")),
        })),
    }))
    .unwrap();
    assert!(
        decode_and_verify_envelope(&bytes, &other, &lc, lc.resolution_root()).is_err()
    );
}

#[test]
fn visible_projected_distinct_over_bag_union_proves_the_exact_set_and_rejects_tamper() {
    // GROUND TRUTH for the compiler relaxation: a VISIBLE top-level projected Distinct over a
    // bag-union (`Or`) inner really proves+verifies through the store, with no enclosing Count.
    //
    // The layer makes the union bag genuinely overlapping AND internally duplicated, so dedup is
    // non-trivial. `?subject` under predicate `p` yields the bag [a, a, b, c]; under predicate `q`
    // it yields [a, c, d]. The union `{?subject p ?o} OR {?subject q ?o}` is therefore the 7-row bag
    // a×3, b×1, c×2, d×1 — whose DISTINCT set is exactly {a, b, c, d} (a and c overlap both
    // branches). We size the distinct set to a power of two (4) so the committed result columns pad
    // cleanly for the re-commit tampers below. (The branches share the object variable `?o` so their
    // schemas unify; the distinct key is the node-position subject.)
    let lc = build_layer(&[
        ("a", "p", "x"),
        ("a", "p", "y"),
        ("b", "p", "x"),
        ("c", "p", "x"),
        ("a", "q", "z"),
        ("c", "q", "z"),
        ("d", "q", "z"),
    ]);
    let q = Query::Distinct(Distinct {
        variables: vec!["subject".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["subject".into()],
            query: Box::new(Query::Or(Or {
                or: vec![tv("subject", "p", "o"), tv("subject", "q", "o")],
            })),
        })),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert!(!compiled.full_row_distinct, "real gadget dedup, not the full-row no-op");
    let CompiledRelationStatement::Distinct { input, .. } = &compiled.relation.statement else {
        panic!("expected a Distinct statement, got {:?}", compiled.relation.statement);
    };
    assert!(
        input.as_bag_union().is_some(),
        "the distinct gadget must wrap the bag-union inner"
    );
    assert_eq!(compiled.count_variable, None);
    assert_eq!(compiled.projection, Some(vec![0]));

    let proved = compiled.prove(&lc).unwrap();
    // The exact distinct set is {a, b, c, d}: four output tuples collapsing the seven input rows.
    assert_eq!(proved.result_len, 4);
    proved.verify(&compiled, &lc, lc.resolution_root()).unwrap();

    // The resolved distinct subjects are exactly {a, b, c, d} (order-agnostic).
    let resolved = proved
        .verify_and_resolve(&compiled, &lc, lc.resolution_root())
        .unwrap();
    let got: std::collections::BTreeSet<CanonicalObject> = resolved[0].iter().cloned().collect();
    let want: std::collections::BTreeSet<CanonicalObject> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|n| CanonicalObject::Node(iri(n).into_bytes()))
        .collect();
    assert_eq!(got, want);

    let bytes = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, lc.resolution_root()).unwrap();

    // NEGATIVE 1 — a DUPLICATED output tuple: re-present {a, a, ...}. Re-commit so the tamper is not
    // caught merely by a commitment mismatch; the distinct gadget's own uniqueness check rejects.
    let mut duplicated = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    duplicated.result_columns[0][1] = duplicated.result_columns[0][0];
    duplicated.result_values[0][1] = duplicated.result_values[0][0].clone();
    duplicated.result_commitments[0] = commit_column(&duplicated.result_columns[0]).unwrap();
    assert!(decode_and_verify_envelope(
        &duplicated.encode().unwrap(),
        &compiled,
        &lc,
        lc.resolution_root(),
    )
    .is_err());

    // NEGATIVE 2 — a DROPPED distinct value: present only {a}. The equality classes no longer cover
    // every input row of the union bag, so the gadget rejects (class-coverage != input cardinality).
    let mut dropped = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    dropped.result_len = 1;
    dropped.result_log_domain = 1;
    for column in &mut dropped.result_columns {
        column.truncate(1);
        column.resize(2, ArkFr::zero());
    }
    for values in &mut dropped.result_values {
        values.truncate(1);
    }
    for (column, commitment) in dropped
        .result_columns
        .iter()
        .zip(&mut dropped.result_commitments)
    {
        *commitment = commit_column(column).unwrap();
    }
    assert!(decode_and_verify_envelope(
        &dropped.encode().unwrap(),
        &compiled,
        &lc,
        lc.resolution_root(),
    )
    .is_err());

    // ANTI-SUBSTITUTION — the bag-union `DISTINCT ?subject` proof must not verify as a DIFFERENT
    // valid query's plan. Re-aiming this envelope at the single-branch `DISTINCT ?subject WHERE {
    // ?subject p ?object }` (a valid Distinct-over-BGP, predicate `p` present in the layer) folds a
    // different query into the Fiat-Shamir transcript, so verification rejects.
    let other = plan_bgp(&Query::Distinct(Distinct {
        variables: vec!["subject".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["subject".into()],
            query: Box::new(tv("subject", "p", "object")),
        })),
    }))
    .unwrap();
    assert!(matches!(
        other.relation.statement,
        CompiledRelationStatement::Distinct { .. }
    ));
    assert!(
        decode_and_verify_envelope(&bytes, &other, &lc, lc.resolution_root()).is_err()
    );
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
    assert_eq!(compiled.relation.schema, vec!["x"]);
    assert!(compiled.bgp_plan().atoms[0].subject_object_equal);
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 2);
    proved
        .verify(&compiled, &lc, lc.resolution_root())
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
        .verify_and_resolve(&compiled, &lc, lc.resolution_root())
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
        .verify_objects(&compiled, &lc, lc.resolution_root(), &tampered_objects,)
        .is_err());

    let encoded = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    let decoded =
        decode_and_verify_envelope(&encoded, &compiled, &lc, lc.resolution_root()).unwrap();
    assert_eq!(decoded.proved.result_len, proved.result_len);
    assert_eq!(decoded.result_objects, objects);

    let mut corrupt = encoded.clone();
    corrupt[20] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &lc, lc.resolution_root(),).is_err()
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
            lc.resolution_root(),
            &["x".into()],
            &[vec![id("c")], vec![id("a")]],
        )
        .unwrap();
    // Sorting normalizes enumeration order, but it must not collapse duplicates.
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["x".into()],
            &[vec![id("a")], vec![id("a")]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["x".into()],
            &[vec![id("a")], vec![id("b")]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["wrong".into()],
            &[vec![id("a")], vec![id("c")]],
        )
        .is_err());

    let mut renamed_schema = compiled.clone();
    renamed_schema.relation.schema[0] = "not_x".into();
    assert!(
        decode_and_verify_envelope(&encoded, &renamed_schema, &lc, lc.resolution_root(),)
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
        .verify_and_resolve(&compiled, &lc, lc.resolution_root())
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
        .verify_objects(&compiled, &lc, lc.resolution_root(), &tampered)
        .is_err());
}

#[test]
fn data_variable_proves_mixed_literals_excludes_nodes_and_composes_with_join() {
    let lc = build_layer_triples(vec![
        ValueTriple::new_value(&iri("alice"), &iri("datum"), u32::make_entry(&42)),
        ValueTriple::new_value(&iri("bob"), &iri("datum"), bool::make_entry(&true)),
        ValueTriple::new_node(&iri("carol"), &iri("datum"), &iri("not-data")),
        ValueTriple::new_node(&iri("alice"), &iri("label"), &iri("alice-label")),
        ValueTriple::new_node(&iri("bob"), &iri("label"), &iri("bob-label")),
        ValueTriple::new_node(&iri("carol"), &iri("label"), &iri("carol-label")),
    ]);
    let query = and_of(vec![
        Query::Data(Data {
            subject: NodeValue::Variable("person".into()),
            predicate: NodeValue::Node(iri("datum")),
            object: DataValue::Variable("value".into()),
            graph: GraphType::Instance,
        }),
        tv("person", "label", "label"),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    assert_eq!(compiled.relation.schema, ["value", "person", "label"]);
    assert_eq!(
        compiled.bgp_plan().atoms[0].object_kind,
        ObjectKindFilter::Literal
    );

    let proved = compiled.prove(&lc).unwrap();
    let objects = proved
        .verify_and_resolve(&compiled, &lc, lc.resolution_root())
        .unwrap();
    assert_eq!(objects[0].len(), 2);
    assert!(objects[0]
        .iter()
        .all(|object| matches!(object, CanonicalObject::Value { .. })));
    assert!(objects[0].iter().any(|object| matches!(
        object,
        CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, .. }
    )));
    assert!(objects[0].iter().any(|object| matches!(
        object,
        CanonicalObject::Value { datatype: CanonicalDatatype::Boolean, .. }
    )));

    let mut wrong_kind = compiled.clone();
    let CompiledRelationStatement::Bgp(plan) = &mut wrong_kind.relation.statement else {
        unreachable!()
    };
    plan.atoms[0].object_kind = ObjectKindFilter::Node;
    assert!(proved
        .verify(&wrong_kind, &lc, lc.resolution_root())
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
        .verify(&compiled, &lc, lc.resolution_root())
        .unwrap();
    assert_eq!(proved.count_binding(&compiled), Some(("count", 2)));
    assert_eq!(proved.projected_commitments(&compiled).count(), 0);
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["count".into()],
            &[vec![2]],
        )
        .unwrap();
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["wrong".into()],
            &[vec![2]],
        )
        .is_err());
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
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
            .verify(&compiled, &lc, lc.resolution_root())
            .unwrap();
        assert_eq!(proved.result_len, expected_len);
        assert_eq!(proved.result_commitments.len(), 1); // hidden UNIT
        assert_eq!(proved.projected_commitments(&compiled).count(), 0);
        assert!(proved
            .verify_and_resolve(&compiled, &lc, lc.resolution_root())
            .unwrap()
            .is_empty());

        let rows = vec![Vec::new(); expected_len];
        proved
            .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &[], &rows)
            .unwrap();
        let wrong_rows = vec![Vec::new(); 1 - expected_len];
        assert!(proved
            .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &[], &wrong_rows)
            .is_err());

        let encoded = proved
            .encode_envelope(&compiled, &lc, lc.resolution_root())
            .unwrap();
        let decoded =
            decode_and_verify_envelope(&encoded, &compiled, &lc, lc.resolution_root()).unwrap();
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
                lc.resolution_root(),
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
            .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &[], &rows)
            .unwrap();
        let envelope = proved
            .encode_envelope(&compiled, &lc, lc.resolution_root())
            .unwrap();
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.resolution_root()).unwrap();
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
                lc.resolution_root(),
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
                lc.resolution_root(),
                &["s".into(), "o".into()],
                &swapped_rows,
            )
            .is_err());
        let envelope = proved
            .encode_envelope(&compiled, &lc, lc.resolution_root())
            .unwrap();
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.resolution_root()).unwrap();
        proved.result_log_domain += 1;
        assert!(proved
            .verify(&compiled, &lc, lc.resolution_root())
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
    assert!(compiled.bgp_plan().joins[0].is_cross_product());
    let proved = compiled.prove(&lc).unwrap();
    let variables = ["x".into(), "y".into(), "a".into(), "b".into()];
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
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
            lc.resolution_root(),
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
            lc.resolution_root(),
            &variables,
            &duplicated,
        )
        .is_err());

    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    let reordered = and_of(vec![tv("a", "q", "b"), tv("x", "p", "y")]);
    let reordered = plan_bgp(&reordered).unwrap();
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &lc, lc.resolution_root(),).is_err()
    );

    // A false authenticated ground gate suppresses the complete product without
    // changing its schema, while an empty relational component does the same through
    // the Cartesian join's pinned zero multiplicity.
    let absent = and_of(vec![product, tv_ground("a", "q", "b")]);
    let compiled = plan_bgp(&absent).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &variables, &[])
        .unwrap();

    let empty_component = and_of(vec![tv("x", "p", "y"), tv_subj_const("i", "p", "w")]);
    let compiled = plan_bgp(&empty_component).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
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
        .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &variables, &rows)
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
                lc.resolution_root(),
                &variables,
                &forged,
            )
            .is_err());
    }

    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    let decoded =
        decode_and_verify_envelope(&envelope, &compiled, &lc, lc.resolution_root()).unwrap();
    assert!(decoded.proved.as_bag_union().is_some());
    assert!(decoded.proved.as_single().is_none());
    let mut corrupt = envelope.clone();
    corrupt[20] ^= 1;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &lc, lc.resolution_root()).is_err()
    );

    let single = plan_bgp(&tv("s", "p", "o")).unwrap();
    let single_proved = single.prove(&lc).unwrap();
    assert!(single_proved.as_single().is_some());
    assert!(single_proved.as_bag_union().is_none());
    let single_envelope = single_proved
        .encode_envelope(&single, &lc, lc.resolution_root())
        .unwrap();
    assert!(decode_and_verify_envelope(&envelope, &single, &lc, lc.resolution_root()).is_err());
    assert!(
        decode_and_verify_envelope(&single_envelope, &compiled, &lc, lc.resolution_root(),)
            .is_err()
    );
    let reordered = plan_bgp(&Query::Or(Or {
        or: vec![tv("s", "q", "o"), tv("s", "p", "o")],
    }))
    .unwrap();
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &lc, lc.resolution_root(),).is_err()
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
            lc.resolution_root(),
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
            lc.resolution_root(),
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
        .verify_executed_rows(&compiled, &lc, lc.resolution_root(), &[], &[vec![]])
        .unwrap();
}

// ----------------------------------------------------------------------------
// Fixed-length property paths (`Query::Path`) lowered to BGP / bag union, proven
// end-to-end and checked against an independently computed bag of endpoint pairs.
// ----------------------------------------------------------------------------

fn path_predicate(predicate: &str) -> PathPattern {
    PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    })
}

fn path_inverse(predicate: &str) -> PathPattern {
    PathPattern::InversePredicate(InversePathPredicate {
        predicate: Some(iri(predicate)),
    })
}

fn path_query(subject: &str, pattern: PathPattern, object: &str) -> Query {
    Query::Path(Path {
        subject: Value::Variable(subject.into()),
        pattern,
        object: Value::Variable(object.into()),
        path: None,
    })
}

/// The committed object id of a node IRI in the layer's authenticated catalog.
fn node_object_id(lc: &LayerCommitment, name: &str) -> u64 {
    lc.node_value_catalog
        .iter()
        .find(|record| record.triple.object == CanonicalObject::Node(iri(name).into_bytes()))
        .unwrap()
        .object_id
}

/// `Distinct([endpoint], Path(<bound source>, star/plus(p), ?endpoint))`.
fn distinct_closure_query(source: &str, reflexive: bool, predicate: &str, endpoint: &str) -> Query {
    let inner = PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    });
    let pattern = if reflexive {
        PathPattern::Star(PathStar {
            star: Box::new(inner),
        })
    } else {
        PathPattern::Plus(PathPlus {
            plus: Box::new(inner),
        })
    };
    Query::Distinct(Distinct {
        variables: vec![endpoint.into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Node(iri(source)),
            pattern,
            object: Value::Variable(endpoint.into()),
            path: None,
        })),
    })
}

fn closure_resolved_set(objects: &[Vec<CanonicalObject>]) -> std::collections::BTreeSet<String> {
    objects[0]
        .iter()
        .map(|object| match object {
            CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone()).unwrap(),
            other => panic!("unexpected closure result object {other:?}"),
        })
        .collect()
}

fn assert_transitive_closure_e2e() {
    let lc = build_layer(&[("a", "p", "b"), ("b", "p", "c")]);
    let root = lc.resolution_root();
    let iri_set = |names: &[&str]| {
        names
            .iter()
            .map(|name| iri(name))
            .collect::<std::collections::BTreeSet<String>>()
    };

    // plus from a over a -> b -> c = {b, c}.
    let plus_query = distinct_closure_query("a", false, "p", "y");
    let plus_compiled = plan_bgp(&plus_query).unwrap();
    let plus_proved = plus_compiled.prove(&lc).unwrap();
    let plus_objects = plus_proved.verify_and_resolve(&plus_compiled, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&plus_objects), iri_set(&["b", "c"]));

    // Portable envelope round-trips through the store decode+verify path.
    let plus_bytes = plus_proved.encode_envelope(&plus_compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_compiled, &lc, root).unwrap();

    // star from a = {a, b, c} (adds the reflexive identity a).
    let star_query = distinct_closure_query("a", true, "p", "y");
    let star_compiled = plan_bgp(&star_query).unwrap();
    let star_proved = star_compiled.prove(&lc).unwrap();
    let star_objects = star_proved.verify_and_resolve(&star_compiled, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&star_objects), iri_set(&["a", "b", "c"]));

    // The K-independent digest still binds star vs plus: a plus proof must not verify as the star
    // query, in either the direct or the decoded-envelope path.
    assert!(plus_proved.verify(&star_compiled, &lc, root).is_err());
    assert!(decode_and_verify_envelope(&plus_bytes, &star_compiled, &lc, root).is_err());

    // Bound TARGET via inversion: `Distinct([x], Path(?x, plus(p), c))` = the set of nodes that reach
    // c over the inverse edge = {a, b}. Same edge relation, mirrored traversal.
    let inverse_query = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Variable("x".into()),
            pattern: PathPattern::Plus(PathPlus {
                plus: Box::new(PathPattern::Predicate(PathPredicate {
                    predicate: Some(iri("p")),
                })),
            }),
            object: Value::Node(iri("c")),
            path: None,
        })),
    });
    let inverse_compiled = plan_bgp(&inverse_query).unwrap();
    let inverse_proved = inverse_compiled.prove(&lc).unwrap();
    let inverse_objects = inverse_proved
        .verify_and_resolve(&inverse_compiled, &lc, root)
        .unwrap();
    assert_eq!(closure_resolved_set(&inverse_objects), iri_set(&["a", "b"]));
    // The forward plus (into ?y from a) and the inverse plus (into c from ?x) are distinct queries.
    assert!(inverse_proved.verify(&plus_compiled, &lc, root).is_err());
}

#[test]
fn transitive_closure_star_plus_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-transitive-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_transitive_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// `path(source, star|plus(p), object)` with BOTH endpoints bound node constants (P4) — a ground
/// boolean reachability query.
fn both_bound_closure_query(source: &str, reflexive: bool, predicate: &str, object: &str) -> Query {
    let inner = PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    });
    let pattern = if reflexive {
        PathPattern::Star(PathStar {
            star: Box::new(inner),
        })
    } else {
        PathPattern::Plus(PathPlus {
            plus: Box::new(inner),
        })
    };
    Query::Path(Path {
        subject: Value::Node(iri(source)),
        pattern,
        object: Value::Node(iri(object)),
        path: None,
    })
}

fn assert_both_bound_closure_e2e() {
    // Acyclic chain a -> b -> c and a cyclic variant a -> b -> c -> a.
    let acyclic = build_layer(&[("a", "p", "b"), ("b", "p", "c")]);
    let acyclic_root = acyclic.resolution_root();
    let cyclic = build_layer(&[("a", "p", "b"), ("b", "p", "c"), ("c", "p", "a")]);
    let cyclic_root = cyclic.resolution_root();

    // Prove + verify a both-bound reachability query, returning its ground-existence row count
    // (≥1 = reachable outcome, 0 = unreachable outcome).
    let reach_len = |lc: &Arc<LayerCommitment>, source: &str, reflexive: bool, object: &str| -> usize {
        let q = both_bound_closure_query(source, reflexive, "p", object);
        let compiled = plan_bgp(&q).unwrap();
        let proved = compiled.prove(lc).unwrap();
        proved
            .verify(&compiled, lc, lc.resolution_root())
            .unwrap();
        proved.result_len
    };

    // REACHABLE (`plus`): b and c are ≥1-hop reachable from a → one solution each.
    assert_eq!(reach_len(&acyclic, "a", false, "c"), 1, "c is reachable from a (a->b->c)");
    assert_eq!(reach_len(&acyclic, "a", false, "b"), 1, "b is reachable from a (a->b)");
    // NOT REACHABLE (`plus`): nothing points back to a in the acyclic chain → zero solutions.
    assert_eq!(reach_len(&acyclic, "c", false, "a"), 0, "a is NOT reachable from c");
    // STAR REFLEXIVE: `path(a, star(p), a)` succeeds via the identity branch even with NO self-loop.
    assert_eq!(reach_len(&acyclic, "a", true, "a"), 1, "a reaches itself reflexively under star");
    // PLUS is NOT reflexive without a cycle: `path(a, plus(p), a)` needs a ≥1-hop walk back to a,
    // which the acyclic chain has not → zero solutions (distinguishes plus from star reflexivity).
    assert_eq!(reach_len(&acyclic, "a", false, "a"), 0, "a is not on a cycle → plus(a,a) fails");

    // On the CYCLIC graph, a IS on a cycle (a->b->c->a), so `path(a, plus(p), a)` now succeeds, and
    // star still succeeds reflexively.
    assert_eq!(reach_len(&cyclic, "a", false, "a"), 1, "a is on a cycle → plus(a,a) holds");
    assert_eq!(reach_len(&cyclic, "a", true, "a"), 1, "star(a,a) holds on the cyclic graph too");

    // ---- Soundness: the delivered outcome cannot be cross-replayed or forged ----
    let reachable_q = both_bound_closure_query("a", false, "p", "c");
    let reachable_compiled = plan_bgp(&reachable_q).unwrap();
    let reachable_proved = reachable_compiled.prove(&acyclic).unwrap();
    reachable_proved
        .verify(&reachable_compiled, &acyclic, acyclic_root)
        .unwrap();
    assert_eq!(reachable_proved.result_len, 1);

    // (i) OPERATOR binding: a `plus` reachability proof must NOT verify as the `star` query digest.
    let star_compiled = plan_bgp(&both_bound_closure_query("a", true, "p", "c")).unwrap();
    assert!(
        reachable_proved.verify(&star_compiled, &acyclic, acyclic_root).is_err(),
        "a plus both-bound proof must not verify under the star query digest"
    );
    // (ii) TARGET-CONSTANT binding: the reachable-to-`c` proof must NOT verify as the reachable-to-`b`
    // query — the membership constant `b` is folded into the digest, so a prover cannot relabel the
    // target (this is what forbids claiming reachability of a DIFFERENT node).
    let other_target = plan_bgp(&both_bound_closure_query("a", false, "p", "b")).unwrap();
    assert!(
        reachable_proved.verify(&other_target, &acyclic, acyclic_root).is_err(),
        "a reachable-to-c proof must not verify as the reachable-to-b query"
    );
    // (iii) VARIANT binding: the both-bound boolean must NOT verify as the single-seed SET closure
    // `Distinct([y], path(a, plus(p), ?y))` (the membership gate + empty projection change the digest).
    let set_closure = plan_bgp(&distinct_closure_query("a", false, "p", "y")).unwrap();
    assert!(
        reachable_proved.verify(&set_closure, &acyclic, acyclic_root).is_err(),
        "the both-bound boolean must not verify as the single-seed set closure"
    );

    // (iv) TAMPERED RESULT: re-committing a forged reachable row does not bypass the root-authenticated
    // membership. The delivered result is bound to the Equality-over-A_K output; a spurious id fails.
    let envelope = reachable_proved
        .encode_envelope(&reachable_compiled, &acyclic, acyclic_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &reachable_compiled, &acyclic, acyclic_root).unwrap();
    let mut tampered = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(9_999_999);
    tampered.result_commitments[0] = commit_column(&pad(
        &tampered.result_columns[0],
        1usize << tampered.result_log_domain,
    ))
    .unwrap();
    assert!(
        decode_and_verify_envelope(
            &tampered.encode().unwrap(),
            &reachable_compiled,
            &acyclic,
            acyclic_root,
        )
        .is_err(),
        "a forged reachable row must not verify against the authenticated A_K membership"
    );

    // (v) WRONG ROOT: the reachable proof must not verify against a different layer's root (the cyclic
    // graph), binding the whole existence to the committed instance graph.
    assert!(
        reachable_proved.verify(&reachable_compiled, &cyclic, cyclic_root).is_err(),
        "the reachable proof must not verify against a different committed graph"
    );
}

#[test]
fn both_bound_closure_reachability_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-both-bound-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_both_bound_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

// ---- P3: COMPOUND-body single-seed closures `(p|q)+` / `(p/q)+` ----

fn closure_pred_e2e(predicate: &str) -> PathPattern {
    PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    })
}

/// `Distinct([endpoint], Path(Node(source), star|plus(inner), ?endpoint))` — a single-seed closure
/// over an arbitrary (compound) `inner` body.
fn distinct_compound_closure(source: &str, inner: PathPattern, reflexive: bool, endpoint: &str) -> Query {
    let pattern = if reflexive {
        PathPattern::Star(PathStar { star: Box::new(inner) })
    } else {
        PathPattern::Plus(PathPlus { plus: Box::new(inner) })
    };
    Query::Distinct(Distinct {
        variables: vec![endpoint.into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Node(iri(source)),
            pattern,
            object: Value::Variable(endpoint.into()),
            path: None,
        })),
    })
}

/// `(p|q)` alternation body.
fn alt_body() -> PathPattern {
    PathPattern::Or(PathOr {
        or: vec![closure_pred_e2e("p"), closure_pred_e2e("q")],
    })
}

/// `(p/q)` two-hop sequence body (distinct predicates → edge-simple-faithful).
fn seq_body() -> PathPattern {
    PathPattern::Sequence(PathSequence {
        sequence: vec![closure_pred_e2e("p"), closure_pred_e2e("q")],
    })
}

fn assert_compound_alternation_closure_e2e() {
    // Graph exercising BOTH predicates: a -p-> b -q-> c -p-> d, plus a -q-> e (a q-branch off a).
    // The derived union edge E' = {(a,b), (b,c), (c,d), (a,e)}.
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("b", "q", "c"),
        ("c", "p", "d"),
        ("a", "q", "e"),
    ]);
    let root = lc.resolution_root();
    let iri_set = |names: &[&str]| {
        names.iter().map(|name| iri(name)).collect::<std::collections::BTreeSet<String>>()
    };

    // plus((p|q)) from a = {b, c, d, e} (a->b->c->d alternating, plus a->e).
    let plus_q = distinct_compound_closure("a", alt_body(), false, "y");
    let plus_c = plan_bgp(&plus_q).unwrap();
    let plus_p = plus_c.prove(&lc).unwrap();
    let plus_obj = plus_p.verify_and_resolve(&plus_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&plus_obj), iri_set(&["b", "c", "d", "e"]));

    // Portable envelope round-trips through the store decode+verify path.
    let plus_bytes = plus_p.encode_envelope(&plus_c, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_c, &lc, root).unwrap();

    // star((p|q)) from a = {a, b, c, d, e} (adds the reflexive identity a).
    let star_q = distinct_compound_closure("a", alt_body(), true, "y");
    let star_c = plan_bgp(&star_q).unwrap();
    let star_p = star_c.prove(&lc).unwrap();
    let star_obj = star_p.verify_and_resolve(&star_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&star_obj), iri_set(&["a", "b", "c", "d", "e"]));

    // Bound TARGET via inversion: nodes reaching d over (p|q)+ = {a, b, c} (a->b->c->d).
    let inv_q = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Variable("x".into()),
            pattern: PathPattern::Plus(PathPlus { plus: Box::new(alt_body()) }),
            object: Value::Node(iri("d")),
            path: None,
        })),
    });
    let inv_c = plan_bgp(&inv_q).unwrap();
    let inv_p = inv_c.prove(&lc).unwrap();
    let inv_obj = inv_p.verify_and_resolve(&inv_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&inv_obj), iri_set(&["a", "b", "c"]));

    // ---- Soundness / non-interchangeability ----
    // (i) OPERATOR: a plus proof must NOT verify as the star query digest (and vice versa).
    assert!(plus_p.verify(&star_c, &lc, root).is_err());
    assert!(decode_and_verify_envelope(&plus_bytes, &star_c, &lc, root).is_err());
    // (ii) EDGE STRUCTURE: the compound `(p|q)+` proof must NOT verify as the SEQUENCE `(p/q)+` query
    // (different derived edge → different digest), nor as the single-predicate `p+` closure.
    let seq_c = plan_bgp(&distinct_compound_closure("a", seq_body(), false, "y")).unwrap();
    assert!(plus_p.verify(&seq_c, &lc, root).is_err(), "(p|q)+ must not verify as (p/q)+");
    let single_p_c = plan_bgp(&distinct_closure_query("a", false, "p", "y")).unwrap();
    assert!(plus_p.verify(&single_p_c, &lc, root).is_err(), "(p|q)+ must not verify as p+");
    // (iii) TAMPERED RESULT: injecting a forged reachable node into the delivered column fails against
    // the root-authenticated accumulator A_K.
    let mut tampered = RecursiveRelationEnvelope::decode(&plus_bytes).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(9_999_999);
    tampered.result_commitments[0] =
        commit_column(&pad(&tampered.result_columns[0], 1usize << tampered.result_log_domain)).unwrap();
    assert!(
        decode_and_verify_envelope(&tampered.encode().unwrap(), &plus_c, &lc, root).is_err(),
        "a forged reachable node must not verify against the authenticated A_K"
    );
}

fn assert_compound_sequence_closure_e2e() {
    // Chain of TWO compound hops: a =(p/q)=> b =(p/q)=> c, via intermediates x1, x2.
    //   a -p-> x1 -q-> b  and  b -p-> x2 -q-> c.
    // The derived sequence edge E' = p∘q = {(a,b), (b,c)} — NOT the intermediates x1/x2.
    let lc = build_layer(&[
        ("a", "p", "x1"),
        ("x1", "q", "b"),
        ("b", "p", "x2"),
        ("x2", "q", "c"),
    ]);
    let root = lc.resolution_root();
    let iri_set = |names: &[&str]| {
        names.iter().map(|name| iri(name)).collect::<std::collections::BTreeSet<String>>()
    };

    // plus((p/q)) from a = {b, c}. Crucially EXCLUDES the single-hop intermediates x1, x2 (they are not
    // reachable by a full (p/q) compound hop) — this is what proves the sequence edge requires BOTH hops.
    let plus_q = distinct_compound_closure("a", seq_body(), false, "y");
    let plus_c = plan_bgp(&plus_q).unwrap();
    let plus_p = plus_c.prove(&lc).unwrap();
    let plus_obj = plus_p.verify_and_resolve(&plus_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&plus_obj), iri_set(&["b", "c"]));

    let plus_bytes = plus_p.encode_envelope(&plus_c, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_c, &lc, root).unwrap();

    // star((p/q)) from a = {a, b, c} (reflexive identity a added).
    let star_q = distinct_compound_closure("a", seq_body(), true, "y");
    let star_c = plan_bgp(&star_q).unwrap();
    let star_p = star_c.prove(&lc).unwrap();
    let star_obj = star_p.verify_and_resolve(&star_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&star_obj), iri_set(&["a", "b", "c"]));

    // ---- Soundness / non-interchangeability ----
    // (i) OPERATOR: plus vs star.
    assert!(plus_p.verify(&star_c, &lc, root).is_err());
    // (ii) EDGE STRUCTURE: `(p/q)+` must NOT verify as the reversed `(q/p)+` query (different join
    // order → different derived edge → different digest), nor as the alternation `(p|q)+`.
    let rev_seq = PathPattern::Sequence(PathSequence {
        sequence: vec![closure_pred_e2e("q"), closure_pred_e2e("p")],
    });
    let rev_c = plan_bgp(&distinct_compound_closure("a", rev_seq, false, "y")).unwrap();
    assert!(plus_p.verify(&rev_c, &lc, root).is_err(), "(p/q)+ must not verify as (q/p)+");
    let alt_c = plan_bgp(&distinct_compound_closure("a", alt_body(), false, "y")).unwrap();
    assert!(plus_p.verify(&alt_c, &lc, root).is_err(), "(p/q)+ must not verify as (p|q)+");
    // (iii) TAMPERED RESULT: injecting the excluded intermediate x1's id must fail against A_K.
    let mut tampered = RecursiveRelationEnvelope::decode(&plus_bytes).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(7_777_777);
    tampered.result_commitments[0] =
        commit_column(&pad(&tampered.result_columns[0], 1usize << tampered.result_log_domain)).unwrap();
    assert!(
        decode_and_verify_envelope(&tampered.encode().unwrap(), &plus_c, &lc, root).is_err(),
        "a forged reachable node must not verify against the authenticated A_K"
    );
    // (iv) WRONG ROOT: the proof must not verify against a different committed graph.
    let other = build_layer(&[("a", "p", "x1"), ("x1", "q", "b")]);
    assert!(
        plus_p.verify(&plus_c, &other, other.resolution_root()).is_err(),
        "the compound closure proof must not verify against a different committed graph"
    );
}

#[test]
fn compound_alternation_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-compound-alt-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_compound_alternation_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn compound_sequence_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-compound-seq-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_compound_sequence_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

// ---- P5b: single-hop wildcard `.` and single-seed wildcard `star|plus(.)` closure ----

/// A bare single-hop wildcard `.` (any predicate) predicate pattern.
fn wildcard_pred() -> PathPattern {
    PathPattern::Predicate(PathPredicate { predicate: None })
}

/// P5b single-hop: `path(a, ., ?y)` = the direct objects of `a` regardless of predicate, delivered as a
/// BAG (one row per stored triple). Proves the full-triple projection over the root-bound predicate
/// column, and that a prover cannot RESTRICT the predicate range to drop (or a different one to inject) a
/// real edge.
#[test]
fn single_hop_wildcard_proves_all_direct_objects_regardless_of_predicate() {
    // MIXED predicates: a reaches b via TWO predicates (knows, likes) and c via a third (owns); the
    // unrelated `x -knows-> z` edge is not a direct object of a.
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("a", "likes", "b"),
        ("a", "owns", "c"),
        ("x", "knows", "z"),
    ]);
    let id = |name: &str| node_object_id(&lc, name);
    let root = lc.resolution_root();

    let q = Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: wildcard_pred(),
        object: Value::Variable("y".into()),
        path: None,
    });
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["y"]);
    let proved = compiled.prove(&lc).unwrap();

    // The exact BAG of direct objects: b appears TWICE (knows + likes), c once. This is path.pl's bag
    // multiplicity for a single wildcard hop (one solution per stored triple).
    proved
        .verify_executed_rows(&compiled, &lc, root, &["y".into()], &[vec![id("b")], vec![id("b")], vec![id("c")]])
        .unwrap();

    // SOUNDNESS — a prover cannot RESTRICT the predicate range to drop a real edge: claiming only the
    // `knows`/`owns` objects (dropping the `likes` copy of b) fails; the full-triple relation is
    // root-bound, so every predicate's edge is delivered with its bag multiplicity.
    assert!(proved
        .verify_executed_rows(&compiled, &lc, root, &["y".into()], &[vec![id("b")], vec![id("c")]])
        .is_err());
    // SOUNDNESS — a prover cannot INJECT an object of a DIFFERENT subject (`z` is a direct object of x,
    // not a): a forged extra row fails.
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            root,
            &["y".into()],
            &[vec![id("b")], vec![id("b")], vec![id("c")], vec![id("z")]],
        )
        .is_err());

    // NON-INTERCHANGEABILITY — the wildcard single hop must NOT verify as a CONCRETE single-predicate path
    // (`path(a, knows, ?y)` = {b}): the free predicate column vs a resolved `knows` id gives a different
    // query digest, so a wildcard proof cannot be replayed as a concrete-predicate one.
    let concrete = plan_bgp(&Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: path_predicate("knows"),
        object: Value::Variable("y".into()),
        path: None,
    }))
    .unwrap();
    assert!(
        proved
            .verify_executed_rows(&concrete, &lc, root, &["y".into()], &[vec![id("b")], vec![id("b")], vec![id("c")]])
            .is_err(),
        "a wildcard single-hop proof must not verify under the concrete `knows` path query"
    );
}

/// P5b single-hop, both-free: `path(?x, ., ?y)` = the exact BAG `{(s,o) : ∃p (s,p,o)}` over ALL stored
/// triples regardless of predicate — the full instance edge relation projected to its endpoints.
#[test]
fn single_hop_wildcard_both_free_proves_full_edge_relation() {
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("a", "likes", "b"),
        ("c", "owns", "d"),
    ]);
    let id = |name: &str| node_object_id(&lc, name);
    let root = lc.resolution_root();

    let compiled = plan_bgp(&path_query("s", wildcard_pred(), "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    let proved = compiled.prove(&lc).unwrap();
    // (a,b) appears twice (two predicates), (c,d) once — the full instance triple bag, projected.
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            root,
            &["s".into(), "o".into()],
            &[vec![id("a"), id("b")], vec![id("a"), id("b")], vec![id("c"), id("d")]],
        )
        .unwrap();
}

/// P5b single-seed closure: `Distinct([y], path(a, plus|star(.), ?y))` reaches ACROSS DIFFERENT
/// predicates and delivers the EXACT reachable set. Proves the wildcard-edge compound closure end to end
/// + the result-binding soundness negatives (tampered A_K row; a prover cannot RESTRICT the predicate
/// range to a concrete-predicate closure; plus ≠ star; wrong root). Kept lean (2 proves over a 2-hop
/// graph) because debug-mode closure proving is expensive; bound-TARGET inversion is covered by the
/// plan test `single_seed_wildcard_closure_bound_target_is_inverted` + the compound-closure e2e.
fn assert_single_seed_wildcard_closure_e2e() {
    // Predicate changes at each hop: a -p1-> b -p2-> c, plus a q-branch a -q-> d. The wildcard closure
    // crosses ALL predicates; a concrete `p1+` closure would stop at b.
    let lc = build_layer(&[
        ("a", "p1", "b"),
        ("b", "p2", "c"),
        ("a", "q", "d"),
    ]);
    let root = lc.resolution_root();
    let iri_set = |names: &[&str]| {
        names.iter().map(|name| iri(name)).collect::<std::collections::BTreeSet<String>>()
    };

    // plus(.) from a = {b, c, d} — reached across p1 THEN p2 (crossing predicates), plus the q-branch.
    let plus_q = distinct_compound_closure("a", wildcard_pred(), false, "y");
    let plus_c = plan_bgp(&plus_q).unwrap();
    let plus_p = plus_c.prove(&lc).unwrap();
    let plus_obj = plus_p.verify_and_resolve(&plus_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&plus_obj), iri_set(&["b", "c", "d"]));

    // Portable envelope round-trips through the store decode+verify path.
    let plus_bytes = plus_p.encode_envelope(&plus_c, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_c, &lc, root).unwrap();

    // star(.) from a = {a, b, c, d} (adds the reflexive identity a).
    let star_q = distinct_compound_closure("a", wildcard_pred(), true, "y");
    let star_c = plan_bgp(&star_q).unwrap();
    let star_p = star_c.prove(&lc).unwrap();
    let star_obj = star_p.verify_and_resolve(&star_c, &lc, root).unwrap();
    assert_eq!(closure_resolved_set(&star_obj), iri_set(&["a", "b", "c", "d"]));

    // ---- Soundness / non-interchangeability ----
    // (i) OPERATOR: a plus proof must NOT verify as the star query digest.
    assert!(plus_p.verify(&star_c, &lc, root).is_err());
    assert!(decode_and_verify_envelope(&plus_bytes, &star_c, &lc, root).is_err());
    // (ii) EDGE STRUCTURE: the wildcard closure must NOT verify as a CONCRETE `p1+` closure (the digest
    // binds the wildcard edge — a predicate-variable atom vs a resolved `p1` id). This is what forbids a
    // prover RESTRICTING the predicate range to a single predicate.
    let concrete_c = plan_bgp(&distinct_closure_query("a", false, "p1", "y")).unwrap();
    assert!(
        plus_p.verify(&concrete_c, &lc, root).is_err(),
        "a wildcard `plus(.)` proof must not verify as the concrete `plus(p1)` closure"
    );
    // (iii) TAMPERED RESULT: injecting a forged reachable node into the delivered column fails against the
    // root-authenticated accumulator A_K (a prover cannot add a node not reachable over any predicate).
    let mut tampered = RecursiveRelationEnvelope::decode(&plus_bytes).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(9_999_999);
    tampered.result_commitments[0] =
        commit_column(&pad(&tampered.result_columns[0], 1usize << tampered.result_log_domain)).unwrap();
    assert!(
        decode_and_verify_envelope(&tampered.encode().unwrap(), &plus_c, &lc, root).is_err(),
        "a forged reachable node must not verify against the authenticated A_K"
    );
    // (iv) WRONG ROOT: the proof must not verify against a different committed graph.
    let other = build_layer(&[("a", "p1", "b")]);
    assert!(
        plus_p.verify(&plus_c, &other, other.resolution_root()).is_err(),
        "the wildcard closure proof must not verify against a different committed graph"
    );
}

#[test]
fn single_seed_wildcard_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-wildcard-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_single_seed_wildcard_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// `Distinct([subject, object], Path(?subject, star|plus(p), ?object))` — the all-pairs closure gate.
fn all_pairs_closure_query(subject: &str, reflexive: bool, predicate: &str, object: &str) -> Query {
    let inner = PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    });
    let pattern = if reflexive {
        PathPattern::Star(PathStar {
            star: Box::new(inner),
        })
    } else {
        PathPattern::Plus(PathPlus {
            plus: Box::new(inner),
        })
    };
    Query::Distinct(Distinct {
        variables: vec![subject.into(), object.into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Variable(subject.into()),
            pattern,
            object: Value::Variable(object.into()),
            path: None,
        })),
    })
}

fn closure_resolved_pairs(
    objects: &[Vec<CanonicalObject>],
) -> std::collections::BTreeSet<(String, String)> {
    let node = |o: &CanonicalObject| match o {
        CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("unexpected closure result object {other:?}"),
    };
    objects[0]
        .iter()
        .zip(&objects[1])
        .map(|(x, y)| (node(x), node(y)))
        .collect()
}

/// P2: all-pairs `plus` (`Distinct([x,y], path(?x, plus(p), ?y))`) proves the exact 2-column
/// reachability relation over a graph WITH A CYCLE, and rejects every result tamper; all-pairs `star`
/// reject-closes at compile (its reflexive diagonal is not faithfully reconstructible).
fn assert_all_pairs_closure_e2e() {
    // 3-cycle a -> b -> c -> a plus a feeder d -> a (d lies on no cycle). The closure TERMINATES.
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("b", "p", "c"),
        ("c", "p", "a"),
        ("d", "p", "a"),
    ]);
    let root = lc.resolution_root();

    let plus_query = all_pairs_closure_query("x", false, "p", "y");
    let plus_compiled = plan_bgp(&plus_query).unwrap();
    let plus_proved = plus_compiled.prove(&lc).unwrap();
    let plus_objects = plus_proved.verify_and_resolve(&plus_compiled, &lc, root).unwrap();
    // a, b, c each reach every cycle node incl. themselves; d reaches {a, b, c} but not itself. 12 pairs.
    let expected: std::collections::BTreeSet<(String, String)> = [
        ("a", "a"), ("a", "b"), ("a", "c"),
        ("b", "a"), ("b", "b"), ("b", "c"),
        ("c", "a"), ("c", "b"), ("c", "c"),
        ("d", "a"), ("d", "b"), ("d", "c"),
    ]
    .iter()
    .map(|(x, y)| (iri(x), iri(y)))
    .collect();
    assert_eq!(closure_resolved_pairs(&plus_objects), expected);

    // Portable envelope round-trips through the store decode+verify path.
    let plus_bytes = plus_proved.encode_envelope(&plus_compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_compiled, &lc, root).unwrap();

    // All-pairs STAR reject-closes at COMPILE time (the reflexive diagonal is not faithfully
    // reconstructible — path.pl's both-free identity is a single unbound row, not a domain diagonal).
    let star_query = all_pairs_closure_query("x", true, "p", "y");
    assert!(plan_bgp(&star_query).is_err());

    // ---- Result-binding soundness negatives (both columns authenticated) ----

    // (i) A forged result_len fails verify (the closure binds A_K's AUTHENTICATED pair count).
    let mut forged_len = plus_compiled.prove(&lc).unwrap();
    forged_len.result_len += 1;
    assert!(forged_len.verify(&plus_compiled, &lc, root).is_err());

    // (ii) DROP a reachable pair from the delivered result -> no longer equals the proven pairs set.
    let mut dropped = plus_compiled.prove(&lc).unwrap();
    dropped.result_columns[0].pop();
    dropped.result_columns[1].pop();
    dropped.result_len -= 1;
    assert!(dropped.verify(&plus_compiled, &lc, root).is_err());

    // (iii) INJECT a spurious pair (duplicate the first row over BOTH columns) -> reject.
    let mut injected = plus_compiled.prove(&lc).unwrap();
    let s0 = injected.result_columns[0][0];
    let o0 = injected.result_columns[1][0];
    injected.result_columns[0].push(s0);
    injected.result_columns[1].push(o0);
    injected.result_len += 1;
    assert!(injected.verify(&plus_compiled, &lc, root).is_err());
}

#[test]
fn all_pairs_transitive_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-all-pairs-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_all_pairs_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// `Count(n, Distinct([x,y], path(?x, plus(p), ?y)))` — count over the all-pairs closure.
fn count_all_pairs_query(
    subject: &str,
    reflexive: bool,
    predicate: &str,
    object: &str,
    count: &str,
) -> Query {
    Query::Count(Count {
        query: Box::new(all_pairs_closure_query(subject, reflexive, predicate, object)),
        count: DataValue::Variable(count.into()),
    })
}

/// P2c: count-over-all-pairs (`Count(n, Distinct([x,y], path(?x, plus(p), ?y)))`) proves
/// `n = |pairs relation|` (the number of distinct connected pairs) over the SAME 12-pair cyclic graph
/// as [`assert_all_pairs_closure_e2e`]. The count is delivered over the pairs closure's OWN
/// authenticated `result_len` — the number of ROWS (pairs), NOT columns — exactly as P1 delivers |A_K|
/// for the single-seed set; the count "just works" over a 2-column relation because it counts rows.
fn assert_all_pairs_closure_count_e2e() {
    // 3-cycle a -> b -> c -> a plus a feeder d -> a (d lies on no cycle). The closure TERMINATES;
    // the reachability relation has exactly 12 pairs (see `assert_all_pairs_closure_e2e`).
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("b", "p", "c"),
        ("c", "p", "a"),
        ("d", "p", "a"),
    ]);
    let root = lc.resolution_root();

    // plus over both-free endpoints = the 12-pair reachability relation => n = 12. The pairs SET is
    // hidden; the single delivered binding is the count.
    let query = count_all_pairs_query("x", false, "p", "y", "n");
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.count_binding(&compiled), Some(("n", 12)));
    proved
        .verify_executed_rows(&compiled, &lc, root, &["n".into()], &[vec![12]])
        .unwrap();
    // An off-by-one / tampered count value fails to verify (mirrors CountProof rejecting m_J +/- 1).
    assert!(proved
        .verify_executed_rows(&compiled, &lc, root, &["n".into()], &[vec![11]])
        .is_err());
    assert!(proved
        .verify_executed_rows(&compiled, &lc, root, &["n".into()], &[vec![13]])
        .is_err());

    // The portable envelope round-trips (the count is bound by the pairs closure proof + query digest).
    let bytes = proved.encode_envelope(&compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, root).unwrap();

    // A prover-side forged `result_len` fails verify: the count binds the pairs closure's AUTHENTICATED
    // accumulator length (the accumulate-and-dedupe rounds + the empty-delta fixpoint), not a free
    // scalar — a forged pair count / off-by-one `n` cannot pass.
    let mut forged = compiled.prove(&lc).unwrap();
    forged.result_len += 1;
    assert!(forged.verify(&compiled, &lc, root).is_err());

    // NON-INTERCHANGEABILITY: a count-over-all-pairs proof must NOT verify under the plain (un-counted)
    // all-pairs closure plan — the count binding + the zero-column projection are folded into the query
    // digest — in either the direct or the decoded-envelope path.
    let uncounted = plan_bgp(&all_pairs_closure_query("x", false, "p", "y")).unwrap();
    assert!(proved.verify(&uncounted, &lc, root).is_err());
    assert!(decode_and_verify_envelope(&bytes, &uncounted, &lc, root).is_err());
}

#[test]
fn all_pairs_transitive_closure_count_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-all-pairs-closure-count-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_all_pairs_closure_count_e2e)
        .unwrap()
        .join()
        .unwrap();
}

// ---- W1: all-pairs closure over a DERIVED edge (wildcard `.` / compound `(p|q)+`, `(p/q)+`) ----

/// `Distinct([x,y], Path(?x, star|plus(inner), ?y))` — the all-pairs closure gate over an arbitrary body.
fn all_pairs_compound_query(x: &str, reflexive: bool, inner: PathPattern, y: &str) -> Query {
    let pattern = if reflexive {
        PathPattern::Star(PathStar { star: Box::new(inner) })
    } else {
        PathPattern::Plus(PathPlus { plus: Box::new(inner) })
    };
    Query::Distinct(Distinct {
        variables: vec![x.into(), y.into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Variable(x.into()),
            pattern,
            object: Value::Variable(y.into()),
            path: None,
        })),
    })
}

/// W1: all-pairs wildcard `plus(.)` (`Distinct([x,y], path(?x, plus(.), ?y))`) proves the EXACT 2-column
/// reachability relation over a graph WITH A CYCLE across DIFFERENT predicates (the wildcard edge crosses
/// every predicate), and rejects every result tamper + a predicate-range restriction. All-pairs wildcard
/// `star` reject-closes at compile.
fn assert_all_pairs_wildcard_closure_e2e() {
    // 3-cycle across MIXED predicates a -p-> b -q-> c -r-> a, plus a feeder d -s-> a (d lies on no cycle).
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("b", "q", "c"),
        ("c", "r", "a"),
        ("d", "s", "a"),
    ]);
    let root = lc.resolution_root();

    let plus_compiled = plan_bgp(&all_pairs_compound_query("x", false, wildcard_pred(), "y")).unwrap();
    let plus_proved = plus_compiled.prove(&lc).unwrap();
    let plus_objects = plus_proved.verify_and_resolve(&plus_compiled, &lc, root).unwrap();
    // Same reachability topology as the single-predicate all-pairs test, but reached ACROSS predicates:
    // a,b,c each reach every cycle node incl. themselves; d reaches {a,b,c} but not itself. 12 pairs.
    let expected: std::collections::BTreeSet<(String, String)> = [
        ("a", "a"), ("a", "b"), ("a", "c"),
        ("b", "a"), ("b", "b"), ("b", "c"),
        ("c", "a"), ("c", "b"), ("c", "c"),
        ("d", "a"), ("d", "b"), ("d", "c"),
    ]
    .iter()
    .map(|(x, y)| (iri(x), iri(y)))
    .collect();
    assert_eq!(closure_resolved_pairs(&plus_objects), expected);

    // Portable envelope round-trips through the store decode+verify path.
    let plus_bytes = plus_proved.encode_envelope(&plus_compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_compiled, &lc, root).unwrap();

    // All-pairs wildcard STAR reject-closes at COMPILE (the reflexive diagonal is not reconstructible).
    assert!(plan_bgp(&all_pairs_compound_query("x", true, wildcard_pred(), "y")).is_err());

    // ---- Result-binding soundness negatives (both columns authenticated) ----
    // (i) A forged result_len fails (the closure binds A_K's AUTHENTICATED pair count).
    let mut forged_len = plus_compiled.prove(&lc).unwrap();
    forged_len.result_len += 1;
    assert!(forged_len.verify(&plus_compiled, &lc, root).is_err());
    // (ii) DROP a reachable pair from the delivered result.
    let mut dropped = plus_compiled.prove(&lc).unwrap();
    dropped.result_columns[0].pop();
    dropped.result_columns[1].pop();
    dropped.result_len -= 1;
    assert!(dropped.verify(&plus_compiled, &lc, root).is_err());
    // (iii) INJECT a spurious pair (duplicate the first row over BOTH columns).
    let mut injected = plus_compiled.prove(&lc).unwrap();
    let s0 = injected.result_columns[0][0];
    let o0 = injected.result_columns[1][0];
    injected.result_columns[0].push(s0);
    injected.result_columns[1].push(o0);
    injected.result_len += 1;
    assert!(injected.verify(&plus_compiled, &lc, root).is_err());
    // (iv) PREDICATE-RANGE RESTRICTION: the wildcard proof must NOT verify as a CONCRETE single-predicate
    // all-pairs closure (`plus(p)` — a raw `TransitiveClosurePairs` with a DIFFERENT digest tag). A prover
    // cannot restrict the wildcard's any-predicate edge to one predicate.
    let concrete_c = plan_bgp(&all_pairs_closure_query("x", false, "p", "y")).unwrap();
    assert!(
        plus_proved.verify(&concrete_c, &lc, root).is_err(),
        "a wildcard all-pairs proof must not verify as the concrete `plus(p)` all-pairs closure"
    );
    // (v) WRONG ROOT.
    let other = build_layer(&[("a", "p", "b")]);
    assert!(plus_proved
        .verify(&plus_compiled, &other, other.resolution_root())
        .is_err());
}

#[test]
fn all_pairs_wildcard_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-all-pairs-wildcard-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_all_pairs_wildcard_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// W1: all-pairs `(p|q)+` proves the EXACT 2-column reachability relation over a cyclic graph exercising
/// BOTH predicates, and is non-interchangeable with the sequence / single-predicate / wildcard closures.
fn assert_all_pairs_alternation_closure_e2e() {
    // 3-cycle alternating predicates a -p-> b -q-> c -p-> a, plus a feeder d -q-> a.
    let lc = build_layer(&[
        ("a", "p", "b"),
        ("b", "q", "c"),
        ("c", "p", "a"),
        ("d", "q", "a"),
    ]);
    let root = lc.resolution_root();

    let plus_c = plan_bgp(&all_pairs_compound_query("x", false, alt_body(), "y")).unwrap();
    let plus_p = plus_c.prove(&lc).unwrap();
    let plus_obj = plus_p.verify_and_resolve(&plus_c, &lc, root).unwrap();
    // E' = union of p-edges {(a,b),(c,a)} and q-edges {(b,c),(d,a)} — the same topology as the raw
    // all-pairs test, so 12 reachable pairs.
    let expected: std::collections::BTreeSet<(String, String)> = [
        ("a", "a"), ("a", "b"), ("a", "c"),
        ("b", "a"), ("b", "b"), ("b", "c"),
        ("c", "a"), ("c", "b"), ("c", "c"),
        ("d", "a"), ("d", "b"), ("d", "c"),
    ]
    .iter()
    .map(|(x, y)| (iri(x), iri(y)))
    .collect();
    assert_eq!(closure_resolved_pairs(&plus_obj), expected);

    let plus_bytes = plus_p.encode_envelope(&plus_c, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_c, &lc, root).unwrap();

    // All-pairs alternation STAR reject-closes at compile.
    assert!(plan_bgp(&all_pairs_compound_query("x", true, alt_body(), "y")).is_err());

    // NON-INTERCHANGEABILITY: `(p|q)+` all-pairs must NOT verify as `(p/q)+`, the wildcard, nor single-pred
    // `p+` (each is a different derived edge / tag → different query digest). Note the wildcard edge over
    // THIS graph yields the SAME pair set, yet the digest binding still rejects it.
    let seq_c = plan_bgp(&all_pairs_compound_query("x", false, seq_body(), "y")).unwrap();
    assert!(plus_p.verify(&seq_c, &lc, root).is_err(), "(p|q)+ must not verify as (p/q)+");
    let wild_c = plan_bgp(&all_pairs_compound_query("x", false, wildcard_pred(), "y")).unwrap();
    assert!(plus_p.verify(&wild_c, &lc, root).is_err(), "(p|q)+ must not verify as wildcard");
    let single_c = plan_bgp(&all_pairs_closure_query("x", false, "p", "y")).unwrap();
    assert!(plus_p.verify(&single_c, &lc, root).is_err(), "(p|q)+ must not verify as p+");

    // TAMPERED RESULT: a forged reachable node fails against the root-authenticated accumulator A_K.
    let mut tampered = RecursiveRelationEnvelope::decode(&plus_bytes).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(9_999_999);
    tampered.result_commitments[0] =
        commit_column(&pad(&tampered.result_columns[0], 1usize << tampered.result_log_domain)).unwrap();
    assert!(decode_and_verify_envelope(&tampered.encode().unwrap(), &plus_c, &lc, root).is_err());

    // WRONG ROOT.
    let other = build_layer(&[("a", "p", "b")]);
    assert!(plus_p.verify(&plus_c, &other, other.resolution_root()).is_err());
}

#[test]
fn all_pairs_alternation_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-all-pairs-alt-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_all_pairs_alternation_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// W1: all-pairs `(p/q)+` proves the EXACT 2-column reachability relation over a graph of 2-hop compound
/// edges, EXCLUDING the single-hop intermediates, and is non-interchangeable with the reversed sequence /
/// alternation.
fn assert_all_pairs_sequence_closure_e2e() {
    // 3-cycle of compound (p/q) hops: a =(p/q)=> b =(p/q)=> c =(p/q)=> a, via intermediates m1, m2, m3.
    let lc = build_layer(&[
        ("a", "p", "m1"), ("m1", "q", "b"),
        ("b", "p", "m2"), ("m2", "q", "c"),
        ("c", "p", "m3"), ("m3", "q", "a"),
    ]);
    let root = lc.resolution_root();

    let plus_c = plan_bgp(&all_pairs_compound_query("x", false, seq_body(), "y")).unwrap();
    let plus_p = plus_c.prove(&lc).unwrap();
    let plus_obj = plus_p.verify_and_resolve(&plus_c, &lc, root).unwrap();
    // E' = p∘q = {(a,b),(b,c),(c,a)} — a 3-cycle over {a,b,c}. Closure = {a,b,c}×{a,b,c} = 9 pairs.
    // Crucially EXCLUDES the single-hop intermediates m1/m2/m3 (unreachable by a full (p/q) compound hop).
    let expected: std::collections::BTreeSet<(String, String)> = [
        ("a", "a"), ("a", "b"), ("a", "c"),
        ("b", "a"), ("b", "b"), ("b", "c"),
        ("c", "a"), ("c", "b"), ("c", "c"),
    ]
    .iter()
    .map(|(x, y)| (iri(x), iri(y)))
    .collect();
    assert_eq!(closure_resolved_pairs(&plus_obj), expected);

    let plus_bytes = plus_p.encode_envelope(&plus_c, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_c, &lc, root).unwrap();

    // All-pairs sequence STAR reject-closes at compile.
    assert!(plan_bgp(&all_pairs_compound_query("x", true, seq_body(), "y")).is_err());

    // NON-INTERCHANGEABILITY: `(p/q)+` must NOT verify as the reversed `(q/p)+` (different join order →
    // different derived edge) nor the alternation `(p|q)+`.
    let rev_seq = PathPattern::Sequence(PathSequence {
        sequence: vec![closure_pred_e2e("q"), closure_pred_e2e("p")],
    });
    let rev_c = plan_bgp(&all_pairs_compound_query("x", false, rev_seq, "y")).unwrap();
    assert!(plus_p.verify(&rev_c, &lc, root).is_err(), "(p/q)+ must not verify as (q/p)+");
    let alt_c = plan_bgp(&all_pairs_compound_query("x", false, alt_body(), "y")).unwrap();
    assert!(plus_p.verify(&alt_c, &lc, root).is_err(), "(p/q)+ must not verify as (p|q)+");

    // TAMPERED RESULT: injecting the EXCLUDED intermediate's id must fail against A_K.
    let mut tampered = RecursiveRelationEnvelope::decode(&plus_bytes).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(7_777_777);
    tampered.result_commitments[0] =
        commit_column(&pad(&tampered.result_columns[0], 1usize << tampered.result_log_domain)).unwrap();
    assert!(decode_and_verify_envelope(&tampered.encode().unwrap(), &plus_c, &lc, root).is_err());

    // WRONG ROOT.
    let other = build_layer(&[("a", "p", "m1"), ("m1", "q", "b")]);
    assert!(plus_p.verify(&plus_c, &other, other.resolution_root()).is_err());
}

#[test]
fn all_pairs_sequence_closure_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-all-pairs-seq-closure-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_all_pairs_sequence_closure_e2e)
        .unwrap()
        .join()
        .unwrap();
}

/// `Count(n, Distinct([endpoint], Path(<bound source>, star/plus(p), ?endpoint)))`.
fn count_closure_query(
    source: &str,
    reflexive: bool,
    predicate: &str,
    endpoint: &str,
    count: &str,
) -> Query {
    Query::Count(Count {
        query: Box::new(distinct_closure_query(source, reflexive, predicate, endpoint)),
        count: DataValue::Variable(count.into()),
    })
}

/// P1: `Count(n, Distinct([y], Path(a, star|plus(p), ?y)))` proves `n = |reachable set|` over the
/// SAME graph as [`assert_transitive_closure_e2e`]. The count is delivered over the closure's OWN
/// deduped accumulator `A_K` (commitment continuity), for star and plus, forward and inverse.
fn assert_transitive_closure_count_e2e() {
    let lc = build_layer(&[("a", "p", "b"), ("b", "p", "c")]);
    let root = lc.resolution_root();

    // plus from a over a -> b -> c = {b, c} => n = 2. The endpoint set is hidden; the single
    // delivered binding is the count.
    let plus_query = count_closure_query("a", false, "p", "y", "n");
    let plus_compiled = plan_bgp(&plus_query).unwrap();
    let plus_proved = plus_compiled.prove(&lc).unwrap();
    assert_eq!(plus_proved.count_binding(&plus_compiled), Some(("n", 2)));
    plus_proved
        .verify_executed_rows(&plus_compiled, &lc, root, &["n".into()], &[vec![2]])
        .unwrap();
    // An off-by-one / tampered count value fails to verify (mirrors CountProof rejecting m_J +/- 1).
    assert!(plus_proved
        .verify_executed_rows(&plus_compiled, &lc, root, &["n".into()], &[vec![1]])
        .is_err());
    assert!(plus_proved
        .verify_executed_rows(&plus_compiled, &lc, root, &["n".into()], &[vec![3]])
        .is_err());

    // The portable envelope round-trips (the count is bound by the closure proof + query digest).
    let plus_bytes = plus_proved.encode_envelope(&plus_compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&plus_bytes, &plus_compiled, &lc, root).unwrap();

    // star from a = {a, b, c} (the reflexive seed a is counted EXACTLY once) => n = 3.
    let star_query = count_closure_query("a", true, "p", "y", "n");
    let star_compiled = plan_bgp(&star_query).unwrap();
    let star_proved = star_compiled.prove(&lc).unwrap();
    assert_eq!(star_proved.count_binding(&star_compiled), Some(("n", 3)));
    star_proved
        .verify_executed_rows(&star_compiled, &lc, root, &["n".into()], &[vec![3]])
        .unwrap();
    assert!(star_proved
        .verify_executed_rows(&star_compiled, &lc, root, &["n".into()], &[vec![2]])
        .is_err());

    // OPERATOR NON-INTERCHANGEABILITY: a count-of-plus proof must not verify under the count-of-star
    // plan (n differs, and the K-independent digest binds star vs plus), in either the direct or the
    // decoded-envelope path; nor under the plain (un-counted) closure plan (the count binding + the
    // zero-column projection are folded into the query digest).
    assert!(plus_proved.verify(&star_compiled, &lc, root).is_err());
    assert!(decode_and_verify_envelope(&plus_bytes, &star_compiled, &lc, root).is_err());
    let plus_uncounted = plan_bgp(&distinct_closure_query("a", false, "p", "y")).unwrap();
    assert!(plus_proved.verify(&plus_uncounted, &lc, root).is_err());

    // A prover-side forged `result_len` fails verify: the count binds the closure's AUTHENTICATED
    // A_K length (the accumulate-and-dedupe rounds + the empty-delta fixpoint), not a free scalar.
    let mut forged = plus_compiled.prove(&lc).unwrap();
    forged.result_len += 1;
    assert!(forged.verify(&plus_compiled, &lc, root).is_err());

    // INVERSE (bound target): `Count(n, Distinct([x], Path(?x, plus(p), c)))` counts the nodes that
    // reach c over the inverse edge = {a, b} => n = 2. Same edge relation, mirrored traversal.
    let inverse_query = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["x".into()],
            query: Box::new(Query::Path(Path {
                subject: Value::Variable("x".into()),
                pattern: PathPattern::Plus(PathPlus {
                    plus: Box::new(PathPattern::Predicate(PathPredicate {
                        predicate: Some(iri("p")),
                    })),
                }),
                object: Value::Node(iri("c")),
                path: None,
            })),
        })),
        count: DataValue::Variable("n".into()),
    });
    let inverse_compiled = plan_bgp(&inverse_query).unwrap();
    let inverse_proved = inverse_compiled.prove(&lc).unwrap();
    assert_eq!(inverse_proved.count_binding(&inverse_compiled), Some(("n", 2)));
    inverse_proved
        .verify_executed_rows(&inverse_compiled, &lc, root, &["n".into()], &[vec![2]])
        .unwrap();
    // The forward count (into ?y from a) and the inverse count (into c from ?x) are distinct queries.
    assert!(inverse_proved.verify(&plus_compiled, &lc, root).is_err());
}

#[test]
fn transitive_closure_count_prove_and_verify_end_to_end() {
    std::thread::Builder::new()
        .name("woql-transitive-closure-count-proof".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_transitive_closure_count_e2e)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn path_predicate_proves_endpoint_pairs() {
    let lc = build_layer(&[("a", "knows", "b"), ("b", "knows", "c"), ("x", "likes", "y")]);
    let id = |name: &str| node_object_id(&lc, name);

    let compiled = plan_bgp(&path_query("s", path_predicate("knows"), "o")).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    // Exact bag of (s, o) pairs connected by a single `knows` edge.
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[vec![id("a"), id("b")], vec![id("b"), id("c")]],
        )
        .unwrap();
    // A pair the query does NOT connect must fail to verify.
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[vec![id("a"), id("b")], vec![id("x"), id("y")]],
        )
        .is_err());
}

#[test]
fn path_inverse_predicate_proves_reversed_pairs() {
    // Two people share a parent `b`. Path(?s, <parent, ?o) lowers to Triple(?o, parent, ?s):
    // for every stored (x, parent, y) it binds o=x, s=y (schema order [o, s]).
    let lc = build_layer(&[("a", "parent", "b"), ("c", "parent", "b")]);
    let id = |name: &str| node_object_id(&lc, name);

    let compiled = plan_bgp(&path_query("s", path_inverse("parent"), "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["o", "s"]);
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["o".into(), "s".into()],
            &[vec![id("a"), id("b")], vec![id("c"), id("b")]],
        )
        .unwrap();
}

#[test]
fn path_two_hop_sequence_proves_projected_bag_of_endpoint_pairs() {
    // `knows,likes`. (a) reaches g through TWO intermediates (b and d) -> (a,g) has
    // multiplicity two; e reaches h once; x is a dead end (no `likes` from y).
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("b", "likes", "g"),
        ("a", "knows", "d"),
        ("d", "likes", "g"),
        ("e", "knows", "f"),
        ("f", "likes", "h"),
        ("x", "knows", "y"),
    ]);
    let id = |name: &str| node_object_id(&lc, name);

    let pattern = PathPattern::Sequence(PathSequence {
        sequence: vec![path_predicate("knows"), path_predicate("likes")],
    });
    let compiled = plan_bgp(&path_query("s", pattern, "o")).unwrap();
    assert!(compiled.projection.is_some());
    let proved = compiled.prove(&lc).unwrap();
    // Independently computed bag of connected (s, o) pairs, projecting the intermediate out.
    // BAG multiplicity: (a,g) appears twice (two distinct 2-edge paths).
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[
                vec![id("a"), id("g")],
                vec![id("a"), id("g")],
                vec![id("e"), id("h")],
            ],
        )
        .unwrap();
    // Dropping one of the duplicate (a,g) rows must fail — bag multiplicity is authenticated.
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[vec![id("a"), id("g")], vec![id("e"), id("h")]],
        )
        .is_err());
}

#[test]
fn path_or_of_two_predicates_proves_exact_bag_union() {
    let lc = build_layer(&[
        ("a", "knows", "b"),
        ("c", "likes", "d"),
        ("c", "knows", "d"),
        ("e", "likes", "f"),
    ]);
    let id = |name: &str| node_object_id(&lc, name);

    let pattern = PathPattern::Or(PathOr {
        or: vec![path_predicate("knows"), path_predicate("likes")],
    });
    let compiled = plan_bgp(&path_query("s", pattern, "o")).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    // knows: (a,b),(c,d) ; likes: (c,d),(e,f). Bag union keeps the overlapping (c,d) twice.
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[
                vec![id("a"), id("b")],
                vec![id("c"), id("d")],
                vec![id("c"), id("d")],
                vec![id("e"), id("f")],
            ],
        )
        .unwrap();
}

#[test]
fn path_exact_times_two_two_proves_via_sequence_on_distinct_predicates() {
    // Times{2,2} over the SAME predicate rejects closed (edge-simple semantics), but its
    // intended two-hop meaning is provable when the two hops use DISTINCT predicates: the
    // sequence `parent,name`, repeated content of a fixed two-length path.
    let lc = build_layer(&[
        ("kid", "parent", "mum"),
        ("mum", "name", "MumName"),
        ("kid2", "parent", "mum"),
    ]);
    let id = |name: &str| node_object_id(&lc, name);

    let pattern = PathPattern::Sequence(PathSequence {
        sequence: vec![path_predicate("parent"), path_predicate("name")],
    });
    let compiled = plan_bgp(&path_query("s", pattern, "o")).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    // kid->mum->MumName and kid2->mum->MumName.
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[
                vec![id("kid"), id("MumName")],
                vec![id("kid2"), id("MumName")],
            ],
        )
        .unwrap();
}

#[test]
fn path_times_one_one_proves_single_hop() {
    let lc = build_layer(&[("a", "knows", "b"), ("b", "knows", "c")]);
    let id = |name: &str| node_object_id(&lc, name);

    let pattern = PathPattern::Times(PathTimes {
        times: Box::new(path_predicate("knows")),
        from: 1,
        to: 1,
    });
    let compiled = plan_bgp(&path_query("s", pattern, "o")).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    proved
        .verify_executed_rows(
            &compiled,
            &lc,
            lc.resolution_root(),
            &["s".into(), "o".into()],
            &[vec![id("a"), id("b")], vec![id("b"), id("c")]],
        )
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
    assert_eq!(c.relation.schema, vec!["x", "y", "z", "w"]);
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, c.bgp_plan()).unwrap();
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
    assert_eq!(c.relation.schema, vec!["a", "b", "c", "d"]);
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, c.bgp_plan()).unwrap();
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
    assert_eq!(c.relation.schema, vec!["a", "b", "x", "c"]);
    // (a in {a1,a2}) x (b1) x (c in {c1,c2}) = 2 * 1 * 2 = 4 rows.
    let (_p, _coms, m, _ids) = prove_layer_bgp(&lc, c.bgp_plan()).unwrap();
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

    let (proof, result_coms, m, ids) = prove_layer_bgp(&lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        &lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
        &ids,
        &result_coms,
        m,
        &proof,
    )
    .unwrap();

    // Projected columns are exactly the true-join columns at the selected indices.
    let (exp_cols, domain) = expected_bgp(&lc, &ids, &compiled.bgp_plan().joins);
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
        source: ColumnSource::Materialized,
    };
    let (proof, result_coms, m_bogus, ids) = prove_layer_bgp(&lc, &bogus).unwrap();
    // It VERIFIES — the crypto is faithful to the (wrong) plan.
    verify_layer_bgp(
        &lc,
        lc.resolution_root(),
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
    assert_eq!(compiled.bgp_plan().joins[0].additional_keys, vec![(1, 1)]);
    let (exact_proof, exact_coms, exact_m, exact_ids) =
        prove_layer_bgp(&lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        &lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
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
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(compiled.bgp_plan().joins[1].additional_keys, vec![(0, 1)]);

    let lc = build_layer(&[
        ("a", "p", "b"),
        ("a", "p", "c"),
        ("b", "q", "d"),
        ("c", "q", "d"),
        ("d", "r", "a"),
        ("d", "r", "e"),
    ]);
    let (proof, commitments, result_len, ids) = prove_layer_bgp(&lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        &lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
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
    let (proof, coms, m, ids) = prove_layer_bgp(lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
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
        &compiled.bgp_plan().atoms,
        &ids,
        &subject_ids,
        &object_ids,
        &compiled.bgp_plan().joins,
    );
    assert_eq!(
        coms.len(),
        compiled.relation.schema.len(),
        "one commitment per schema column"
    );
    assert_eq!(exp_cols.len(), compiled.relation.schema.len());
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
            compiled.relation.schema[c]
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
    assert_eq!(c.relation.schema, vec!["z", "x"]);
    assert!(matches!(c.bgp_plan().atoms[1].object, AtomTerm::NodeConstant(_)));
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
    assert_eq!(c.relation.schema, vec!["y", "z"]);
    assert!(matches!(c.bgp_plan().atoms[0].subject, AtomTerm::NodeConstant(_)));
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
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (1, 0));
    assert_const_join_end_to_end(&lc, &q, 2);
}

#[test]
fn absent_node_constant_authenticates_empty_join_and_envelope() {
    let lc = build_layer(&[("a", "knows", "b"), ("b", "likes", "c")]);
    let q = and_of(vec![
        tv("x", "knows", "y"),
        tv_obj_node("y", "likes", "absent"),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 0);
    assert_eq!(proved.result_commitments.len(), 2);
    proved
        .verify(&compiled, &lc, lc.resolution_root())
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.resolution_root())
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &lc, lc.resolution_root()).unwrap();

    let present_variant = plan_bgp(&and_of(vec![
        tv("x", "knows", "y"),
        tv_obj_node("y", "likes", "c"),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &present_variant,
        &lc,
        lc.resolution_root(),
    )
    .is_err());
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
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert!(matches!(
        c.bgp_plan().atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
    // The compiled value constant is exactly the store's canonicalization of "hi".
    match &c.bgp_plan().atoms[0].object {
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
fn absent_string_literal_authenticates_empty_join() {
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
    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 0);
    proved
        .verify(&compiled, &lc, lc.resolution_root())
        .unwrap();
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
    assert_value_constant_matches(&compiled.bgp_plan().atoms[0].object, store_decimal(&wanted));
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
    assert_value_constant_matches(&compiled.bgp_plan().atoms[0].object, entry(vec![0xde, 0xad]));
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
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.bgp_plan().atoms[0].object, bool::make_entry(&true));
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
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.bgp_plan().atoms[0].object, i64::make_entry(&42i64));
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
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant_matches(&c.bgp_plan().atoms[0].object, f64::make_entry(&3.5f64));
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
        .bgp_plan()
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

    let (proof, coms, m, ids) = prove_layer_bgp(lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
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
        .bgp_plan()
        .atoms
        .iter()
        .position(|a| a.object_range.is_some())
        .unwrap();
    let name_idx = 1 - age_idx;
    let (exp_cols, domain) = expected_range_join(lc, ids[age_idx], ids[name_idx], op, k as u64);
    assert_eq!(coms.len(), compiled.relation.schema.len());
    assert_eq!(exp_cols.len(), compiled.relation.schema.len());
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
            compiled.relation.schema[c]
        );
    }

    // schema is [a, x, n]; the Select reads ?x, ?n -> indices [1, 2].
    assert_eq!(compiled.relation.schema, vec!["a", "x", "n"]);
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
    let range = compiled.bgp_plan().atoms[0].object_range.as_ref().unwrap();
    assert_eq!(range.op, RangeOp::Ge);
    assert_eq!(range.second.as_ref().unwrap().op, RangeOp::Lt);

    let proved = compiled.prove(&lc).unwrap();
    assert_eq!(proved.result_len, 3);
    proved
        .verify(&compiled, &lc, lc.resolution_root())
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

/// The order-preserving `u64` code the store's range gadget compares for a committed value,
/// as an INDEPENDENT oracle (decoded straight from tdb-succinct, not via the store's private
/// `node_value_op_value`): a big-endian read of the fixed-width numeric lexical, OR — for
/// `date`/`dateTime` — the decoded i64-nanoseconds key in offset-binary. Mirrors the store's
/// encoding, which the store's own unit tests validate against first-principles instants.
fn op_u64(datatype: CanonicalDatatype, lexical: &[u8]) -> u64 {
    // Shared offset-binary transform of a signed i64 key: `(n as u64) ^ (1<<63)`.
    fn offset_binary(n: i64) -> u64 {
        (n as u64) ^ (1u64 << 63)
    }
    // i64 nanoseconds since the epoch for a UTC `NaiveDateTime`, saturating outside i64.
    fn nanos(ndt: chrono::NaiveDateTime) -> i64 {
        let utc = ndt.and_utc();
        utc.timestamp_nanos_opt()
            .unwrap_or(if utc.timestamp() >= 0 { i64::MAX } else { i64::MIN })
    }
    match datatype {
        CanonicalDatatype::DateTime => {
            let mut cursor = lexical;
            offset_binary(nanos(tdb_succinct::tfc::datetime::storage_to_datetime(
                &mut cursor,
            )))
        }
        CanonicalDatatype::Date => {
            use chrono::NaiveDate;
            use tdb_succinct::{Date, FromLexical};
            let d = <Date as FromLexical<Date>>::from_lexical(lexical);
            let base = NaiveDate::from_ymd_opt(d.year as i32, u32::from(d.month), u32::from(d.day))
                .map(|date| nanos(date.and_hms_opt(0, 0, 0).unwrap()))
                .unwrap_or(if d.year >= 0 { i64::MAX } else { i64::MIN });
            let key = match base {
                i64::MIN | i64::MAX => base,
                _ => base.saturating_add(i64::from(d.offset)),
            };
            offset_binary(key)
        }
        _ => {
            let mut buf = [0u8; 8];
            buf[8 - lexical.len()..].copy_from_slice(lexical);
            u64::from_be_bytes(buf)
        }
    }
}

/// The committed `(datatype, order-preserving u64)` behind a node/value dict id, or `None`
/// for a `Node` object — the SAME code the store's range gadget compares.
fn op_value_of(lc: &LayerCommitment, object_id: u64) -> Option<(CanonicalDatatype, u64)> {
    lc.node_value_catalog
        .iter()
        .find(|r| r.object_id == object_id)
        .and_then(|r| match &r.triple.object {
            CanonicalObject::Value { datatype, lexical } => Some((*datatype, op_u64(*datatype, lexical))),
            CanonicalObject::Node(_) => None,
        })
}

/// The store's committed OP encoding of a typed dict entry as a `u64`. The compiler's
/// emitted `c` must equal this. Covers the fixed-width numerics AND date/dateTime.
fn store_op_value(entry: TypedDictEntry) -> u64 {
    match CanonicalObject::from_store_object(&ValueTriple::new_value("", "", entry).object) {
        CanonicalObject::Value { datatype, lexical } => op_u64(datatype, &lexical),
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
        .bgp_plan()
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

    let (proof, coms, m, ids) = prove_layer_bgp(lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
        &ids,
        &coms,
        m,
        &proof,
    )
    .unwrap();
    assert_eq!(m, expected_m, "proven row count (op {:?})", op);

    let val_idx = compiled
        .bgp_plan()
        .atoms
        .iter()
        .position(|a| a.object_range.is_some())
        .unwrap();
    let name_idx = 1 - val_idx;
    let (exp_cols, domain) =
        expected_range_join_generic(lc, ids[val_idx], ids[name_idx], op, c_op, datatype);
    assert_eq!(coms.len(), compiled.relation.schema.len());
    assert_eq!(exp_cols.len(), compiled.relation.schema.len());
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
            compiled.relation.schema[c]
        );
    }
    assert_eq!(compiled.relation.schema, vec!["a", "x", "n"]);
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

#[test]
fn object_range_int8_ge_zero_authenticates_signed_width_and_dictionary_tag() {
    let lc = build_val_name_layer(
        "tiny_count",
        vec![
            ("minimum", i8::make_entry(&i8::MIN)),
            ("negative", i8::make_entry(&-1i8)),
            ("zero", i8::make_entry(&0i8)),
            ("maximum", i8::make_entry(&i8::MAX)),
        ],
    );
    check_range_generic(
        &lc,
        "tiny_count",
        gte(
            dvar("a"),
            DataValue::Data(XSDAnySimpleType::Byte(0)),
        ),
        RangeOp::Ge,
        CanonicalDatatype::Int8,
        store_op_value(i8::make_entry(&0i8)),
        2,
    );
}

#[test]
fn object_range_float64_reject_closes_at_compile() {
    // Float object-value ranges REJECT-CLOSE for fidelity — there is no e2e prove because the
    // reject happens at COMPILE time, before any committed layer is touched. The store keys
    // floats by IEEE TOTAL order (tdb-succinct `f64::to_lexical`): `-0.0` sorts strictly BELOW
    // `+0.0` and `NaN` above `+inf`, diverging from WOQL's numeric `<` (`-0.0 == +0.0`, `NaN`
    // unordered). `-0.0` is REACHABLE in committed data via the document/JSON-LD ingest path
    // (empirically: SWI-Prolog `float(-0.0)` preserves the sign, a JSON `-0.0` field round-trips
    // to a stored `-0.0`), so no float range — zero-boundary OR non-zero — is provable faithful.
    use terminusdb_woql2::control::Select;
    use terminusdb_woql2::query::Query;
    for cmp in [greater(dvar("a"), dfloat(0.0)), lte(dvar("a"), dfloat(1.5))] {
        let bgp = and_of(vec![tv("x", "score", "a"), tv("x", "name", "n"), cmp]);
        let q = Query::Select(Select {
            variables: vec!["x".into(), "n".into()],
            query: Box::new(bgp),
        });
        assert!(
            matches!(
                plan_bgp(&q),
                Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "float" })
            ),
            "float object-value range must reject-close at compile time",
        );
    }
}

// -------------------------------------------------------------------------------------------
// date / dateTime object-value RANGE filters — end-to-end prove+verify. Each builds a
// homogeneous `t(person,<pred>,<date>)` + `t(person,name,<node>)` layer (>= 2 real rows so
// the Dory domain pads cleanly), compiles a range query, proves+verifies it, checks the
// committed columns against the independent range-filtered join oracle, and asserts the
// compiled `c`/datatype tag match the store's committed OP encoding. The OP key is the
// DECODED i64-nanoseconds order (chronological for dateTime; (y,m,d,offset)-lexical for
// date), NOT a big-endian read of the variable-width lexical.
// -------------------------------------------------------------------------------------------

/// UTC `NaiveDateTime` entry, from a (y-mo-d h:mi:s) instant.
fn dt_entry(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> TypedDictEntry {
    use chrono::{TimeZone, Utc};
    chrono::NaiveDateTime::make_entry(&Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap().naive_utc())
}

/// `xsd:date` entry with an explicit timezone `offset` (minutes); offset 0 for the common case.
fn date_entry_off(y: i64, mo: u8, d: u8, offset: i16) -> TypedDictEntry {
    tdb_succinct::Date::make_entry(&tdb_succinct::Date {
        year: y,
        month: mo,
        day: d,
        offset,
    })
}

/// dateTimes straddling 2020-06-15T12:00:00Z: 2019, 2020-06-15T12 (== the constant), 2021, 2022.
fn datetime_fixture() -> Arc<LayerCommitment> {
    build_val_name_layer(
        "ts",
        vec![
            ("alice", dt_entry(2019, 1, 1, 0, 0, 0)),
            ("bob", dt_entry(2020, 6, 15, 12, 0, 0)),
            ("carol", dt_entry(2021, 1, 1, 0, 0, 0)),
            ("dave", dt_entry(2022, 12, 31, 23, 59, 59)),
        ],
    )
}

#[test]
fn object_range_datetime_gt_excludes_equal_and_earlier() {
    // ?d > 2020-06-15T12:00:00Z -> {carol(2021), dave(2022)} = 2 (bob is EQUAL → excluded).
    check_range_generic(
        &datetime_fixture(),
        "ts",
        greater(dvar("a"), ddatetime(2020, 6, 15, 12, 0, 0)),
        RangeOp::Gt,
        CanonicalDatatype::DateTime,
        store_op_value(dt_entry(2020, 6, 15, 12, 0, 0)),
        2,
    );
}

#[test]
fn object_range_datetime_ge_includes_equal_boundary() {
    // ?d >= 2020-06-15T12:00:00Z -> {bob(==), carol, dave} = 3 (boundary INCLUDED).
    check_range_generic(
        &datetime_fixture(),
        "ts",
        gte(dvar("a"), ddatetime(2020, 6, 15, 12, 0, 0)),
        RangeOp::Ge,
        CanonicalDatatype::DateTime,
        store_op_value(dt_entry(2020, 6, 15, 12, 0, 0)),
        3,
    );
}

#[test]
fn object_range_datetime_lt_selects_only_earlier() {
    // ?d < 2020-06-15T12:00:00Z -> {alice(2019)} = 1.
    check_range_generic(
        &datetime_fixture(),
        "ts",
        less(dvar("a"), ddatetime(2020, 6, 15, 12, 0, 0)),
        RangeOp::Lt,
        CanonicalDatatype::DateTime,
        store_op_value(dt_entry(2020, 6, 15, 12, 0, 0)),
        1,
    );
}

/// xsd:date birthdays (offset 0) straddling 2000-06-15.
fn date_fixture() -> Arc<LayerCommitment> {
    build_val_name_layer(
        "bday",
        vec![
            ("alice", date_entry_off(1990, 1, 1, 0)),
            ("bob", date_entry_off(2000, 6, 15, 0)),
            ("carol", date_entry_off(2010, 12, 31, 0)),
            ("dave", date_entry_off(2020, 1, 1, 0)),
        ],
    )
}

#[test]
fn object_range_date_ge_includes_equal_boundary() {
    // ?d >= 2000-06-15 -> {bob(==), carol, dave} = 3.
    check_range_generic(
        &date_fixture(),
        "bday",
        gte(dvar("a"), ddate(2000, 6, 15)),
        RangeOp::Ge,
        CanonicalDatatype::Date,
        store_op_value(date_entry_off(2000, 6, 15, 0)),
        3,
    );
}

#[test]
fn object_range_date_lt_selects_only_earlier() {
    // ?d < 2000-06-15 -> {alice(1990)} = 1.
    check_range_generic(
        &date_fixture(),
        "bday",
        less(dvar("a"), ddate(2000, 6, 15)),
        RangeOp::Lt,
        CanonicalDatatype::Date,
        store_op_value(date_entry_off(2000, 6, 15, 0)),
        1,
    );
}

#[test]
fn woql_in_range_date_proves_half_open_filtered_join() {
    // ?d in [2000-01-01, 2015-01-01) -> {bob(2000-06-15), carol(2010-12-31)} = 2
    // (alice 1990 below start, dave 2020 at/after end).
    let lc = date_fixture();
    let q = and_of(vec![
        tv("x", "bday", "a"),
        tv("x", "name", "n"),
        Query::InRange(InRange {
            value: dvar("a"),
            start: ddate(2000, 1, 1),
            end: ddate(2015, 1, 1),
        }),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    let (proof, coms, m, ids) = prove_layer_bgp(&lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        &lc,
        lc.resolution_root(),
        compiled.bgp_plan(),
        &ids,
        &coms,
        m,
        &proof,
    )
    .unwrap();
    assert_eq!(m, 2, "half-open date InRange selects exactly bob and carol");
}

#[test]
fn object_range_date_offset_tiebreaker_matches_woql_compound_order() {
    // SOUNDNESS of the offset tiebreaker: three subjects share the SAME calendar day
    // 2000-06-15 but differ by timezone offset. WOQL compares stored dates as
    // `date(Y,M,D,Offset)` in standard order, so with constant 2000-06-15 (offset 0):
    //   offset -60 < offset 0 (== const) < offset +60.
    // Thus `?d > 2000-06-15` selects ONLY the +60 row (east), not the 0 or -60 rows.
    let lc = build_val_name_layer(
        "bday",
        vec![
            ("west", date_entry_off(2000, 6, 15, -60)), // < const
            ("utc", date_entry_off(2000, 6, 15, 0)),    // == const
            ("east", date_entry_off(2000, 6, 15, 60)),  // > const
        ],
    );
    check_range_generic(
        &lc,
        "bday",
        greater(dvar("a"), ddate(2000, 6, 15)),
        RangeOp::Gt,
        CanonicalDatatype::Date,
        store_op_value(date_entry_off(2000, 6, 15, 0)),
        1,
    );
}

// -------------------------------------------------------------------------------------------
// WOQL `Eval` integer arithmetic — end-to-end prove+verify via the VERIFIER-RECOMPUTE path.
// -------------------------------------------------------------------------------------------

/// A base layer with two integer predicates `p`,`q` per subject: `t(subj,p,a)`, `t(subj,q,b)`.
fn eval_two_predicate_layer(rows: &[(&str, u32, u32)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, p, q) in rows {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(subject),
                &iri("p"),
                u32::make_entry(p),
            ))
            .unwrap();
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(subject),
                &iri("q"),
                u32::make_entry(q),
            ))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn ea_var(name: &str) -> ArithmeticExpression {
    ArithmeticExpression::Value(ArithmeticValue::Variable(name.into()))
}

fn ea_lit(value: XSDAnySimpleType) -> ArithmeticExpression {
    ArithmeticExpression::Value(ArithmeticValue::Data(value))
}

fn ea_plus(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Plus(Plus {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn ea_minus(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Minus(Minus {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn ea_times(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Times(Times {
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// `t(?x,p,?a) & t(?x,q,?b) & a>=0 & b>=0 & Eval(expr, ?result)`.
fn eval_ab_query(expr: ArithmeticExpression, result: &str) -> Query {
    and_of(vec![
        tv("x", "p", "a"),
        tv("x", "q", "b"),
        gte(dvar("a"), duint(0)),
        gte(dvar("b"), duint(0)),
        Query::Eval(Eval {
            expression: expr,
            result_value: ArithmeticValue::Variable(result.into()),
        }),
    ])
}

fn eval_u32(value: &CanonicalResultValue) -> u32 {
    match value {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::UInt32,
            lexical,
        }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()),
        other => panic!("unexpected operand value {other:?}"),
    }
}

fn eval_i64(value: &CanonicalResultValue) -> i64 {
    match value {
        CanonicalResultValue::GeneratedDecimalI64(value) => *value,
        other => panic!("unexpected derived value {other:?}"),
    }
}

/// Prove+verify `expr`, returning per-row `(a, b, r)` triples in result order.
fn eval_ab_rows(
    layer: &LayerCommitment,
    expr: ArithmeticExpression,
) -> (CompiledBgp, Vec<(u32, u32, i64)>) {
    let compiled = plan_bgp(&eval_ab_query(expr, "r")).unwrap();
    let proved = compiled.prove(layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, layer, layer.resolution_root())
        .unwrap();
    let index = |name: &str| {
        compiled
            .relation
            .schema
            .iter()
            .position(|variable| variable == name)
            .unwrap()
    };
    let (ai, bi, ri) = (index("a"), index("b"), index("r"));
    let rows = (0..values[ri].len())
        .map(|row| {
            (
                eval_u32(&values[ai][row]),
                eval_u32(&values[bi][row]),
                eval_i64(&values[ri][row]),
            )
        })
        .collect();
    (compiled, rows)
}

#[test]
fn eval_plus_proves_per_row_sum() {
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 10, 2), ("n3", 7, 7)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_plus(ea_var("a"), ea_var("b")));
    assert_eq!(rows.len(), 3);
    for (a, b, r) in rows {
        assert_eq!(r, i64::from(a) + i64::from(b));
    }
}

#[test]
fn eval_times_proves_per_row_product() {
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 10, 2)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_times(ea_var("a"), ea_var("b")));
    assert_eq!(rows.len(), 2);
    for (a, b, r) in rows {
        assert_eq!(r, i64::from(a) * i64::from(b));
    }
}

#[test]
fn eval_minus_proves_per_row_difference_including_negatives() {
    let layer = eval_two_predicate_layer(&[("n1", 10, 3), ("n2", 2, 5)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_minus(ea_var("a"), ea_var("b")));
    assert_eq!(rows.len(), 2);
    let mut saw_negative = false;
    for (a, b, r) in rows {
        assert_eq!(r, i64::from(a) - i64::from(b));
        saw_negative |= r < 0;
    }
    assert!(saw_negative, "the b>a row must produce a negative i64 result");
}

#[test]
fn eval_nested_tree_plus_times_literal_proves() {
    // Plus(Times(?a, 2), ?b) — nesting is free under verifier recompute.
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 10, 2)]);
    let expr = ea_plus(ea_times(ea_var("a"), ea_lit(XSDAnySimpleType::Int(2))), ea_var("b"));
    let (_compiled, rows) = eval_ab_rows(&layer, expr);
    assert_eq!(rows.len(), 2);
    for (a, b, r) in rows {
        assert_eq!(r, i64::from(a) * 2 + i64::from(b));
    }
}

#[test]
fn eval_wrong_result_column_fails_verification() {
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 10, 2)]);
    let compiled = plan_bgp(&eval_ab_query(ea_plus(ea_var("a"), ea_var("b")), "r")).unwrap();
    let ri = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "r")
        .unwrap();

    // (a) A wrong derived value WITH a rebuilt (internally consistent) commitment: passes the
    //     commitment binding but the VERIFIER RE-DERIVATION rejects it — this is the core
    //     soundness property (the delivered result must equal the recomputed arithmetic).
    let mut wrong_value = compiled.prove(&layer).unwrap();
    wrong_value.result_columns[ri][0] = generated_decimal_i64_field(9_999);
    let domain = 1usize << wrong_value.result_log_domain;
    wrong_value.result_commitments[ri] =
        commit_column(&pad(&wrong_value.result_columns[ri], domain)).unwrap();
    assert!(wrong_value
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());

    // (b) A corrupted derived-column COMMITMENT with the correct column values: the recompute
    //     matches, but the result-column-to-commitment binding (`verify_columns`) rejects it.
    let mut wrong_commit = compiled.prove(&layer).unwrap();
    let domain_b = 1usize << wrong_commit.result_log_domain;
    wrong_commit.result_commitments[ri] =
        commit_column(&pad(&[generated_decimal_i64_field(1)], domain_b)).unwrap();
    assert!(wrong_commit
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn eval_tampered_operand_passthrough_fails_verification() {
    // Change a carried-through OPERAND column (with a rebuilt commitment). The Eval arm pins
    // the operand columns to the authenticated child, so an operand that differs from the
    // store-derived value the result was computed from is rejected.
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 10, 2)]);
    let compiled = plan_bgp(&eval_ab_query(ea_plus(ea_var("a"), ea_var("b")), "r")).unwrap();
    let ai = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "a")
        .unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    // Swap two rows of the operand column so its value no longer matches the child's proven
    // column (but the column still opens to a rebuilt commitment).
    tampered.result_columns[ai].swap(0, 1);
    let domain = 1usize << tampered.result_log_domain;
    tampered.result_commitments[ai] =
        commit_column(&pad(&tampered.result_columns[ai], domain)).unwrap();
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn eval_times_overflowing_i64_rejects_without_field_wraparound() {
    // u32::MAX * u32::MAX ~= 1.84e19 exceeds i64::MAX (~9.2e18) but fits i128. The checked i64
    // narrowing rejects it at prove time: the proof cannot be produced, so no wrapped field is
    // ever committed.
    let layer = eval_two_predicate_layer(&[("n1", u32::MAX, u32::MAX)]);
    let compiled = plan_bgp(&eval_ab_query(ea_times(ea_var("a"), ea_var("b")), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

fn ea_div(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Div(Div {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn ea_exp(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Exp(Exp {
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// Reference FLOOR division (rounds toward −∞), matching SWI-Prolog `div/2` — the semantics the
/// verifier recompute must reproduce. Deliberately NOT `div_euclid` (which differs when `b<0`).
fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

#[test]
fn eval_div_proves_per_row_floor_quotient() {
    let layer = eval_two_predicate_layer(&[("n1", 7, 2), ("n2", 10, 3), ("n3", 9, 3)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_div(ea_var("a"), ea_var("b")));
    assert_eq!(rows.len(), 3);
    for (a, b, r) in rows {
        assert_eq!(r, floor_div(i64::from(a), i64::from(b)));
    }
}

#[test]
fn eval_div_negative_divisor_floors_toward_negative_infinity() {
    // divisor = 2 - b is NEGATIVE here; the result must FLOOR (toward −∞), which distinguishes
    // Prolog `div` from truncation-toward-zero AND from Rust `div_euclid`.
    let layer = eval_two_predicate_layer(&[("n1", 7, 4), ("n2", 10, 5)]);
    let expr = ea_div(
        ea_var("a"),
        ea_minus(ea_lit(XSDAnySimpleType::Int(2)), ea_var("b")),
    );
    let (_compiled, rows) = eval_ab_rows(&layer, expr);
    let mut saw_flooring = false;
    for (a, b, r) in rows {
        let divisor = 2 - i64::from(b);
        assert_eq!(r, floor_div(i64::from(a), divisor));
        if a == 7 && b == 4 {
            // floor(7 / -2) = -4; truncation OR div_euclid would give -3.
            assert_eq!(r, -4);
            saw_flooring = true;
        }
    }
    assert!(saw_flooring, "the (7 div -2) row must demonstrate flooring to -4");
}

#[test]
fn eval_div_by_zero_fails_at_prove() {
    // A zero divisor makes Prolog `div/2` THROW, so the query fails: the proof must FAIL CLOSED
    // (never silently drop the row). `b >= 0` admits `b == 0`, so this exercises a real row.
    let layer = eval_two_predicate_layer(&[("n1", 5, 0)]);
    let compiled = plan_bgp(&eval_ab_query(ea_div(ea_var("a"), ea_var("b")), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn eval_div_wrong_result_fails_verification() {
    // A wrong Div result with a rebuilt (internally consistent) commitment still fails the
    // verifier RE-DERIVATION — the core soundness property.
    let layer = eval_two_predicate_layer(&[("n1", 7, 2)]);
    let compiled = plan_bgp(&eval_ab_query(ea_div(ea_var("a"), ea_var("b")), "r")).unwrap();
    let ri = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "r")
        .unwrap();
    let mut wrong = compiled.prove(&layer).unwrap();
    wrong.result_columns[ri][0] = generated_decimal_i64_field(999);
    let domain = 1usize << wrong.result_log_domain;
    wrong.result_commitments[ri] = commit_column(&pad(&wrong.result_columns[ri], domain)).unwrap();
    assert!(wrong
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn eval_exp_proves_per_row_power() {
    let layer = eval_two_predicate_layer(&[("n1", 3, 4), ("n2", 2, 10), ("n3", 5, 0)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_exp(ea_var("a"), ea_var("b")));
    assert_eq!(rows.len(), 3);
    for (a, b, r) in rows {
        // 3^4 = 81, 2^10 = 1024, 5^0 = 1 (integer result, matching Prolog `**`).
        assert_eq!(r, i64::from(a).pow(b));
    }
}

#[test]
fn eval_exp_overflowing_i64_fails_at_prove() {
    // 2^63 = i64::MAX + 1 overflows the declared i64 width → FAIL CLOSED (no field wraparound).
    let layer = eval_two_predicate_layer(&[("n1", 2, 63)]);
    let compiled = plan_bgp(&eval_ab_query(ea_exp(ea_var("a"), ea_var("b")), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn eval_exp_runtime_negative_exponent_fails_at_prove() {
    // exponent = b - 5 is NEGATIVE for small b → rational result (out of the integer layer) →
    // FAIL CLOSED in the recompute (a runtime-negative exponent, not a static literal).
    let layer = eval_two_predicate_layer(&[("n1", 3, 2)]);
    let expr = ea_exp(
        ea_var("a"),
        ea_minus(ea_var("b"), ea_lit(XSDAnySimpleType::Int(5))),
    );
    let compiled = plan_bgp(&eval_ab_query(expr, "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `Typecast` integer-width conversion — end-to-end prove+verify via VERIFIER RECOMPUTE.
// -------------------------------------------------------------------------------------------

/// A single-predicate layer: `t(subj, p, value)` for each `(subj, value)`.
fn typecast_single_predicate_layer(rows: &[(&str, u32)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, value) in rows {
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(subject),
                &iri("p"),
                u32::make_entry(value),
            ))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// A full XSD type IRI node value for a local type name.
fn xsd_type_pv(name: &str) -> NodeValue {
    NodeValue::Node(format!("http://www.w3.org/2001/XMLSchema#{name}"))
}

/// `t(?x,p,?a) & a>=0 & Typecast(?a, target, ?r)` — pins `?a` to xsd:unsignedInt (UInt32).
fn typecast_query_e2e(target: NodeValue, result: &str) -> Query {
    and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        Query::Typecast(Typecast {
            value: Value::Variable("a".into()),
            type_uri: target,
            result_value: Value::Variable(result.into()),
        }),
    ])
}

/// The cast column may be delivered signed OR unsigned depending on the target; read either.
fn typecast_result_i128(value: &CanonicalResultValue) -> i128 {
    match value {
        CanonicalResultValue::GeneratedDecimalI64(v) => i128::from(*v),
        CanonicalResultValue::GeneratedDecimalU64(v) => i128::from(*v),
        other => panic!("unexpected cast result {other:?}"),
    }
}

/// Prove+verify a Typecast of `?a` to `target`, returning per-row `(source, cast)` in order.
fn typecast_rows(layer: &LayerCommitment, target: NodeValue) -> (CompiledBgp, Vec<(u32, i128)>) {
    let compiled = plan_bgp(&typecast_query_e2e(target, "r")).unwrap();
    let proved = compiled.prove(layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, layer, layer.resolution_root())
        .unwrap();
    let index = |name: &str| {
        compiled
            .relation
            .schema
            .iter()
            .position(|variable| variable == name)
            .unwrap()
    };
    let (ai, ri) = (index("a"), index("r"));
    let rows = (0..values[ri].len())
        .map(|row| (eval_u32(&values[ai][row]), typecast_result_i128(&values[ri][row])))
        .collect();
    (compiled, rows)
}

#[test]
fn typecast_widening_unsigned_int_to_long_proves_identity() {
    // Widening is always in range; the value is unchanged (lossless identity).
    let layer = typecast_single_predicate_layer(&[("n1", 100), ("n2", u32::MAX)]);
    let (_compiled, rows) = typecast_rows(&layer, xsd_type_pv("long"));
    assert_eq!(rows.len(), 2);
    for (a, r) in rows {
        assert_eq!(r, i128::from(a));
    }
}

#[test]
fn typecast_xsd_prefixed_long_proves_identity() {
    // No-regression: the standard `xsd:long` PREFIXED target (not the full IRI) compiles and proves
    // the same widening identity — the namespace-aware acceptance covers both spellings.
    let layer = typecast_single_predicate_layer(&[("n1", 100), ("n2", u32::MAX)]);
    let (compiled, rows) = typecast_rows(&layer, NodeValue::Node("xsd:long".into()));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Typecast { .. }
    ));
    assert_eq!(rows.len(), 2);
    for (a, r) in rows {
        assert_eq!(r, i128::from(a));
    }
}

#[test]
fn typecast_unsigned_target_uses_unsigned_namespace_and_proves_identity() {
    // An UNSIGNED target (unsignedLong) delivers the cast column in the U64 namespace.
    // >= 2 rows: a query proof pads to a power of two and requires at least 2 real rows.
    let layer = typecast_single_predicate_layer(&[("n1", u32::MAX), ("n2", 42)]);
    let (compiled, rows) = typecast_rows(&layer, xsd_type_pv("unsignedLong"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Typecast { .. }
    ));
    for (a, r) in rows {
        assert_eq!(r, i128::from(a));
    }
}

#[test]
fn typecast_valid_narrowing_to_byte_proves_identity() {
    // In-range narrowing to xsd:byte [-128,127] is a lossless identity on the value.
    let layer = typecast_single_predicate_layer(&[("n1", 7), ("n2", 100)]);
    let (_compiled, rows) = typecast_rows(&layer, xsd_type_pv("byte"));
    assert_eq!(rows.len(), 2);
    for (a, r) in rows {
        assert_eq!(r, i128::from(a));
    }
}

#[test]
fn typecast_out_of_range_narrowing_to_byte_fails_at_prove() {
    // 300 does not fit xsd:byte [-128,127]: Prolog `typecast` THROWS casting_error, so the proof
    // must FAIL CLOSED (never wrap/truncate to 44). >= 2 rows (one in range, one not) so the
    // failure is provably the range check, not the "too few rows" floor.
    let layer = typecast_single_predicate_layer(&[("n1", 7), ("n2", 300)]);
    let compiled = plan_bgp(&typecast_query_e2e(xsd_type_pv("byte"), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn typecast_wrong_result_column_fails_verification() {
    // A wrong cast value with a rebuilt commitment still fails the verifier RE-DERIVATION.
    // >= 2 rows: a query proof requires at least 2 real rows (power-of-two padding).
    let layer = typecast_single_predicate_layer(&[("n1", 100), ("n2", 200)]);
    let compiled = plan_bgp(&typecast_query_e2e(xsd_type_pv("long"), "r")).unwrap();
    let ri = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "r")
        .unwrap();
    let mut wrong = compiled.prove(&layer).unwrap();
    wrong.result_columns[ri][0] = generated_decimal_i64_field(9_999);
    let domain = 1usize << wrong.result_log_domain;
    wrong.result_commitments[ri] = commit_column(&pad(&wrong.result_columns[ri], domain)).unwrap();
    assert!(wrong
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL number↔string `Typecast` — end-to-end prove+verify via the TERMINAL verifier-recompute.
// -------------------------------------------------------------------------------------------

/// A base layer with one xsd:string value per subject at predicate `p`.
fn string_p_layer(rows: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let mut triples = Vec::new();
    for (subject, value) in rows {
        triples.push(ValueTriple::new_string_value(&iri(subject), &iri("p"), value));
    }
    build_layer_triples(triples)
}

/// A base layer with one xsd:decimal value per subject at predicate `d`.
fn decimal_p_layer(rows: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, dec) in rows {
        let entry = Decimal::make_entry(&Decimal::new((*dec).to_string()).unwrap());
        builder
            .add_value_triple(ValueTriple::new_value(&iri(subject), &iri("d"), entry))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// `t(?x, pred, ?source) & Typecast(?source, target, ?r)` — an UNPINNED source (its type is enforced
/// at recompute), the number↔string cast shape.
fn cast_query(pred: &str, source: &str, target: NodeValue, result: &str) -> Query {
    and_of(vec![
        tv("x", pred, source),
        Query::Typecast(Typecast {
            value: Value::Variable(source.into()),
            type_uri: target,
            result_value: Value::Variable(result.into()),
        }),
    ])
}

fn pv_string_object(object: &CanonicalObject) -> String {
    match object {
        CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        } => String::from_utf8(lexical.clone()).unwrap(),
        other => panic!("expected an xsd:string result object, got {other:?}"),
    }
}

fn pv_numeric_object(object: &CanonicalObject) -> (CanonicalDatatype, String) {
    match object {
        CanonicalObject::Value { datatype, lexical } => {
            (*datatype, String::from_utf8(lexical.clone()).unwrap())
        }
        other => panic!("expected a value result object, got {other:?}"),
    }
}

/// Prove+verify a number→string cast, returning the delivered xsd:string lexicals (sorted).
fn cast_to_string_outputs(layer: &LayerCommitment, query: &Query) -> Vec<String> {
    let compiled = plan_bgp(query).unwrap();
    let proved = compiled.prove(layer).unwrap();
    proved
        .verify(&compiled, layer, layer.resolution_root())
        .unwrap();
    let mut got: Vec<String> = proved
        .typecast_to_string_result_objects()
        .expect("expected a number→string Typecast proof")
        .iter()
        .map(pv_string_object)
        .collect();
    got.sort();
    got
}

/// Prove+verify a string→number cast, returning `(datatype, lexical)` per row (sorted by lexical).
fn cast_from_string_outputs(
    layer: &LayerCommitment,
    query: &Query,
) -> Vec<(CanonicalDatatype, String)> {
    let compiled = plan_bgp(query).unwrap();
    let proved = compiled.prove(layer).unwrap();
    proved
        .verify(&compiled, layer, layer.resolution_root())
        .unwrap();
    let mut got: Vec<(CanonicalDatatype, String)> = proved
        .typecast_from_string_result_objects()
        .expect("expected a string→number Typecast proof")
        .iter()
        .map(pv_numeric_object)
        .collect();
    got.sort();
    got
}

#[test]
fn typecast_integer_to_string_proves_base10() {
    // A fixed-width integer source cast to xsd:string is `format ~w` = base-10 (no leading zeros).
    let layer = typecast_single_predicate_layer(&[("n1", 100), ("n2", u32::MAX)]);
    let got = cast_to_string_outputs(&layer, &cast_query("p", "a", xsd_type_pv("string"), "r"));
    assert_eq!(got, sorted(vec!["100".to_string(), "4294967295".to_string()]));
}

#[test]
fn typecast_decimal_to_string_proves_terminating_and_20_digit_rounding() {
    // Terminating "3.5"/"12.34" render verbatim; a stored decimal with MORE than 20 fractional
    // digits renders ROUNDED to 20 (`rational_to_decimal_string_20`), matching casting.pl's
    // `rational_to_decimal_string(Val, S, 20)`.
    let layer = decimal_p_layer(&[
        ("n1", "3.5"),
        ("n2", "12.34"),
        ("n3", "0.333333333333333333333333"),
    ]);
    let got = cast_to_string_outputs(&layer, &cast_query("d", "dd", xsd_type_pv("string"), "r"));
    assert_eq!(
        got,
        sorted(vec![
            "3.5".to_string(),
            "12.34".to_string(),
            "0.33333333333333333333".to_string(),
        ])
    );
}

#[test]
fn typecast_number_to_string_wrong_result_fails_verification() {
    let layer = typecast_single_predicate_layer(&[("n1", 100), ("n2", 200)]);
    let compiled = plan_bgp(&cast_query("p", "a", xsd_type_pv("string"), "r")).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::TypecastNumberToString { result_objects, .. } =
        &mut tampered.statement
    {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: b"9999".to_vec(),
        };
    } else {
        panic!("expected a TypecastNumberToString proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn typecast_string_to_integer_proves_base10_grammar() {
    // `integer//1` grammar: leading zeros collapse ("007" -> 7), "-7" -> -7, plain "42" -> 42; all
    // delivered as the canonical base-10 xsd:integer (CanonicalDatatype::BigInt) lexical.
    let layer = string_p_layer(&[("n1", "42"), ("n2", "-7"), ("n3", "007")]);
    let got = cast_from_string_outputs(&layer, &cast_query("p", "s", xsd_type_pv("integer"), "r"));
    assert_eq!(
        got,
        sorted(vec![
            (CanonicalDatatype::BigInt, "42".to_string()),
            (CanonicalDatatype::BigInt, "-7".to_string()),
            (CanonicalDatatype::BigInt, "7".to_string()),
        ])
    );
}

#[test]
fn typecast_string_to_decimal_proves_terminating_and_20_digit_rounding() {
    // `string_decimal_to_rational` parse: "1.5"->3/2, "0.25"->1/4, and a 24-fractional-digit input
    // parses to an EXACT rational then renders ROUNDED to 20 digits (client-facing xsd:decimal).
    let layer = string_p_layer(&[
        ("n1", "1.5"),
        ("n2", "0.25"),
        ("n3", "0.333333333333333333333333"),
    ]);
    let got = cast_from_string_outputs(&layer, &cast_query("p", "s", xsd_type_pv("decimal"), "r"));
    assert_eq!(
        got,
        sorted(vec![
            (CanonicalDatatype::Decimal, "1.5".to_string()),
            (CanonicalDatatype::Decimal, "0.25".to_string()),
            (CanonicalDatatype::Decimal, "0.33333333333333333333".to_string()),
        ])
    );
}

#[test]
fn typecast_string_to_number_wrong_result_fails_verification() {
    let layer = string_p_layer(&[("n1", "1.5"), ("n2", "2.5")]);
    let compiled = plan_bgp(&cast_query("p", "s", xsd_type_pv("decimal"), "r")).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::TypecastStringToNumber { result_objects, .. } =
        &mut tampered.statement
    {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical: b"9.9".to_vec(),
        };
    } else {
        panic!("expected a TypecastStringToNumber proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn typecast_string_to_integer_malformed_fails_closed() {
    // "12a" is not `[+-]?[0-9]+`: SWI `phrase(integer(_),[])` fails -> `typecast` throws. The proof
    // must FAIL CLOSED (never fabricate a value like 12). >= 2 rows so it is not the row floor.
    let layer = string_p_layer(&[("n1", "7"), ("n2", "12a")]);
    let compiled = plan_bgp(&cast_query("p", "s", xsd_type_pv("integer"), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn typecast_string_to_decimal_scientific_fails_closed() {
    // "1e5" is parsed by SWI's `atom_number` into a FLOAT (1.0e+05), NOT a decimal/rational — a
    // DIFFERENT value/type. We reject-close scientific notation, so the proof FAILS CLOSED here
    // rather than fabricate 100000. (A trailing-dot "5." — which SWI also rejects — fails closed too.)
    let layer = string_p_layer(&[("n1", "1.5"), ("n2", "1e5")]);
    let compiled = plan_bgp(&cast_query("p", "s", xsd_type_pv("decimal"), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn typecast_string_to_decimal_trailing_dot_fails_closed() {
    // "5." (trailing dot) — SWI `atom_number("")` fails so `string_decimal_to_rational` fails and
    // `typecast` throws. We reject-close it; the proof FAILS CLOSED (never fabricate 5).
    let layer = string_p_layer(&[("n1", "1.5"), ("n2", "5.")]);
    let compiled = plan_bgp(&cast_query("p", "s", xsd_type_pv("decimal"), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `Concatenate` — end-to-end prove+verify via the VERIFIER-RECOMPUTE (byte) path.
// -------------------------------------------------------------------------------------------

/// A base layer with a `name` and a `greet` xsd:string per subject: `t(subj,name,a)`,
/// `t(subj,greet,b)`. Callers pass `>= 2` subjects (a query proof needs `>= 2` real rows for
/// power-of-two padding). One row below uses non-ASCII UTF-8 to show concat is byte-exact and
/// locale-independent (unlike the deferred upper/lower).
fn concat_name_greet_layer(rows: &[(&str, &str, &str)]) -> Arc<LayerCommitment> {
    let mut triples = Vec::new();
    for (subject, name, greet) in rows {
        triples.push(ValueTriple::new_string_value(
            &iri(subject),
            &iri("name"),
            name,
        ));
        triples.push(ValueTriple::new_string_value(
            &iri(subject),
            &iri("greet"),
            greet,
        ));
    }
    build_layer_triples(triples)
}

/// `t(?x,name,?a) & t(?x,greet,?b) & concat([?a, " ", ?b], ?r)` — includes a literal element.
fn concat_name_greet_query() -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Concatenate(Concatenate {
            list: ListOrVariable::List(vec![
                DataValue::Variable("a".into()),
                DataValue::Data(XSDAnySimpleType::String(" ".into())),
                DataValue::Variable("b".into()),
            ]),
            result_string: DataValue::Variable("r".into()),
        }),
    ])
}

fn concat_string(object: &CanonicalObject) -> String {
    match object {
        CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        } => String::from_utf8(lexical.clone()).unwrap(),
        other => panic!("unexpected concat result object {other:?}"),
    }
}

#[test]
fn concat_proves_per_row_concatenation_with_literal_separator() {
    let layer = concat_name_greet_layer(&[
        ("n1", "Hello", "World"),
        ("n2", "Foo", "Bar"),
        // Non-ASCII UTF-8: concat is a byte concatenation, so it must round-trip exactly.
        ("n3", "héllo", "wörld"),
    ]);
    let compiled = plan_bgp(&concat_name_greet_query()).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    // The synthesized strings are the terminal, revealed, verifier-re-derived output. Reading
    // them after a successful verify is sound (verify re-derives and requires an exact match).
    let mut got: Vec<String> = proved
        .concat_result_objects()
        .unwrap()
        .iter()
        .map(concat_string)
        .collect();
    got.sort();
    let mut expected = vec![
        "Hello World".to_string(),
        "Foo Bar".to_string(),
        "héllo wörld".to_string(),
    ];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn concat_tampered_result_string_fails_verification() {
    let layer = concat_name_greet_layer(&[("n1", "Hello", "World"), ("n2", "Foo", "Bar")]);
    let compiled = plan_bgp(&concat_name_greet_query()).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    // Replace one DELIVERED concatenation with a wrong string; the verifier RE-DERIVATION over
    // the root-authenticated operands rejects the mismatch.
    if let ProvedQueryStatement::Concat { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: b"TAMPERED".to_vec(),
        };
    } else {
        panic!("expected a Concat proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn concat_tampered_operand_column_fails_verification() {
    // Swapping an operand column's rows (with a rebuilt commitment) breaks the delivered columns'
    // agreement with the root-authenticated child; verify rejects (operands are pinned to root).
    let layer = concat_name_greet_layer(&[("n1", "Hello", "World"), ("n2", "Foo", "Bar")]);
    let compiled = plan_bgp(&concat_name_greet_query()).unwrap();
    let ai = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "a")
        .unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_columns[ai].swap(0, 1);
    let domain = 1usize << tampered.result_log_domain;
    tampered.result_commitments[ai] =
        commit_column(&pad(&tampered.result_columns[ai], domain)).unwrap();
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `Join` end-to-end faithfulness (separator-interposed concat).
// -------------------------------------------------------------------------------------------

/// `t(?x,name,?a) & t(?x,greet,?b) & join([?a, "END", ?b], "-", ?r)`. Exercises column elements,
/// a LITERAL element, and a literal separator interposed BETWEEN each pair — so the expected row
/// is `a + "-" + "END" + "-" + b` (intersperse then interpolate_string, `utils:join/3`).
fn join_name_greet_query() -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Join(Join {
            list: ListOrVariable::List(vec![
                DataValue::Variable("a".into()),
                DataValue::Data(XSDAnySimpleType::String("END".into())),
                DataValue::Variable("b".into()),
            ]),
            separator: DataValue::Data(XSDAnySimpleType::String("-".into())),
            result_string: DataValue::Variable("r".into()),
        }),
    ])
}

#[test]
fn join_proves_per_row_separator_interposition() {
    let layer = concat_name_greet_layer(&[
        ("n1", "Hello", "World"),
        ("n2", "Foo", "Bar"),
        // Non-ASCII UTF-8: join is a byte concatenation with the separator interposed, so it must
        // round-trip exactly (locale-independent, unlike the deferred upper/lower).
        ("n3", "héllo", "wörld"),
    ]);
    let compiled = plan_bgp(&join_name_greet_query()).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut got: Vec<String> = proved
        .join_result_objects()
        .unwrap()
        .iter()
        .map(concat_string)
        .collect();
    got.sort();
    let mut expected = vec![
        "Hello-END-World".to_string(),
        "Foo-END-Bar".to_string(),
        "héllo-END-wörld".to_string(),
    ];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn join_single_element_list_emits_no_separator() {
    // intersperse([e0]) = [e0] — the separator is NOT emitted for a single-element list.
    let layer = concat_name_greet_layer(&[("n1", "Hello", "World"), ("n2", "Foo", "Bar")]);
    let query = and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Join(Join {
            list: ListOrVariable::List(vec![DataValue::Variable("a".into())]),
            separator: DataValue::Data(XSDAnySimpleType::String("SEP".into())),
            result_string: DataValue::Variable("r".into()),
        }),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let mut got: Vec<String> = proved
        .join_result_objects()
        .unwrap()
        .iter()
        .map(concat_string)
        .collect();
    got.sort();
    // Just the `name` column verbatim — no separator.
    assert_eq!(got, vec!["Foo".to_string(), "Hello".to_string()]);
}

#[test]
fn join_tampered_result_string_fails_verification() {
    let layer = concat_name_greet_layer(&[("n1", "Hello", "World"), ("n2", "Foo", "Bar")]);
    let compiled = plan_bgp(&join_name_greet_query()).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::Join { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: b"TAMPERED".to_vec(),
        };
    } else {
        panic!("expected a Join proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn join_tampered_operand_column_fails_verification() {
    let layer = concat_name_greet_layer(&[("n1", "Hello", "World"), ("n2", "Foo", "Bar")]);
    let compiled = plan_bgp(&join_name_greet_query()).unwrap();
    let ai = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "a")
        .unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_columns[ai].swap(0, 1);
    let domain = 1usize << tampered.result_log_domain;
    tampered.result_commitments[ai] =
        commit_column(&pad(&tampered.result_columns[ai], domain)).unwrap();
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `Substring` end-to-end faithfulness (per-row constant-index CHARACTER substring).
// -------------------------------------------------------------------------------------------

/// A base layer with a `word` xsd:string per subject: `t(subj, word, w)`.
fn word_layer(rows: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let mut triples = Vec::new();
    for (subject, word) in rows {
        triples.push(ValueTriple::new_string_value(
            &iri(subject),
            &iri("word"),
            word,
        ));
    }
    build_layer_triples(triples)
}

/// `t(?x, word, ?s) & sub_string(?s, before, length, ?after, ?sub)`.
fn substring_word_query(before: usize, length: usize) -> Query {
    and_of(vec![
        tv("x", "word", "s"),
        Query::Substring(Substring {
            string: DataValue::Variable("s".into()),
            before: DataValue::Data(XSDAnySimpleType::UnsignedInt(before)),
            length: DataValue::Data(XSDAnySimpleType::UnsignedInt(length)),
            after: DataValue::Variable("after".into()),
            substring: DataValue::Variable("sub".into()),
        }),
    ])
}

#[test]
fn substring_proves_per_row_character_indexed_slice() {
    // Every string has a MULTI-BYTE character (é/ö/ü) at code-point index 1, BEFORE the slice
    // window [1,4). Char-slicing yields "éll"/"örl"/"üen"; a naive BYTE slice would cut the
    // 2-byte UTF-8 sequence and differ — so a passing proof demonstrates CODE-POINT indexing.
    let layer = word_layer(&[
        ("w1", "héllo"), // chars: h é l l o  -> [1,4) = "éll"
        ("w2", "wörld"), // chars: w ö r l d  -> [1,4) = "örl"
        ("w3", "füenf"), // chars: f ü e n f  -> [1,4) = "üen"
    ]);
    let compiled = plan_bgp(&substring_word_query(1, 3)).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut got: Vec<String> = proved
        .substring_result_objects()
        .unwrap()
        .iter()
        .map(concat_string)
        .collect();
    got.sort();
    let mut expected = vec!["éll".to_string(), "örl".to_string(), "üen".to_string()];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn substring_tampered_result_fails_verification() {
    let layer = word_layer(&[("w1", "héllo"), ("w2", "wörld")]);
    let compiled = plan_bgp(&substring_word_query(1, 3)).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::Substring { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: "xxx".as_bytes().to_vec(),
        };
    } else {
        panic!("expected a Substring proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn substring_out_of_range_fails_closed() {
    // "abc" has 3 chars; before+length = 1+5 = 6 > 3 => reference sub_string FAILS on that row.
    // We FAIL CLOSED at prove time (never fabricate an out-of-range substring). >= 2 real rows.
    let layer = word_layer(&[("w1", "abc"), ("w2", "abcdef")]);
    let compiled = plan_bgp(&substring_word_query(1, 5)).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `Pad` end-to-end faithfulness (per-row constant single-char left-pad to a target length).
// -------------------------------------------------------------------------------------------

/// `t(?x, word, ?s) & pad(?s, pad_char, target_len, ?r)`.
fn pad_word_query(pad_char: &str, target_len: usize) -> Query {
    and_of(vec![
        tv("x", "word", "s"),
        Query::Pad(Pad {
            string: DataValue::Variable("s".into()),
            char: DataValue::Data(XSDAnySimpleType::String(pad_char.into())),
            times: DataValue::Data(XSDAnySimpleType::UnsignedInt(target_len)),
            result_string: DataValue::Variable("r".into()),
        }),
    ])
}

#[test]
fn pad_proves_per_row_left_pad_to_target_character_length() {
    // Multi-BYTE pad char '→' (U+2192, 3 bytes, 1 CODE POINT) and a multi-BYTE input char 'é'
    // (2 bytes, 1 code point) at target CHARACTER length 4:
    //   "é"    -> 1 char  -> prepend 3 '→' -> "→→→é"  (a BYTE-length impl would prepend only 2 and
    //                                                   differ, so a passing proof proves CHAR-length)
    //   "ab"   -> 2 chars -> prepend 2 '→' -> "→→ab"
    //   "wxyz" -> 4 chars -> prepend 0     -> "wxyz"  (L2 == 0 boundary: operand unchanged)
    let layer = word_layer(&[("w1", "é"), ("w2", "ab"), ("w3", "wxyz")]);
    let compiled = plan_bgp(&pad_word_query("→", 4)).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut got: Vec<String> = proved
        .pad_result_objects()
        .unwrap()
        .iter()
        .map(concat_string)
        .collect();
    got.sort();
    let mut expected = vec![
        "→→→é".to_string(),
        "→→ab".to_string(),
        "wxyz".to_string(),
    ];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn pad_tampered_result_fails_verification() {
    let layer = word_layer(&[("w1", "é"), ("w2", "ab")]);
    let compiled = plan_bgp(&pad_word_query("→", 4)).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::Pad { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: b"xxxx".to_vec(),
        };
    } else {
        panic!("expected a Pad proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn pad_input_longer_than_target_fails_closed() {
    // "abcdef" has 6 chars; target 3 => L2 = 3 - 6 < 0 => reference utils:pad throws bad_pad_length.
    // We FAIL CLOSED at prove time (never fabricate a truncation). >= 2 real rows.
    let layer = word_layer(&[("w1", "abcdef"), ("w2", "ab")]);
    let compiled = plan_bgp(&pad_word_query("0", 3)).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

// -------------------------------------------------------------------------------------------
// WOQL `HashKey`/`hash` end-to-end faithfulness (per-row `idgen_hash` document-ID).
// Expected ids are CROSS-VALIDATED against the reference `idgen_hash/3` (SWI) — see the proof
// module's `expected_hashkey_objects` for the exact recipe.
// -------------------------------------------------------------------------------------------

/// Extract a revealed `CanonicalObject::Node` (a URI/id) as a String.
fn node_string(object: &CanonicalObject) -> String {
    match object {
        CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("unexpected hashkey result object {other:?}"),
    }
}

/// `t(?x, word, ?s) & hash(base, [?s], ?id)` — single authenticated arg.
fn hashkey_word_query(base: &str) -> Query {
    and_of(vec![
        tv("x", "word", "s"),
        Query::HashKey(HashKey {
            base: DataValue::Data(XSDAnySimpleType::String(base.into())),
            key_list: vec![DataValue::Variable("s".into())],
            uri: NodeValue::Variable("id".into()),
        }),
    ])
}

/// `t(?x, name, ?a) & t(?x, greet, ?b) & hash(base, [?a, ?b], ?id)` — two authenticated args.
fn hashkey_name_greet_query(base: &str) -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::HashKey(HashKey {
            base: DataValue::Data(XSDAnySimpleType::String(base.into())),
            key_list: vec![
                DataValue::Variable("a".into()),
                DataValue::Variable("b".into()),
            ],
            uri: NodeValue::Variable("id".into()),
        }),
    ])
}

#[test]
fn hashkey_proves_per_row_idgen_hash_matching_reference() {
    // base "P/", single arg = the word (all-passthrough, so suffix == the word):
    //   hello -> "P/" + sha256("hello");  world -> "P/" + sha256("world").
    let layer = word_layer(&[("w1", "hello"), ("w2", "world")]);
    let compiled = plan_bgp(&hashkey_word_query("P/")).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut got: Vec<String> = proved
        .hashkey_result_objects()
        .unwrap()
        .iter()
        .map(node_string)
        .collect();
    got.sort();
    let mut expected = vec![
        "P/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string(),
        "P/486ea46224d1bb4fb680f34f7c9ad96a8f24ec88be73ea8e5a6c65260e9cb8a7".to_string(),
    ];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn hashkey_proves_idgen_encoding_of_special_chars_matching_reference() {
    // Two args exercise `encode_id_fragment`: '+' -> %2B, ' ' -> %20, 'é' -> %C3%A9 (per-UTF-8-byte,
    // UPPERCASE hex), fragments joined by a literal '+'. Cross-validated against reference idgen_hash:
    //   ["a+b","c d"] -> suffix "a%2Bb+c%20d" -> "Doc/2fa05155..."
    //   ["café","x"]  -> suffix "caf%C3%A9+x" -> "Doc/2d3ff81b..."
    let layer = concat_name_greet_layer(&[("n1", "a+b", "c d"), ("n2", "café", "x")]);
    let compiled = plan_bgp(&hashkey_name_greet_query("Doc/")).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    proved
        .verify(&compiled, &layer, layer.resolution_root())
        .unwrap();

    let mut got: Vec<String> = proved
        .hashkey_result_objects()
        .unwrap()
        .iter()
        .map(node_string)
        .collect();
    got.sort();
    let mut expected = vec![
        "Doc/2fa05155d278d27d8be398a8828abf346db29fdd91780951fa72fd8ef2220dd2".to_string(),
        "Doc/2d3ff81b3c1ce688026368c20da8dd7395fbc4035d74e0abe4b44effaac26f0c".to_string(),
    ];
    expected.sort();
    assert_eq!(got, expected);
}

#[test]
fn hashkey_tampered_result_fails_verification() {
    let layer = word_layer(&[("w1", "hello"), ("w2", "world")]);
    let compiled = plan_bgp(&hashkey_word_query("P/")).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    // Replace one DELIVERED id with a wrong node; the verifier RE-DERIVATION rejects the mismatch.
    if let ProvedQueryStatement::HashKey { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Node(b"P/deadbeef".to_vec());
    } else {
        panic!("expected a HashKey proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn hashkey_tampered_operand_column_fails_verification() {
    // Swapping the arg column's rows (with a rebuilt commitment) breaks its agreement with the
    // root-authenticated child; verify rejects (the operand bytes are pinned to the trusted root).
    let layer = word_layer(&[("w1", "hello"), ("w2", "world")]);
    let compiled = plan_bgp(&hashkey_word_query("P/")).unwrap();
    let si = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "s")
        .unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_columns[si].swap(0, 1);
    let domain = 1usize << tampered.result_log_domain;
    tampered.result_commitments[si] =
        commit_column(&pad(&tampered.result_columns[si], domain)).unwrap();
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// =============================================================================================
// WOQL exact-rational (xsd:decimal) value layer: Floor / Divide / decimal operands.
// =============================================================================================

use decimal_rs::Decimal as RsDecimal;
use std::str::FromStr as _;

/// A base layer of `t(?x, d, <decimal>)` triples (stored xsd:decimal values).
fn decimal_d_layer(rows: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, dec) in rows {
        let entry = Decimal::make_entry(&Decimal::new((*dec).to_string()).unwrap());
        builder
            .add_value_triple(ValueTriple::new_value(&iri(subject), &iri("d"), entry))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn ea_divide(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Divide(Divide {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn ea_floor(argument: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Floor(Floor {
        argument: Box::new(argument),
    })
}

fn ea_dec(lexical: &str) -> ArithmeticExpression {
    ArithmeticExpression::Value(ArithmeticValue::Data(XSDAnySimpleType::Decimal(
        RsDecimal::from_str(lexical).unwrap(),
    )))
}

/// `t(?x,d,?dd) & Eval(expr, ?result)`.
fn eval_d_query(expr: ArithmeticExpression, result: &str) -> Query {
    and_of(vec![
        tv("x", "d", "dd"),
        Query::Eval(Eval {
            expression: expr,
            result_value: ArithmeticValue::Variable(result.into()),
        }),
    ])
}

fn decimal_object_string(object: &CanonicalObject) -> String {
    match object {
        CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical,
        } => String::from_utf8(lexical.clone()).unwrap(),
        other => panic!("unexpected rational-arith result object {other:?}"),
    }
}

/// Prove+verify a `RationalArith` query and return the delivered per-row xsd:decimal lexicals
/// (as a sorted multiset, since row order is not observed here).
fn rational_arith_strings(layer: &LayerCommitment, query: &Query) -> (CompiledBgp, Vec<String>) {
    let compiled = plan_bgp(query).unwrap();
    let proved = compiled.prove(layer).unwrap();
    proved
        .verify(&compiled, layer, layer.resolution_root())
        .unwrap();
    let mut got: Vec<String> = proved
        .rational_arith_result_objects()
        .expect("expected a RationalArith proof")
        .iter()
        .map(decimal_object_string)
        .collect();
    got.sort();
    (compiled, got)
}

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
}

// ---- Floor ----

#[test]
fn floor_of_integer_column_proves_identity() {
    // floor of an integer column is the identity; delivered as the generated-i64 result column.
    let layer = eval_two_predicate_layer(&[("n1", 7, 2), ("n2", 3, 5)]);
    let (_compiled, rows) = eval_ab_rows(&layer, ea_floor(ea_var("a")));
    assert_eq!(rows.len(), 2);
    for (a, _b, r) in rows {
        assert_eq!(r, i64::from(a));
    }
}

#[test]
fn floor_of_decimal_column_floors_toward_negative_infinity() {
    // floor(3.5)=3, floor(-3.5)=-4 (toward −∞, NOT toward zero), floor(5)=5.
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "-3.5"), ("n3", "5")]);
    let compiled = plan_bgp(&eval_d_query(ea_floor(ea_var("dd")), "r")).unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let ri = compiled
        .relation
        .schema
        .iter()
        .position(|v| v == "r")
        .unwrap();
    let got: Vec<i64> = values[ri]
        .iter()
        .map(|v| match v {
            CanonicalResultValue::GeneratedDecimalI64(x) => *x,
            other => panic!("unexpected floor result {other:?}"),
        })
        .collect();
    assert_eq!(sorted(got), vec![-4, 3, 5]);
}

#[test]
fn floor_wrong_result_fails_verification() {
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "-3.5")]);
    let compiled = plan_bgp(&eval_d_query(ea_floor(ea_var("dd")), "r")).unwrap();
    let ri = compiled
        .relation
        .schema
        .iter()
        .position(|v| v == "r")
        .unwrap();
    let mut wrong = compiled.prove(&layer).unwrap();
    // A wrong floored value with a rebuilt (internally consistent) commitment still fails the
    // verifier RE-DERIVATION.
    wrong.result_columns[ri][0] = generated_decimal_i64_field(999);
    let domain = 1usize << wrong.result_log_domain;
    wrong.result_commitments[ri] = commit_column(&pad(&wrong.result_columns[ri], domain)).unwrap();
    assert!(wrong
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// ---- Divide ----

#[test]
fn divide_proves_terminating_and_nonterminating_decimals() {
    // Terminating 7/2=3.5, 6/2=3, 1/4=0.25; non-terminating 1/3 rendered to the reference's exact
    // 20-fractional-digit lexical "0.33333333333333333333".
    let layer = eval_two_predicate_layer(&[("n1", 7, 2), ("n2", 6, 2), ("n3", 1, 4), ("n4", 1, 3)]);
    let (_compiled, got) =
        rational_arith_strings(&layer, &eval_ab_query(ea_divide(ea_var("a"), ea_var("b")), "r"));
    assert_eq!(
        got,
        sorted(vec![
            "3.5".to_string(),
            "3".to_string(),
            "0.25".to_string(),
            "0.33333333333333333333".to_string(),
        ])
    );
}

#[test]
fn divide_by_zero_column_fails_at_prove() {
    // A runtime zero divisor (b=0 in one row) FAILS CLOSED at prove — Prolog `rdiv` throws.
    let layer = eval_two_predicate_layer(&[("n1", 7, 2), ("n2", 5, 0)]);
    let compiled = plan_bgp(&eval_ab_query(ea_divide(ea_var("a"), ea_var("b")), "r")).unwrap();
    assert!(compiled.prove(&layer).is_err());
}

#[test]
fn divide_tampered_result_fails_verification() {
    let layer = eval_two_predicate_layer(&[("n1", 7, 2), ("n2", 1, 4)]);
    let compiled = plan_bgp(&eval_ab_query(ea_divide(ea_var("a"), ea_var("b")), "r")).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::RationalArith { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical: b"9.9".to_vec(),
        };
    } else {
        panic!("expected a RationalArith proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// ---- Decimal operands in Plus / Times ----

#[test]
fn plus_with_stored_decimal_operand_proves() {
    // Plus(?d, 1.5): 3.5+1.5=5 (whole -> "5"), 2.5+1.5=4 (whole -> "4").
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "2.5")]);
    let (_compiled, got) = rational_arith_strings(
        &layer,
        &eval_d_query(ea_plus(ea_var("dd"), ea_dec("1.5")), "r"),
    );
    assert_eq!(got, sorted(vec!["5".to_string(), "4".to_string()]));
}

#[test]
fn times_with_stored_decimal_operand_proves() {
    // Times(?d, 2.5): 3.5*2.5=8.75, 2.5*2.5=6.25.
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "2.5")]);
    let (_compiled, got) = rational_arith_strings(
        &layer,
        &eval_d_query(ea_times(ea_var("dd"), ea_dec("2.5")), "r"),
    );
    assert_eq!(got, sorted(vec!["8.75".to_string(), "6.25".to_string()]));
}

#[test]
fn divide_two_stored_decimal_operands_proves() {
    // Divide(?d, 2): 3.5/2=1.75, 7.5/2=3.75 — two stored decimals authenticated and divided.
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "7.5")]);
    let (_compiled, got) = rational_arith_strings(
        &layer,
        &eval_d_query(ea_divide(ea_var("dd"), ea_dec("2")), "r"),
    );
    assert_eq!(got, sorted(vec!["1.75".to_string(), "3.75".to_string()]));
}

#[test]
fn decimal_operand_tampered_result_fails_verification() {
    let layer = decimal_d_layer(&[("n1", "3.5"), ("n2", "2.5")]);
    let compiled = plan_bgp(&eval_d_query(ea_times(ea_var("dd"), ea_dec("2.5")), "r")).unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::RationalArith { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical: b"0.1".to_vec(),
        };
    } else {
        panic!("expected a RationalArith proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// ============================================================================================
// WOQL single-input CALENDAR ops — end-to-end prove+verify over committed date layers.
// Expected values are the SWI-Prolog reference vectors, differentially validated against a live
// SWI-Prolog 10.0.2 (weekday numbering, iso_week boundaries, day arithmetic, month boundaries).
// ============================================================================================

/// An `xsd:gYearMonth` typed dict entry (offset 0 for the common case).
fn gym_entry(y: i64, mo: u8, offset: i16) -> TypedDictEntry {
    tdb_succinct::GYearMonth::make_entry(&tdb_succinct::GYearMonth {
        year: y,
        month: mo,
        offset,
    })
}

/// Decode a resolved `xsd:date` operand value to `(year, month, day)`.
fn cal_date_of(value: &CanonicalResultValue) -> (i64, u8, u8) {
    match value {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::Date,
            lexical,
        }) => {
            use tdb_succinct::{Date, FromLexical};
            let date = <Date as FromLexical<Date>>::from_lexical(&lexical[..]);
            (date.year, date.month, date.day)
        }
        other => panic!("unexpected operand value {other:?}"),
    }
}

/// Decode a revealed calendar DATE result object to its client-facing `YYYY-MM-DD` string.
fn cal_date_string(object: &CanonicalObject) -> String {
    match object {
        CanonicalObject::Value {
            datatype: CanonicalDatatype::Date,
            lexical,
        } => String::from_utf8(lexical.clone()).unwrap(),
        other => panic!("unexpected calendar date result {other:?}"),
    }
}

/// A layer of `xsd:date` values under predicate `d` (offset 0), one per subject.
fn cal_date_layer(rows: &[(&str, i64, u8, u8)]) -> Arc<LayerCommitment> {
    build_val_name_layer(
        "d",
        rows.iter()
            .map(|(subject, y, mo, day)| (*subject, date_entry_off(*y, *mo, *day, 0)))
            .collect(),
    )
}

/// Prove+verify a single INTEGER-result calendar op over the `d` date layer, returning per-row
/// `((y, mo, d), result_i64)` pairs (operand date paired with the derived integer).
fn cal_int_rows(
    layer: &LayerCommitment,
    calendar: Query,
    result_var: &str,
) -> Vec<((i64, u8, u8), i64)> {
    let compiled = plan_bgp(&and_of(vec![tv("x", "d", "date"), calendar])).unwrap();
    let proved = compiled.prove(layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, layer, layer.resolution_root())
        .unwrap();
    let index = |name: &str| {
        compiled
            .relation
            .schema
            .iter()
            .position(|variable| variable == name)
            .unwrap()
    };
    let (di, ri) = (index("date"), index(result_var));
    (0..values[ri].len())
        .map(|row| (cal_date_of(&values[di][row]), eval_i64(&values[ri][row])))
        .collect()
}

/// Prove+verify a single DATE-result calendar op, returning the SORTED revealed result date strings.
fn cal_date_out_sorted(layer: &LayerCommitment, pred: &str, calendar: Query) -> Vec<String> {
    let compiled = plan_bgp(&and_of(vec![tv("x", pred, "date"), calendar])).unwrap();
    let proved = compiled.prove(layer).unwrap();
    proved
        .verify(&compiled, layer, layer.resolution_root())
        .unwrap();
    let mut got: Vec<String> = proved
        .calendar_date_result_objects()
        .unwrap()
        .iter()
        .map(cal_date_string)
        .collect();
    got.sort();
    got
}

#[test]
fn weekday_proves_iso_monday_to_sunday() {
    // 2024-01-01 Mon=1, 2024-01-06 Sat=6, 2024-01-07 Sun=7, 2024-02-29 (leap) Thu=4.
    let layer = cal_date_layer(&[
        ("mon", 2024, 1, 1),
        ("sat", 2024, 1, 6),
        ("sun", 2024, 1, 7),
        ("thu", 2024, 2, 29),
    ]);
    let mut rows = cal_int_rows(
        &layer,
        Query::Weekday(Weekday {
            date: dvar("date"),
            weekday: dvar("w"),
        }),
        "w",
    );
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ((2024, 1, 1), 1),
            ((2024, 1, 6), 6),
            ((2024, 1, 7), 7),
            ((2024, 2, 29), 4),
        ]
    );
}

#[test]
fn weekday_sunday_start_proves_us_sunday_one() {
    // Sunday=1..Saturday=7: 2024-01-07 Sun=1, 2024-01-06 Sat=7, 2024-01-01 Mon=2.
    let layer = cal_date_layer(&[("sun", 2024, 1, 7), ("sat", 2024, 1, 6), ("mon", 2024, 1, 1)]);
    let mut rows = cal_int_rows(
        &layer,
        Query::WeekdaySundayStart(WeekdaySundayStart {
            date: dvar("date"),
            weekday: dvar("w"),
        }),
        "w",
    );
    rows.sort();
    assert_eq!(
        rows,
        vec![((2024, 1, 1), 2), ((2024, 1, 6), 7), ((2024, 1, 7), 1)]
    );
}

#[test]
fn weekday_wrong_result_column_fails_verification() {
    let layer = cal_date_layer(&[("mon", 2024, 1, 1), ("sun", 2024, 1, 7)]);
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "d", "date"),
        Query::Weekday(Weekday {
            date: dvar("date"),
            weekday: dvar("w"),
        }),
    ]))
    .unwrap();
    let wi = compiled
        .relation
        .schema
        .iter()
        .position(|variable| variable == "w")
        .unwrap();
    // A wrong derived weekday WITH a rebuilt (internally consistent) commitment: the VERIFIER
    // RE-DERIVATION rejects it (delivered result must equal the recomputed weekday).
    let mut wrong = compiled.prove(&layer).unwrap();
    wrong.result_columns[wi][0] = generated_decimal_i64_field(5);
    let domain = 1usize << wrong.result_log_domain;
    wrong.result_commitments[wi] = commit_column(&pad(&wrong.result_columns[wi], domain)).unwrap();
    assert!(wrong
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

#[test]
fn iso_week_proves_year_and_week_across_boundaries() {
    // 2024-01-01 -> 2024/w1; 2024-12-30 -> 2025/w1; 2023-01-01 -> 2022/w52; 2020-12-31 -> 2020/w53.
    let layer = cal_date_layer(&[
        ("a", 2024, 1, 1),
        ("b", 2024, 12, 30),
        ("c", 2023, 1, 1),
        ("e", 2020, 12, 31),
    ]);
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "d", "date"),
        Query::IsoWeek(IsoWeek {
            date: dvar("date"),
            week: dvar("wk"),
            year: dvar("yr"),
        }),
    ]))
    .unwrap();
    let proved = compiled.prove(&layer).unwrap();
    let values = proved
        .verify_and_resolve_values(&compiled, &layer, layer.resolution_root())
        .unwrap();
    let index = |name: &str| {
        compiled
            .relation
            .schema
            .iter()
            .position(|variable| variable == name)
            .unwrap()
    };
    let (di, yi, wi) = (index("date"), index("yr"), index("wk"));
    let mut rows: Vec<((i64, u8, u8), (i64, i64))> = (0..values[di].len())
        .map(|row| {
            (
                cal_date_of(&values[di][row]),
                (eval_i64(&values[yi][row]), eval_i64(&values[wi][row])),
            )
        })
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ((2020, 12, 31), (2020, 53)),
            ((2023, 1, 1), (2022, 52)),
            ((2024, 1, 1), (2024, 1)),
            ((2024, 12, 30), (2025, 1)),
        ]
    );
}

#[test]
fn day_after_proves_across_month_and_year_and_leap_boundaries() {
    let layer = cal_date_layer(&[
        ("eom", 2024, 1, 31),  // -> 2024-02-01
        ("eoy", 2024, 12, 31), // -> 2025-01-01
        ("leap", 2024, 2, 28), // -> 2024-02-29
        ("mid", 2024, 3, 1),   // -> 2024-03-02
    ]);
    let got = cal_date_out_sorted(
        &layer,
        "d",
        Query::DayAfter(DayAfter {
            date: dvar("date"),
            next: dvar("n"),
        }),
    );
    assert_eq!(
        got,
        vec![
            "2024-02-01".to_string(),
            "2024-02-29".to_string(),
            "2024-03-02".to_string(),
            "2025-01-01".to_string(),
        ]
    );
}

#[test]
fn day_before_proves_across_month_and_leap_boundaries() {
    let layer = cal_date_layer(&[
        ("bom", 2024, 1, 31),  // -> 2024-01-30
        ("boy", 2024, 12, 31), // -> 2024-12-30
        ("feb", 2024, 2, 28),  // -> 2024-02-27
        ("mar1", 2024, 3, 1),  // -> 2024-02-29 (leap)
    ]);
    let got = cal_date_out_sorted(
        &layer,
        "d",
        Query::DayBefore(DayBefore {
            date: dvar("date"),
            previous: dvar("p"),
        }),
    );
    assert_eq!(
        got,
        vec![
            "2024-01-30".to_string(),
            "2024-02-27".to_string(),
            "2024-02-29".to_string(),
            "2024-12-30".to_string(),
        ]
    );
}

#[test]
fn day_after_wrong_result_fails_verification() {
    let layer = cal_date_layer(&[("eom", 2024, 1, 31), ("eoy", 2024, 12, 31)]);
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "d", "date"),
        Query::DayAfter(DayAfter {
            date: dvar("date"),
            next: dvar("n"),
        }),
    ]))
    .unwrap();
    // Mutate one revealed result date; the VERIFIER RE-DERIVATION rejects it.
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::CalendarDate { result_objects, .. } = &mut tampered.statement {
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::Date,
            lexical: b"1999-09-09".to_vec(),
        };
    } else {
        panic!("expected a CalendarDate proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

/// A layer of `xsd:gYearMonth` values under predicate `ym` (offset 0), one per subject.
fn cal_yearmonth_layer(rows: &[(&str, i64, u8)]) -> Arc<LayerCommitment> {
    build_val_name_layer(
        "ym",
        rows.iter()
            .map(|(subject, y, mo)| (*subject, gym_entry(*y, *mo, 0)))
            .collect(),
    )
}

#[test]
fn month_start_date_proves_first_of_month() {
    let layer = cal_yearmonth_layer(&[("jan", 2024, 1), ("feb", 2024, 2), ("dec", 2024, 12)]);
    let got = cal_date_out_sorted(
        &layer,
        "ym",
        Query::MonthStartDate(MonthStartDate {
            year_month: dvar("date"),
            date: dvar("out"),
        }),
    );
    assert_eq!(
        got,
        vec![
            "2024-01-01".to_string(),
            "2024-02-01".to_string(),
            "2024-12-01".to_string(),
        ]
    );
}

#[test]
fn month_end_date_proves_last_of_month_with_leap_rules() {
    // 2024-01 -> 31; 2024-02 (leap) -> 29; 2023-02 (non-leap) -> 28; 2024-12 -> 31.
    let layer = cal_yearmonth_layer(&[
        ("jan", 2024, 1),
        ("febL", 2024, 2),
        ("febN", 2023, 2),
        ("dec", 2024, 12),
    ]);
    let got = cal_date_out_sorted(
        &layer,
        "ym",
        Query::MonthEndDate(MonthEndDate {
            year_month: dvar("date"),
            date: dvar("out"),
        }),
    );
    assert_eq!(
        got,
        vec![
            "2023-02-28".to_string(),
            "2024-01-31".to_string(),
            "2024-02-29".to_string(),
            "2024-12-31".to_string(),
        ]
    );
}

#[test]
fn month_end_date_wrong_result_fails_verification() {
    let layer = cal_yearmonth_layer(&[("febL", 2024, 2), ("jan", 2024, 1)]);
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "ym", "date"),
        Query::MonthEndDate(MonthEndDate {
            year_month: dvar("date"),
            date: dvar("out"),
        }),
    ]))
    .unwrap();
    let mut tampered = compiled.prove(&layer).unwrap();
    if let ProvedQueryStatement::CalendarDate { result_objects, .. } = &mut tampered.statement {
        // Claim 2024-02 ends on the 28th (it is a leap year → 29): recompute rejects.
        result_objects[0] = CanonicalObject::Value {
            datatype: CanonicalDatatype::Date,
            lexical: b"2024-02-28".to_vec(),
        };
    } else {
        panic!("expected a CalendarDate proof");
    }
    assert!(tampered
        .verify(&compiled, &layer, layer.resolution_root())
        .is_err());
}

// ============================================================================
// Finite REPEATED-PREDICATE property paths (`Times{n,n}` / `Times{a,b}` / p^n).
//
// A single-predicate repeated path lowers to a conjunctive bag-join plus a
// `TupleDisequality` EDGE-SIMPLE filter (drop rows where a hop-pair reuses a stored
// edge) and endpoint projection. These e2e tests prove the compiled plan against a
// committed layer, VERIFY, and check the resolved result equals the EDGE-SIMPLE bag
// of the leading-Prolog `path.pl` oracle — INCLUDING node-repeating solutions
// (`a->b->a`) and EXCLUDING only the self-loop-traversed-twice over-count.
// ============================================================================

/// The self-loop fixture: edges (all predicate `p`) a->a (self-loop), a->b, b->a, b->c.
fn self_loop_layer() -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, o) in [("a", "a"), ("a", "b"), ("b", "a"), ("b", "c")] {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri("p"), &iri(o)))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// `path(subject, Times(p, from, to), object)`.
fn times_path(subject: Value, from: u64, to: u64, object: Value) -> Query {
    Query::Path(Path {
        subject,
        pattern: PathPattern::Times(PathTimes {
            times: Box::new(PathPattern::Predicate(PathPredicate {
                predicate: Some(iri("p")),
            })),
            from,
            to,
        }),
        object,
        path: None,
    })
}

#[test]
fn path_times_two_two_constant_source_drops_self_loop_twice() {
    // KEY test. `path(a, p^2, ?t)` over the self-loop graph. Edge-simple 2-hop bag from a:
    //   a->a->b  (edges a->a, a->b — distinct)   -> t=b
    //   a->b->a  (edges a->b, b->a — distinct)   -> t=a   (NODE repeat a..a kept!)
    //   a->b->c  (edges a->b, b->c — distinct)   -> t=c
    // EXCLUDED: a->a->a via the SAME edge (a,p,a) twice. A plain bag-join returns 4 rows
    // (t = a,a,b,c); the TupleDisequality drops exactly the self-loop-twice t=a.
    let layer = self_loop_layer();
    let root = layer.resolution_root();

    let compiled = plan_bgp(&times_path(
        Value::Node(iri("a")),
        2,
        2,
        Value::Variable("t".into()),
    ))
    .unwrap();
    assert_eq!(compiled.relation.schema, ["t"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 3, "edge-simple drops exactly the self-loop-twice row");
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(
        join_rows(&values),
        vec![vec![iri("a")], vec![iri("b")], vec![iri("c")]],
        "endpoints t = {{a (a->b->a), b (a->a->b), c (a->b->c)}}"
    );

    // Envelope round-trips and re-verifies.
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

#[test]
fn path_times_two_two_variable_source_edge_simple_bag() {
    // `path(?s, p^2, ?t)`: the WHOLE-graph edge-simple 2-hop bag.
    //   a->a->b (a,b) | a->b->a (a,a) | a->b->c (a,c) | b->a->a (b,a) | b->a->b (b,b)
    // (a->a->a via edge (a,p,a) twice is EXCLUDED.)
    let layer = self_loop_layer();
    let root = layer.resolution_root();
    let compiled = plan_bgp(&times_path(
        Value::Variable("s".into()),
        2,
        2,
        Value::Variable("t".into()),
    ))
    .unwrap();
    assert_eq!(compiled.relation.schema, ["s", "t"]);
    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 5);
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(
        join_rows(&values),
        vec![
            vec![iri("a"), iri("a")],
            vec![iri("a"), iri("b")],
            vec![iri("a"), iri("c")],
            vec![iri("b"), iri("a")],
            vec![iri("b"), iri("b")],
        ]
    );
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

/// Fixture for the length-3 / range tests: self-loop at a plus a linear tail a->b->c.
fn loop_tail_layer() -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, o) in [("a", "a"), ("a", "b"), ("b", "c")] {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri("p"), &iri(o)))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

#[test]
fn path_times_three_three_edge_simple_bag() {
    // `path(a, p^3, ?t)` over {a->a, a->b, b->c}. The only edge-simple 3-hop path from a is
    //   a->a->b->c  (edges a->a, a->b, b->c all distinct)  -> t=c.
    // The reuse walks a->a->a->{a,b} (edge a->a twice) are dropped by the C(3,2)=3 filters.
    let layer = loop_tail_layer();
    let root = layer.resolution_root();
    let compiled = plan_bgp(&times_path(
        Value::Node(iri("a")),
        3,
        3,
        Value::Variable("t".into()),
    ))
    .unwrap();
    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 1);
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(join_rows(&values), vec![vec![iri("c")]]);
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

#[test]
fn path_times_range_two_three_is_bag_union() {
    // `path(a, Times{2,3}, ?t)` over {a->a, a->b, b->c}: the BAG union of the exact lengths.
    //   p^2 from a (edge-simple): a->a->b (t=b), a->b->c (t=c)
    //   p^3 from a (edge-simple): a->a->b->c (t=c)
    // Bag union -> {b, c, c}.
    let layer = loop_tail_layer();
    let root = layer.resolution_root();
    let compiled = plan_bgp(&times_path(
        Value::Node(iri("a")),
        2,
        3,
        Value::Variable("t".into()),
    ))
    .unwrap();
    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 3);
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(
        join_rows(&values),
        vec![vec![iri("b")], vec![iri("c")], vec![iri("c")]]
    );
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

#[test]
fn path_times_two_two_soundness_negatives() {
    let layer = self_loop_layer();
    let root = layer.resolution_root();
    let compiled = plan_bgp(&times_path(
        Value::Node(iri("a")),
        2,
        2,
        Value::Variable("t".into()),
    ))
    .unwrap();
    let proof = compiled.prove(&layer).unwrap();
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();

    // (a) The PLAIN 2-hop bag-join (no edge-simple filter) over-counts: it returns 4 walks
    // from a (including the self-loop traversed twice) vs the path's 3. This is the exact
    // over-count the TupleDisequality removes.
    let plain = plan_bgp(&and_of(vec![
        tv_subj_const("a", "p", "m"),
        tv("m", "p", "t"),
    ]))
    .unwrap();
    let plain_proof = plain.prove(&layer).unwrap();
    assert_eq!(plain_proof.result_len, 4);
    assert!(plain_proof.result_len > proof.result_len);

    // (b) The disequality envelope cannot be re-verified as the plain join (the filter is
    // bound into the plan/digest and the result shape).
    assert!(decode_and_verify_envelope(&envelope, &plain, &layer, root).is_err());
    // ... nor as a different length (p^3).
    let p3 = plan_bgp(&times_path(
        Value::Node(iri("a")),
        3,
        3,
        Value::Variable("t".into()),
    ))
    .unwrap();
    assert!(decode_and_verify_envelope(&envelope, &p3, &layer, root).is_err());

    // (c) A tampered result (claiming the dropped self-loop-twice row survived) fails.
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_len += 1;
    assert!(tampered.verify(&compiled, &layer, root).is_err());
}

// ============================================================================
// MIXED-PREDICATE finite repeat (P5a): `(p/q)^n` unrolls to a chain p,q,p,q,...
// whose hops are NOT all the same predicate. The partitioned `TupleDisequality`
// constrains ONLY same-predicate hop-pairs (a different-predicate pair is always
// a distinct stored edge), so a same-predicate edge reuse is dropped exactly as
// in the single-predicate case, per shared predicate — faithful to `path.pl`'s
// edge-simple `Open_Set` semantics.
// ============================================================================

/// Mixed-predicate fixture for `(p/q)^2`: a valid 4-distinct-edge walk
/// a-p->b-q->c-p->d-q->e, PLUS a q-shortcut `b -q-> a` whose only continuations must REUSE the
/// stored p-edge `(a, p, b)`. So the edge-simple result is exactly `{e}` while a plain bag over-counts.
fn mixed_pq_layer() -> Arc<LayerCommitment> {
    build_layer(&[
        ("a", "p", "b"),
        ("c", "p", "d"),
        ("b", "q", "c"),
        ("d", "q", "e"),
        ("b", "q", "a"),
    ])
}

/// `path(subject, (p/q)^{from..to}, object)` — a `Times` over the 2-hop DISTINCT-predicate
/// sequence `p/q`, so each repetition contributes one `p` hop and one `q` hop (a mixed repeat).
fn pq_times_path(subject: Value, from: u64, to: u64, object: Value) -> Query {
    Query::Path(Path {
        subject,
        pattern: PathPattern::Times(PathTimes {
            times: Box::new(PathPattern::Sequence(PathSequence {
                sequence: vec![path_predicate("p"), path_predicate("q")],
            })),
            from,
            to,
        }),
        object,
        path: None,
    })
}

#[test]
fn path_mixed_predicate_two_fold_drops_same_predicate_edge_reuse() {
    // `path(a, (p/q)^2, ?o)` over {p:(a,b),(c,d); q:(b,c),(d,e),(b,a)}. The chain p,q,p,q from a:
    //   a-p->b-q->c-p->d-q->e   (all 4 edges distinct)          -> o=e  KEPT
    //   a-p->b-q->a-p->b-q->c   (reuses stored p-edge (a,p,b))  -> o=c  DROPPED by the p-pair
    //   a-p->b-q->a-p->b-q->a   (reuses (a,p,b) and (b,q,a))    -> o=a  DROPPED
    // Both p-hops are FORWARD, so the stored-edge disequality is byte-for-byte path.pl's
    // path-direction edge. Edge-simple (partitioned disequality) result = exactly {e}.
    let layer = mixed_pq_layer();
    let root = layer.resolution_root();

    let compiled = plan_bgp(&pq_times_path(
        Value::Node(iri("a")),
        2,
        2,
        Value::Variable("o".into()),
    ))
    .unwrap();
    assert_eq!(compiled.relation.schema, ["o"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(
        proof.result_len, 1,
        "the same-predicate p-edge reuse walks are dropped"
    );
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(join_rows(&values), vec![vec![iri("e")]]);

    // Envelope round-trips and re-verifies.
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

#[test]
fn path_mixed_predicate_two_fold_soundness_negatives() {
    let layer = mixed_pq_layer();
    let root = layer.resolution_root();
    let compiled = plan_bgp(&pq_times_path(
        Value::Node(iri("a")),
        2,
        2,
        Value::Variable("o".into()),
    ))
    .unwrap();
    let proof = compiled.prove(&layer).unwrap();
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();

    // (a) The PLAIN p,q,p,q bag-join (no edge-simple filter) OVER-COUNTS: 3 walks from a (including
    // the two that reuse stored p-edge (a,p,b)) vs the path's 1 — the exact over-count the
    // partitioned TupleDisequality removes.
    let plain = plan_bgp(&and_of(vec![
        tv_subj_const("a", "p", "m1"),
        tv("m1", "q", "m2"),
        tv("m2", "p", "m3"),
        tv("m3", "q", "o"),
    ]))
    .unwrap();
    let plain_proof = plain.prove(&layer).unwrap();
    assert_eq!(plain_proof.result_len, 3);
    assert!(plain_proof.result_len > proof.result_len);

    // (b) The disequality envelope cannot be re-verified as the plain join (the filter is bound
    // into the plan/digest and the result shape).
    assert!(decode_and_verify_envelope(&envelope, &plain, &layer, root).is_err());

    // (c) A tampered result (claiming a dropped same-predicate-reuse row survived) fails.
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_len += 1;
    assert!(tampered.verify(&compiled, &layer, root).is_err());
}

// ============================================================================
// MIXED-ORIENTATION same-predicate repeat (P5a): a chain that reuses ONE predicate `p` in BOTH a
// forward `p` hop and an inverse `n(p)` hop. `path.pl`'s edge-simple `Open_Set` keys on the
// WALK-directed step `make_edge(walk-from, p, walk-to)`, NOT the stored triple, so the edge-simple
// disequality must fingerprint each hop by its WALK edge. A forward hop stores `(walk-from, p,
// walk-to)` (walk == stored) while an inverse hop stores `(walk-to, p, walk-from)` (walk == stored
// REVERSED). Fingerprinting by the stored orientation is UNSOUND (case 1: certifies a walk `path.pl`
// rejects) and INCOMPLETE (case 2: drops a walk `path.pl` keeps). These two tests pin both directions
// of the fix against `path.pl` ground truth.
// ============================================================================

/// `path(<a>, Sequence(hops), ?object)` — a bound `a` source, a variable object, and an explicit hop
/// list (each hop a `Predicate`/`InversePredicate` `PathPattern`).
fn seq_from_a(hops: Vec<PathPattern>, object: &str) -> Query {
    Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: PathPattern::Sequence(PathSequence { sequence: hops }),
        object: Value::Variable(object.into()),
        path: None,
    })
}

#[test]
fn path_forward_inverse_same_predicate_soundness_empty() {
    // SOUNDNESS (the P5a hole). `path(a, (p / r / n(p)), ?o)` over {(a,p,b),(b,r,a),(b,p,a)}.
    // The join threads a-p->b-r->a and then the inverse p-hop from a: stored (o, p, a) matches
    // (b,p,a) -> o=b, a single join row {h0=b, h1=a, o=b}. But in WALK space the forward p-hop's edge
    // is (a -> b) and the inverse p-hop's edge is ALSO (a -> b) (walk-from a, walk-to b), so the two
    // p-hops reuse the SAME walk edge -> `path.pl`'s `in_open_set` fires -> {} (empty). The
    // walk-oriented disequality drops the row (Δ=0); a STORED-orientation fingerprint would keep it
    // (stored (a,b) vs (b,a) differ) and CERTIFY the non-solution {o=b}. Correct result: {}.
    let layer = build_layer(&[("a", "p", "b"), ("b", "r", "a"), ("b", "p", "a")]);
    let root = layer.resolution_root();

    let compiled = plan_bgp(&seq_from_a(
        vec![path_predicate("p"), path_predicate("r"), path_inverse("p")],
        "o",
    ))
    .unwrap();
    assert_eq!(compiled.relation.schema, ["o"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(
        proof.result_len, 0,
        "both p-hops share the walk edge (a,p,b), so path.pl fails -> empty"
    );
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert!(
        join_rows(&values).is_empty(),
        "the spurious {{o=b}} row (kept only under stored-orientation fingerprints) is not certified"
    );

    // Envelope round-trips and re-verifies (as the empty result).
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();

    // SOUNDNESS-NEGATIVES — a prover cannot certify the spurious {o=b} row.
    // (a) The PLAIN join (no edge-simple filter) OVER-COUNTS: it returns the 1 spurious walk from a
    // (the stored inverse hop is Triple(?o, p, ?m1)) vs the path's 0.
    let plain = plan_bgp(&and_of(vec![
        tv_subj_const("a", "p", "m0"),
        tv("m0", "r", "m1"),
        tv("o", "p", "m1"),
    ]))
    .unwrap();
    let plain_proof = plain.prove(&layer).unwrap();
    assert_eq!(plain_proof.result_len, 1, "the plain join keeps the walk-edge-reusing row");
    assert!(plain_proof.result_len > proof.result_len);

    // (b) The disequality envelope cannot be re-verified as the plain join (the filter is bound into
    // the plan/digest and the result shape).
    assert!(decode_and_verify_envelope(&envelope, &plain, &layer, root).is_err());

    // (c) A tampered result (claiming the dropped spurious row survived) fails.
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_len += 1;
    assert!(tampered.verify(&compiled, &layer, root).is_err());
}

#[test]
fn path_forward_inverse_same_predicate_completeness_keeps_valid() {
    // COMPLETENESS (a pre-existing bug the SAME fix closes). `path(a, (p / n(p)), ?o)` over
    // {(a,p,b)}. The join a-p->b then the inverse p-hop from b: stored (o, p, b) matches (a,p,b) ->
    // o=a, a single row {h0=b, o=a}. In WALK space the forward p-hop's edge is (a -> b) and the
    // inverse p-hop's edge is (b -> a) — DIFFERENT walk edges — so `path.pl` KEEPS the solution
    // {o=a}. A STORED-orientation fingerprint would see both hops' stored (a,p,b) as EQUAL and
    // wrongly DROP the row. Correct result: {o=a}. (The `(c,q,d)` edge is disjoint — different
    // predicate and nodes — so it never joins the p/n(p) chain; it only pads the layer to the
    // proof system's >= 2-row minimum.)
    let layer = build_layer(&[("a", "p", "b"), ("c", "q", "d")]);
    let root = layer.resolution_root();

    let compiled = plan_bgp(&seq_from_a(vec![path_predicate("p"), path_inverse("p")], "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["o"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(
        proof.result_len, 1,
        "walk edges (a,b) and (b,a) differ, so the o=a solution is KEPT"
    );
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(join_rows(&values), vec![vec![iri("a")]]);

    // Envelope round-trips and re-verifies.
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

// ============================================================================
// W2 — FIXED-LENGTH multi-hop WILDCARD paths (`Sequence([.,.])`, `Times(., n, n)`, mixed
// `Sequence([.,p])`). Two wildcard hops can traverse the SAME stored edge with no compile-time
// same-predicate guarantee, so the edge-simple `TupleDisequality` folds the PREDICATE id into a
// 3-column FULL-TRIPLE fingerprint: two hops are the same stored edge iff subject AND predicate AND
// object coincide — exactly `path.pl`'s `make_edge(subject, predicate, object)` `Open_Set` key. A
// pair whose (s,o) coincide but whose PREDICATE differs is KEPT (two distinct multi-predicate edges),
// the P5b-style multiplicity a 2-column (predicate-blind) fold would wrongly drop.
// ============================================================================

/// `path(a, Sequence([., .]), ?y)` — a bound source `a`, a variable object, two wildcard hops.
fn two_hop_wildcard_from_a(object: &str) -> Query {
    Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: PathPattern::Sequence(PathSequence {
            sequence: vec![
                PathPattern::Predicate(PathPredicate { predicate: None }),
                PathPattern::Predicate(PathPredicate { predicate: None }),
            ],
        }),
        object: Value::Variable(object.into()),
        path: None,
    })
}

/// `Triple(<iri(subj)>, ?pred, ?obj)` — a bound-subject, free-predicate, free-object atom (the plain
/// wildcard first hop, for the over-count soundness comparison).
fn triple_subj_const_pred_var(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subj)),
        predicate: NodeValue::Variable(pred.into()),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

#[test]
fn two_hop_wildcard_drops_full_edge_reuse_and_keeps_predicate_difference() {
    // KEY test. `path(a, (./.), ?y)` over a graph with TWO self-loops at `a` (distinct predicates p, q)
    // plus a p-edge a->b. Edge-simple 2-hop wildcard walks from a (both hops DISTINCT stored edges):
    //   a-(a,p,a)->a-(a,q,a)->a   distinct edges (p != q)          -> y=a  KEPT (SAME (s,o), diff pred!)
    //   a-(a,p,a)->a-(a,p,b)->b   distinct edges                   -> y=b  KEPT
    //   a-(a,q,a)->a-(a,p,a)->a   distinct edges                   -> y=a  KEPT (SAME (s,o), diff pred!)
    //   a-(a,q,a)->a-(a,p,b)->b   distinct edges                   -> y=b  KEPT
    // DROPPED (the same stored edge traversed twice): (a,p,a)+(a,p,a) and (a,q,a)+(a,q,a).
    // Edge-simple bag = {a, a, b, b}; a plain bag-join returns 6 (the two self-reuse walks too).
    let layer = build_layer(&[("a", "p", "a"), ("a", "q", "a"), ("a", "p", "b")]);
    let root = layer.resolution_root();

    let compiled = plan_bgp(&two_hop_wildcard_from_a("y")).unwrap();
    assert_eq!(compiled.relation.schema, ["y"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(
        proof.result_len, 4,
        "the two same-edge (full-triple) reuse walks drop; the diff-predicate self-loop pairs are KEPT"
    );
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(
        join_rows(&values),
        vec![vec![iri("a")], vec![iri("a")], vec![iri("b")], vec![iri("b")]],
        "y = {{a (×2, differing-predicate self-loop pairs), b (×2)}}"
    );

    // Envelope round-trips and re-verifies.
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
}

#[test]
fn two_hop_wildcard_soundness_negatives() {
    let layer = build_layer(&[("a", "p", "a"), ("a", "q", "a"), ("a", "p", "b")]);
    let root = layer.resolution_root();
    let compiled = plan_bgp(&two_hop_wildcard_from_a("y")).unwrap();
    let proof = compiled.prove(&layer).unwrap();
    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();

    // (a) The PLAIN 2-hop wildcard bag-join (no edge-simple filter) OVER-COUNTS: it returns 6 walks
    // from a (INCLUDING the two full-triple self-reuse walks) vs the path's 4 — the exact over-count
    // the full-triple TupleDisequality removes.
    let plain = plan_bgp(&and_of(vec![
        triple_subj_const_pred_var("a", "p0", "m"),
        tv_pred_var("m", "p1", "y"),
    ]))
    .unwrap();
    let plain_proof = plain.prove(&layer).unwrap();
    assert_eq!(plain_proof.result_len, 6);
    assert!(plain_proof.result_len > proof.result_len);

    // (b) The disequality envelope cannot be re-verified as the plain join (the full-triple filter is
    // bound into the plan/digest and the result shape).
    assert!(decode_and_verify_envelope(&envelope, &plain, &layer, root).is_err());

    // (c) A tampered result (claiming a dropped full-triple-reuse row survived) fails.
    let mut tampered = compiled.prove(&layer).unwrap();
    tampered.result_len += 1;
    assert!(tampered.verify(&compiled, &layer, root).is_err());
}

#[test]
fn mixed_wildcard_concrete_sequence_folds_predicate_constant_end_to_end() {
    // MIXED `path(a, (./p), ?y)`: hop0 wildcard, hop1 concrete `p`, over the two self-loops {(a,p,a),
    // (a,q,a)}. hop0 reaches a via p OR q; hop1 (concrete p) then walks (a,p,a). The full-triple pair
    // folds hop0's predicate COLUMN against hop1's predicate CONSTANT `p`, so a reuse fires ONLY when
    // hop0's runtime predicate is ALSO `p`:
    //   hop0=(a,p,a), hop1=(a,p,a)  same edge (a,p,a) twice           -> DROP
    //   hop0=(a,q,a), hop1=(a,p,a)  distinct edges (q != p)           -> y=a  KEEP
    // Edge-simple result = {a}; a plain bag-join returns 2.
    let layer = build_layer(&[("a", "p", "a"), ("a", "q", "a")]);
    let root = layer.resolution_root();

    let compiled = plan_bgp(&Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: PathPattern::Sequence(PathSequence {
            sequence: vec![
                PathPattern::Predicate(PathPredicate { predicate: None }),
                PathPattern::Predicate(PathPredicate {
                    predicate: Some(iri("p")),
                }),
            ],
        }),
        object: Value::Variable("y".into()),
        path: None,
    }))
    .unwrap();
    assert_eq!(compiled.relation.schema, ["y"]);

    let proof = compiled.prove(&layer).unwrap();
    assert_eq!(proof.result_len, 1, "the p-then-p self-reuse drops; the q-then-p walk is kept");
    let values = proof.verify_and_resolve_values(&compiled, &layer, root).unwrap();
    assert_eq!(join_rows(&values), vec![vec![iri("a")]]);

    // SOUNDNESS — the plain `Triple(a, ?p0, ?m) AND Triple(?m, p, ?y)` OVER-COUNTS (2 vs 1): it keeps
    // the p-then-p self-reuse the full-triple filter removes.
    let plain = plan_bgp(&and_of(vec![
        triple_subj_const_pred_var("a", "p0", "m"),
        tv("m", "p", "y"),
    ]))
    .unwrap();
    let plain_proof = plain.prove(&layer).unwrap();
    assert_eq!(plain_proof.result_len, 2);
    assert!(plain_proof.result_len > proof.result_len);

    let envelope = proof.encode_envelope(&compiled, &layer, root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, root).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &plain, &layer, root).is_err());
}

// ===================================================================================================
// SCHEMA-GRAPH FOUNDATION — `Subsumption` proved+verified over a REAL committed schema layer, driven
// end-to-end through the two-layer entry `CompiledBgp::prove_on_layers` and anchored to the combined
// `database_root`. This is the acceptance bar: `prove_on_layers` actually loads/commits the schema
// layer, the graph selector routes the closure to the schema commitment, and the verifier validates
// BOTH sub-commitments against the externally-trusted combined root.
// ===================================================================================================

/// The `sys:inherits` subclass edge IRI the Subsumption compiler emits.
const SUBSUMPTION_SYS_INHERITS: &str = "http://terminusdb.com/schema/sys#inherits";

/// A committed SCHEMA-graph `SyncStoreLayer` from `(subclass, superclass)` `sys:inherits` edges.
fn sync_schema_layer(edges: &[(&str, &str)]) -> terminus_store::store::sync::SyncStoreLayer {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (sub, sup) in edges {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(sub), SUBSUMPTION_SYS_INHERITS, &iri(sup)))
            .unwrap();
    }
    builder.commit().unwrap()
}

/// A committed INSTANCE-graph `SyncStoreLayer` of plain node triples.
fn sync_node_layer(triples: &[(&str, &str, &str)]) -> terminus_store::store::sync::SyncStoreLayer {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, p, o) in triples {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o)))
            .unwrap();
    }
    builder.commit().unwrap()
}

fn subsumption_query(child: NodeValue, parent: NodeValue) -> Query {
    Query::Subsumption(Subsumption { child, parent })
}

fn iri_set(names: &[&str]) -> std::collections::BTreeSet<String> {
    names.iter().map(|n| iri(n)).collect()
}

fn assert_subsumption_prove_on_layers() {
    // Diamond: A <: B, A <: C, B <: D, C <: D (A subclasses B and C; both subclass D).
    let schema = sync_schema_layer(&[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")]);
    let instance = sync_node_layer(&[("i1", "p", "i2"), ("i2", "p", "i3")]);

    // The externally-trusted combined anchor is computed by the prover-side helper.
    let db_root = CompiledBgp::prove_on_layers_root(&instance, &schema).unwrap();
    let instance_view = instance.proof_commitment().unwrap();
    let schema_view = schema.proof_commitment().unwrap();

    // One-endpoint-bound: prove via `prove_on_layers` (which selects+commits the SCHEMA layer) and
    // verify+resolve through the combined-root path (validates BOTH sub-commitments).
    let run = |q: &Query| -> std::collections::BTreeSet<String> {
        let compiled = plan_bgp(q).unwrap();
        let proved = compiled.prove_on_layers(&instance, &schema).unwrap();
        let objects = proved
            .verify_over_graphs_and_resolve(&compiled, &*instance_view, &*schema_view, db_root)
            .unwrap();
        closure_resolved_set(&objects)
    };

    // `A << ?y` = reflexive-transitive supers of A = {A, B, C, D}. Exact set — a spurious super
    // (any class not reflexive-transitively above A) would break this equality.
    assert_eq!(
        run(&subsumption_query(NodeValue::Node(iri("A")), NodeValue::Variable("y".into()))),
        iri_set(&["A", "B", "C", "D"]),
    );
    // `?x << D` = subclasses of D (inverted closure) = {A, B, C, D}.
    assert_eq!(
        run(&subsumption_query(NodeValue::Variable("x".into()), NodeValue::Node(iri("D")))),
        iri_set(&["A", "B", "C", "D"]),
    );
    // `B << ?y` = supers of B = {B, D}.
    assert_eq!(
        run(&subsumption_query(NodeValue::Node(iri("B")), NodeValue::Variable("y".into()))),
        iri_set(&["B", "D"]),
    );

    // Both-bound existence via the membership gate.
    let holds = |child: &str, parent: &str| -> bool {
        let q = subsumption_query(NodeValue::Node(iri(child)), NodeValue::Node(iri(parent)));
        let compiled = plan_bgp(&q).unwrap();
        let proved = compiled.prove_on_layers(&instance, &schema).unwrap();
        proved
            .verify_over_graphs(&compiled, &*instance_view, &*schema_view, db_root)
            .unwrap();
        proved.result_len >= 1
    };
    assert!(holds("A", "C"), "A << C (direct edge) holds");
    assert!(holds("A", "B"), "A << B holds");
    assert!(holds("A", "D"), "A << D (through the diamond) holds");
    assert!(holds("A", "A"), "A << A (reflexive) holds");
    // SPURIOUS SUPER: A is a SUB of everything; nothing subsumes A but A, so these are false.
    assert!(!holds("C", "A"), "C << A is false (A is not a super of C)");
    assert!(!holds("D", "A"), "D << A is false");
    assert!(!holds("B", "C"), "B << C is false (siblings under D)");
}

#[test]
fn subsumption_prove_on_layers_over_committed_schema_end_to_end() {
    std::thread::Builder::new()
        .name("subsumption-prove-on-layers".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_subsumption_prove_on_layers)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_subsumption_soundness_negatives() {
    let schema = sync_schema_layer(&[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")]);
    let instance = sync_node_layer(&[("i1", "p", "i2"), ("i2", "p", "i3")]);
    let db_root = CompiledBgp::prove_on_layers_root(&instance, &schema).unwrap();
    let instance_view = instance.proof_commitment().unwrap();
    let schema_view = schema.proof_commitment().unwrap();
    let schema_root = schema_view.resolution_root();
    let instance_root = instance_view.resolution_root();

    let query = subsumption_query(NodeValue::Node(iri("A")), NodeValue::Variable("y".into()));
    let compiled = plan_bgp(&query).unwrap();
    let proved = compiled.prove_on_layers(&instance, &schema).unwrap();

    // Baseline: honest proof verifies and delivers exactly {A, B, C, D}.
    let objects = proved
        .verify_over_graphs_and_resolve(&compiled, &*instance_view, &*schema_view, db_root)
        .unwrap();
    assert_eq!(closure_resolved_set(&objects), iri_set(&["A", "B", "C", "D"]));

    // (i) WRONG GRAPH — prover side: the INSTANCE-only entry rejects a schema query closed (it holds
    // no schema layer, so it cannot honestly anchor the closure).
    assert!(
        compiled.prove_on_layer(&instance).is_err(),
        "the instance-only entry must reject a schema-reading query"
    );

    // (ii) WRONG GRAPH — verifier anchor: the schema view swapped for the instance view. The verifier
    // reconstructs graph=Schema and anchors to the (wrong) instance commitment; the seed class / edge
    // are absent there, so verification fails.
    assert!(
        proved
            .verify_over_graphs(
                &compiled,
                &*instance_view,
                &*instance_view,
                database_root(&instance_root, &instance_root),
            )
            .is_err(),
        "verifying a schema proof against the instance commitment must fail"
    );
    // The low-level verify against the instance view + instance root likewise fails.
    assert!(
        proved.verify(&compiled, &*instance_view, instance_root).is_err(),
        "a schema proof must not verify against the instance root"
    );

    // (iii) TAMPERED database_root: the combined anchor no longer matches the two sub-roots.
    let mut bad_db = db_root;
    bad_db[0] ^= 1;
    assert!(
        proved
            .verify_over_graphs(&compiled, &*instance_view, &*schema_view, bad_db)
            .is_err(),
        "a tampered database_root must be rejected"
    );

    // (iv) TAMPERED result: flip a byte in the portable envelope → decode/verify fails closed. (The
    // low-level portable path anchors directly to the schema view + schema root.)
    let bytes = proved.encode_envelope(&compiled, &*schema_view, schema_root).unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &*schema_view, schema_root).unwrap();
    let mut tampered = bytes.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 1;
    assert!(
        decode_and_verify_envelope(&tampered, &compiled, &*schema_view, schema_root).is_err(),
        "a tampered subsumption envelope must fail to verify"
    );

    // (v) SPURIOUS SUPER via a tampered schema: injecting an extra `sys:inherits` edge changes the
    // schema commitment root, so it can no longer combine into the trusted database_root.
    let spurious = sync_schema_layer(&[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D"), ("A", "Evil")]);
    let spurious_view = spurious.proof_commitment().unwrap();
    assert_ne!(spurious_view.resolution_root(), schema_root);
    assert!(
        proved
            .verify_over_graphs(&compiled, &*instance_view, &*spurious_view, db_root)
            .is_err(),
        "a schema with a spurious super cannot combine into the trusted database_root"
    );
}

#[test]
fn subsumption_soundness_negatives_end_to_end() {
    std::thread::Builder::new()
        .name("subsumption-soundness".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_subsumption_soundness_negatives)
        .unwrap()
        .join()
        .unwrap();
}

fn assert_instance_only_backward_compat_via_prove_on_layer() {
    // BACKWARD-COMPAT: an existing instance-only BGP proved through the UNCHANGED `prove_on_layer`
    // entry must prove+verify, byte-identically across runs — the schema-graph foundation is additive.
    let instance = sync_node_layer(&[("a", "p", "b"), ("b", "p", "c")]);
    let commitment = instance.proof_commitment().unwrap();
    let root = commitment.resolution_root();

    let query = Query::Triple(terminusdb_woql2::triple::Triple {
        subject: NodeValue::Variable("s".into()),
        predicate: NodeValue::Node(iri("p")),
        object: Value::Variable("o".into()),
        graph: None,
    });
    let compiled = plan_bgp(&query).unwrap();

    // `prove_on_layer` (the instance-only entry) is unchanged; two runs yield byte-identical envelopes.
    let proved_a = compiled.prove_on_layer(&instance).unwrap();
    let proved_b = compiled.prove_on_layer(&instance).unwrap();
    let bytes_a = proved_a.encode_envelope(&compiled, &commitment, root).unwrap();
    let bytes_b = proved_b.encode_envelope(&compiled, &commitment, root).unwrap();
    assert_eq!(bytes_a, bytes_b, "instance-only proving must be byte-identical across runs");
    proved_a.verify(&compiled, &commitment, root).unwrap();
    decode_and_verify_envelope(&bytes_a, &compiled, &commitment, root).unwrap();
}

#[test]
fn instance_only_backward_compat_via_prove_on_layer_byte_identical() {
    std::thread::Builder::new()
        .name("subsumption-backward-compat".into())
        .stack_size(128 * 1024 * 1024)
        .spawn(assert_instance_only_backward_compat_via_prove_on_layer)
        .unwrap()
        .join()
        .unwrap();
}

// -----------------------------------------------------------------------------------------------
// ReadDocument increment 2a — compiler mirror for the store's `OrderedListWalk` leaf.
//
// The store already PROVES the ordered walk of a stored `rdf:List` (see the store test
// `ordered_list_walk_end_to_end`). This suite exercises the COMPILER side: a directly-constructed
// `CompiledRelationStatement::OrderedListWalk` proves + verifies + resolves end-to-end through the
// compiler->store path, carrying the store envelope WHOLESALE (its composite proof is store-private,
// like `CrossRootHashJoin`). No WOQL construct lowers to this leaf yet (that is increment 2b), so it
// is reached only by direct construction — which is exactly what 2a delivers.
// -----------------------------------------------------------------------------------------------

fn rdf_first() -> String {
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#first".to_string()
}
fn rdf_rest() -> String {
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest".to_string()
}
fn rdf_nil() -> String {
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil".to_string()
}
fn cons_cell(i: usize) -> String {
    format!("http://ex/cell{i}")
}

/// Build a base layer from RAW node triples (IRIs used VERBATIM — NOT `iri()`-wrapped — so the RDF
/// Collection constants keep their exact well-known IRIs). Mirrors the store's `list_layer`.
fn list_layer(triples: Vec<(String, String, String)>) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, p, o) in &triples {
        builder
            .add_value_triple(ValueTriple::new_node(s.as_str(), p.as_str(), o.as_str()))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// The standard cons chain `S -P-> [elements...]` using cells `cell0, cell1, …` and the RDF
/// `first`/`rest`/`nil` constants. Mirrors the store's `chain_triples`.
fn chain_triples(
    subject: &str,
    predicate: &str,
    elements: &[&str],
) -> Vec<(String, String, String)> {
    let mut t = Vec::new();
    if elements.is_empty() {
        t.push((subject.to_string(), predicate.to_string(), rdf_nil()));
        return t;
    }
    t.push((subject.to_string(), predicate.to_string(), cons_cell(0)));
    for (i, e) in elements.iter().enumerate() {
        t.push((cons_cell(i), rdf_first(), iri(e)));
        let rest = if i + 1 < elements.len() {
            cons_cell(i + 1)
        } else {
            rdf_nil()
        };
        t.push((cons_cell(i), rdf_rest(), rest));
    }
    t
}

/// A directly-constructed `CompiledBgp` for the ordered walk of `(subject, predicate)`'s list. The
/// fields are pub, so the test crate builds the leaf without any WOQL query (there is none yet).
fn ordered_list_walk_compiled(subject: &str, predicate: &str) -> CompiledBgp {
    CompiledBgp {
        relation: CompiledRelation {
            schema: vec!["elem".to_string()],
            domains: vec![None],
            statement: CompiledRelationStatement::OrderedListWalk {
                subject: CanonicalObject::Node(iri(subject).into_bytes()),
                predicate: iri(predicate).into_bytes(),
                graph: ProofGraph::Instance,
            },
        },
        projection: None,
        collection: None,
        count_variable: None,
        full_row_distinct: false,
    }
}

/// The single ordered element column, resolved to node IRIs (order AND multiplicity preserved).
fn walk_resolved_elements(objects: &[Vec<CanonicalObject>]) -> Vec<String> {
    objects[0]
        .iter()
        .map(|object| match object {
            CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone()).unwrap(),
            other => panic!("unexpected list element object {other:?}"),
        })
        .collect()
}

fn assert_ordered_list_walk_e2e() {
    // A list with a REPEATED element: both order and multiplicity are non-trivial (`a` at positions
    // 0 and 2). A silent dedup or reorder in the mirror would be caught here.
    let lc = list_layer(chain_triples(&iri("doc"), &iri("tracks"), &["a", "b", "a"]));
    let root = lc.resolution_root();

    let compiled = ordered_list_walk_compiled("doc", "tracks");
    let proved = compiled.prove(&lc).unwrap();

    // The proof node carries the store envelope wholesale (the increment-2a mirror shape).
    assert!(matches!(
        proved.statement,
        ProvedQueryStatement::OrderedListWalk { .. }
    ));
    assert_eq!(proved.result_len, 3);

    // Pure verify (instance graph) passes against the trusted root.
    proved.verify(&compiled, &lc, root).unwrap();

    // Verify + resolve delivers the ordered elements `[a, b, a]` — order and the duplicate preserved.
    let objects = proved.verify_and_resolve(&compiled, &lc, root).unwrap();
    assert_eq!(
        walk_resolved_elements(&objects),
        vec![iri("a"), iri("b"), iri("a")],
    );

    // Portable envelope round-trips through the store decode+verify path.
    let bytes = proved.encode_envelope(&compiled, &lc, root).unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, root).unwrap();

    // Single-element list: the smallest non-empty chain still walks correctly.
    let solo = list_layer(chain_triples(&iri("solo"), &iri("tracks"), &["z"]));
    let solo_root = solo.resolution_root();
    let solo_compiled = ordered_list_walk_compiled("solo", "tracks");
    let solo_proved = solo_compiled.prove(&solo).unwrap();
    let solo_objects = solo_proved
        .verify_and_resolve(&solo_compiled, &solo, solo_root)
        .unwrap();
    assert_eq!(walk_resolved_elements(&solo_objects), vec![iri("z")]);

    // ---- NEGATIVE 1: a TAMPERED result column fails verify. ----
    // Corrupt one committed element id in the OUTER proof node (leaving the wholesale envelope
    // intact). The self==envelope binding in the verify arm — and the final `verify_columns` —
    // reject it, so a caller cannot swap in a different answer than the store proved.
    let mut tampered = compiled.prove(&lc).unwrap();
    tampered.result_columns[0][0] = ArkFr::from_u64(0xDEAD_BEEF);
    assert!(
        tampered.verify(&compiled, &lc, root).is_err(),
        "a tampered result column must fail verify",
    );

    // ---- NEGATIVE 2: ANTI-SUBSTITUTION — the proof is bound to its (subject, predicate). ----
    // Aim the honest proof at a DIFFERENT `(subject, predicate)` OrderedListWalk plan. The statement
    // digest folds (subject, predicate), and the store's verify rejects any envelope whose query
    // digest does not match the trusted plan — so the walk proof does not verify as another list.
    let other_predicate = ordered_list_walk_compiled("doc", "elements");
    assert!(
        proved.verify(&other_predicate, &lc, root).is_err(),
        "the walk proof must not verify against a different predicate",
    );
    let other_subject = ordered_list_walk_compiled("other", "tracks");
    assert!(
        proved.verify(&other_subject, &lc, root).is_err(),
        "the walk proof must not verify against a different subject",
    );
}

#[test]
fn ordered_list_walk_compiler_mirror_end_to_end() {
    std::thread::Builder::new()
        .name("ordered-list-walk-proof".into())
        .stack_size(512 * 1024 * 1024)
        .spawn(assert_ordered_list_walk_e2e)
        .unwrap()
        .join()
        .unwrap();
}

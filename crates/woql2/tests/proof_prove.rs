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
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::envelope::RecursiveRelationEnvelope;
use terminus_store::proof::join2::{binary_join_domain, compute_binary_join};
use terminus_store::proof::query::{
    prove_layer_bgp, verify_layer_bgp, generated_decimal_i64_field, AtomTerm, CanonicalResultValue, LayerBgpAtom, LayerBgpPlan,
    ObjectKindFilter, ResultIdNamespace,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::{open_sync_memory_store, ValueTriple};

use tdb_succinct::{TdbDataType, TypedDictEntry};

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::control::{When, WoqlOptional};
use terminusdb_woql2::control::{Distinct, Select, Using};
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::proof::{
    compiled_query_digest, decode_and_verify_envelope,
    decode_verifier_commitment_and_verify_envelope, plan_bgp,
    BgpPlanError, CompiledBgp, CompiledRelationStatement, ExecutedResultValue,
};
use terminusdb_woql2::triple::{Data, Link, TripleNext, TriplePrevious, TripleSlice, TripleSliceRev};
use terminusdb_woql2::{
    misc::{Count, InRange, Limit, RangeMax, RangeMin, Start, TripleCount},
    order::{GroupBy, Order, OrderBy, OrderTemplate},
    query::{Not, Or, Query},
    value::{DataValue, NodeValue, Value},
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

#[test]
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
            .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(3)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.state.commitment_root,
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
                layer.state.commitment_root,
                &["Count".into()],
                &[vec![wrong]],
            )
            .is_err());
    }

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let decoded = decode_and_verify_envelope(
        &envelope,
        &compiled,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
    assert!(decode_and_verify_envelope(
        &envelope,
        &compiled,
        &base_commitment,
        base_commitment.state.commitment_root,
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
        layer.state.commitment_root,
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
            .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(0)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.state.commitment_root,
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
            .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
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
        .verify_and_resolve_values(&forward, &layer, layer.state.commitment_root)
        .unwrap();
    assert_eq!(resolved_u32_column(&forward_values), [20, 30]);

    let reverse = plan_bgp(&triple_slice_query(true)).unwrap();
    let reverse_proof = reverse.prove(&layer).unwrap();
    let reverse_values = reverse_proof
        .verify_and_resolve_values(&reverse, &layer, layer.state.commitment_root)
        .unwrap();
    assert_eq!(resolved_u32_column(&reverse_values), [30, 20]);

    let envelope = forward_proof
        .encode_envelope(&forward, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &reverse,
        &layer,
        layer.state.commitment_root,
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
            .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .verify(&compiled, layer, layer.state.commitment_root)
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
        .encode_envelope(&forward, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &reverse, &layer, layer.state.commitment_root,
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &previous, &layer, layer.state.commitment_root,
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();

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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();

    let ascending = plan_bgp(&Query::Limit(Limit {
        limit: 2,
        query: Box::new(Query::Start(Start {
            start: 1,
            query: Box::new(exact_scalar_order_query(0)),
        })),
    }))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope, &ascending, &layer, layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn desc_ordered_start_limit_slice_proves_and_rejects_tamper() {
    std::thread::Builder::new()
        .name("woql-ordered-slice-proof".into())
        .stack_size(24 * 1024 * 1024)
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
    let values = proved.verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root).unwrap();
    let number = |value: &CanonicalResultValue| match value { CanonicalResultValue::Object(CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, lexical }) => u32::from_be_bytes(lexical.as_slice().try_into().unwrap()), other => panic!("unexpected {other:?}") };
    assert_eq!((number(&values[0][0]),number(&values[1][0])), (1,4));
    assert_eq!((number(&values[0][1]),number(&values[1][1])), (1,4));
    let envelope = proved.encode_envelope(&compiled, &layer, layer.state.commitment_root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();
    let ascending = plan_bgp(&Query::Limit(Limit { limit: 2, query: Box::new(ordered(Order::Asc)) })).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &ascending, &layer, layer.state.commitment_root).is_err());
    let schema_order = plan_bgp(&Query::Limit(Limit { limit: 2, query: Box::new(Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "x".into(), order: Order::Asc }, OrderTemplate { variable: "y".into(), order: Order::Desc }],
        query: Box::new(duplicate_child()),
    })) })).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &schema_order, &layer, layer.state.commitment_root).is_err());
    let mut changed = RecursiveRelationEnvelope::decode(&envelope).unwrap();
    let replacement = layer.node_value_catalog.iter()
        .find(|record| matches!(record.triple.object, CanonicalObject::Value { datatype: CanonicalDatatype::UInt32, .. })
            && ArkFr::from_u64(record.object_id) != changed.result_columns[0][1])
        .map(|record| ArkFr::from_u64(record.object_id)).unwrap();
    changed.result_columns[0][1] = replacement;
    for (commitment,column) in changed.result_commitments.iter_mut().zip(&changed.result_columns) { *commitment = commit_column(&pad(column, 1usize << changed.result_log_domain)).unwrap(); }
    assert!(decode_and_verify_envelope(&changed.encode().unwrap(), &compiled, &layer, layer.state.commitment_root).is_err());
}

#[test]
fn multikey_order_nested_limit_e2e() {
    std::thread::Builder::new().name("woql-multikey-order".into()).stack_size(32*1024*1024)
        .spawn(assert_multikey_order_nested_limit_e2e).unwrap().join().unwrap();
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
    let values = proved.verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root).unwrap();
    let node = |value: &CanonicalResultValue| match value {
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("unexpected {other:?}"),
    };
    let rows = (0..proved.result_len).map(|row| (node(&values[0][row]), node(&values[1][row]))).collect::<Vec<_>>();
    assert_eq!(rows, vec![(iri("a"), iri("b")), (iri("a"), iri("b")), (iri("c"), iri("d"))]);
    let envelope = proved.encode_envelope(&compiled, &layer, layer.state.commitment_root).unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();
    let substituted = plan_bgp(&query("s")).unwrap();
    assert!(decode_and_verify_envelope(&envelope, &substituted, &layer, layer.state.commitment_root).is_err());
}

#[test]
fn bounded_when_success_plus_true_e2e() {
    std::thread::Builder::new().name("woql-when-proof".into()).stack_size(48 * 1024 * 1024)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(
        &envelope,
        &compiled,
        &layer,
        layer.state.commitment_root,
    )
    .unwrap();
    let substituted = plan_bgp(&query("s")).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn when_existential_local_projection_preserves_success_multiplicity() {
    std::thread::Builder::new()
        .name("woql-when-existential-proof".into())
        .stack_size(48 * 1024 * 1024)
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
            .encode_envelope(&compiled, &layer, layer.state.commitment_root)
            .unwrap(),
        explicit
            .encode_envelope(&compiled, &layer, layer.state.commitment_root)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn two_key_group_count_proves_duplicate_cardinality_and_comparison() {
    std::thread::Builder::new()
        .name("woql-two-key-group-count-proof".into())
        .stack_size(32 * 1024 * 1024)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn two_key_signed_sum_proves_declared_tuple_order_and_comparison() {
    std::thread::Builder::new()
        .name("woql-two-key-signed-sum-proof".into())
        .stack_size(32 * 1024 * 1024)
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
            .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(5)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &layer,
            layer.state.commitment_root,
            &["total".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(5)]],
        )
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let keyed = plan_bgp(&grouped_u32_sum_query()).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &keyed,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn global_u32_sum_proves_one_generated_decimal_row() {
    std::thread::Builder::new()
        .name("woql-global-sum-proof".into())
        .stack_size(32 * 1024 * 1024)
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
            .verify_and_resolve_values(&compiled, &exact, exact.state.commitment_root)
            .unwrap(),
        vec![vec![CanonicalResultValue::GeneratedDecimalU64(u64::MAX)]]
    );
    proved
        .verify_executed_values(
            &compiled,
            &exact,
            exact.state.commitment_root,
            &["total".into()],
            &[vec![ExecutedResultValue::GeneratedDecimalU64(u64::MAX)]],
        )
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &exact, exact.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &exact, exact.state.commitment_root)
        .unwrap();

    let overflow = layer_with(&[("p", u64::MAX), ("q", 1)]);
    assert!(compiled.prove(&overflow).is_err());
}

#[test]
fn global_u64_sum_accepts_exact_max_and_rejects_actual_overflow() {
    std::thread::Builder::new()
        .name("woql-global-u64-sum-proof".into())
        .stack_size(32 * 1024 * 1024)
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
        )
        .unwrap();
    decode_and_verify_envelope(
        &filtered_envelope,
        &filtered_compiled,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
    let mut wrong_domain = RecursiveRelationEnvelope::decode(&filtered_envelope).unwrap();
    wrong_domain.result_descriptors[1].namespace = ResultIdNamespace::GeneratedDecimalU64;
    assert!(decode_and_verify_envelope(
        &wrong_domain.encode().unwrap(),
        &filtered_compiled,
        &layer,
        layer.state.commitment_root,
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
fn grouped_i64_sum_proves_signed_values_and_rejects_overflow() {
    std::thread::Builder::new()
        .name("woql-grouped-i64-sum-proof".into())
        .stack_size(32 * 1024 * 1024)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
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
            layer.state.commitment_root,
            &["category".into(), "total".into()],
            &executed,
        )
        .unwrap();

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
fn grouped_u32_sum_proves_duplicates_safe_max_and_canonical_child_stack_key_order() {
    std::thread::Builder::new()
        .name("woql-group-sum-proof".into())
        .stack_size(32 * 1024 * 1024)
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
            layer.state.commitment_root,
            &["groups".into()],
            &[vec![0]],
        )
        .unwrap();
}

#[test]
fn grouped_u32_sum_empty_and_outer_count_are_exact() {
    std::thread::Builder::new()
        .name("woql-group-sum-empty-proof".into())
        .stack_size(32 * 1024 * 1024)
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
        .verify_and_resolve_values(compiled, layer, layer.state.commitment_root)
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

#[test]
fn grouped_min_and_max_prove_recursive_envelopes_and_compact_artifacts() {
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
            .encode_envelope(&compiled, &layer, layer.state.commitment_root)
            .unwrap();
        let decoded =
            decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
                .unwrap();
        assert_eq!(decoded.proved.result_len, 2);
        let compact = layer.verifier_commitment().unwrap().encode().unwrap();
        decode_verifier_commitment_and_verify_envelope(
            &envelope,
            &compiled,
            &compact,
            layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
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
            layer.state.commitment_root,
            &["count".into()],
            &[vec![2]],
        )
        .unwrap();
    assert!(proved
        .verify_executed_rows(
            &compiled,
            &layer,
            layer.state.commitment_root,
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert_eq!(values[2][2], CanonicalResultValue::Null);
    assert!(values[2][..2]
        .iter()
        .all(|value| matches!(value, CanonicalResultValue::Object(_))));
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
            &["x".into(), "y".into(), "z".into()],
            &null_in_shared,
        )
        .is_err());

    let mut fake_null = proved;
    fake_null.result_columns[2][0] = ArkFr::zero();
    assert!(fake_null
        .verify(&compiled, &layer, layer.state.commitment_root)
        .is_err());
}

#[test]
fn link_form_absent_from_prolog_json_rejects_before_proving() {
    let query = Query::Link(Link {
        subject: terminusdb_woql2::value::NodeValue::Variable("x".into()),
        predicate: terminusdb_woql2::value::NodeValue::Node(iri("p")),
        object: terminusdb_woql2::value::NodeValue::Variable("y".into()),
        graph: GraphType::Instance,
    });
    assert!(matches!(
        plan_bgp(&query),
        Err(BgpPlanError::UnsupportedConstruct { construct: "Link" })
    ));
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
            layer.state.commitment_root,
        )
        .unwrap();
    decode_and_verify_envelope(
        &constant_envelope,
        &constant_compiled,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        other_layer.state.commitment_root,
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
        .encode_envelope(&columns_compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(
        &columns_envelope,
        &columns_compiled,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
    let mut reordered_schema = columns_compiled.clone();
    reordered_schema.relation.schema.swap(0, 1);
    reordered_schema.relation.domains.swap(0, 1);
    assert!(decode_and_verify_envelope(
        &columns_envelope,
        &reordered_schema,
        &layer,
        layer.state.commitment_root,
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
        .encode_envelope(&false_gate, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(
        &false_envelope,
        &false_gate,
        &layer,
        layer.state.commitment_root,
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
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert_eq!(values[2], [CanonicalResultValue::Null]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();

    // Simulate archive/reopen, derive the compact public view, then perform remote-style
    // decode + verification without retaining any native/materialized witness records.
    let reopened = LayerCommitment::decode(&layer.encode().unwrap()).unwrap();
    let compact = reopened.verifier_commitment().unwrap().encode().unwrap();
    let verified = decode_verifier_commitment_and_verify_envelope(
        &envelope,
        &compiled,
        &compact,
        layer.state.commitment_root,
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
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        layer.state.commitment_root,
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert_eq!(values[2], [CanonicalResultValue::Null]);
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(values[2]
        .iter()
        .all(|value| *value == CanonicalResultValue::Null));

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();

    // The absence claim is part of the recursive query statement. Replacing one absent
    // subject with a dictionary-present subject changes both semantics and the digest.
    let substituted = plan_bgp(&branch("p", "anchor")).unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &substituted,
        &layer,
        layer.state.commitment_root,
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
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();
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
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();

    let mut swapped = compiled.clone();
    swapped.relation.as_left_outer_mut().unwrap().swap(0, 1);
    assert!(proved
        .verify(&swapped, &layer, layer.state.commitment_root)
        .is_err());
    let mut fake_null = proved;
    let matched = fake_null.result_columns[2]
        .iter()
        .position(|value| !value.is_zero())
        .unwrap();
    fake_null.result_columns[2][matched] = ArkFr::zero();
    assert!(fake_null
        .verify(&compiled, &layer, layer.state.commitment_root)
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
        .verify_and_resolve_values(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(values[2..]
        .iter()
        .all(|column| column.iter().all(|value| *value == CanonicalResultValue::Null)));

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root).unwrap();

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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
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
            layer.state.commitment_root,
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
        .verify_and_resolve(&compiled, &layer, layer.state.commitment_root)
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
            layer.state.commitment_root,
            &forged_objects,
        )
        .is_err());

    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
            .unwrap();
    assert_eq!(verified.result_objects, objects);
    let mut reordered_schema = compiled.clone();
    reordered_schema.relation.schema.swap(0, 1);
    assert!(decode_and_verify_envelope(
        &envelope,
        &reordered_schema,
        &layer,
        layer.state.commitment_root,
    )
    .is_err());

    let mut forged = proved;
    forged.result_columns[1][0] = ArkFr::from_u64(999_999);
    assert!(forged
        .verify(&compiled, &layer, layer.state.commitment_root)
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
        .verify_and_resolve(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(objects[1].iter().all(|predicate| matches!(
        predicate,
        CanonicalObject::Node(iri) if iri == &crate::proof_common::iri("knows").into_bytes()
            || iri == &crate::proof_common::iri("likes").into_bytes()
    )));
}

#[test]
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
            child_commitment.state.commitment_root,
        )
        .unwrap();
    assert_eq!(proved.result_len, 2);
    assert!(objects[1].contains(&CanonicalObject::Node(iri("inherited").into_bytes())));
    assert!(objects[1].contains(&CanonicalObject::Node(iri("new").into_bytes())));
    assert!(proved
        .verify(
            &compiled,
            &child_commitment,
            base_commitment.state.commitment_root
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
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let verified =
        decode_and_verify_envelope(&envelope, &compiled, &layer, layer.state.commitment_root)
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
        layer.state.commitment_root,
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
        .verify(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &layer, layer.state.commitment_root)
        .unwrap();
    let mut reordered = compiled.clone();
    reordered.relation.as_left_anti_mut().unwrap().swap(0, 1);
    assert!(
        decode_and_verify_envelope(&envelope, &reordered, &layer, layer.state.commitment_root,)
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
        .verify(&retained_compiled, &layer, layer.state.commitment_root)
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
        .verify(&suppressed_compiled, &layer, layer.state.commitment_root)
        .unwrap();
    assert!(suppressed_proved
        .verify_columns(&retained_proved.result_columns)
        .is_err());
    let envelope = suppressed_proved
        .encode_envelope(&suppressed_compiled, &layer, layer.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(
        &envelope,
        &suppressed_compiled,
        &layer,
        layer.state.commitment_root,
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
        layer.state.commitment_root,
    )
    .is_err());
}

#[test]
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
            base_commitment.state.commitment_root,
        )
        .unwrap();
    let child_proved = compiled.prove(&child_commitment).unwrap();
    assert_eq!(child_proved.result_len, 0);
    child_proved
        .verify(
            &compiled,
            &child_commitment,
            child_commitment.state.commitment_root,
        )
        .unwrap();
    assert!(child_proved
        .verify(
            &compiled,
            &child_commitment,
            base_commitment.state.commitment_root,
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
        lc.state.commitment_root,
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
    proved.verify(&compiled, &lc, lc.state.commitment_root).unwrap();

    let bytes = proved
        .encode_envelope(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&bytes, &compiled, &lc, lc.state.commitment_root).unwrap();

    let mut duplicate = RecursiveRelationEnvelope::decode(&bytes).unwrap();
    duplicate.result_columns[0][1] = duplicate.result_columns[0][0];
    duplicate.result_values[0][1] = duplicate.result_values[0][0].clone();
    duplicate.result_commitments[0] = commit_column(&duplicate.result_columns[0]).unwrap();
    let duplicate_bytes = duplicate.encode().unwrap();
    assert!(decode_and_verify_envelope(
        &duplicate_bytes,
        &compiled,
        &lc,
        lc.state.commitment_root,
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
        lc.state.commitment_root,
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
    proved.verify(&compiled, &lc, lc.state.commitment_root).unwrap();
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
    renamed_schema.relation.schema[0] = "not_x".into();
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
        .verify_and_resolve(&compiled, &lc, lc.state.commitment_root)
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
        .verify(&wrong_kind, &lc, lc.state.commitment_root)
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
    assert!(compiled.bgp_plan().joins[0].is_cross_product());
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
        lc.state.commitment_root,
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
    assert_eq!(compiled.bgp_plan().joins[0].additional_keys, vec![(1, 1)]);
    let (exact_proof, exact_coms, exact_m, exact_ids) =
        prove_layer_bgp(&lc, compiled.bgp_plan()).unwrap();
    verify_layer_bgp(
        &lc,
        lc.state.commitment_root,
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
        lc.state.commitment_root,
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
        lc.state.commitment_root,
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
        .verify(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    let envelope = proved
        .encode_envelope(&compiled, &lc, lc.state.commitment_root)
        .unwrap();
    decode_and_verify_envelope(&envelope, &compiled, &lc, lc.state.commitment_root).unwrap();

    let present_variant = plan_bgp(&and_of(vec![
        tv("x", "knows", "y"),
        tv_obj_node("y", "likes", "c"),
    ]))
    .unwrap();
    assert!(decode_and_verify_envelope(
        &envelope,
        &present_variant,
        &lc,
        lc.state.commitment_root,
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
        .verify(&compiled, &lc, lc.state.commitment_root)
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
        lc.state.commitment_root,
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
        lc.state.commitment_root,
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

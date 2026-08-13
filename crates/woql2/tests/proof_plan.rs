//! CP2 — `plan_bgp` shape guard: accept the safe fragment, reject everything else
//! closed. No proving here (see `prove.rs` for the end-to-end faithfulness tests).

mod proof_common;
use proof_common::*;

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::collection::Dot;
use terminusdb_woql2::compare::{IsA, TypeOf, Typecast};
use terminusdb_woql2::control::{
    Distinct, From, Immediately, Once, Pin, Select, Using, When, WoqlOptional,
};
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::misc::{
    Collect, Comment, Count, HashKey, InRange, Limit, Size, Start, TripleCount,
};
use terminusdb_woql2::order::{GroupBy, Order, OrderBy, OrderTemplate};
use terminusdb_woql2::path::{
    InversePathPredicate, PathOr, PathPattern, PathPlus, PathPredicate, PathSequence, PathStar,
    PathTimes,
};
use terminusdb_woql2::expression::{
    ArithmeticExpression, ArithmeticValue, Div, Divide, Exp, Floor, Minus, Plus, Times,
};
use terminusdb_woql2::interval::{
    DayAfter, DayBefore, Interval, IsoWeek, MonthEndDate, MonthStartDate, Weekday,
    WeekdaySundayStart,
};
use terminusdb_woql2::query::{Eval, Not, Or, Path, Query, True};
use terminusdb_woql2::string::{Concatenate, Join, Pad, Substring, Upper};
use terminusdb_woql2::triple::{
    AddedTriple, Data, DeletedTriple, Link, TripleNext, TriplePrevious, TripleSlice,
    TripleSliceRev,
};
use terminusdb_woql2::value::{DataValue, ListOrVariable, NodeValue, Value};

use terminus_store::proof::envelope::{
    CollectionSumTemplate, GeneratedU64ComparisonOp, RelationEqualityPlan,
};
use terminus_store::proof::query::{AtomTerm, ColumnSource, ObjectKindFilter, ObjectRangeSpec};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::ValueTriple;

use tdb_succinct::{TdbDataType, TypedDictEntry};

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use decimal_rs::Decimal;
use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminusdb_woql2::compare::Subsumption;
use terminusdb_woql2::proof::{
    compiled_query_digest, decode_executed_result_inputs, plan_bgp, BgpPlanError, CompiledBgp,
    CompiledRelationStatement, ExecutedResultInput, ExecutedResultValue, ProofGraph,
    StringNumericTarget,
};

/// The `sys:inherits` edge IRI the Subsumption compiler emits.
const SYS_INHERITS_IRI: &str = "http://terminusdb.com/schema/sys#inherits";

/// Assert `plan_bgp` rejects `q` with exactly `want`. (`CompiledBgp` is not
/// `PartialEq` — the plan types are foreign — so compare only the error.)
#[track_caller]
fn reject(q: Query, want: BgpPlanError) {
    assert_eq!(plan_bgp(&q).unwrap_err(), want);
}

fn triple_count(resource: &str, output: DataValue) -> Query {
    Query::TripleCount(TripleCount {
        resource: resource.into(),
        count: output,
    })
}

fn decimal_value(value: &str) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Decimal(value.parse().unwrap()))
}

#[test]
fn explicit_instance_triple_count_compiles_as_authenticated_generated_scalar() {
    let resource = "admin/db/local/branch/main/instance";
    let compiled = plan_bgp(&triple_count(
        resource,
        DataValue::Variable("Count".into()),
    ))
    .unwrap();

    assert_eq!(compiled.relation.schema, ["Count"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::LayerTripleCount
    ));
    assert_eq!(
        compiled.collection.as_deref(),
        Some("admin/db/local/branch/main")
    );
}

#[test]
fn triple_count_composes_with_select_and_outer_count() {
    let source = triple_count(
        "admin/db/local/branch/main/instance",
        DataValue::Variable("Count".into()),
    );
    let selected = plan_bgp(&Query::Select(Select {
        variables: vec!["Count".into()],
        query: Box::new(source.clone()),
    }))
    .unwrap();
    assert_eq!(selected.projection, Some(vec![0]));

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(source),
        count: DataValue::Variable("Rows".into()),
    }))
    .unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("Rows"));
}

fn added_triple(subj: &str, pred: &str, obj: &str) -> Query {
    Query::AddedTriple(AddedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

fn deleted_triple(subj: &str, pred: &str, obj: &str) -> Query {
    Query::DeletedTriple(DeletedTriple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}

#[test]
fn added_and_deleted_triples_compile_to_source_tagged_bgp() {
    let added = plan_bgp(&added_triple("s", "p", "o")).unwrap();
    assert_eq!(added.bgp_plan().source, ColumnSource::Additions);
    assert_eq!(added.relation.schema, ["s", "o"]);

    let deleted = plan_bgp(&deleted_triple("s", "p", "o")).unwrap();
    assert_eq!(deleted.bgp_plan().source, ColumnSource::Removals);

    // A plain Triple stays Materialized — the unchanged base behavior.
    let materialized = plan_bgp(&tv("s", "p", "o")).unwrap();
    assert_eq!(materialized.bgp_plan().source, ColumnSource::Materialized);

    // Outer Select composes: the BGP still proves the full delta relation, projection is
    // read-side. (Count/Distinct compose identically to the base BGP path.)
    let selected = plan_bgp(&Query::Select(Select {
        variables: vec!["s".into()],
        query: Box::new(added_triple("s", "p", "o")),
    }))
    .unwrap();
    assert_eq!(selected.bgp_plan().source, ColumnSource::Additions);
    assert_eq!(selected.projection, Some(vec![0]));
}

#[test]
fn same_source_multi_atom_delta_bgp_compiles() {
    // And(AddedTriple(?s,p,?o), AddedTriple(?o,q,?z)) is a same-source self-join over the
    // additions relation — supported, mirroring the base layer's k>=2 join support.
    let compiled = plan_bgp(&and_of(vec![
        added_triple("s", "p", "o"),
        added_triple("o", "q", "z"),
    ]))
    .unwrap();
    assert_eq!(compiled.bgp_plan().source, ColumnSource::Additions);
    assert_eq!(compiled.bgp_plan().atoms.len(), 2);
    assert_eq!(compiled.bgp_plan().joins.len(), 1);
}

#[test]
fn cross_source_conjunctions_now_compile_with_per_atom_sources() {
    // A delta atom joined with a materialized Triple: additions ⋈ materialized. Each atom
    // carries its own source; the plan default is Materialized (mixed).
    let add_mat = plan_bgp(&and_of(vec![
        added_triple("s", "p", "o"),
        tv("o", "q", "z"),
    ]))
    .unwrap();
    assert_eq!(add_mat.bgp_plan().source, ColumnSource::Materialized);
    assert_eq!(add_mat.bgp_plan().atoms.len(), 2);
    assert_eq!(add_mat.bgp_plan().joins.len(), 1);
    // atoms[0] is the additions leaf (relational atoms precede ground; both are relational and
    // input order is stable), atoms[1] the materialized leaf.
    assert_eq!(add_mat.bgp_plan().atoms[0].source, ColumnSource::Additions);
    assert_eq!(add_mat.bgp_plan().atoms[1].source, ColumnSource::Materialized);
    assert_eq!(add_mat.relation.schema, ["s", "o", "z"]);

    // Additions joined with removals.
    let add_rem = plan_bgp(&and_of(vec![
        added_triple("s", "p", "o"),
        deleted_triple("o", "q", "z"),
    ]))
    .unwrap();
    // No uniform source -> plan default is Materialized.
    assert_eq!(add_rem.bgp_plan().source, ColumnSource::Materialized);
    assert_eq!(add_rem.bgp_plan().atoms[0].source, ColumnSource::Additions);
    assert_eq!(add_rem.bgp_plan().atoms[1].source, ColumnSource::Removals);
}

#[test]
fn schema_graph_delta_reads_reject_closed() {
    for graph in [GraphType::Schema] {
        reject(
            Query::AddedTriple(AddedTriple {
                subject: NodeValue::Variable("s".into()),
                predicate: NodeValue::Node(iri("p")),
                object: Value::Variable("o".into()),
                graph: Some(graph.clone()),
            }),
            BgpPlanError::SchemaGraphUnsupported,
        );
        reject(
            Query::DeletedTriple(DeletedTriple {
                subject: NodeValue::Variable("s".into()),
                predicate: NodeValue::Node(iri("p")),
                object: Value::Variable("o".into()),
                graph: Some(graph),
            }),
            BgpPlanError::SchemaGraphUnsupported,
        );
    }
}

#[test]
fn triple_count_rejects_uncommitted_resource_semantics_and_constant_output() {
    reject(
        triple_count("admin/db", DataValue::Variable("Count".into())),
        BgpPlanError::TripleCountUnsupportedResource,
    );
    reject(
        triple_count(
            "admin/db/local/branch/main/schema",
            DataValue::Variable("Count".into()),
        ),
        BgpPlanError::TripleCountUnsupportedResource,
    );
    reject(
        triple_count(
            "admin/db/local/branch/main/instance",
            DataValue::Data(XSDAnySimpleType::UnsignedLong(3)),
        ),
        BgpPlanError::TripleCountOutputNotVariable,
    );
    reject(
        Query::Size(Size {
            resource: "admin/db/local/branch/main/instance".into(),
            size: DataValue::Variable("Size".into()),
        }),
        BgpPlanError::UnsupportedConstruct { construct: "Size" },
    );
}

#[test]
fn triple_count_accepts_exact_integral_decimal_comparisons_and_binds_their_plan() {
    let source = triple_count(
        "admin/db/local/branch/main/instance",
        DataValue::Variable("Count".into()),
    );
    let query = and_of(vec![
        source,
        greater(dvar("Count"), decimal_value("0")),
        lte(decimal_value("18446744073709551615"), dvar("Count")),
    ]);
    let compiled = plan_bgp(&query).unwrap();
    let CompiledRelationStatement::GeneratedU64Filter {
        input,
        op,
        constant,
        constant_on_left,
        ..
    } = &compiled.relation.statement
    else {
        panic!("expected outer generated-u64 filter");
    };
    assert_eq!(*op, GeneratedU64ComparisonOp::Le);
    assert_eq!(*constant, u64::MAX);
    assert!(*constant_on_left);
    assert!(matches!(
        input.statement,
        CompiledRelationStatement::GeneratedU64Filter {
            op: GeneratedU64ComparisonOp::Gt,
            constant: 0,
            constant_on_left: false,
            ..
        }
    ));
}

#[test]
fn triple_count_equals_and_outer_count_compose_without_store_id_coercion() {
    let source = triple_count(
        "admin/db/local/branch/main/instance",
        DataValue::Variable("Count".into()),
    );
    let equality = equals(
        Value::Variable("Count".into()),
        Value::Data(XSDAnySimpleType::Decimal("3.000".parse().unwrap())),
    );
    let compiled = plan_bgp(&Query::Count(Count {
        query: Box::new(and_of(vec![source, equality])),
        count: DataValue::Variable("Rows".into()),
    }))
    .unwrap();
    assert_eq!(compiled.count_variable.as_deref(), Some("Rows"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GeneratedU64Filter {
            op: GeneratedU64ComparisonOp::Eq,
            constant: 3,
            ..
        }
    ));
}

#[test]
fn triple_count_comparisons_reject_fraction_negative_scientific_wrong_type_and_shape() {
    let source = || {
        triple_count(
            "admin/db/local/branch/main/instance",
            DataValue::Variable("Count".into()),
        )
    };
    for constant in ["1.5", "-1", "1e2", "18446744073709551616"] {
        reject(
            and_of(vec![source(), greater(dvar("Count"), decimal_value(constant))]),
            BgpPlanError::TripleCountComparisonUnsupportedConstant,
        );
    }
    reject(
        and_of(vec![
            source(),
            greater(
                dvar("Count"),
                DataValue::Data(XSDAnySimpleType::UnsignedLong(3)),
            ),
        ]),
        BgpPlanError::TripleCountComparisonUnsupportedConstant,
    );
    reject(
        and_of(vec![source(), greater(dvar("Other"), decimal_value("3"))]),
        BgpPlanError::TripleCountComparisonUnsupportedShape,
    );
    reject(
        and_of(vec![greater(dvar("Count"), decimal_value("3")), source()]),
        BgpPlanError::TripleCountComparisonUnsupportedShape,
    );
}

fn grouped_sum_with_bound(bound: DataValue) -> Query {
    let child = and_of(vec![
        tv("category", "score", "score"),
        gte(dvar("score"), bound),
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

fn grouped_u32_sum() -> Query {
    grouped_sum_with_bound(duint(0))
}

fn two_node_key_sum_with_bound(bound: DataValue) -> Query {
    let child = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("left".into()),
            predicate: NodeValue::Node(iri("link")),
            object: NodeValue::Variable("right".into()),
            graph: GraphType::Instance,
        }),
        tv("right", "score", "score"),
        gte(dvar("score"), bound),
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
    ])
}

fn two_node_key_count_with_bound(bound: DataValue) -> Query {
    let child = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("left".into()),
            predicate: NodeValue::Node(iri("link")),
            object: NodeValue::Variable("right".into()),
            graph: GraphType::Instance,
        }),
        tv("right", "score", "score"),
        gte(dvar("score"), bound),
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
    ])
}

fn collect_length_query() -> Query {
    and_of(vec![
        Query::Collect(Collect {
            template: Value::Variable("score".into()),
            into: Value::Variable("scores".into()),
            query: Box::new(and_of(vec![
                tv("subject", "score", "score"),
                gte(dvar("score"), duint(0)),
            ])),
        }),
        Query::Length(Length {
            list: DataValue::Variable("scores".into()),
            length: DataValue::Variable("cardinality".into()),
        }),
    ])
}

fn collect_sum_query(template: Value, bound: DataValue) -> Query {
    and_of(vec![
        Query::Collect(Collect {
            template,
            into: Value::Variable("scores".into()),
            query: Box::new(and_of(vec![
                tv("subject", "score", "score"),
                gte(dvar("score"), bound),
            ])),
        }),
        Query::Sum(Sum {
            list: DataValue::Variable("scores".into()),
            result: DataValue::Variable("total".into()),
        }),
    ])
}

#[test]
fn immediate_collect_sum_compiles_exact_variable_ground_and_comparison_modes() {
    for (bound, datatype) in [
        (duint(0), CanonicalDatatype::UInt32),
        (
            DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
            CanonicalDatatype::Int64,
        ),
    ] {
        let compiled = plan_bgp(&collect_sum_query(Value::Variable("score".into()), bound)).unwrap();
        assert_eq!(compiled.relation.schema, ["total"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::CollectionSum {
                template: CollectionSumTemplate::Column(1),
                datatype: actual,
                ..
            } if actual == datatype
        ));
    }

    let ground = plan_bgp(&collect_sum_query(
        Value::Data(XSDAnySimpleType::UnsignedShort(5)),
        duint(0),
    ))
    .unwrap();
    assert!(matches!(
        ground.relation.statement,
        CompiledRelationStatement::CollectionSum {
            template: CollectionSumTemplate::UnsignedConstant(5),
            datatype: CanonicalDatatype::UInt16,
            ..
        }
    ));

    let Query::And(mut nested) = collect_sum_query(Value::Variable("score".into()), duint(0)) else {
        unreachable!()
    };
    let Query::Collect(collect) = &mut nested.and[0] else {
        unreachable!()
    };
    let branch = (*collect.query).clone();
    collect.query = Box::new(Query::Or(Or {
        or: vec![branch.clone(), branch],
    }));
    let nested = plan_bgp(&Query::And(nested)).unwrap();
    assert!(matches!(
        nested.relation.statement,
        CompiledRelationStatement::CollectionSum { input, .. }
            if matches!(input.statement, CompiledRelationStatement::BagUnion { .. })
    ));

    let Query::And(mut filtered) = collect_sum_query(
        Value::Variable("score".into()),
        DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
    ) else {
        unreachable!()
    };
    filtered.and.push(less(dvar("total"), decimal_value("0")));
    let filtered = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::And(filtered)),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(filtered.count_variable.as_deref(), Some("rows"));
    assert!(matches!(
        filtered.relation.statement,
        CompiledRelationStatement::GeneratedI64Filter { column: 0, .. }
    ));

    for template in [
        Value::Variable("unbound".into()),
        Value::Data(XSDAnySimpleType::Decimal("1".parse().unwrap())),
        Value::Data(XSDAnySimpleType::Float(1.0)),
    ] {
        reject(
            collect_sum_query(template, duint(0)),
            BgpPlanError::CollectionSumUnsupportedValue,
        );
    }
    let Query::And(valid) = collect_sum_query(Value::Variable("score".into()), duint(0)) else {
        unreachable!()
    };
    reject(
        and_of(vec![valid.and[1].clone(), valid.and[0].clone()]),
        BgpPlanError::CollectionSumUnsupportedShape,
    );
    let Query::Sum(mut wrong_list) = valid.and[1].clone() else {
        unreachable!()
    };
    wrong_list.list = DataValue::Variable("other".into());
    reject(
        and_of(vec![valid.and[0].clone(), Query::Sum(wrong_list)]),
        BgpPlanError::CollectionSumUnsupportedShape,
    );
}

#[test]
fn immediate_collect_length_compiles_distinct_empty_policy_and_wrappers() {
    let compiled = plan_bgp(&collect_length_query()).unwrap();
    assert_eq!(compiled.relation.schema, ["cardinality"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CollectionCount { .. }
    ));

    let Query::And(mut nested) = collect_length_query() else {
        unreachable!()
    };
    let Query::Collect(collect) = &mut nested.and[0] else {
        unreachable!()
    };
    let branch = (*collect.query).clone();
    collect.query = Box::new(Query::Or(Or {
        or: vec![branch.clone(), branch],
    }));
    let nested = plan_bgp(&Query::And(nested)).unwrap();
    assert!(matches!(
        nested.relation.statement,
        CompiledRelationStatement::CollectionCount { input }
            if matches!(input.statement, CompiledRelationStatement::BagUnion { .. })
    ));

    let Query::And(mut filtered) = collect_length_query() else {
        unreachable!()
    };
    filtered
        .and
        .push(gte(dvar("cardinality"), decimal_value("2")));
    let filtered = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::And(filtered)),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(filtered.count_variable.as_deref(), Some("rows"));
    assert!(matches!(
        filtered.relation.statement,
        CompiledRelationStatement::GeneratedU64Filter { column: 0, .. }
    ));

    let Query::And(valid) = collect_length_query() else {
        unreachable!()
    };
    reject(
        and_of(vec![valid.and[1].clone(), valid.and[0].clone()]),
        BgpPlanError::CollectionCountUnsupportedShape,
    );
    let Query::Length(mut wrong_list) = valid.and[1].clone() else {
        unreachable!()
    };
    wrong_list.list = DataValue::Variable("other".into());
    reject(
        and_of(vec![valid.and[0].clone(), Query::Length(wrong_list)]),
        BgpPlanError::CollectionCountUnsupportedShape,
    );
    reject(
        valid.and[1].clone(),
        BgpPlanError::UnsupportedConstruct { construct: "Length" },
    );
}

#[test]
fn immediate_group_length_binds_keys_generated_count_and_comparisons() {
    let compiled = plan_bgp(&two_node_key_count_with_bound(duint(0))).unwrap();
    assert_eq!(compiled.relation.schema, ["right", "left", "cardinality"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GroupCount { ref key_columns, .. }
            if key_columns == &[1, 0]
    ));

    let Query::And(mut filtered) = two_node_key_count_with_bound(duint(0)) else {
        unreachable!()
    };
    filtered
        .and
        .push(gte(dvar("cardinality"), decimal_value("2")));
    let filtered = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::And(filtered)),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(filtered.count_variable.as_deref(), Some("rows"));
    assert!(matches!(
        filtered.relation.statement,
        CompiledRelationStatement::GeneratedU64Filter { column: 2, .. }
    ));

    let Query::And(mut global) = two_node_key_count_with_bound(duint(0)) else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut global.and[0] else {
        unreachable!()
    };
    group.group_by.clear();
    let global = plan_bgp(&Query::And(global)).unwrap();
    assert_eq!(global.relation.schema, ["cardinality"]);
    assert!(matches!(
        global.relation.statement,
        CompiledRelationStatement::GroupCount { ref key_columns, .. }
            if key_columns.is_empty()
    ));

    let Query::And(valid) = two_node_key_count_with_bound(duint(0)) else {
        unreachable!()
    };
    let Query::GroupBy(group) = &valid.and[0] else {
        unreachable!()
    };
    for group_by in [
        vec!["left".into(), "left".into()],
        vec!["left".into(), "right".into(), "extra".into()],
    ] {
        let mut invalid = group.clone();
        invalid.group_by = group_by;
        reject(
            and_of(vec![Query::GroupBy(invalid), valid.and[1].clone()]),
            BgpPlanError::GroupCountUnsupportedShape,
        );
    }
}

#[test]
fn two_node_key_group_sum_binds_declared_priority_domains_and_result_order() {
    for (bound, datatype) in [
        (duint(0), CanonicalDatatype::UInt32),
        (
            DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
            CanonicalDatatype::Int64,
        ),
    ] {
        let compiled = plan_bgp(&two_node_key_sum_with_bound(bound)).unwrap();
        assert_eq!(compiled.relation.schema, ["right", "left", "total"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupSum {
                ref key_columns,
                value_column: 2,
                datatype: actual,
                ..
            } if key_columns == &[1, 0] && actual == datatype
        ));
    }

    let Query::And(mut filtered) = two_node_key_sum_with_bound(DataValue::Data(
        XSDAnySimpleType::Long(i64::MIN),
    )) else {
        unreachable!()
    };
    filtered
        .and
        .push(less(dvar("total"), decimal_value("0")));
    let filtered = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::And(filtered)),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(filtered.count_variable.as_deref(), Some("rows"));
    assert!(matches!(
        filtered.relation.statement,
        CompiledRelationStatement::GeneratedI64Filter { column: 2, .. }
    ));

    let Query::And(valid) = two_node_key_sum_with_bound(duint(0)) else {
        unreachable!()
    };
    let Query::GroupBy(group) = &valid.and[0] else {
        unreachable!()
    };
    for group_by in [
        vec!["left".into(), "left".into()],
        vec!["left".into(), "right".into(), "extra".into()],
    ] {
        let mut invalid = group.clone();
        invalid.group_by = group_by;
        reject(
            and_of(vec![Query::GroupBy(invalid), valid.and[1].clone()]),
            BgpPlanError::GroupSumUnsupportedShape,
        );
    }
}

#[test]
fn immediate_group_sum_compiles_node_key_and_generated_decimal_result() {
    let compiled = plan_bgp(&grouped_u32_sum()).unwrap();
    assert_eq!(compiled.relation.schema, ["category", "total"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GroupSum {
            ref key_columns,
            value_column: 1,
            datatype: CanonicalDatatype::UInt32,
            ..
        } if key_columns == &[0]
    ));

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(grouped_u32_sum()),
        count: DataValue::Variable("groups".into()),
    }))
    .unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("groups"));

    for (bound, expected) in [
        (
            DataValue::Data(XSDAnySimpleType::UnsignedByte(0)),
            CanonicalDatatype::UInt8,
        ),
        (
            DataValue::Data(XSDAnySimpleType::UnsignedShort(0)),
            CanonicalDatatype::UInt16,
        ),
        (
            DataValue::Data(XSDAnySimpleType::UnsignedLong(0)),
            CanonicalDatatype::UInt64,
        ),
        (
            DataValue::Data(XSDAnySimpleType::Byte(0)),
            CanonicalDatatype::Int8,
        ),
        (
            DataValue::Data(XSDAnySimpleType::Short(0)),
            CanonicalDatatype::Int16,
        ),
        (
            DataValue::Data(XSDAnySimpleType::Int(0)),
            CanonicalDatatype::Int32,
        ),
        (
            DataValue::Data(XSDAnySimpleType::Long(0)),
            CanonicalDatatype::Int64,
        ),
        (
            DataValue::Data(XSDAnySimpleType::Integer(0)),
            CanonicalDatatype::Int64,
        ),
    ] {
        let compiled = plan_bgp(&grouped_sum_with_bound(bound)).unwrap();
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupSum { datatype, .. } if datatype == expected
        ));
    }
}

#[test]
fn signed_group_sum_comparisons_bind_domain_operator_order_and_i64_constants() {
    let Query::And(mut signed) = grouped_sum_with_bound(DataValue::Data(
        XSDAnySimpleType::Long(i64::MIN),
    )) else {
        unreachable!()
    };
    signed
        .and
        .push(greater(dvar("total"), decimal_value("-1")));
    signed
        .and
        .push(lte(decimal_value("9223372036854775807"), dvar("total")));
    let query = Query::And(signed.clone());
    let compiled = plan_bgp(&query).unwrap();
    let CompiledRelationStatement::GeneratedI64Filter {
        input,
        op,
        constant,
        constant_on_left,
        ..
    } = &compiled.relation.statement
    else {
        panic!("expected outer generated-i64 filter")
    };
    assert_eq!(*op, GeneratedU64ComparisonOp::Le);
    assert_eq!(*constant, i64::MAX);
    assert!(*constant_on_left);
    assert!(matches!(
        input.statement,
        CompiledRelationStatement::GeneratedI64Filter {
            op: GeneratedU64ComparisonOp::Gt,
            constant: -1,
            constant_on_left: false,
            ..
        }
    ));

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(query),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("rows"));

    let mut gated = signed.clone();
    gated.and.truncate(2);
    let gate = tv_obj_node("category", "gate", "yes");
    gated.and.push(Query::If(When {
        query: Box::new(gate.clone()),
        consequent: Box::new(gate),
    }));
    let gated = plan_bgp(&Query::Count(Count {
        query: Box::new(Query::And(gated)),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(gated.count_variable.as_deref(), Some("rows"));
    assert!(matches!(
        gated.relation.statement,
        CompiledRelationStatement::When { .. }
    ));

    for constant in [
        "1.5",
        "1e2",
        "9223372036854775808",
        "-9223372036854775809",
    ] {
        let mut invalid = signed.clone();
        invalid.and.truncate(2);
        invalid
            .and
            .push(greater(dvar("total"), decimal_value(constant)));
        reject(
            Query::And(invalid),
            BgpPlanError::GroupSumComparisonUnsupportedConstant,
        );
    }
    let mut wrong_variable = signed;
    wrong_variable.and.truncate(2);
    wrong_variable
        .and
        .push(greater(dvar("other"), decimal_value("0")));
    reject(
        Query::And(wrong_variable),
        BgpPlanError::GroupSumComparisonUnsupportedShape,
    );

    let Query::And(mut unsigned) = grouped_u32_sum() else {
        unreachable!()
    };
    unsigned
        .and
        .push(greater(dvar("total"), decimal_value("-1")));
    reject(
        Query::And(unsigned),
        BgpPlanError::GroupSumComparisonUnsupportedConstant,
    );
}

fn global_u32_sum() -> Query {
    let Query::And(mut query) = grouped_u32_sum() else {
        unreachable!()
    };
    let Query::GroupBy(group) = &mut query.and[0] else {
        unreachable!()
    };
    group.group_by.clear();
    Query::And(query)
}

#[test]
fn immediate_zero_key_group_sum_compiles_one_generated_decimal_column() {
    let compiled = plan_bgp(&global_u32_sum()).unwrap();
    assert_eq!(compiled.relation.schema, ["total"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::GroupSum {
            ref key_columns,
            value_column: 1,
            datatype: CanonicalDatatype::UInt32,
            ..
        } if key_columns.is_empty()
    ));
    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(global_u32_sum()),
        count: DataValue::Variable("rows".into()),
    }))
    .unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("rows"));
}

#[test]
fn generated_decimal_foreign_inputs_are_descriptor_driven_and_canonical() {
    let unsigned = plan_bgp(&grouped_u32_sum()).unwrap();
    assert_eq!(
        decode_executed_result_inputs(
            &unsigned,
            &["category".into(), "total".into()],
            &[vec![
                ExecutedResultInput::Id(7),
                ExecutedResultInput::GeneratedDecimal("5".into()),
            ]],
        )
        .unwrap(),
        vec![vec![
            ExecutedResultValue::Id(7),
            ExecutedResultValue::GeneratedDecimalU64(5),
        ]]
    );

    let signed = plan_bgp(&grouped_sum_with_bound(DataValue::Data(
        XSDAnySimpleType::Long(i64::MIN),
    )))
    .unwrap();
    for (lexical, expected) in [
        ("-1", ExecutedResultValue::GeneratedDecimalI64(-1)),
        ("0", ExecutedResultValue::GeneratedDecimalI64(0)),
        ("1", ExecutedResultValue::GeneratedDecimalI64(1)),
    ] {
        let decoded = decode_executed_result_inputs(
            &signed,
            &["category".into(), "total".into()],
            &[vec![
                ExecutedResultInput::Id(7),
                ExecutedResultInput::GeneratedDecimal(lexical.into()),
            ]],
        )
        .unwrap();
        assert_eq!(decoded[0][1], expected);
    }
    for (compiled, lexical) in [
        (&unsigned, "-1"),
        (&unsigned, "01"),
        (&signed, "18446744073709551615"),
    ] {
        assert!(decode_executed_result_inputs(
            compiled,
            &["category".into(), "total".into()],
            &[vec![
                ExecutedResultInput::Id(7),
                ExecutedResultInput::GeneratedDecimal(lexical.into()),
            ]],
        )
        .is_err());
    }
    assert!(decode_executed_result_inputs(
        &signed,
        &["category".into(), "total".into()],
        &[vec![ExecutedResultInput::Id(7), ExecutedResultInput::Id(1)]],
    )
    .is_err());

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(global_u32_sum()),
        count: DataValue::Variable("groups".into()),
    }))
    .unwrap();
    assert_eq!(
        decode_executed_result_inputs(
            &counted,
            &["groups".into()],
            &[vec![ExecutedResultInput::GeneratedDecimal("0".into())]],
        )
        .unwrap(),
        vec![vec![ExecutedResultValue::Id(0)]]
    );
}

#[test]
fn immediate_group_sum_rejects_list_leak_order_wrong_type_and_non_node_key() {
    let Query::And(valid) = grouped_u32_sum() else {
        unreachable!()
    };
    reject(
        and_of(vec![valid.and[1].clone(), valid.and[0].clone()]),
        BgpPlanError::GroupSumUnsupportedShape,
    );

    let Query::GroupBy(mut group) = valid.and[0].clone() else {
        unreachable!()
    };
    group.grouped_value = Value::Variable("leaked".into());
    reject(
        and_of(vec![Query::GroupBy(group), valid.and[1].clone()]),
        BgpPlanError::GroupSumUnsupportedShape,
    );

    let unsupported_child = and_of(vec![
        tv("category", "score", "score"),
        gte(
            dvar("score"),
            DataValue::Data(XSDAnySimpleType::Float(0.0)),
        ),
    ]);
    let unsupported_group = Query::GroupBy(GroupBy {
        template: Value::Variable("score".into()),
        group_by: vec!["category".into()],
        grouped_value: Value::Variable("scores".into()),
        query: Box::new(unsupported_child),
    });
    reject(
        and_of(vec![unsupported_group, valid.and[1].clone()]),
        BgpPlanError::GroupSumUnsupportedValue,
    );

    let object_key_group = Query::GroupBy(GroupBy {
        template: Value::Variable("score".into()),
        group_by: vec!["category".into()],
        grouped_value: Value::Variable("scores".into()),
        query: Box::new(and_of(vec![
            tv("subject", "score", "category"),
            gte(dvar("category"), duint(0)),
            tv("subject", "other", "score"),
            gte(dvar("score"), duint(0)),
        ])),
    });
    reject(
        and_of(vec![object_key_group, valid.and[1].clone()]),
        BgpPlanError::GroupSumUnsupportedShape,
    );
}

fn typed_object_query(value: XSDAnySimpleType) -> Query {
    and_of(vec![
        Query::Triple(terminusdb_woql2::triple::Triple {
            subject: NodeValue::Variable("x".into()),
            predicate: NodeValue::Node(iri("p")),
            object: Value::Data(value),
            graph: None,
        }),
        tv("x", "q", "z"),
    ])
}

/// The store's own canonicalization (datatype tag + lexical bytes) of a typed dict entry —
/// the bytes the compiler must emit for a matching object value constant.
fn store_value_of(entry: TypedDictEntry) -> (CanonicalDatatype, Vec<u8>) {
    match CanonicalObject::from_store_object(&ValueTriple::new_value("", "", entry).object) {
        CanonicalObject::Value { datatype, lexical } => (datatype, lexical),
        CanonicalObject::Node(_) => unreachable!("a value entry canonicalizes to a Value object"),
    }
}

/// The store's committed order-preserving encoding of a typed dict entry as a `u64` (the
/// big-endian read of its fixed-width `to_lexical` bytes) — the compiler's `c` must match.
fn store_op_value(entry: TypedDictEntry) -> u64 {
    let (_dt, lexical) = store_value_of(entry);
    let mut buf = [0u8; 8];
    buf[8 - lexical.len()..].copy_from_slice(&lexical);
    u64::from_be_bytes(buf)
}

/// Assert the compiled atom for `pred` carries a `ValueConstant` byte-identical to the
/// store's own canonicalization of `entry` (datatype tag + lexical).
#[track_caller]
fn assert_value_constant(c: &CompiledBgp, pred: &str, entry: TypedDictEntry) {
    let term = iri(pred).into_bytes();
    let atom = c
        .bgp_plan()
        .atoms
        .iter()
        .find(|a| a.predicate_term == term)
        .expect("an atom with that predicate");
    let (edt, elex) = store_value_of(entry);
    match &atom.object {
        AtomTerm::ValueConstant(dt, lex) => {
            assert_eq!(*dt, edt, "value-constant datatype tag");
            assert_eq!(*lex, elex, "value-constant lexical bytes");
        }
        other => panic!("expected a ValueConstant object, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// ACCEPT + correct plan structure (variable-blind).
// ----------------------------------------------------------------------------

#[test]
fn accepts_three_atom_path_with_correct_plan() {
    // t(?x,p,?y), t(?y,p,?z), t(?z,p,?w)
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "p", "z"),
        tv("z", "p", "w"),
    ]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.relation.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.bgp_plan().atoms.len(), 3);
    assert_eq!(c.bgp_plan().joins.len(), 2);
    // Left-deep on the shared middle variable each step: matches the store's own
    // `layer_bgp_three_atom_path_end_to_end` plan (JoinStep{1,0}, {2,0}).
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (1, 0));
    assert_eq!((c.bgp_plan().joins[1].acc_key, c.bgp_plan().joins[1].rel_key), (2, 0));
    // Predicate terms are the IRI bytes, in placement order.
    assert_eq!(c.bgp_plan().atoms[0].predicate_term, iri("p").into_bytes());
    assert!(c.projection.is_none());
}

#[test]
fn accepts_star_shared_subject_three_atoms() {
    // t(?c,p,?a), t(?c,q,?b), t(?c,r,?d) — c is the shared key in all three atoms.
    let q = and_of(vec![
        tv("c", "p", "a"),
        tv("c", "q", "b"),
        tv("c", "r", "d"),
    ]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.relation.schema, vec!["a", "b", "c", "d"]);
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (0, 0));
    assert_eq!((c.bgp_plan().joins[1].acc_key, c.bgp_plan().joins[1].rel_key), (1, 0));
}

#[test]
fn accepts_variable_shared_by_three_atoms_mixed_positions() {
    // t(?a,p,?x), t(?x,q,?b), t(?x,r,?c) — x appears in 3 atoms (object then subject).
    let q = and_of(vec![
        tv("a", "p", "x"),
        tv("x", "q", "b"),
        tv("x", "r", "c"),
    ]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.relation.schema, vec!["a", "b", "x", "c"]);
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (1, 0));
    assert_eq!((c.bgp_plan().joins[1].acc_key, c.bgp_plan().joins[1].rel_key), (1, 0));
}

#[test]
fn flattens_nested_ands() {
    // And[ And[t1, t2], t3 ] flattens to the same 3-atom path.
    let inner = and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")]);
    let q = and_of(vec![inner, tv("z", "p", "w")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.bgp_plan().atoms.len(), 3);
}

#[test]
fn select_records_projection_indices() {
    let bgp = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "p", "z"),
        tv("z", "p", "w"),
    ]);
    let q = Query::Select(Select {
        variables: vec!["x".into(), "w".into()],
        query: Box::new(bgp),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z", "w"]);
    // Proof still proves the FULL BGP; projection just names read columns.
    assert_eq!(c.projection, Some(vec![0, 3]));
}

#[test]
fn nested_selects_compose_and_outermost_controls_visible_columns() {
    let bgp = and_of(vec![tv("x", "p", "y"), tv("y", "q", "z")]);
    let inner = Query::Select(Select {
        variables: vec!["x".into(), "z".into()],
        query: Box::new(bgp),
    });
    let q = Query::Select(Select {
        variables: vec!["z".into()],
        query: Box::new(inner),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(c.projection, Some(vec![2]));
}

#[test]
fn nested_select_rejects_a_variable_hidden_by_its_inner_select() {
    let bgp = and_of(vec![tv("x", "p", "y"), tv("y", "q", "z")]);
    let inner = Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(bgp),
    });
    let q = Query::Select(Select {
        variables: vec!["z".into()],
        query: Box::new(inner),
    });
    reject(
        q,
        BgpPlanError::SelectedVariableNotInBgp { var: "z".into() },
    );
}

#[test]
fn link_and_data_compile_to_explicit_object_kind_filters() {
    let link = Query::Link(Link {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("knows")),
        object: NodeValue::Variable("y".into()),
        graph: GraphType::Instance,
    });
    let link = plan_bgp(&link).unwrap();
    assert_eq!(link.relation.schema, ["x", "y"]);
    assert_eq!(
        link.bgp_plan().atoms[0].object_kind,
        ObjectKindFilter::Node
    );

    let data = Query::Data(Data {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("active")),
        object: DataValue::Variable("active".into()),
        graph: GraphType::Instance,
    });
    let data = plan_bgp(&data).unwrap();
    assert_eq!(data.relation.schema, ["x", "active"]);
    assert_eq!(
        data.bgp_plan().atoms[0].object_kind,
        ObjectKindFilter::Literal
    );
}

#[test]
fn data_numeric_range_subsumes_literal_kind_and_link_range_rejects() {
    let data = Query::Data(Data {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("score")),
        object: DataValue::Variable("score".into()),
        graph: GraphType::Instance,
    });
    let compiled = plan_bgp(&and_of(vec![
        data,
        greater(dvar("score"), duint(10)),
    ]))
    .unwrap();
    let atom = &compiled.bgp_plan().atoms[0];
    assert_eq!(atom.object_kind, ObjectKindFilter::Any);
    assert!(atom.object_range.is_some());

    let link = Query::Link(Link {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("knows")),
        object: NodeValue::Variable("score".into()),
        graph: GraphType::Instance,
    });
    reject(
        and_of(vec![link, greater(dvar("score"), duint(10))]),
        BgpPlanError::RangeOnNodeObject {
            var: "score".into(),
        },
    );
}

#[test]
fn data_and_link_retain_existing_graph_and_constant_guards() {
    for query in [
        Query::Data(Data {
            subject: NodeValue::Variable("x".into()),
            predicate: NodeValue::Node(iri("score")),
            object: DataValue::Variable("score".into()),
            graph: GraphType::Schema,
        }),
        Query::Link(Link {
            subject: NodeValue::Variable("x".into()),
            predicate: NodeValue::Node(iri("knows")),
            object: NodeValue::Variable("y".into()),
            graph: GraphType::Schema,
        }),
    ] {
        reject(query, BgpPlanError::SchemaGraphUnsupported);
    }

    let constant = Query::Data(Data {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("score")),
        object: DataValue::Data(XSDAnySimpleType::UnsignedInt(10)),
        graph: GraphType::Instance,
    });
    let compiled = plan_bgp(&constant).unwrap();
    assert!(matches!(
        compiled.bgp_plan().atoms[0].object,
        AtomTerm::ValueConstant(CanonicalDatatype::UInt32, _)
    ));
    assert_eq!(
        compiled.bgp_plan().atoms[0].object_kind,
        ObjectKindFilter::Any
    );
}

#[test]
fn retains_using_scope_and_strips_instance_from_and_pin() {
    let q = Query::Using(Using {
        collection: "admin/music/local/branch/main".into(),
        query: Box::new(Query::From(From {
            graph: "instance".into(),
            query: Box::new(Query::Pin(Pin {
                query: Box::new(and_of(vec![tv("x", "p", "y")])),
            })),
        })),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(
        c.collection.as_deref(),
        Some("admin/music/local/branch/main")
    );
    assert_eq!(c.relation.schema, vec!["x", "y"]);
}

#[test]
fn nested_using_retains_the_innermost_collection_scope() {
    let q = Query::Using(Using {
        collection: "admin/outer/local/branch/main".into(),
        query: Box::new(Query::Using(Using {
            collection: "admin/inner/local/branch/main".into(),
            query: Box::new(tv("x", "p", "y")),
        })),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(
        c.collection.as_deref(),
        Some("admin/inner/local/branch/main")
    );
}

#[test]
fn transparent_execution_controls_can_wrap_individual_conjuncts() {
    let q = and_of(vec![
        Query::Pin(Pin {
            query: Box::new(tv("x", "p", "y")),
        }),
        Query::Immediately(Immediately {
            query: Box::new(Query::From(From {
                graph: "instance".into(),
                query: Box::new(tv("y", "q", "z")),
            })),
        }),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(c.bgp_plan().atoms.len(), 2);
}

#[test]
fn true_and_comment_are_binding_free_conjuncts() {
    let q = and_of(vec![
        Query::True(True {}),
        Query::Comment(Comment {
            comment: dstr("why this query exists"),
            query: Some(Box::new(tv("ignored", "ignored", "ignored_too"))),
        }),
        tv("x", "p", "y"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!(c.bgp_plan().atoms.len(), 1);
}

#[test]
fn count_retains_the_output_binding_and_hides_inner_columns() {
    let q = Query::Count(Count {
        query: Box::new(and_of(vec![tv("x", "p", "y")])),
        count: dvar("count"),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!(c.projection, Some(Vec::new()));
    assert_eq!(c.count_variable.as_deref(), Some("count"));
}

#[test]
fn rejects_count_into_a_constant() {
    let q = Query::Count(Count {
        query: Box::new(and_of(vec![tv("x", "p", "y")])),
        count: dstr("not-a-variable"),
    });
    reject(q, BgpPlanError::CountOutputNotVariable);
}

#[test]
fn count_accepts_an_inner_projection_but_exposes_only_the_count() {
    let inner = Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(and_of(vec![tv("x", "p", "y")])),
    });
    let q = Query::Count(Count {
        query: Box::new(inner),
        count: dvar("count"),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.count_variable.as_deref(), Some("count"));
    assert_eq!(c.projection, Some(Vec::new()));
}

#[test]
fn select_outside_count_can_only_expose_the_count_binding() {
    let count = Query::Count(Count {
        query: Box::new(and_of(vec![tv("x", "p", "y")])),
        count: dvar("count"),
    });
    let valid = Query::Select(Select {
        variables: vec!["count".into()],
        query: Box::new(count.clone()),
    });
    let c = plan_bgp(&valid).unwrap();
    assert_eq!(c.count_variable.as_deref(), Some("count"));
    assert_eq!(c.projection, Some(Vec::new()));

    let invalid = Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(count),
    });
    reject(
        invalid,
        BgpPlanError::SelectedVariableNotInBgp { var: "x".into() },
    );
}

#[test]
fn dsl_macro_ast_compiles_identically() {
    // Prove real-AST compatibility: build the same path with woql2's DSL macros.
    use terminusdb_woql2::{and, t, triple, var};
    let p = iri("p");
    let q = and!(
        t!(var!(x), p.as_str(), var!(y)),
        t!(var!(y), p.as_str(), var!(z)),
        t!(var!(z), p.as_str(), var!(w)),
    );
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.bgp_plan().atoms.len(), 3);
    assert_eq!((c.bgp_plan().joins[1].acc_key, c.bgp_plan().joins[1].rel_key), (2, 0));
}

// ----------------------------------------------------------------------------
// ACCEPT/REJECT boundaries for compound operators.
// ----------------------------------------------------------------------------

#[test]
fn accepts_compatible_or_and_rejects_incompatible_schemas() {
    let q = Query::Or(Or {
        or: vec![tv("x", "p", "y"), tv("x", "q", "y")],
    });
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, vec!["x", "y"]);
    let (branches, mappings) = compiled.relation.as_bag_union().unwrap();
    assert_eq!(branches.len(), 2);
    assert_eq!(mappings, [vec![0, 1], vec![0, 1]]);

    let complex = Query::Or(Or {
        or: vec![
            and_of(vec![tv("x", "p", "y"), tv("a", "r", "b")]),
            and_of(vec![tv("x", "q", "y"), tv("a", "s", "b")]),
        ],
    });
    let complex = plan_bgp(&complex).unwrap();
    assert_eq!(complex.relation.schema, vec!["x", "y", "a", "b"]);
    assert!(complex
        .relation
        .as_bag_union()
        .unwrap()
        .0
        .iter()
        .all(|branch| branch.as_bgp().unwrap().joins[0].is_cross_product()));

    let incompatible = Query::Or(Or {
        or: vec![tv("x", "p", "y"), tv("x", "q", "z")],
    });
    reject(incompatible, BgpPlanError::OrIncompatibleSchema);

    reject(
        Query::Or(Or { or: vec![] }),
        BgpPlanError::OrTooFewBranches { count: 0 },
    );

    let routed = Query::Or(Or {
        or: vec![
            Query::Using(Using {
                collection: "admin/one".into(),
                query: Box::new(tv("x", "p", "y")),
            }),
            Query::Using(Using {
                collection: "admin/two".into(),
                query: Box::new(tv("x", "q", "y")),
            }),
        ],
    });
    reject(routed, BgpPlanError::OrRoutingConflict);
}

#[test]
fn accepts_correlated_not_as_scalar_left_anti_join() {
    let q = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("y", "q", "z")),
        }),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, vec!["x", "y"]);
    assert_eq!(compiled.relation.as_left_anti().unwrap().2, [(1, 0)]);
}

#[test]
fn accepts_global_and_two_key_not() {
    let uncorrelated = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("a", "q", "b")),
        }),
    ]);
    let global = plan_bgp(&uncorrelated).unwrap();
    assert!(global.relation.as_left_anti().unwrap().2.is_empty());
    let two_keys = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("x", "q", "y")),
        }),
    ]);
    let compiled = plan_bgp(&two_keys).unwrap();
    assert_eq!(compiled.relation.as_left_anti().unwrap().2, [(0, 0), (1, 1)]);
}

#[test]
fn optional_requires_outer_bindings_and_compiles_correlated_join() {
    let q = Query::WoqlOptional(WoqlOptional {
        query: Box::new(tv("x", "p", "y")),
    });
    reject(q, BgpPlanError::OptionalMissingOuter);

    let q = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
    ]);
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(compiled.relation.as_left_outer().unwrap().2, [(1, 0)]);

    let selected = Query::Select(Select {
        variables: vec!["x".into(), "z".into()],
        query: Box::new(q.clone()),
    });
    let selected = plan_bgp(&selected).unwrap();
    assert_eq!(selected.projection, Some(vec![0, 2]));
    assert!(selected.relation.as_left_outer().is_some());

    let counted = Query::Count(Count {
        query: Box::new(q),
        count: dvar("count"),
    });
    let counted = plan_bgp(&counted).unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("count"));
    assert!(counted.relation.as_left_outer().is_some());

    let global = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("a", "q", "b")),
        }),
    ]);
    let global = plan_bgp(&global).unwrap();
    assert_eq!(global.relation.schema, vec!["x", "y", "a", "b"]);
    assert!(global.relation.as_left_outer().unwrap().2.is_empty());
}

#[test]
fn recursive_operators_accept_composed_relations_and_flatten_nested_or() {
    let optional_branch = |predicate: &str| {
        and_of(vec![
            tv("x", predicate, "y"),
            Query::WoqlOptional(WoqlOptional {
                query: Box::new(tv("y", "q", "z")),
            }),
        ])
    };
    let or_of_optional = Query::Or(Or {
        or: vec![optional_branch("p"), optional_branch("r")],
    });
    let compiled = plan_bgp(&or_of_optional).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "y", "z"]);
    assert!(compiled
        .relation
        .as_bag_union()
        .unwrap()
        .0
        .iter()
        .all(|branch| branch.as_left_outer().is_some()));

    let optional_right_or = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(Query::Or(Or {
                or: vec![tv("y", "q", "z"), tv("y", "r", "z")],
            })),
        }),
    ]);
    let compiled = plan_bgp(&optional_right_or).unwrap();
    assert!(compiled.relation.as_left_outer().unwrap().1.as_bag_union().is_some());

    let not_over_or = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(Query::Or(Or {
                or: vec![tv("y", "q", "z"), tv("y", "r", "z")],
            })),
        }),
    ]);
    let compiled = plan_bgp(&not_over_or).unwrap();
    assert!(compiled.relation.as_left_anti().unwrap().1.as_bag_union().is_some());

    let nested = Query::Or(Or {
        or: vec![
            tv("x", "p", "y"),
            Query::Or(Or {
                or: vec![tv("x", "q", "y"), tv("x", "r", "y")],
            }),
        ],
    });
    assert_eq!(
        plan_bgp(&nested)
            .unwrap()
            .relation
            .as_bag_union()
            .unwrap()
            .0
            .len(),
        3
    );
}

#[test]
fn sequential_operators_allow_sound_keys_and_reject_nullable_correlation() {
    let sequential_optional = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("x", "r", "w")),
        }),
    ]);
    let compiled = plan_bgp(&sequential_optional).unwrap();
    let (prior, _, _) = compiled.relation.as_left_outer().unwrap();
    assert!(prior.as_left_outer().is_some());

    let nullable_key = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("z", "r", "w")),
        }),
    ]);
    reject(nullable_key, BgpPlanError::OptionalUnsupportedSubplan);

    // A nullable key on the independently proved right side is Prolog-unbound
    // and could become bound by the left row; the proof composition is strict
    // equality, so this must fail closed rather than model different semantics.
    let nullable_right_key = and_of(vec![
        tv("x", "p", "z"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(and_of(vec![
                tv("a", "q", "b"),
                Query::WoqlOptional(WoqlOptional {
                    query: Box::new(tv("b", "r", "z")),
                }),
            ])),
        }),
    ]);
    reject(
        nullable_right_key,
        BgpPlanError::OptionalUnsupportedSubplan,
    );

    let sequential_not = and_of(vec![
        tv("x", "p", "y"),
        Query::Not(Not {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::Not(Not {
            query: Box::new(tv("x", "r", "w")),
        }),
    ]);
    let compiled = plan_bgp(&sequential_not).unwrap();
    let (prior, _, _) = compiled.relation.as_left_anti().unwrap();
    assert!(prior.as_left_anti().is_some());

    let optional_then_not = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::Not(Not {
            query: Box::new(tv("x", "r", "w")),
        }),
    ]);
    let compiled = plan_bgp(&optional_then_not).unwrap();
    let (prior, _, _) = compiled.relation.as_left_anti().unwrap();
    assert!(prior.as_left_outer().is_some());

    let nullable_then_not = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        Query::Not(Not {
            query: Box::new(tv("z", "r", "w")),
        }),
    ]);
    reject(nullable_then_not, BgpPlanError::NotUnsupportedSubplan);
}

#[test]
fn equals_accepts_only_bound_nonnullable_exact_domains() {
    let node_constant = and_of(vec![
        tv("x", "p", "y"),
        equals(value_var("x"), value_node("a")),
    ]);
    let compiled = plan_bgp(&node_constant).unwrap();
    assert!(matches!(
        compiled.relation.as_equality().unwrap().1,
        RelationEqualityPlan::ColumnConstant {
            column: 0,
            constant_on_left: false,
            ..
        }
    ));

    let node_columns = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        equals(value_var("x"), value_var("y")),
    ]);
    assert!(matches!(
        plan_bgp(&node_columns)
            .unwrap()
            .relation
            .as_equality()
            .unwrap()
            .1,
        RelationEqualityPlan::Columns { .. }
    ));

    reject(
        and_of(vec![
            tv("x", "p", "y"),
            equals(value_var("y"), value_node("a")),
        ]),
        BgpPlanError::EqualsUnknownDomain { var: "y".into() },
    );
    reject(
        and_of(vec![
            equals(value_var("x"), value_node("a")),
            tv("x", "p", "y"),
        ]),
        BgpPlanError::ConjunctNotTriple {
            construct: "Triple",
        },
    );
    reject(
        and_of(vec![
            tv_pred_var("x", "predicate", "y"),
            equals(value_var("predicate"), value_var("x")),
        ]),
        BgpPlanError::EqualsDomainMismatch,
    );
}

#[test]
fn equals_exact_typed_domains_reject_executor_numeric_coercion() {
    let exact = and_of(vec![
        tv("x", "age", "age"),
        greater(dvar("age"), duint(0)),
        equals(value_var("age"), value_uint(7)),
    ]);
    assert!(plan_bgp(&exact).unwrap().relation.as_equality().is_some());

    reject(
        and_of(vec![
            tv("x", "age", "age"),
            greater(dvar("age"), duint(0)),
            equals(value_var("age"), value_integer(7)),
        ]),
        BgpPlanError::EqualsDomainMismatch,
    );
    reject(
        and_of(vec![
            tv("x", "a", "a"),
            tv("x", "b", "b"),
            greater(dvar("a"), duint(0)),
            greater(dvar("b"), dint(0)),
            equals(value_var("a"), value_var("b")),
        ]),
        BgpPlanError::EqualsDomainMismatch,
    );
    reject(
        and_of(vec![
            tv("x", "p", "y"),
            equals(value_uint(7), value_integer(7)),
        ]),
        BgpPlanError::EqualsCoerciveNumeric,
    );
}

#[test]
fn equals_composes_with_recursive_relations_projection_and_count() {
    let filtered = and_of(vec![
        tv("x", "p", "y"),
        equals(value_node("a"), value_var("x")),
    ]);
    let optional = and_of(vec![
        filtered.clone(),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(and_of(vec![
                tv("y", "q", "z"),
                equals(value_var("y"), value_node("b")),
            ])),
        }),
    ]);
    let union = Query::Or(Or {
        or: vec![optional.clone(), optional],
    });
    assert!(plan_bgp(&union).unwrap().relation.as_bag_union().is_some());

    let selected = Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(filtered.clone()),
    });
    assert_eq!(plan_bgp(&selected).unwrap().projection, Some(vec![0]));
    let counted = Query::Count(Count {
        query: Box::new(filtered),
        count: DataValue::Variable("count".into()),
    });
    assert_eq!(plan_bgp(&counted).unwrap().count_variable.as_deref(), Some("count"));

    let nullable = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("y", "q", "z")),
        }),
        equals(value_var("z"), value_node("u")),
    ]);
    reject(
        nullable,
        BgpPlanError::EqualsNullableVariable { var: "z".into() },
    );
}

#[test]
fn equals_constant_gates_preserve_operand_order() {
    let false_gate = and_of(vec![
        tv("x", "p", "y"),
        equals(value_node("a"), value_node("b")),
    ]);
    assert!(matches!(
        plan_bgp(&false_gate)
            .unwrap()
            .relation
            .as_equality()
            .unwrap()
            .1,
        RelationEqualityPlan::Constants { .. }
    ));
}

#[test]
fn planner_rejects_recursive_depth_and_node_bombs() {
    let mut deep = vec![tv("x", "p", "y")];
    for _ in 0..64 {
        deep.push(Query::Not(Not {
            query: Box::new(tv("a", "q", "b")),
        }));
    }
    reject(
        and_of(deep),
        BgpPlanError::RecursiveStructureLimit,
    );

    let branches = (0..4096).map(|_| tv("x", "p", "y")).collect();
    reject(
        Query::Or(Or { or: branches }),
        BgpPlanError::RecursiveStructureLimit,
    );
}

#[test]
fn bounded_when_compiles_prolog_success_plus_true_and_bounds_existential_locals() {
    let when = Query::If(When {
        query: Box::new(tv("x", "q", "y")),
        consequent: Box::new(tv("x", "r", "y")),
    });
    let compiled = plan_bgp(&and_of(vec![tv("x", "p", "y"), when.clone()])).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "y"]);
    let terminusdb_woql2::proof::CompiledRelationStatement::When { expanded } = &compiled.relation.statement else { panic!("expected canonical When wrapper") };
    let terminusdb_woql2::proof::CompiledRelationStatement::BagUnion { branches, mappings } = &expanded.statement else { panic!("expected successful branch bag-unioned with true") };
    assert_eq!(branches.len(), 2);
    assert_eq!(mappings, &[vec![0, 1], vec![0, 1]]);
    assert!(matches!(branches[0].statement, terminusdb_woql2::proof::CompiledRelationStatement::InnerJoin { .. }));

    reject(when, BgpPlanError::WhenMissingOuter);
    let local_when = || Query::If(When {
        query: Box::new(tv("x", "q", "z")),
        consequent: Box::new(tv("z", "r", "y")),
    });
    let local_body = || and_of(vec![tv("x", "p", "y"), local_when()]);
    reject(
        local_body(),
        BgpPlanError::WhenBranchLocalsEscape {
            vars: vec!["z".into()],
        },
    );

    let selected = plan_bgp(&Query::Select(Select {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(local_body()),
    }))
    .unwrap();
    assert_eq!(selected.relation.schema, ["x", "y"]);
    let terminusdb_woql2::proof::CompiledRelationStatement::When { expanded } =
        &selected.relation.statement
    else {
        panic!("expected When wrapper")
    };
    let terminusdb_woql2::proof::CompiledRelationStatement::BagUnion { branches, .. } =
        &expanded.statement
    else {
        panic!("expected When bag union")
    };
    assert!(matches!(
        branches[0].statement,
        terminusdb_woql2::proof::CompiledRelationStatement::BagProjection { .. }
    ));

    let counted = plan_bgp(&Query::Count(Count {
        query: Box::new(local_body()),
        count: DataValue::Variable("count".into()),
    }))
    .unwrap();
    assert_eq!(counted.count_variable.as_deref(), Some("count"));

    let nested_select = plan_bgp(&Query::Select(Select {
        variables: vec!["x".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into(), "y".into()],
            query: Box::new(local_body()),
        })),
    }))
    .unwrap();
    assert_eq!(nested_select.projection, Some(vec![0]));

    reject(
        Query::Select(Select {
            variables: vec!["x".into(), "y".into()],
            query: Box::new(and_of(vec![
                tv("x", "p", "y"),
                local_when(),
                equals(value_var("z"), value_node("kept")),
            ])),
        }),
        BgpPlanError::WhenBranchLocalUsedLater { var: "z".into() },
    );
}

#[test]
fn bounded_when_composes_in_or_optional_not_and_preserves_fail_closed_nullable_scope() {
    let bounded = || Query::If(When {
        query: Box::new(Query::Or(Or { or: vec![tv("x", "q", "y"), tv("x", "q", "y")] })),
        consequent: Box::new(tv("x", "r", "y")),
    });
    let branch = and_of(vec![tv("x", "p", "y"), bounded()]);
    assert!(plan_bgp(&Query::Or(Or { or: vec![branch.clone(), tv("x", "p", "y")] })).is_ok());
    assert!(plan_bgp(&and_of(vec![
        tv("x", "outer", "y"),
        Query::Not(Not { query: Box::new(branch.clone()) }),
    ])).is_ok());
    assert!(plan_bgp(&and_of(vec![
        tv("x", "outer", "y"),
        Query::WoqlOptional(WoqlOptional { query: Box::new(branch) }),
    ])).is_ok());

    reject(
        and_of(vec![
            tv_subj_const("root", "p", "x"),
            Query::WoqlOptional(WoqlOptional { query: Box::new(tv("x", "q", "y")) }),
            Query::If(When {
                query: Box::new(tv("x", "r", "y")),
                consequent: Box::new(tv("x", "s", "y")),
            }),
        ]),
        BgpPlanError::WhenUnsupportedSubplan,
    );
}

#[test]
fn rejects_path_star() {
    // `Star` is now handled by the transitive-closure planner, but only under an endpoint `Distinct`
    // and with exactly one bound endpoint. With BOTH endpoints free it needs a domain-authenticated
    // diagonal (deferred), so it rejects closed with the specific closure error.
    let q = Query::Path(Path {
        subject: Value::Variable("x".into()),
        pattern: PathPattern::Star(PathStar {
            star: Box::new(PathPattern::Predicate(PathPredicate {
                predicate: Some(iri("p")),
            })),
        }),
        object: Value::Variable("y".into()),
        path: None,
    });
    reject(q, BgpPlanError::ClosureBothEndpointsFree);
}

// ---------------------------------------------------------------------------
// Fixed-length property-path lowering: `Query::Path` -> BGP / bag-union / projection.
// ---------------------------------------------------------------------------

fn path_pred(predicate: &str) -> PathPattern {
    PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    })
}

fn path_inverse(predicate: &str) -> PathPattern {
    PathPattern::InversePredicate(InversePathPredicate {
        predicate: Some(iri(predicate)),
    })
}

fn path_var_query(subject: &str, pattern: PathPattern, object: &str) -> Query {
    Query::Path(Path {
        subject: Value::Variable(subject.into()),
        pattern,
        object: Value::Variable(object.into()),
        path: None,
    })
}

/// The output-schema variable names after applying the compiled projection.
fn projected_schema(compiled: &CompiledBgp) -> Vec<String> {
    match &compiled.projection {
        Some(indices) => indices
            .iter()
            .map(|i| compiled.relation.schema[*i].clone())
            .collect(),
        None => compiled.relation.schema.clone(),
    }
}

#[test]
fn path_predicate_lowers_to_single_atom_bgp() {
    let compiled = plan_bgp(&path_var_query("s", path_pred("knows"), "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    assert!(compiled.projection.is_none());
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Bgp(_)
    ));
}

#[test]
fn path_inverse_predicate_swaps_endpoints() {
    // InversePredicate(p) from ?s to ?o lowers to Triple(?o, p, ?s): a single atom whose
    // subject is ?o and object is ?s.
    let compiled = plan_bgp(&path_var_query("s", path_inverse("knows"), "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["o", "s"]);
    assert!(compiled.projection.is_none());
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Bgp(_)
    ));
}

#[test]
fn path_two_hop_sequence_projects_out_intermediate() {
    let pattern = PathPattern::Sequence(PathSequence {
        sequence: vec![path_pred("knows"), path_pred("likes")],
    });
    let compiled = plan_bgp(&path_var_query("s", pattern, "o")).unwrap();
    // Three variables (s, fresh intermediate, o); projection drops the intermediate and
    // yields exactly the declared endpoints in [subject, object] order.
    assert_eq!(compiled.relation.schema.len(), 3);
    assert!(compiled.projection.is_some());
    assert_eq!(projected_schema(&compiled), ["s", "o"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Bgp(_)
    ));
}

#[test]
fn path_or_of_two_predicates_is_bag_union() {
    let pattern = PathPattern::Or(PathOr {
        or: vec![path_pred("knows"), path_pred("likes")],
    });
    let compiled = plan_bgp(&path_var_query("s", pattern, "o")).unwrap();
    assert_eq!(projected_schema(&compiled), ["s", "o"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::BagUnion { .. }
    ));
}

#[test]
fn path_exact_times_one_is_a_single_hop() {
    let pattern = PathPattern::Times(PathTimes {
        times: Box::new(path_pred("knows")),
        from: 1,
        to: 1,
    });
    let compiled = plan_bgp(&path_var_query("s", pattern, "o")).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    assert!(compiled.projection.is_none());
}

#[test]
fn rejects_path_plus() {
    // `Plus` with two free endpoints is a domain diagonal (deferred): the transitive-closure planner
    // rejects it closed with the specific closure error rather than the generic unbounded-path error.
    let q = path_var_query(
        "s",
        PathPattern::Plus(PathPlus {
            plus: Box::new(path_pred("knows")),
        }),
        "o",
    );
    reject(q, BgpPlanError::ClosureBothEndpointsFree);
}

// ---------------------------------------------------------------------------
// Transitive closure (`Star`/`Plus`) under an endpoint `Distinct`.
// ---------------------------------------------------------------------------

fn closure_pred(predicate: &str) -> PathPattern {
    PathPattern::Predicate(PathPredicate {
        predicate: Some(iri(predicate)),
    })
}

/// `Distinct([endpoint], Path(bound_source, pattern, endpoint))`.
fn distinct_closure(source: Value, pattern: PathPattern, endpoint: &str) -> Query {
    Query::Distinct(Distinct {
        variables: vec![endpoint.into()],
        query: Box::new(Query::Path(Path {
            subject: source,
            pattern,
            object: Value::Variable(endpoint.into()),
            path: None,
        })),
    })
}

#[test]
fn plus_under_distinct_bound_source_compiles_to_transitive_closure() {
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Plus(PathPlus {
            plus: Box::new(closure_pred("p")),
        }),
        "y",
    );
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, ["y"]);
    assert!(c.full_row_distinct, "endpoint Distinct is redundant over the already-set closure");
    let CompiledRelationStatement::TransitiveClosure {
        edge,
        seed,
        reflexive,
        inverted,
        ..
    } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure statement, got {:?}", c.relation.statement);
    };
    assert!(!reflexive, "plus is one-or-more (non-reflexive)");
    assert!(!inverted, "a bound source with a forward predicate is a forward walk");
    assert_eq!(
        *seed,
        terminus_store::proof::CanonicalObject::Node(iri("a").into_bytes())
    );
    assert_eq!(edge.atoms.len(), 1);
    assert_eq!(edge.atoms[0].predicate_term, iri("p").into_bytes());
}

#[test]
fn star_under_distinct_bound_source_is_reflexive() {
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Star(PathStar {
            star: Box::new(closure_pred("p")),
        }),
        "y",
    );
    let c = plan_bgp(&q).unwrap();
    let CompiledRelationStatement::TransitiveClosure { reflexive, .. } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure statement");
    };
    assert!(reflexive, "star is zero-or-more (reflexive identity)");
}

#[test]
fn plus_bound_target_compiles_to_inverted_closure() {
    // `Distinct([x], Path(?x, plus(p), b))`: the reachable set INTO b. path.pl mirrors this to the
    // inverse edge, so it compiles to an INVERTED closure seeded at b.
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Variable("x".into()),
            pattern: PathPattern::Plus(PathPlus {
                plus: Box::new(closure_pred("p")),
            }),
            object: Value::Node(iri("b")),
            path: None,
        })),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, ["x"]);
    let CompiledRelationStatement::TransitiveClosure {
        seed, inverted, reflexive, ..
    } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure statement");
    };
    assert!(inverted, "a bound target mirrors to the inverse edge");
    assert!(!reflexive);
    assert_eq!(
        *seed,
        terminus_store::proof::CanonicalObject::Node(iri("b").into_bytes())
    );
}

#[test]
fn plus_inverse_predicate_forward_compiles_to_inverted_closure() {
    // `Distinct([y], Path(a, plus(<p), ?y))`: an inverse-predicate closure from a bound source also
    // walks the inverse edge, so it too compiles to an INVERTED closure (seeded at a).
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Plus(PathPlus {
            plus: Box::new(PathPattern::InversePredicate(InversePathPredicate {
                predicate: Some(iri("p")),
            })),
        }),
        "y",
    );
    let c = plan_bgp(&q).unwrap();
    let CompiledRelationStatement::TransitiveClosure { seed, inverted, .. } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure statement");
    };
    assert!(inverted, "an inverse predicate walks the inverse edge");
    assert_eq!(
        *seed,
        terminus_store::proof::CanonicalObject::Node(iri("a").into_bytes())
    );
}

#[test]
fn star_bound_source_inverse_predicate_cancels_to_forward() {
    // Two mirrorings cancel: a bound SOURCE with a forward predicate is a plain forward closure.
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Star(PathStar {
            star: Box::new(closure_pred("p")),
        }),
        "y",
    );
    let c = plan_bgp(&q).unwrap();
    let CompiledRelationStatement::TransitiveClosure { inverted, .. } = &c.relation.statement else {
        panic!("expected a TransitiveClosure statement");
    };
    assert!(!inverted, "bound source + forward predicate is a forward walk");
}

#[test]
fn bare_plus_without_distinct_rejects_requires_distinct() {
    // Bound source, but NO endpoint Distinct: the reference emits an edge-simple BAG, so shipping a
    // set would be unfaithful. Reject closed.
    let q = Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: PathPattern::Plus(PathPlus {
            plus: Box::new(closure_pred("p")),
        }),
        object: Value::Variable("y".into()),
        path: None,
    });
    reject(q, BgpPlanError::ClosureRequiresDistinct);
}

#[test]
fn closure_over_compound_subpattern_rejects() {
    // `star` of a two-hop sequence needs the general BGP-in-the-loop lowering (deferred).
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Star(PathStar {
            star: Box::new(PathPattern::Sequence(PathSequence {
                sequence: vec![closure_pred("p"), closure_pred("q")],
            })),
        }),
        "y",
    );
    reject(q, BgpPlanError::ClosureCompoundSubpattern);
}

#[test]
fn closure_with_two_bound_endpoints_rejects() {
    // `path(a, plus(p), b)` is a boolean reachability check, not a set query (deferred).
    let q = Query::Path(Path {
        subject: Value::Node(iri("a")),
        pattern: PathPattern::Plus(PathPlus {
            plus: Box::new(closure_pred("p")),
        }),
        object: Value::Node(iri("b")),
        path: None,
    });
    reject(q, BgpPlanError::ClosureBothEndpointsBound);
}

#[test]
fn closure_with_wildcard_predicate_rejects() {
    let q = distinct_closure(
        Value::Node(iri("a")),
        PathPattern::Plus(PathPlus {
            plus: Box::new(PathPattern::Predicate(PathPredicate { predicate: None })),
        }),
        "y",
    );
    reject(q, BgpPlanError::PathWildcardUnsupported);
}

#[test]
fn closure_count_gate_is_deferred() {
    // `Count(Distinct([y], Path(a, plus(p), ?y)))` is a follow-up; do the Distinct form first.
    let q = Query::Count(Count {
        query: Box::new(distinct_closure(
            Value::Node(iri("a")),
            PathPattern::Plus(PathPlus {
                plus: Box::new(closure_pred("p")),
            }),
            "y",
        )),
        count: dvar("count"),
    });
    reject(q, BgpPlanError::ClosureCountGateDeferred);
}

#[test]
fn path_times_two_two_repeated_predicate_uses_tuple_disequality() {
    // Times{2,2} unrolls to `knows,knows` — a single repeated predicate. Edge-simple Prolog
    // path semantics (no re-traversed edge) are recovered by a TupleDisequality edge-simple
    // filter over the join's node columns, then endpoint projection — NOT rejected.
    let q = path_var_query(
        "s",
        PathPattern::Times(PathTimes {
            times: Box::new(path_pred("knows")),
            from: 2,
            to: 2,
        }),
        "o",
    );
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    assert!(compiled.projection.is_none());
    // BagProjection( TupleDisequality( join ) ), one hop-pair for a 2-hop chain.
    let CompiledRelationStatement::BagProjection { input, .. } = &compiled.relation.statement
    else {
        panic!("expected endpoint BagProjection, got {:?}", compiled.relation.statement);
    };
    let CompiledRelationStatement::TupleDisequality { input, pairs } = &input.statement else {
        panic!("expected TupleDisequality filter, got {:?}", input.statement);
    };
    assert_eq!(pairs.len(), 1);
    assert!(matches!(input.statement, CompiledRelationStatement::Bgp(_)));
}

#[test]
fn path_sequence_repeated_predicate_uses_tuple_disequality() {
    let q = path_var_query(
        "s",
        PathPattern::Sequence(PathSequence {
            sequence: vec![path_pred("knows"), path_pred("knows")],
        }),
        "o",
    );
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    let CompiledRelationStatement::BagProjection { input, .. } = &compiled.relation.statement
    else {
        panic!("expected endpoint BagProjection, got {:?}", compiled.relation.statement);
    };
    assert!(matches!(
        input.statement,
        CompiledRelationStatement::TupleDisequality { .. }
    ));
}

#[test]
fn path_times_three_three_has_three_hop_pairs() {
    // p^3 -> nodes [s, h0, h1, o], 3 edges -> C(3,2) = 3 hop-pair disequalities.
    let q = path_var_query(
        "s",
        PathPattern::Times(PathTimes {
            times: Box::new(path_pred("knows")),
            from: 3,
            to: 3,
        }),
        "o",
    );
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["s", "o"]);
    let CompiledRelationStatement::BagProjection { input, .. } = &compiled.relation.statement
    else {
        panic!("expected endpoint BagProjection");
    };
    let CompiledRelationStatement::TupleDisequality { pairs, .. } = &input.statement else {
        panic!("expected TupleDisequality filter");
    };
    assert_eq!(pairs.len(), 3);
}

#[test]
fn path_times_range_two_three_is_bag_union_of_filtered_chains() {
    // Times{2,3} -> bag union over exact lengths 2 and 3, each an edge-simple-filtered chain.
    let q = path_var_query(
        "s",
        PathPattern::Times(PathTimes {
            times: Box::new(path_pred("knows")),
            from: 2,
            to: 3,
        }),
        "o",
    );
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(projected_schema(&compiled), ["s", "o"]);
    let CompiledRelationStatement::BagUnion { branches, .. } = &compiled.relation.statement else {
        panic!("expected BagUnion over the range, got {:?}", compiled.relation.statement);
    };
    assert_eq!(branches.len(), 2);
    // Each length-branch is BagProjection( TupleDisequality( join ) ).
    for branch in branches {
        let CompiledRelationStatement::BagProjection { input, .. } = &branch.statement else {
            panic!("expected per-length endpoint BagProjection");
        };
        assert!(matches!(
            input.statement,
            CompiledRelationStatement::TupleDisequality { .. }
        ));
    }
}

#[test]
fn rejects_path_mixed_predicate_repeat() {
    // `knows, likes, knows` repeats `knows` but is MIXED-predicate: the predicate id must be
    // folded into the edge fingerprint (a later sub-effort), so it rejects closed here.
    let q = path_var_query(
        "s",
        PathPattern::Sequence(PathSequence {
            sequence: vec![path_pred("knows"), path_pred("likes"), path_pred("knows")],
        }),
        "o",
    );
    reject(q, BgpPlanError::PathMixedPredicateRepeatUnsupported);
}

#[test]
fn rejects_path_times_zero_identity() {
    let q = path_var_query(
        "s",
        PathPattern::Times(PathTimes {
            times: Box::new(path_pred("knows")),
            from: 0,
            to: 2,
        }),
        "o",
    );
    reject(q, BgpPlanError::PathIdentityUnsupported);
}

#[test]
fn rejects_path_times_range_too_large() {
    let q = path_var_query(
        "s",
        PathPattern::Times(PathTimes {
            times: Box::new(path_pred("knows")),
            from: 1,
            to: u64::MAX,
        }),
        "o",
    );
    reject(q, BgpPlanError::UnboundedPathUnsupported);
}

#[test]
fn rejects_path_wildcard_predicate() {
    let q = path_var_query(
        "s",
        PathPattern::Predicate(PathPredicate { predicate: None }),
        "o",
    );
    reject(q, BgpPlanError::PathWildcardUnsupported);
}

#[test]
fn rejects_path_edge_list_capture() {
    let q = Query::Path(Path {
        subject: Value::Variable("s".into()),
        pattern: path_pred("knows"),
        object: Value::Variable("o".into()),
        path: Some(Value::Variable("edges".into())),
    });
    reject(q, BgpPlanError::PathWitnessCaptureUnsupported);
}

#[test]
fn rejects_path_or_with_multi_hop_branch() {
    // One alternative is a two-hop chain (introduces an intermediate) — unsupported in a union.
    let pattern = PathPattern::Or(PathOr {
        or: vec![
            path_pred("knows"),
            PathPattern::Sequence(PathSequence {
                sequence: vec![path_pred("likes"), path_pred("owns")],
            }),
        ],
    });
    reject(
        path_var_query("s", pattern, "o"),
        BgpPlanError::PathUnionBranchMultiHop,
    );
}

#[test]
fn rejects_distinct_on_a_schema_subset() {
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
    });
    reject(q, BgpPlanError::DistinctNotFullSchema);
}

#[test]
fn accepts_distinct_on_the_complete_bgp_schema() {
    let q = Query::Distinct(Distinct {
        variables: vec!["z".into(), "x".into(), "y".into()],
        query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert!(compiled.full_row_distinct);
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
}

#[test]
fn rejects_full_schema_distinct_over_duplicate_producing_children() {
    // Or/BagUnion: the honest-pipeline break `Distinct(["x","y"], Or(T, T))` must no longer
    // be a silent no-op returning a NON-deduplicated bag marked as Distinct.
    let over_or = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(Query::Or(Or {
            or: vec![tv("x", "p", "y"), tv("x", "p", "y")],
        })),
    });
    reject(over_or, BgpPlanError::DistinctOverDuplicateProducingChild);

    // Not/LeftAnti under a full-schema Distinct is likewise a duplicate-producing child.
    let over_not = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(and_of(vec![
            tv("x", "p", "y"),
            Query::Not(Not {
                query: Box::new(tv("y", "q", "z")),
            }),
        ])),
    });
    reject(over_not, BgpPlanError::DistinctOverDuplicateProducingChild);

    // Optional/LeftOuter under a full-schema Distinct is duplicate-producing as well.
    let over_optional = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into(), "z".into()],
        query: Box::new(and_of(vec![
            tv("x", "p", "y"),
            Query::WoqlOptional(WoqlOptional {
                query: Box::new(tv("y", "q", "z")),
            }),
        ])),
    });
    reject(over_optional, BgpPlanError::DistinctOverDuplicateProducingChild);
}

#[test]
fn visible_projected_distinct_one_column_compiles_to_the_store_gadget() {
    // `SELECT DISTINCT ?x WHERE { ?x p ?y }` — ?x is duplicate-producing (many ?y per ?x), so
    // the visible top-level Distinct routes the projected key column through the audited store
    // distinct gadget (real bag dedup), NOT the transparent full-row no-op.
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(tv("x", "p", "y")),
        })),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["x"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Distinct { ref key_columns, .. } if key_columns == &[0]
    ));
    assert_eq!(compiled.count_variable, None);
    assert!(!compiled.full_row_distinct);
    assert_eq!(compiled.projection, Some(vec![0]));
}

#[test]
fn visible_projected_distinct_two_columns_compiles_to_the_store_gadget() {
    // `SELECT DISTINCT ?x ?y WHERE { ?x p ?y . ?y q ?z }` — distinct over the (x,y) pair, a
    // strict subset of the {x,y,z} BGP schema.
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into(), "y".into()],
            query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "q", "z")])),
        })),
    });
    let compiled = plan_bgp(&q).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "y"]);
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Distinct { ref key_columns, .. } if key_columns.len() == 2
    ));
    assert_eq!(compiled.count_variable, None);
    assert!(!compiled.full_row_distinct);
    assert_eq!(compiled.projection, Some(vec![0, 1]));
}

#[test]
fn visible_projected_distinct_over_three_columns_rejects_the_gadget_limit() {
    // Formerly a blanket DistinctWithProjectionOrCount reject; the projected-distinct gadget
    // path is now reached, but the store gadget supports at most two key columns.
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into(), "z".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into(), "y".into(), "z".into()],
            query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
        })),
    });
    reject(q, BgpPlanError::DistinctUnsupportedKeyCount { count: 3 });
}

#[test]
fn visible_projected_distinct_over_a_bag_union_compiles_to_the_store_gadget() {
    // `SELECT DISTINCT ?subject WHERE { {?subject p ?o} OR {?subject q ?o} }` — the two Or branches
    // produce an overlapping/duplicate bag of ?subject. The visible top-level projected Distinct now
    // routes the node-position key column through the audited store distinct gadget over a BAG-UNION
    // inner (real bag dedup) — the SAME audited gadget the beneath-Count path already uses (see
    // `count_projected_distinct_composes_over_duplicate_bag_union`). The bag union is just an
    // authenticated bag, so the dedup gadget proving `output == distinct keys of the input bag`
    // collapses the branch overlap exactly as it would for a single BGP bag.
    //
    // (The two branches share the object variable `?o` so their schemas unify into `[subject, o]`;
    // the distinct key is the node-position subject — the only column with a committed single
    // namespace domain, since a bare object is a mixed node/literal `Value` with no distinct key.)
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
    assert_eq!(compiled.relation.schema, ["subject"]);
    let CompiledRelationStatement::Distinct { input, key_columns } = &compiled.relation.statement
    else {
        panic!(
            "expected a Distinct statement over a bag union, got {:?}",
            compiled.relation.statement
        );
    };
    assert_eq!(key_columns, &[0]);
    assert!(
        input.as_bag_union().is_some(),
        "the visible projected distinct gadget must wrap the bag-union inner"
    );
    assert_eq!(compiled.count_variable, None);
    assert!(!compiled.full_row_distinct);
    assert_eq!(compiled.projection, Some(vec![0]));
}

#[test]
fn visible_projected_distinct_over_a_non_bag_union_non_bgp_inner_still_rejects() {
    // A bag-union (Or) inner is now accepted on the visible path (previous test), but every OTHER
    // non-BGP inner keeps rejecting closed. Here `?x p ?y . NOT { ?y q ?z }` compiles to a
    // `LeftAnti` statement — an anti-join is NOT a plain authenticated bag the dedup gadget can key
    // on — so a visible projected Distinct over it must still fail closed at the same guard.
    let anti = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(and_of(vec![
                tv("x", "p", "y"),
                Query::Not(Not {
                    query: Box::new(tv("y", "q", "z")),
                }),
            ])),
        })),
    });
    reject(anti, BgpPlanError::DistinctWithProjectionOrCount);
}

#[test]
fn visible_projected_distinct_over_mixed_domain_object_rejects() {
    // The projected key must be a node-position variable. A bare OBJECT (`?y` in `?x p ?y`) is a
    // mixed node/literal `Value` position with no committed single-namespace domain, so it cannot
    // be a distinct key even on the visible (no enclosing Count) path — reject closed, exactly as
    // the beneath-Count path does (see `projected_distinct_rejects_visible_unknown_nullable_and_oversized_witnesses`).
    let q = Query::Distinct(Distinct {
        variables: vec!["y".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["y".into()],
            query: Box::new(tv("x", "p", "y")),
        })),
    });
    reject(q, BgpPlanError::DistinctUnsupportedDomain { var: "y".into() });
}

#[test]
fn count_accepts_exact_projected_distinct_over_one_or_two_known_domains() {
    let one = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["x".into()],
            query: Box::new(tv("x", "p", "y")),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&one).unwrap();
    assert_eq!(compiled.relation.schema, ["x"]);
    assert!(matches!(
        compiled.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::Distinct {
            ref key_columns, ..
        } if key_columns == &[0]
    ));
    assert_eq!(compiled.count_variable.as_deref(), Some("count"));
    assert_eq!(compiled.projection, Some(Vec::new()));

    let two = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["x".into(), "age".into()],
            query: Box::new(and_of(vec![
                tv("x", "age", "age"),
                gte(dvar("age"), duint(0)),
            ])),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&two).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "age"]);
    assert!(matches!(
        compiled.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::Distinct {
            ref key_columns, ..
        } if key_columns == &[0, 1]
    ));
}

#[test]
fn count_projected_distinct_composes_over_duplicate_bag_union() {
    let branch = tv("x", "p", "y");
    let query = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["x".into()],
            query: Box::new(Query::Or(Or {
                or: vec![branch.clone(), branch],
            })),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&query).unwrap();
    let terminusdb_woql2::proof::CompiledRelationStatement::Distinct { input, .. } =
        &compiled.relation.statement
    else {
        panic!("expected exact distinct wrapper")
    };
    assert!(input.as_bag_union().is_some());
}

#[test]
fn count_projected_distinct_accepts_an_authenticated_ordered_child_but_hides_order() {
    let ordered = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "score".into(),
            order: Order::Desc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("metric", "score", "score"),
            gte(dvar("score"), duint(0)),
        ])),
    });
    let query = Query::Count(Count {
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["score".into()],
            query: Box::new(ordered),
        })),
        count: dvar("count"),
    });
    let compiled = plan_bgp(&query).unwrap();
    let terminusdb_woql2::proof::CompiledRelationStatement::Distinct { input, .. } =
        &compiled.relation.statement
    else {
        panic!("expected exact distinct wrapper")
    };
    assert!(input.as_exact_scalar_order().is_some());
    assert_eq!(compiled.count_variable.as_deref(), Some("count"));
}

#[test]
fn projected_distinct_rejects_visible_unknown_nullable_and_oversized_witnesses() {
    reject(
        Query::Count(Count {
            query: Box::new(Query::Distinct(Distinct {
                variables: vec!["y".into()],
                query: Box::new(tv("x", "p", "y")),
            })),
            count: dvar("count"),
        }),
        BgpPlanError::DistinctUnsupportedDomain { var: "y".into() },
    );
    reject(
        Query::Count(Count {
            query: Box::new(Query::Distinct(Distinct {
                variables: vec!["x".into(), "y".into(), "z".into()],
                query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "q", "z")])),
            })),
            count: dvar("count"),
        }),
        BgpPlanError::DistinctUnsupportedKeyCount { count: 3 },
    );
    let nullable = and_of(vec![
        tv("x", "p", "y"),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv("x", "q", "z")),
        }),
    ]);
    reject(
        Query::Count(Count {
            query: Box::new(Query::Distinct(Distinct {
                variables: vec!["z".into()],
                query: Box::new(nullable),
            })),
            count: dvar("count"),
        }),
        BgpPlanError::DistinctUnsupportedDomain { var: "z".into() },
    );
}

#[test]
fn rejects_comparison_var_vs_var() {
    // A comparison with variables on BOTH sides is not a range filter (Phase 7.4 routes
    // comparisons, but only `?var op const`).
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "p", "z"),
        greater(dvar("y"), dvar("z")),
    ]);
    reject(q, BgpPlanError::RangeBothVariables);
}

#[test]
fn accepts_predicate_variable_and_predicate_to_predicate_join() {
    let q = and_of(vec![
        tv_pred_var("x", "pv", "y"),
        tv_pred_var("y", "pv", "z"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "pv", "y", "z"]);
    assert!(c.bgp_plan().atoms.iter().all(|atom| atom.predicate_variable));
    assert!(c
        .bgp_plan()
        .atoms
        .iter()
        .all(|atom| atom.predicate_term.is_empty()));
    assert_eq!(
        (c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key),
        (2, 0)
    );
    assert_eq!(c.bgp_plan().joins[0].additional_keys, vec![(1, 1)]);
}

#[test]
fn rejects_variable_shared_across_predicate_and_node_value_namespaces() {
    let q = and_of(vec![tv_pred_var("x", "pv", "y"), tv("pv", "p", "z")]);
    reject(
        q,
        BgpPlanError::VariableNamespaceConflict { var: "pv".into() },
    );
}

#[test]
fn predicate_variables_compose_with_projection_count_or_and_not() {
    let base = tv_pred_var("s", "p", "o");
    let selected = Query::Select(Select {
        variables: vec!["p".into()],
        query: Box::new(base.clone()),
    });
    assert_eq!(plan_bgp(&selected).unwrap().projection, Some(vec![1]));

    let counted = Query::Count(Count {
        query: Box::new(base.clone()),
        count: dvar("count"),
    });
    assert_eq!(
        plan_bgp(&counted).unwrap().count_variable.as_deref(),
        Some("count")
    );

    let union = Query::Or(Or {
        or: vec![base.clone(), base.clone()],
    });
    assert_eq!(plan_bgp(&union).unwrap().relation.as_bag_union().unwrap().0.len(), 2);

    let negated = and_of(vec![
        base,
        Query::Not(Not {
            query: Box::new(tv_pred_var("o", "p", "z")),
        }),
    ]);
    let compiled = plan_bgp(&negated).unwrap();
    assert_eq!(compiled.relation.as_left_anti().unwrap().2.len(), 2);
}

#[test]
fn accepts_subject_node_constant() {
    // t(<s>, p, ?y), t(?y, q, ?z): the subject-const atom keeps only O = y (arity 1),
    // then joins on y with the all-variable atom's subject.
    let q = and_of(vec![tv_subj_const("s", "p", "y"), tv("y", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["y", "z"]);
    assert!(matches!(c.bgp_plan().atoms[0].subject, AtomTerm::NodeConstant(_)));
    assert!(matches!(c.bgp_plan().atoms[0].object, AtomTerm::Variable));
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (0, 0));
}

#[test]
fn accepts_object_node_constant() {
    // t(?x, p, <o>), t(?x, q, ?z): the object-const atom keeps only S = x (arity 1),
    // then joins on x with the all-variable atom's subject.
    let q = and_of(vec![tv_obj_node("x", "p", "o"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert!(matches!(c.bgp_plan().atoms[0].object, AtomTerm::NodeConstant(_)));
    assert!(matches!(c.bgp_plan().atoms[0].subject, AtomTerm::Variable));
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (0, 0));
}

#[test]
fn accepts_object_hexbinary_literal() {
    let q = and_of(vec![tv_obj_data("x", "p"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    let expected = tdb_succinct::HexBinary(vec![0xde, 0xad, 0xbe, 0xef]);
    assert_value_constant(&c, "p", tdb_succinct::HexBinary::make_entry(&expected));
}

#[test]
fn rejects_invalid_hexbinary_lexical_value() {
    let q = typed_object_query(XSDAnySimpleType::HexBinary("not-hex".into()));
    reject(
        q,
        BgpPlanError::UnsupportedValueDatatype {
            datatype: "hexBinary (invalid lexical value)",
        },
    );
}

#[test]
fn accepts_object_integer_literal() {
    // xsd:integer object literal -> store Int64 value constant (byte-aligned).
    let q = and_of(vec![tv_obj_integer("x", "p", 42), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert!(matches!(
        c.bgp_plan().atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
    assert_value_constant(&c, "p", i64::make_entry(&42i64));
}

#[test]
fn accepts_object_boolean_literal() {
    // xsd:boolean object literal -> store Boolean value constant.
    let q = and_of(vec![tv_obj_bool("x", "p", true), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", bool::make_entry(&true));
}

#[test]
fn accepts_object_float_literal() {
    // xsd:float/double object literal -> store Float64 value constant.
    let q = and_of(vec![tv_obj_float("x", "p", 3.5), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", f64::make_entry(&3.5f64));
}

#[test]
fn accepts_object_unsigned_int_literal() {
    // xsd:unsignedInt object literal -> store UInt32 value constant.
    let q = and_of(vec![tv_obj_uint("x", "p", 7), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", u32::make_entry(&7u32));
}

#[test]
fn accepts_object_string_literal() {
    // t(?x, p, "hi"), t(?x, q, ?z): the string-literal atom keeps only S = x (arity 1).
    let q = and_of(vec![tv_obj_string("x", "p", "hi"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "z"]);
    assert!(matches!(
        c.bgp_plan().atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
    assert_eq!((c.bgp_plan().joins[0].acc_key, c.bgp_plan().joins[0].rel_key), (0, 0));
}

#[test]
fn accepts_remaining_store_aligned_scalar_equalities() {
    let decimal: Decimal = "123.450".parse().unwrap();
    let datetime = Utc.with_ymd_and_hms(2025, 6, 7, 8, 9, 10).unwrap();
    let date = NaiveDate::from_ymd_opt(2025, 6, 7).unwrap();
    let time = NaiveTime::from_hms_opt(8, 9, 10).unwrap();
    let cases = [
        (
            XSDAnySimpleType::Decimal(decimal.clone()),
            tdb_succinct::Decimal::make_entry(
                &tdb_succinct::Decimal::new(decimal.to_string()).unwrap(),
            ),
        ),
        (
            XSDAnySimpleType::URI("https://example.com/value".into()),
            tdb_succinct::AnyURI::make_entry(&"https://example.com/value"),
        ),
        (
            XSDAnySimpleType::DateTime(datetime),
            chrono::NaiveDateTime::make_entry(&datetime.naive_utc()),
        ),
        (
            XSDAnySimpleType::Date(date),
            tdb_succinct::Date::make_entry(&tdb_succinct::Date {
                year: 2025,
                month: 6,
                day: 7,
                offset: 0,
            }),
        ),
        (
            XSDAnySimpleType::Time(time),
            chrono::NaiveTime::make_entry(&time),
        ),
    ];

    for (value, expected) in cases {
        let compiled = plan_bgp(&typed_object_query(value)).unwrap();
        assert_value_constant(&compiled, "p", expected);
    }
}

#[test]
fn accepts_all_ground_and_mixed_arity_conjunctions() {
    // t(<s>, p, <o>) is a zero-arity existence relation whose authenticated UNIT
    // result column remains hidden from the WOQL schema.
    let c = plan_bgp(&tv_ground("s", "p", "o")).unwrap();
    assert!(c.relation.schema.is_empty());
    assert_eq!(c.projection, Some(Vec::new()));
    assert_eq!(c.bgp_plan().atoms.len(), 1);
    assert!(c.bgp_plan().joins.is_empty());

    let all_ground = and_of(vec![tv_ground("s", "p", "o"), tv_ground("y", "q", "z")]);
    let compiled = plan_bgp(&all_ground).unwrap();
    assert!(compiled.relation.schema.is_empty());
    assert_eq!(compiled.bgp_plan().atoms.len(), 2);
    assert!(compiled.bgp_plan().joins.is_empty());

    let mixed = and_of(vec![tv_ground("s", "p", "o"), tv("y", "q", "z")]);
    let compiled = plan_bgp(&mixed).unwrap();
    assert_eq!(compiled.relation.schema, vec!["y", "z"]);
    assert_eq!(compiled.bgp_plan().atoms.len(), 2);
    assert!(compiled.bgp_plan().joins.is_empty());
    assert!(matches!(compiled.bgp_plan().atoms[0].subject, AtomTerm::Variable));
    assert!(!matches!(
        compiled.bgp_plan().atoms[1].subject,
        AtomTerm::Variable
    ));
}

#[test]
fn accepts_repeated_variable_as_same_row_equality() {
    let q = and_of(vec![tv("x", "p", "x"), tv("x", "q", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert!(c.bgp_plan().atoms[0].subject_object_equal);
    assert_eq!(c.bgp_plan().joins.len(), 1);
}

#[test]
fn accepts_composite_key() {
    // t(?x,p,?y), t(?x,q,?y) — the second atom shares BOTH x and y.
    let q = and_of(vec![tv("x", "p", "y"), tv("x", "q", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!(c.bgp_plan().joins[0].additional_keys, vec![(1, 1)]);
}

#[test]
fn accepts_triangle_cycle() {
    // t(?x,p,?y), t(?y,q,?z), t(?z,r,?x) — a 3-cycle.
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        tv("z", "r", "x"),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "z"]);
    assert_eq!(c.bgp_plan().joins.len(), 2);
    assert_eq!(c.bgp_plan().joins[1].additional_keys, vec![(0, 1)]);
}

#[test]
fn accepts_disconnected_components_in_stable_schema_order() {
    // t(?x,p,?y), t(?a,q,?b) — two components, no shared variable.
    let q = and_of(vec![tv("x", "p", "y"), tv("a", "q", "b")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "a", "b"]);
    assert_eq!(c.bgp_plan().joins.len(), 1);
    assert!(c.bgp_plan().joins[0].is_cross_product());

    // A same-named variable makes the atoms connected, even if a caller intended two
    // logical components; variable identity, not atom position, defines connectivity.
    let collision = and_of(vec![tv("x", "p", "y"), tv("a", "q", "x")]);
    let c = plan_bgp(&collision).unwrap();
    assert_eq!(c.relation.schema, vec!["y", "x", "a"]);
    assert!(!c.bgp_plan().joins[0].is_cross_product());

    let three = and_of(vec![
        tv("x", "p", "y"),
        tv("a", "q", "b"),
        tv("m", "r", "n"),
    ]);
    let c = plan_bgp(&three).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y", "a", "b", "m", "n"]);
    assert!(c.bgp_plan().joins.iter().all(|step| step.is_cross_product()));
}

#[test]
fn accepts_single_atom() {
    let q = and_of(vec![tv("x", "p", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!(c.bgp_plan().atoms.len(), 1);
    assert!(c.bgp_plan().joins.is_empty());
}

#[test]
fn rejects_empty_conjunction() {
    let q = and_of(vec![]);
    reject(q, BgpPlanError::TooFewAtoms { count: 0 });
}

#[test]
fn rejects_selected_variable_not_in_bgp() {
    let bgp = and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")]);
    let q = Query::Select(Select {
        variables: vec!["nope".into()],
        query: Box::new(bgp),
    });
    reject(
        q,
        BgpPlanError::SelectedVariableNotInBgp { var: "nope".into() },
    );
}

#[test]
fn accepts_bare_triple_body() {
    let q = tv("x", "p", "y");
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.relation.schema, vec!["x", "y"]);
    assert_eq!(c.bgp_plan().atoms.len(), 1);
}

// ----------------------------------------------------------------------------
// Phase 7.4 — numeric object-value RANGE filters compiled from comparison conjuncts.
// A comparison `?a op k` attaches an ObjectRangeSpec to the atom binding ?a as its object.
// ----------------------------------------------------------------------------

/// The ObjectRangeSpec attached to the compiled atom whose predicate is `iri(pred)`.
fn range_of<'a>(c: &'a CompiledBgp, pred: &str) -> Option<&'a ObjectRangeSpec> {
    let term = iri(pred).into_bytes();
    c.bgp_plan()
        .atoms
        .iter()
        .find(|a| a.predicate_term == term)
        .and_then(|a| a.object_range.as_ref())
}

/// Assert a range has `op` and a UInt32 constant `k` (datatype tag + c + b_bits), matching
/// the store's committed dict encoding (tag = `CanonicalDatatype::UInt32`, c = the u32 value).
#[track_caller]
fn assert_uint32_range(spec: &ObjectRangeSpec, op: RangeOp, k: u64) {
    assert_eq!(spec.op, op, "operator");
    assert_eq!(
        spec.datatype,
        ArkFr::from_u64(CanonicalDatatype::UInt32 as u64),
        "datatype tag"
    );
    assert_eq!(spec.c, ArkFr::from_u64(k), "order-preserving constant c");
    assert_eq!(spec.b_bits, OBJECT_RANGE_B_BITS, "bit-width");
}

/// Assert a range has `op`, `datatype` tag, and a `c` equal to the store's committed OP
/// encoding `c_op` (plus the pinned bit-width).
#[track_caller]
fn assert_range(spec: &ObjectRangeSpec, op: RangeOp, datatype: CanonicalDatatype, c_op: u64) {
    assert_eq!(spec.op, op, "operator");
    assert_eq!(
        spec.datatype,
        ArkFr::from_u64(datatype as u64),
        "datatype tag"
    );
    assert_eq!(spec.c, ArkFr::from_u64(c_op), "order-preserving constant c");
    assert_eq!(spec.b_bits, OBJECT_RANGE_B_BITS, "bit-width");
}

/// Compile `t(?x,age,?a), t(?x,name,?n)` plus one comparison conjunct.
fn age_name_range(cmp: Query) -> CompiledBgp {
    let q = and_of(vec![tv("x", "age", "a"), tv("x", "name", "n"), cmp]);
    plan_bgp(&q).unwrap()
}

/// Reject the age/name BGP with a given comparison conjunct.
#[track_caller]
fn reject_range(cmp: Query, want: BgpPlanError) {
    let q = and_of(vec![tv("x", "age", "a"), tv("x", "name", "n"), cmp]);
    reject(q, want);
}

#[test]
fn range_attaches_to_object_binding_atom_and_keeps_it_output() {
    // t(?x,age,?a), t(?x,name,?n), ?a > 18 -> Gt 18 on the age atom; name atom unranged.
    let c = age_name_range(greater(dvar("a"), duint(18)));
    assert!(range_of(&c, "name").is_none(), "name atom is not ranged");
    assert_uint32_range(range_of(&c, "age").unwrap(), RangeOp::Gt, 18);
    // The ranged object variable stays a kept output column.
    assert!(c.relation.schema.contains(&"a".to_string()));
    // schema is [a, x, n] (left-deep join of age then name on the shared x).
    assert_eq!(c.relation.schema, vec!["a", "x", "n"]);
}

#[test]
fn range_flips_when_constant_is_on_the_left() {
    // k > ?a  <=>  ?a < k.
    let c = age_name_range(greater(duint(18), dvar("a")));
    assert_uint32_range(range_of(&c, "age").unwrap(), RangeOp::Lt, 18);
}

#[test]
fn range_all_ops_variable_on_left() {
    for (cmp, op) in [
        (greater(dvar("a"), duint(7)), RangeOp::Gt),
        (less(dvar("a"), duint(7)), RangeOp::Lt),
        (gte(dvar("a"), duint(7)), RangeOp::Ge),
        (lte(dvar("a"), duint(7)), RangeOp::Le),
    ] {
        let c = age_name_range(cmp);
        assert_uint32_range(range_of(&c, "age").unwrap(), op, 7);
    }
}

#[test]
fn range_all_ops_constant_on_left_flip() {
    for (cmp, op) in [
        (greater(duint(7), dvar("a")), RangeOp::Lt), // 7 > a  <=> a < 7
        (less(duint(7), dvar("a")), RangeOp::Gt),    // 7 < a  <=> a > 7
        (gte(duint(7), dvar("a")), RangeOp::Le),     // 7 >= a <=> a <= 7
        (lte(duint(7), dvar("a")), RangeOp::Ge),     // 7 <= a <=> a >= 7
    ] {
        let c = age_name_range(cmp);
        assert_uint32_range(range_of(&c, "age").unwrap(), op, 7);
    }
}

#[test]
fn rejects_range_const_vs_const() {
    reject_range(
        greater(duint(1), duint(2)),
        BgpPlanError::RangeBothConstants,
    );
}

#[test]
fn rejects_range_string_datatype() {
    reject_range(
        greater(dvar("a"), dstr("hi")),
        BgpPlanError::UnsupportedRangeDatatype { datatype: "string" },
    );
}

#[test]
fn accepts_range_integer_datatype() {
    // xsd:integer range -> store Int64 order-preserving constant.
    let c = age_name_range(greater(dvar("a"), dint(5)));
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Gt,
        CanonicalDatatype::Int64,
        store_op_value(i64::make_entry(&5i64)),
    );
}

#[test]
fn accepts_range_integer_negative_datatype() {
    // A negative Int64 constant uses the store's offset-binary OP encoding.
    let c = age_name_range(gte(dvar("a"), dint(-5)));
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Ge,
        CanonicalDatatype::Int64,
        store_op_value(i64::make_entry(&-5i64)),
    );
}

#[test]
fn accepts_every_exact_fixed_width_integer_datatype_at_edges_and_zero() {
    let cases = vec![
        (XSDAnySimpleType::UnsignedByte(u8::MIN), CanonicalDatatype::UInt8, u8::make_entry(&u8::MIN)),
        (XSDAnySimpleType::UnsignedByte(u8::MAX), CanonicalDatatype::UInt8, u8::make_entry(&u8::MAX)),
        (XSDAnySimpleType::Byte(i8::MIN), CanonicalDatatype::Int8, i8::make_entry(&i8::MIN)),
        (XSDAnySimpleType::Byte(0), CanonicalDatatype::Int8, i8::make_entry(&0i8)),
        (XSDAnySimpleType::Byte(i8::MAX), CanonicalDatatype::Int8, i8::make_entry(&i8::MAX)),
        (XSDAnySimpleType::UnsignedShort(u16::MAX), CanonicalDatatype::UInt16, u16::make_entry(&u16::MAX)),
        (XSDAnySimpleType::Short(i16::MIN), CanonicalDatatype::Int16, i16::make_entry(&i16::MIN)),
        (XSDAnySimpleType::Short(0), CanonicalDatatype::Int16, i16::make_entry(&0i16)),
        (XSDAnySimpleType::Int(i32::MIN), CanonicalDatatype::Int32, i32::make_entry(&i32::MIN)),
        (XSDAnySimpleType::Int(0), CanonicalDatatype::Int32, i32::make_entry(&0i32)),
        (XSDAnySimpleType::UnsignedLong(u64::MAX), CanonicalDatatype::UInt64, u64::make_entry(&u64::MAX)),
        (XSDAnySimpleType::Long(i64::MIN), CanonicalDatatype::Int64, i64::make_entry(&i64::MIN)),
        (XSDAnySimpleType::Long(0), CanonicalDatatype::Int64, i64::make_entry(&0i64)),
        (XSDAnySimpleType::Long(i64::MAX), CanonicalDatatype::Int64, i64::make_entry(&i64::MAX)),
    ];
    for (constant, datatype, entry) in cases {
        let compiled = age_name_range(gte(dvar("a"), DataValue::Data(constant)));
        assert_range(
            range_of(&compiled, "age").unwrap(),
            RangeOp::Ge,
            datatype,
            store_op_value(entry),
        );
    }
}

#[test]
fn rejects_mixed_fixed_width_bounds_as_distinct_datatypes() {
    reject_range(
        Query::InRange(InRange {
            value: dvar("a"),
            start: DataValue::Data(XSDAnySimpleType::Byte(0)),
            end: DataValue::Data(XSDAnySimpleType::UnsignedByte(10)),
        }),
        BgpPlanError::RangeDatatypeMismatch { var: "a".into() },
    );
}

#[test]
fn accepts_range_boolean_datatype() {
    // xsd:boolean range -> store Boolean order-preserving constant (false=0 < true=1).
    let c = age_name_range(greater(dvar("a"), dbool(false)));
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Gt,
        CanonicalDatatype::Boolean,
        store_op_value(bool::make_entry(&false)),
    );
}

#[test]
fn accepts_range_float_datatype() {
    // xsd:float/double range -> store Float64 IEEE total-order constant.
    let c = age_name_range(greater(dvar("a"), dfloat(1.5)));
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Gt,
        CanonicalDatatype::Float64,
        store_op_value(f64::make_entry(&1.5f64)),
    );
}

#[test]
fn rejects_range_hexbinary_datatype() {
    // xsd:hexBinary is variable-width and not order-preserving; reject closed.
    reject_range(
        greater(dvar("a"), dhex("deadbeef")),
        BgpPlanError::UnsupportedRangeDatatype {
            datatype: "hexBinary",
        },
    );
}

#[test]
fn rejects_range_list_operand() {
    reject_range(
        greater(dvar("a"), dlist(vec![duint(1)])),
        BgpPlanError::UnsupportedRangeDatatype { datatype: "list" },
    );
}

#[test]
fn rejects_range_on_subject_variable() {
    // ?a is the SUBJECT of the age atom, not a pure object -> reject.
    let q = and_of(vec![
        tv("a", "age", "y"),
        tv("y", "name", "n"),
        greater(dvar("a"), duint(5)),
    ]);
    reject(q, BgpPlanError::RangeVariableIsSubject { var: "a".into() });
}

#[test]
fn rejects_range_on_variable_that_is_no_atoms_object() {
    // ?w does not occur as any atom's object (it is not in the BGP at all).
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        greater(dvar("w"), duint(5)),
    ]);
    reject(q, BgpPlanError::RangeVariableNotObject { var: "w".into() });
}

#[test]
fn rejects_range_on_object_of_two_atoms() {
    // ?a is the object of BOTH atoms (an object-object join key) -> no unique atom to
    // attach the range to.
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("y", "name", "a"),
        greater(dvar("a"), duint(5)),
    ]);
    reject(
        q,
        BgpPlanError::RangeVariableObjectOfMultiple { var: "a".into() },
    );
}

#[test]
fn accepts_two_ranges_on_one_variable_as_an_intersection() {
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        greater(dvar("a"), duint(5)),
        less(dvar("a"), duint(20)),
    ]);
    let c = plan_bgp(&q).unwrap();
    let range = range_of(&c, "age").unwrap();
    assert_uint32_range(range, RangeOp::Gt, 5);
    let second = range.second.as_ref().unwrap();
    assert_eq!(second.op, RangeOp::Lt);
    assert_eq!(second.c, ArkFr::from_u64(20));
}

#[test]
fn in_range_compiles_to_half_open_bounds() {
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        Query::InRange(InRange {
            value: dvar("a"),
            start: duint(18),
            end: duint(40),
        }),
    ]);
    let c = plan_bgp(&q).unwrap();
    let range = range_of(&c, "age").unwrap();
    assert_uint32_range(range, RangeOp::Ge, 18);
    let second = range.second.as_ref().unwrap();
    assert_eq!(second.op, RangeOp::Lt);
    assert_eq!(second.c, ArkFr::from_u64(40));
}

// ----------------------------------------------------------------------------
// date / dateTime object-value RANGE filters. The compiled `c` must equal the store's
// committed OP encoding (the decoded i64-nanoseconds key, offset-binary), and the
// datatype tag must be `DateTime`/`Date`, so the range gadget masks the right rows.
// ----------------------------------------------------------------------------

/// The store's committed OP `c` (as a u64 offset-binary code) for a date/dateTime constant
/// — computed via the store's OWN `node_value_op_value` on the value the compiler builds,
/// then read back as the u64 the range gadget compares.
fn store_date_op_u64(value: XSDAnySimpleType) -> u64 {
    use terminus_store::proof::node_value_op_value;
    let entry = match &value {
        XSDAnySimpleType::DateTime(dt) => chrono::NaiveDateTime::make_entry(&dt.naive_utc()),
        XSDAnySimpleType::Date(d) => {
            use chrono::Datelike;
            tdb_succinct::Date::make_entry(&tdb_succinct::Date {
                year: i64::from(d.year()),
                month: d.month() as u8,
                day: d.day() as u8,
                offset: 0,
            })
        }
        other => panic!("not a date constant: {other:?}"),
    };
    let object = CanonicalObject::from_store_object(&ValueTriple::new_value("", "", entry).object);
    let field = node_value_op_value(&object).unwrap();
    // The gadget compares u64 offset-binary codes; recover it independently from the value's
    // known nanos to cross-check the store field element too.
    let nanos = match &value {
        XSDAnySimpleType::DateTime(dt) => dt.timestamp_nanos_opt().unwrap(),
        XSDAnySimpleType::Date(d) => d
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_nanos_opt()
            .unwrap(),
        _ => unreachable!(),
    };
    let code = (nanos as u64) ^ (1u64 << 63);
    assert_eq!(field, ArkFr::from_u64(code), "store field == offset-binary code");
    code
}

#[test]
fn datetime_range_compiles_with_store_aligned_op_constant() {
    // t(?x,age,?t): ?t > 2020-06-15T12:00:00Z -> Gt on the age atom with the DateTime tag.
    let dt = ddatetime(2020, 6, 15, 12, 0, 0);
    let c = age_name_range(greater(dvar("a"), dt.clone()));
    let konst = match dt {
        DataValue::Data(v) => v,
        _ => unreachable!(),
    };
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Gt,
        CanonicalDatatype::DateTime,
        store_date_op_u64(konst),
    );
    assert_eq!(c.relation.schema, vec!["a", "x", "n"]);
}

#[test]
fn date_range_compiles_with_store_aligned_op_constant() {
    // t(?x,age,?d): ?d <= 2021-01-01 -> Le on the age atom with the Date tag.
    let date = ddate(2021, 1, 1);
    let c = age_name_range(lte(dvar("a"), date.clone()));
    let konst = match date {
        DataValue::Data(v) => v,
        _ => unreachable!(),
    };
    assert_range(
        range_of(&c, "age").unwrap(),
        RangeOp::Le,
        CanonicalDatatype::Date,
        store_date_op_u64(konst),
    );
}

#[test]
fn date_in_range_compiles_to_half_open_date_bounds() {
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        Query::InRange(InRange {
            value: dvar("a"),
            start: ddate(2020, 1, 1),
            end: ddate(2021, 1, 1),
        }),
    ]);
    let c = plan_bgp(&q).unwrap();
    let range = range_of(&c, "age").unwrap();
    assert_eq!(range.op, RangeOp::Ge);
    assert_eq!(
        range.datatype,
        ArkFr::from_u64(CanonicalDatatype::Date as u64)
    );
    let second = range.second.as_ref().unwrap();
    assert_eq!(second.op, RangeOp::Lt);
}

#[test]
fn datetime_range_out_of_i64_nanoseconds_window_rejects_closed() {
    // Year 3000 is past 2262-04-11, so its instant overflows i64 nanoseconds → reject.
    reject_range(
        greater(dvar("a"), ddatetime(3000, 1, 1, 0, 0, 0)),
        BgpPlanError::DateRangeConstantOutOfExactRange,
    );
    // Year 1600 is before 1677-09-21 → underflow → reject.
    reject_range(
        less(dvar("a"), ddate(1600, 1, 1)),
        BgpPlanError::DateRangeConstantOutOfExactRange,
    );
}

#[test]
fn mixing_date_and_numeric_bounds_in_one_range_rejects_closed() {
    // ?a >= 2020-01-01 (Date) AND ?a < 40 (UInt32) — two bounds, different datatypes.
    reject_range(
        and_of(vec![
            gte(dvar("a"), ddate(2020, 1, 1)),
            less(dvar("a"), duint(40)),
        ]),
        BgpPlanError::RangeDatatypeMismatch { var: "a".into() },
    );
}

#[test]
fn triple_slice_rejects_variable_subject_because_complete_order_key_is_not_proved() {
    let q = Query::TripleSlice(TripleSlice {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("age")),
        object: Value::Variable("a".into()),
        low: Value::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(18)),
        high: Value::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(40)),
        graph: None,
    });
    reject(
        q,
        BgpPlanError::TripleValueOrderUnsupportedShape,
    );
}

#[test]
fn triple_slice_rev_rejects_variable_subject_because_complete_order_key_is_not_proved() {
    let q = Query::TripleSliceRev(TripleSliceRev {
        subject: NodeValue::Variable("x".into()),
        predicate: NodeValue::Node(iri("age")),
        object: Value::Variable("a".into()),
        low: Value::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(18)),
        high: Value::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(40)),
        graph: None,
    });
    reject(
        q,
        BgpPlanError::TripleValueOrderUnsupportedShape,
    );
}

#[test]
fn triple_slice_and_reverse_compile_exact_half_open_scalar_order() {
    let make = |reverse| {
        let subject = NodeValue::Node(iri("metric"));
        let predicate = NodeValue::Node(iri("age"));
        let object = Value::Variable("age".into());
        let low = Value::Data(XSDAnySimpleType::UnsignedInt(18));
        let high = Value::Data(XSDAnySimpleType::UnsignedInt(40));
        if reverse {
            Query::TripleSliceRev(TripleSliceRev { subject, predicate, object, low, high, graph: None })
        } else {
            Query::TripleSlice(TripleSlice { subject, predicate, object, low, high, graph: None })
        }
    };

    for reverse in [false, true] {
        let compiled = plan_bgp(&make(reverse)).unwrap();
        assert_eq!(compiled.relation.schema, ["age"]);
        let (input, datatype) = compiled.relation.as_exact_scalar_order().unwrap();
        assert_eq!(datatype, CanonicalDatatype::UInt32);
        let range = &input.as_bgp().unwrap().atoms[0].object_range;
        let range = range.as_ref().unwrap();
        assert_eq!(range.op, RangeOp::Ge);
        assert_eq!(range.c, ArkFr::from_u64(18));
        assert_eq!(range.second.as_ref().unwrap().op, RangeOp::Lt);
        assert_eq!(range.second.as_ref().unwrap().c, ArkFr::from_u64(40));
    }
}

#[test]
fn triple_next_and_previous_compile_strict_nearest_value_in_both_binding_modes() {
    let subject = NodeValue::Node(iri("metric"));
    let predicate = NodeValue::Node(iri("age"));
    let reference = Value::Data(XSDAnySimpleType::UnsignedInt(20));
    let variable = Value::Variable("neighbor".into());
    let queries = [
        Query::TripleNext(TripleNext {
            subject: subject.clone(), predicate: predicate.clone(),
            object: reference.clone(), next: variable.clone(), graph: None,
        }),
        Query::TripleNext(TripleNext {
            subject: subject.clone(), predicate: predicate.clone(),
            object: variable.clone(), next: reference.clone(), graph: None,
        }),
        Query::TriplePrevious(TriplePrevious {
            subject: subject.clone(), predicate: predicate.clone(),
            object: reference.clone(), previous: variable.clone(), graph: None,
        }),
        Query::TriplePrevious(TriplePrevious {
            subject, predicate, object: variable, previous: reference, graph: None,
        }),
    ];
    let expected_ops = [RangeOp::Gt, RangeOp::Lt, RangeOp::Lt, RangeOp::Gt];

    for (query, expected_op) in queries.into_iter().zip(expected_ops) {
        let compiled = plan_bgp(&query).unwrap();
        let (ordered, offset, limit) = compiled.relation.as_exact_ordered_slice().unwrap();
        assert_eq!((offset, limit), (0, Some(1)));
        let (bgp, datatype) = ordered.as_exact_scalar_order().unwrap();
        assert_eq!(datatype, CanonicalDatatype::UInt32);
        assert_eq!(bgp.as_bgp().unwrap().atoms[0].object_range.as_ref().unwrap().op, expected_op);
    }
}

#[test]
fn ordered_triple_value_constructs_reject_schema_strings_and_two_variables() {
    reject(
        Query::TripleSlice(TripleSlice {
            subject: NodeValue::Node(iri("metric")),
            predicate: NodeValue::Node(iri("age")),
            object: Value::Variable("age".into()),
            low: Value::Data(XSDAnySimpleType::String("a".into())),
            high: Value::Data(XSDAnySimpleType::String("z".into())),
            graph: None,
        }),
        BgpPlanError::TripleValueOrderUnsupportedValue,
    );
    reject(
        Query::TripleNext(TripleNext {
            subject: NodeValue::Node(iri("metric")),
            predicate: NodeValue::Node(iri("age")),
            object: Value::Variable("object".into()),
            next: Value::Variable("next".into()),
            graph: Some(GraphType::Schema),
        }),
        BgpPlanError::TripleValueOrderUnsupportedShape,
    );
}

#[test]
fn ground_triple_slices_compile_equality_then_exact_empty_projection() {
    let query = |reverse| {
        let subject = NodeValue::Node(iri("metric"));
        let predicate = NodeValue::Node(iri("age"));
        let object = Value::Data(XSDAnySimpleType::UnsignedInt(20));
        let low = Value::Data(XSDAnySimpleType::UnsignedInt(15));
        let high = Value::Data(XSDAnySimpleType::UnsignedInt(40));
        if reverse {
            Query::TripleSliceRev(TripleSliceRev { subject, predicate, object, low, high, graph: None })
        } else {
            Query::TripleSlice(TripleSlice { subject, predicate, object, low, high, graph: None })
        }
    };
    let forward = plan_bgp(&query(false)).unwrap();
    let reverse = plan_bgp(&query(true)).unwrap();
    for compiled in [&forward, &reverse] {
        assert!(compiled.relation.schema.is_empty());
        assert_eq!(compiled.projection, Some(Vec::new()));
        let CompiledRelationStatement::BagProjection { input, indices } = &compiled.relation.statement else {
            panic!("expected ground bag projection")
        };
        assert!(indices.is_empty());
        let CompiledRelationStatement::Equality { input, .. } = &input.statement else {
            panic!("expected exact ground-object equality")
        };
        assert!(input.as_exact_scalar_order().is_some());
    }
    assert_ne!(compiled_query_digest(&forward).unwrap(), compiled_query_digest(&reverse).unwrap());
}

#[test]
fn fully_ground_next_previous_compile_limit_one_equality_unit_and_reject_wrong_type() {
    let subject = NodeValue::Node(iri("metric"));
    let predicate = NodeValue::Node(iri("age"));
    for query in [
        Query::TripleNext(TripleNext {
            subject: subject.clone(), predicate: predicate.clone(),
            object: Value::Data(XSDAnySimpleType::UnsignedInt(20)),
            next: Value::Data(XSDAnySimpleType::UnsignedInt(30)), graph: None,
        }),
        Query::TriplePrevious(TriplePrevious {
            subject: subject.clone(), predicate: predicate.clone(),
            object: Value::Data(XSDAnySimpleType::UnsignedInt(30)),
            previous: Value::Data(XSDAnySimpleType::UnsignedInt(20)), graph: None,
        }),
    ] {
        let compiled = plan_bgp(&query).unwrap();
        let CompiledRelationStatement::BagProjection { input, indices } = &compiled.relation.statement else {
            panic!("expected ground bag projection")
        };
        assert!(indices.is_empty());
        let CompiledRelationStatement::Equality { input, .. } = &input.statement else {
            panic!("expected adjacency equality")
        };
        assert!(input.as_exact_ordered_slice().is_some());
    }

    reject(
        Query::TripleNext(TripleNext {
            subject, predicate,
            object: Value::Data(XSDAnySimpleType::UnsignedInt(20)),
            next: Value::Data(XSDAnySimpleType::Int(30)), graph: None,
        }),
        BgpPlanError::EqualsDomainMismatch,
    );
}

#[test]
fn exact_scalar_ascending_order_compiles_when_key_is_the_complete_typed_row() {
    let child = and_of(vec![
        tv_subj_const("metric", "score", "score"),
        gte(dvar("score"), duint(0)),
    ]);
    let compiled = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "score".into(),
            order: Order::Asc,
        }],
        query: Box::new(child),
    }))
    .unwrap();
    let (input, datatype) = compiled.relation.as_exact_scalar_order().unwrap();
    assert!(input.as_bgp().is_some());
    assert_eq!(compiled.relation.schema, ["score"]);
    assert_eq!(datatype, CanonicalDatatype::UInt32);

    let boolean = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "enabled".into(),
            order: Order::Asc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("feature", "enabled", "enabled"),
            gte(dvar("enabled"), dbool(false)),
        ])),
    }))
    .unwrap();
    assert_eq!(
        boolean.relation.as_exact_scalar_order().unwrap().1,
        CanonicalDatatype::Boolean
    );
}

#[test]
fn fixed_width_signed_order_compiles_ascending_and_descending_with_exact_tag() {
    let child = and_of(vec![
        tv_subj_const("metric", "score", "score"),
        gte(dvar("score"), DataValue::Data(XSDAnySimpleType::Byte(i8::MIN))),
    ]);
    let ordered = |order| Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "score".into(), order }],
        query: Box::new(child.clone()),
    });
    let ascending = plan_bgp(&ordered(Order::Asc)).unwrap();
    let descending = plan_bgp(&ordered(Order::Desc)).unwrap();
    assert_eq!(ascending.relation.as_exact_scalar_order().unwrap().1, CanonicalDatatype::Int8);
    assert_eq!(descending.relation.as_exact_scalar_order().unwrap().1, CanonicalDatatype::Int8);
    assert_ne!(compiled_query_digest(&ascending).unwrap(), compiled_query_digest(&descending).unwrap());
}

#[test]
fn exact_scalar_order_rejects_unsupported_shapes_closed() {
    let child = and_of(vec![
        tv_subj_const("metric", "score", "score"),
        gte(dvar("score"), duint(0)),
    ]);
    let descending = plan_bgp(&Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Desc,
            }],
            query: Box::new(child.clone()),
        })).unwrap();
    let ascending = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "score".into(), order: Order::Asc }],
        query: Box::new(child.clone()),
    })).unwrap();
    assert_ne!(compiled_query_digest(&descending).unwrap(), compiled_query_digest(&ascending).unwrap());
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![],
            query: Box::new(child),
        }),
        BgpPlanError::OrderByUnsupportedShape,
    );
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Asc,
            }],
            query: Box::new(tv_subj_const("metric", "score", "score")),
        }),
        BgpPlanError::OrderByUnsupportedValue,
    );

    // A free-subject two-column BGP `[subject, score]` sorted by `score` alone is now shape-legal
    // (subset templates are supported), but the appended `subject` tie-break column is a node
    // reference, not a fixed-width numeric value, so the plan rejects on the key datatype instead.
    let two_columns = and_of(vec![tv("subject", "score", "score"), gte(dvar("score"), duint(0))]);
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Asc,
            }],
            query: Box::new(two_columns.clone()),
        }),
        BgpPlanError::OrderByUnsupportedValue,
    );
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![
                OrderTemplate {
                    variable: "score".into(),
                    order: Order::Asc,
                },
                OrderTemplate {
                    variable: "subject".into(),
                    order: Order::Asc,
                },
            ],
            query: Box::new(two_columns),
        }),
        BgpPlanError::OrderByUnsupportedValue,
    );

    let duplicate_generating_union = Query::Or(terminusdb_woql2::query::Or {
        or: vec![
            and_of(vec![
                tv_subj_const("metric", "score", "score"),
                gte(dvar("score"), duint(0)),
            ]),
            and_of(vec![
                tv_subj_const("metric", "score", "score"),
                gte(dvar("score"), duint(0)),
            ]),
        ],
    });
    let duplicate_order = plan_bgp(&Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Asc,
            }],
            query: Box::new(duplicate_generating_union),
        })).unwrap();
    assert!(duplicate_order.relation.as_exact_scalar_order().unwrap().0.as_bag_union().is_some());

    // Ordering by `score` alone is shape-legal, but the appended `label` tie-break column is nullable
    // (bound only under WoqlOptional), so it has no well-defined fixed-width sort key and rejects on
    // the key value/nullability check.
    let nullable_child = and_of(vec![
        tv_subj_const("metric", "score", "score"),
        gte(dvar("score"), duint(0)),
        Query::WoqlOptional(WoqlOptional {
            query: Box::new(tv_subj_const("metric", "label", "label")),
        }),
    ]);
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Asc,
            }],
            query: Box::new(nullable_child),
        }),
        BgpPlanError::OrderByUnsupportedValue,
    );
}

#[test]
fn exact_scalar_order_retains_outer_routing_wrapper() {
    let ordered = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "score".into(),
            order: Order::Asc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("metric", "score", "score"),
            gte(dvar("score"), duint(0)),
        ])),
    });
    let compiled = plan_bgp(&Query::Using(Using {
        collection: "admin/db/local/branch/main".into(),
        query: Box::new(ordered),
    }))
    .unwrap();
    assert_eq!(compiled.collection.as_deref(), Some("admin/db/local/branch/main"));
    assert!(compiled.relation.as_exact_scalar_order().is_some());
}

#[test]
fn exact_complete_row_multikey_order_compiles_and_rejects_key_mappings() {
    let child = and_of(vec![
        tv_subj_const("left", "x", "x"), gte(dvar("x"), duint(0)),
        tv_subj_const("right", "y", "y"), gte(dvar("y"), duint(0)),
    ]);
    let query = |ordering| Query::OrderBy(OrderBy { ordering, query: Box::new(child.clone()) });
    let compiled = plan_bgp(&query(vec![
        OrderTemplate { variable: "y".into(), order: Order::Desc },
        OrderTemplate { variable: "x".into(), order: Order::Asc },
    ])).unwrap();
    assert_eq!(compiled.relation.schema, ["x", "y"]);
    assert!(matches!(compiled.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::ExactMultiOrder { ref key_indices, .. }
            if key_indices == &[1, 0]));
    let duplicate_union = Query::Or(Or { or: vec![child.clone(), child.clone()] });
    let duplicate_compiled = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![
            OrderTemplate { variable: "y".into(), order: Order::Desc },
            OrderTemplate { variable: "x".into(), order: Order::Asc },
        ],
        query: Box::new(duplicate_union),
    })).unwrap();
    let terminusdb_woql2::proof::CompiledRelationStatement::ExactMultiOrder { input, .. } = &duplicate_compiled.relation.statement else { unreachable!() };
    assert!(input.as_bag_union().is_some());
    // SUBSET template: naming only `x` sorts by `x` and appends the unnamed column `y` as an
    // ascending total-order tie-break, so the plan is a full-row `ExactMultiOrder` with key
    // priority [x, y] both ascending.
    let subset = plan_bgp(&query(vec![OrderTemplate { variable: "x".into(), order: Order::Asc }])).unwrap();
    assert_eq!(subset.relation.schema, ["x", "y"]);
    assert!(matches!(subset.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::ExactMultiOrder { ref key_indices, ref directions, .. }
            if key_indices == &[0, 1]
                && directions == &[terminus_store::proof::order::OrderDirection::Ascending; 2]));
    // Naming only `y` descending appends `x` ascending as the tie-break: key priority [y, x].
    let subset_desc = plan_bgp(&query(vec![OrderTemplate { variable: "y".into(), order: Order::Desc }])).unwrap();
    assert!(matches!(subset_desc.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::ExactMultiOrder { ref key_indices, ref directions, .. }
            if key_indices == &[1, 0]
                && directions == &[terminus_store::proof::order::OrderDirection::Descending, terminus_store::proof::order::OrderDirection::Ascending]));
    // A repeated key names the same column twice — meaningless — and still rejects closed.
    assert_eq!(
        plan_bgp(&query(vec![
            OrderTemplate { variable: "x".into(), order: Order::Asc },
            OrderTemplate { variable: "x".into(), order: Order::Desc },
        ])).unwrap_err(),
        BgpPlanError::OrderByUnsupportedShape,
    );

    let child3 = and_of(vec![
        tv_subj_const("left", "x", "x"), gte(dvar("x"), duint(0)),
        tv_subj_const("middle", "y", "y"), gte(dvar("y"), duint(0)),
        tv_subj_const("right", "z", "z"), gte(dvar("z"), duint(0)),
    ]);
    let compiled3 = plan_bgp(&Query::OrderBy(OrderBy {
        ordering: vec![
            OrderTemplate { variable: "z".into(), order: Order::Desc },
            OrderTemplate { variable: "x".into(), order: Order::Asc },
            OrderTemplate { variable: "y".into(), order: Order::Asc },
        ],
        query: Box::new(child3),
    })).unwrap();
    assert_eq!(compiled3.relation.schema, ["x", "y", "z"]);
    assert!(matches!(compiled3.relation.statement,
        terminusdb_woql2::proof::CompiledRelationStatement::ExactMultiOrder { ref key_indices, .. }
            if key_indices == &[2, 0, 1]));
}

fn exact_order_query_for_slice() -> Query {
    Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "score".into(),
            order: Order::Asc,
        }],
        query: Box::new(and_of(vec![
            tv_subj_const("metric", "score", "score"),
            gte(dvar("score"), duint(0)),
        ])),
    })
}

#[test]
fn start_and_limit_preserve_authoritative_inside_out_nesting() {
    let query = Query::Start(Start {
        start: 1,
        query: Box::new(Query::Limit(Limit {
            limit: 2,
            query: Box::new(exact_order_query_for_slice()),
        })),
    });
    let compiled = plan_bgp(&query).unwrap();
    let (limited, offset, limit) = compiled.relation.as_exact_ordered_slice().unwrap();
    assert_eq!((offset, limit), (1, None));
    let (ordered, offset, limit) = limited.as_exact_ordered_slice().unwrap();
    assert_eq!((offset, limit), (0, Some(2)));
    assert!(ordered.as_exact_scalar_order().is_some());

    for query in [
        Query::Limit(Limit {
            limit: 0,
            query: Box::new(exact_order_query_for_slice()),
        }),
        Query::Start(Start {
            start: u64::MAX,
            query: Box::new(exact_order_query_for_slice()),
        }),
    ] {
        assert!(plan_bgp(&query).unwrap().relation.as_exact_ordered_slice().is_some());
    }
}

#[test]
fn start_and_limit_reject_unordered_children_closed() {
    for query in [
        Query::Start(Start {
            start: 1,
            query: Box::new(tv("x", "age", "a")),
        }),
        Query::Limit(Limit {
            limit: 1,
            query: Box::new(tv("x", "age", "a")),
        }),
    ] {
        reject(query, BgpPlanError::OrderedSliceUnsupportedShape);
    }
}

#[test]
fn once_reuses_ordered_limit_one_but_remains_digest_distinct() {
    let once = plan_bgp(&Query::Once(Once {
        query: Box::new(exact_order_query_for_slice()),
    }))
    .unwrap();
    assert_eq!(once.relation.as_exact_ordered_slice().unwrap().2, Some(1));
    let limit = plan_bgp(&Query::Limit(Limit {
        limit: 1,
        query: Box::new(exact_order_query_for_slice()),
    }))
    .unwrap();
    assert_ne!(compiled_query_digest(&once).unwrap(), compiled_query_digest(&limit).unwrap());

    let empty = plan_bgp(&Query::Once(Once {
        query: Box::new(Query::Limit(Limit {
            limit: 0,
            query: Box::new(exact_order_query_for_slice()),
        })),
    }))
    .unwrap();
    let (limited, _, once_limit) = empty.relation.as_exact_ordered_slice().unwrap();
    assert_eq!(once_limit, Some(1));
    assert_eq!(limited.as_exact_ordered_slice().unwrap().2, Some(0));

    reject(
        Query::Once(Once { query: Box::new(tv("x", "age", "a")) }),
        BgpPlanError::OrderedSliceUnsupportedShape,
    );
}

#[test]
fn rejects_a_third_range_on_one_variable() {
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        greater(dvar("a"), duint(5)),
        less(dvar("a"), duint(20)),
        less(dvar("a"), duint(30)),
    ]);
    reject(
        q,
        BgpPlanError::DuplicateRangeOnVariable { var: "a".into() },
    );
}

// ---------------------------------------------------------------------------
// Triple-SHAPED constructs that are NOT store BGP triples: `TypeOf`/`Dot`/`IsA`.
//
// These three woql2 leaves resemble a triple, but direct verification against the
// terminusdb Prolog reference proves none of them denote a store triple pattern, so
// lowering any to `Triple` would authenticate a relation the engine never computes.
// Each rejects closed with its own documented [`BgpPlanError`] (see the
// `reject_non_bgp_typed_leaves` pre-pass in proof.rs), and the pre-pass runs before
// path lowering so it composes with it.
//   * TypeOf  → `typeof/2` is a value-shape classifier: a node IRI → `sys#Top`
//               unconditionally (never reads `rdf:type`); a typed literal → its
//               datatype IRI (woql_compile.pl runtime `typeof/2`).
//   * Dot     → `dot/3` is `get_dict` on a BOUND document dict, reads no store triples
//               (woql_compile.pl `dot_field`; `find_resources(dot(_,_,_),…)` = ∅).
//   * IsA     → `isa/2` = `is_instance` is SUBSUMPTION-closed over `sys:inherits`
//               (instance.pl `is_instance_class` + `class_subsumed`), so a direct
//               rdf:type triple is incomplete.
// ---------------------------------------------------------------------------

fn typeof_leaf(value: Value, type_uri: &str) -> Query {
    Query::TypeOf(TypeOf {
        value,
        type_uri: NodeValue::Node(iri(type_uri)),
    })
}

fn dot_leaf(document: DataValue, field: DataValue, value: DataValue) -> Query {
    Query::Dot(Dot {
        document,
        field,
        value,
    })
}

fn isa_leaf(element: &str, type_of: &str) -> Query {
    Query::IsA(IsA {
        element: NodeValue::Variable(element.into()),
        type_of: NodeValue::Node(iri(type_of)),
    })
}

/// An `IsA` whose CLASS (`type_of`) is a variable — the unbound-class regime.
fn isa_leaf_var_class(element: &str, type_of: &str) -> Query {
    Query::IsA(IsA {
        element: NodeValue::Variable(element.into()),
        type_of: NodeValue::Variable(type_of.into()),
    })
}

#[test]
fn typeof_rejects_closed_as_value_shape_classifier() {
    // Top-level: WOQL `typeof` binds a node IRI's type to `sys#Top` and never reads
    // `rdf:type`, so a `t(value, rdf:type, type)` lowering would authenticate a different
    // relation — reject closed rather than emit an unsound plan.
    reject(
        typeof_leaf(value_var("x"), "Person"),
        BgpPlanError::TypeOfNotStoredType,
    );
    // Nested inside an And inside a Select: the pre-pass recurses the read-composition
    // wrappers, so the precise error still fires (not a generic fall-through).
    reject(
        Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(and_of(vec![
                tv("x", "p", "y"),
                typeof_leaf(value_var("x"), "Person"),
            ])),
        }),
        BgpPlanError::TypeOfNotStoredType,
    );
}

#[test]
fn dot_rejects_closed_as_in_memory_dict_access() {
    // Top-level: WOQL `dot` is `get_dict` on a bound document (no store triples); a
    // store-edge lowering would authenticate a different relation.
    reject(
        dot_leaf(dvar("doc"), dstr("field"), dvar("v")),
        BgpPlanError::DotNotStoreEdge,
    );
    // Nested inside an And: still caught with the precise error.
    reject(
        and_of(vec![
            tv("s", "p", "o"),
            dot_leaf(dvar("doc"), dstr("name"), dvar("v")),
        ]),
        BgpPlanError::DotNotStoreEdge,
    );
}

#[test]
fn isa_bound_class_lowers_to_cross_root_and_nested_rejects() {
    // WOQL `isa` with a BOUND class = the node regime `is_instance_class` (terminusdb
    // src/core/document/instance.pl:80-92): X isa C iff X has a DIRECT `rdf:type Class` with
    // `class_subsumed(Class, C)` (Class in the DOWNWARD sys:inherits closure of C). A standalone
    // `?x isa C` NOW LOWERS to a cross-dictionary two-root hash join (the downward sys:inherits
    // closure of C over the schema graph, hash-joined against the instance rdf:type relation via a
    // full-field LogUp semi-join) — see `plan_isa`. So it PLANS successfully.
    plan_bgp(&isa_leaf("x", "Person")).expect("standalone bound-class IsA lowers to a cross-root plan");

    // NESTED inside a BGP conjunction, IsA still rejects closed: the cross-root node is TOP-LEVEL
    // only (it reads two graphs), so it cannot participate in an instance-graph BGP join.
    reject(
        Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(and_of(vec![tv("x", "p", "y"), isa_leaf("x", "Agent")])),
        }),
        BgpPlanError::IsASubsumptionUnsupported,
    );
}

#[test]
fn isa_variable_class_rejects_closed_needs_bound_class() {
    // `?x isa ?C` — an UNBOUND class cannot seed the downward sys:inherits subsumption closure
    // (`{C} ∪ subclasses(C)`); a free class is a whole-schema enumeration (deferred). This rejects
    // on the free class with its OWN precise reason, DISTINCT from the bound-class cross-dict blocker.
    reject(isa_leaf_var_class("x", "C"), BgpPlanError::IsAVariableClass);
    // Nested occurrence is caught with the same precise reason (the pre-pass recurses wrappers).
    reject(
        Query::Select(Select {
            variables: vec!["x".into()],
            query: Box::new(and_of(vec![tv("x", "p", "y"), isa_leaf_var_class("x", "C")])),
        }),
        BgpPlanError::IsAVariableClass,
    );
}

#[test]
fn isa_element_regimes_variable_ships_bound_defers_literal_unrepresentable() {
    // The LITERAL-subject regime of `is_instance` (XSD datatype lattice, `basetype_subsumption_of`,
    // instance.pl:50-66) is structurally UNREPRESENTABLE from woql2 `IsA`: `element` is a `NodeValue`,
    // only `Node(iri)` or `Variable` — never a typed literal. Of the two constructible element regimes:
    //   * a VARIABLE element (`?x isa Person`) SHIPS as a cross-root plan;
    //   * a BOUND element (`<n> isa Person`, a ground existence check) is DEFERRED (reject closed).
    plan_bgp(&Query::IsA(IsA {
        element: NodeValue::Variable("x".into()),
        type_of: NodeValue::Node(iri("Person")),
    }))
    .expect("variable-element bound-class IsA lowers to a cross-root plan");
    assert_eq!(
        plan_bgp(&Query::IsA(IsA {
            element: NodeValue::Node(iri("some-node")),
            type_of: NodeValue::Node(iri("Person")),
        }))
        .unwrap_err(),
        BgpPlanError::IsASubsumptionUnsupported,
        "a BOUND-element ground existence check is deferred"
    );
}

#[test]
fn non_bgp_leaf_composes_with_path_and_rejects_before_path_lowering() {
    // Composition: a query containing BOTH a `Path` (which would otherwise lower cleanly to
    // a single-atom BGP) and a `TypeOf` must reject on the `TypeOf`, proving the non-BGP-leaf
    // pre-pass runs BEFORE path lowering and the two passes compose.
    let path = Query::Path(Path {
        subject: value_var("s"),
        pattern: PathPattern::Predicate(PathPredicate {
            predicate: Some(iri("knows")),
        }),
        object: value_var("o"),
        path: None,
    });
    let q = and_of(vec![path, typeof_leaf(value_var("s"), "Person")]);
    reject(q, BgpPlanError::TypeOfNotStoredType);
}

// -------------------------------------------------------------------------------------------
// WOQL `Eval` integer arithmetic (verifier-recompute) — plan-level shape guard & reject-closed.
// -------------------------------------------------------------------------------------------

/// `Value(?var)` arithmetic leaf.
fn a_var(name: &str) -> ArithmeticExpression {
    ArithmeticExpression::Value(ArithmeticValue::Variable(name.into()))
}

/// `Value(literal)` arithmetic leaf.
fn a_lit(value: XSDAnySimpleType) -> ArithmeticExpression {
    ArithmeticExpression::Value(ArithmeticValue::Data(value))
}

fn a_plus(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Plus(Plus {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn a_minus(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Minus(Minus {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn a_times(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Times(Times {
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// An `Eval` conjunct binding `?result` to `expr`.
fn eval_conjunct(expr: ArithmeticExpression, result: &str) -> Query {
    Query::Eval(Eval {
        expression: expr,
        result_value: ArithmeticValue::Variable(result.into()),
    })
}

/// The canonical operand-binding child: `t(?x,p,?a) & t(?x,q,?b) & a>=0 & b>=0`, pinning
/// `?a`,`?b` to xsd:unsignedInt (UInt32) exactly as the Sum forms rely on.
fn ab_child() -> Vec<Query> {
    vec![
        tv("x", "p", "a"),
        tv("x", "q", "b"),
        gte(dvar("a"), duint(0)),
        gte(dvar("b"), duint(0)),
    ]
}

fn eval_query(expr: ArithmeticExpression, result: &str) -> Query {
    let mut conjuncts = ab_child();
    conjuncts.push(eval_conjunct(expr, result));
    and_of(conjuncts)
}

#[test]
fn eval_plus_over_two_integer_columns_compiles_to_eval_statement() {
    let compiled = plan_bgp(&eval_query(a_plus(a_var("a"), a_var("b")), "r")).unwrap();
    // The child columns are carried through and `?r` is appended as the derived column.
    assert_eq!(compiled.relation.schema.last().map(String::as_str), Some("r"));
    assert!(compiled.relation.schema.iter().any(|v| v == "a"));
    assert!(compiled.relation.schema.iter().any(|v| v == "b"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

#[test]
fn eval_nested_times_and_literal_tree_compiles() {
    // Plus(Times(?a, 2), ?b) — nesting is free under verifier-recompute.
    let expr = a_plus(a_times(a_var("a"), a_lit(XSDAnySimpleType::Int(2))), a_var("b"));
    let compiled = plan_bgp(&eval_query(expr, "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

#[test]
fn eval_minus_compiles() {
    let compiled = plan_bgp(&eval_query(a_minus(a_var("a"), a_var("b")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

fn a_divide(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Divide(Divide {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn a_floor(argument: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Floor(Floor {
        argument: Box::new(argument),
    })
}

#[test]
fn eval_divide_compiles_to_rational_arith_statement() {
    // `Divide` (rdiv) is now provable via the exact-rational value layer: it delivers a TERMINAL,
    // revealed xsd:decimal result (like Concat), so the statement is `RationalArith` and the child
    // schema is UNCHANGED (the result variable is not a committed column).
    let compiled = plan_bgp(&eval_query(a_divide(a_var("a"), a_var("b")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::RationalArith { .. }
    ));
    // Terminal result: `?r` is NOT appended to the committed schema (unlike integer Eval).
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
    assert!(compiled.relation.schema.iter().any(|v| v == "a"));
}

#[test]
fn eval_divide_by_static_zero_rejects_closed() {
    // A static zero constant divisor is `evaluation_error(zero_divisor)` in Prolog: reject closed.
    reject(
        eval_query(a_divide(a_var("a"), a_lit(XSDAnySimpleType::Integer(0))), "r"),
        BgpPlanError::EvalDivideByZero,
    );
}

#[test]
fn eval_floor_of_integer_column_compiles_to_floor_statement() {
    // Top-level `floor(x)` compiles to `FloorEval` and — like integer Eval — APPENDS the derived
    // integer result column (`?r`) to the committed schema.
    let compiled = plan_bgp(&eval_query(a_floor(a_var("a")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::FloorEval { .. }
    ));
    assert_eq!(compiled.relation.schema.last().map(String::as_str), Some("r"));
}

#[test]
fn eval_floor_of_divide_compiles_to_floor_statement() {
    // `floor(a / b)` — a nested Divide under a top-level Floor still delivers an integer.
    let compiled =
        plan_bgp(&eval_query(a_floor(a_divide(a_var("a"), a_var("b"))), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::FloorEval { .. }
    ));
}

#[test]
fn eval_decimal_literal_operand_routes_to_rational_arith() {
    // A `Plus` with an xsd:decimal literal is a rational signal: it routes to the exact-rational
    // layer (terminal xsd:decimal delivery), NOT the integer Eval path.
    use decimal_rs::Decimal;
    use std::str::FromStr;
    let compiled = plan_bgp(&eval_query(
        a_plus(a_var("a"), a_lit(XSDAnySimpleType::Decimal(Decimal::from_str("1.5").unwrap()))),
        "r",
    ))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::RationalArith { .. }
    ));
}

#[test]
fn eval_pure_integer_plus_still_uses_integer_eval() {
    // No rational signal (no Divide/Floor/decimal) -> the existing integer Eval delivery is kept.
    let compiled = plan_bgp(&eval_query(a_plus(a_var("a"), a_var("b")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

fn a_div(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Div(Div {
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn a_exp(left: ArithmeticExpression, right: ArithmeticExpression) -> ArithmeticExpression {
    ArithmeticExpression::Exp(Exp {
        left: Box::new(left),
        right: Box::new(right),
    })
}

#[test]
fn eval_div_over_two_integer_columns_compiles_to_eval_statement() {
    // Integer FLOOR division is verifier-recompute-provable now.
    let compiled = plan_bgp(&eval_query(a_div(a_var("a"), a_var("b")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

#[test]
fn eval_exp_with_column_exponent_compiles_to_eval_statement() {
    // Integer power over a (runtime) non-negative exponent is provable; a runtime-negative
    // exponent fails closed later in the recompute, not at plan time.
    let compiled = plan_bgp(&eval_query(a_exp(a_var("a"), a_var("b")), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Eval { .. }
    ));
}

#[test]
fn eval_exp_with_static_negative_exponent_rejects_closed() {
    // A STATIC negative literal exponent is a rational result → reject at compile time.
    reject(
        eval_query(a_exp(a_var("a"), a_lit(XSDAnySimpleType::Int(-1))), "r"),
        BgpPlanError::EvalUnsupportedOperator {
            operator: "Exp (negative exponent)",
        },
    );
}

#[test]
fn eval_divide_with_unbound_operand_rejects_closed() {
    // Even in the rational layer, an operand not bound by any store read cannot be authenticated.
    let query = and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        eval_conjunct(a_divide(a_var("a"), a_var("b")), "r"),
    ]);
    reject(query, BgpPlanError::EvalOperandNotBound { var: "b".into() });
}

#[test]
fn eval_float_literal_operand_rejects_closed() {
    reject(
        eval_query(a_plus(a_var("a"), a_lit(XSDAnySimpleType::Float(1.5))), "r"),
        BgpPlanError::EvalUnsupportedLiteral {
            datatype: "xsd:double",
        },
    );
}

#[test]
fn eval_operand_bound_by_no_store_read_rejects_closed() {
    // `?b` is never bound by a store read: it cannot be authenticated.
    let query = and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        eval_conjunct(a_plus(a_var("a"), a_var("b")), "r"),
    ]);
    reject(query, BgpPlanError::EvalOperandNotBound { var: "b".into() });
}

#[test]
fn eval_operand_without_pinned_integer_datatype_rejects_closed() {
    // `?a` is a store object but its datatype is not pinned by a range comparison, so it is
    // not a known exact fixed-width integer — reject rather than guess a datatype.
    let query = and_of(vec![
        tv("x", "p", "a"),
        tv("x", "q", "b"),
        gte(dvar("b"), duint(0)),
        eval_conjunct(a_plus(a_var("a"), a_var("b")), "r"),
    ]);
    reject(query, BgpPlanError::EvalOperandNotInteger { var: "a".into() });
}

#[test]
fn eval_result_already_bound_rejects_closed() {
    // `?a` is already an authenticated binding; it may not be redefined as the derived column.
    reject(
        eval_query(a_plus(a_var("a"), a_var("b")), "a"),
        BgpPlanError::EvalResultAlreadyBound { var: "a".into() },
    );
}

#[test]
fn eval_result_literal_rejects_closed() {
    let query = {
        let mut conjuncts = ab_child();
        conjuncts.push(Query::Eval(Eval {
            expression: a_plus(a_var("a"), a_var("b")),
            result_value: ArithmeticValue::Data(XSDAnySimpleType::Int(0)),
        }));
        and_of(conjuncts)
    };
    reject(query, BgpPlanError::EvalResultNotVariable);
}

// -------------------------------------------------------------------------------------------
// WOQL `Typecast` integer-width conversion (verifier-recompute) — plan-level shape guard.
// -------------------------------------------------------------------------------------------

/// A full XSD type IRI node value for a local type name.
fn xsd_type(name: &str) -> NodeValue {
    NodeValue::Node(format!("http://www.w3.org/2001/XMLSchema#{name}"))
}

/// A `Typecast(?source, target, ?result)` conjunct.
fn typecast_conjunct(source: &str, target: NodeValue, result: &str) -> Query {
    Query::Typecast(Typecast {
        value: Value::Variable(source.into()),
        type_uri: target,
        result_value: Value::Variable(result.into()),
    })
}

/// `t(?x,p,?a) & a>=0 & Typecast(?a, target, ?r)` — pins `?a` to xsd:unsignedInt (UInt32).
fn typecast_query(target: NodeValue, result: &str) -> Query {
    and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        typecast_conjunct("a", target, result),
    ])
}

#[test]
fn typecast_widening_unsigned_int_to_long_compiles() {
    let compiled = plan_bgp(&typecast_query(xsd_type("long"), "r")).unwrap();
    assert_eq!(compiled.relation.schema.last().map(String::as_str), Some("r"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Typecast { .. }
    ));
}

#[test]
fn typecast_narrowing_to_byte_compiles_range_check_deferred_to_prove() {
    // The plan compiles; the per-row out-of-range fail-closed check happens in the recompute.
    let compiled = plan_bgp(&typecast_query(xsd_type("byte"), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Typecast { .. }
    ));
}

#[test]
fn typecast_numeric_to_string_compiles() {
    // A numeric (pinned UInt32) source cast to xsd:string is the number→string cast — a TERMINAL
    // revealed value (the result is NOT appended to the committed schema).
    let compiled = plan_bgp(&typecast_query(xsd_type("string"), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::TypecastNumberToString { .. }
    ));
    // Terminal: the child schema is carried through verbatim (no committed result column).
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
}

/// `t(?x,p,?a) & Typecast(?a, target, ?r)` — `?a` is an UNPINNED store value (no range filter), so
/// its type is enforced only at recompute (the shape used for string→number casts).
fn typecast_unpinned_query(target: NodeValue, result: &str) -> Query {
    and_of(vec![tv("x", "p", "a"), typecast_conjunct("a", target, result)])
}

#[test]
fn typecast_string_to_integer_compiles() {
    let compiled = plan_bgp(&typecast_unpinned_query(xsd_type("integer"), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::TypecastStringToNumber {
            target: StringNumericTarget::Integer,
            ..
        }
    ));
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
}

#[test]
fn typecast_string_to_decimal_compiles() {
    let compiled = plan_bgp(&typecast_unpinned_query(xsd_type("decimal"), "r")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::TypecastStringToNumber {
            target: StringNumericTarget::Decimal,
            ..
        }
    ));
}

#[test]
fn typecast_to_float_target_rejects_closed() {
    // xsd:double is neither a fixed-width integer NOR a number↔string target — reject closed
    // (float↔string is NOT provable: SWI's float `~w` formatting is not reproduced byte-exact).
    reject(
        typecast_query(xsd_type("double"), "r"),
        BgpPlanError::TypecastUnsupportedTarget {
            iri: "http://www.w3.org/2001/XMLSchema#double".into(),
        },
    );
}

#[test]
fn typecast_string_to_float_target_rejects_closed() {
    // xsd:float is not a provable string→number target (float formatting not reproduced) — reject.
    reject(
        typecast_unpinned_query(xsd_type("float"), "r"),
        BgpPlanError::TypecastUnsupportedTarget {
            iri: "http://www.w3.org/2001/XMLSchema#float".into(),
        },
    );
}

#[test]
fn typecast_numeric_source_to_integer_rejects_closed() {
    // A source PINNED to a fixed-width integer (UInt32) cast to xsd:integer would be a string→number
    // cast, but the pinned datatype is not xsd:string — reject closed at compile (like Concat).
    reject(
        typecast_query(xsd_type("integer"), "r"),
        BgpPlanError::TypecastValueSourceInvalid { var: "a".into() },
    );
}

#[test]
fn typecast_variable_target_rejects_closed() {
    let query = and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        typecast_conjunct("a", NodeValue::Variable("t".into()), "r"),
    ]);
    reject(query, BgpPlanError::TypecastTargetNotNode);
}

#[test]
fn typecast_source_bound_by_no_store_read_rejects_closed() {
    // `?b` is never bound by a store read: it cannot be authenticated as a source.
    let query = and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        typecast_conjunct("b", xsd_type("long"), "r"),
    ]);
    reject(query, BgpPlanError::TypecastSourceNotBound { var: "b".into() });
}

#[test]
fn typecast_source_without_pinned_integer_datatype_rejects_closed() {
    // `?a` is a store object but its datatype is not pinned by a range comparison.
    let query = and_of(vec![
        tv("x", "p", "a"),
        typecast_conjunct("a", xsd_type("long"), "r"),
    ]);
    reject(query, BgpPlanError::TypecastSourceNotInteger { var: "a".into() });
}

#[test]
fn typecast_result_already_bound_rejects_closed() {
    let query = and_of(vec![
        tv("x", "p", "a"),
        gte(dvar("a"), duint(0)),
        typecast_conjunct("a", xsd_type("long"), "a"),
    ]);
    reject(query, BgpPlanError::TypecastResultInvalid);
}

// -------------------------------------------------------------------------------------------
// WOQL `Concatenate` (string value circuit) — plan/shape guard.
// Child `t(?x, name, ?a) & t(?x, greet, ?b)` binds two unpinned node/value operand columns;
// their per-row xsd:string-ness is enforced at recompute, so the plan accepts them here.
// -------------------------------------------------------------------------------------------

/// `Query::Concatenate(concat(elements, ?result))`.
fn concat_conjunct(elements: Vec<DataValue>, result: &str) -> Query {
    Query::Concatenate(Concatenate {
        list: ListOrVariable::List(elements),
        result_string: DataValue::Variable(result.to_string()),
    })
}

/// `t(?x, name, ?a) & t(?x, greet, ?b) & concat(elements, ?result)`.
fn concat_query(elements: Vec<DataValue>, result: &str) -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        concat_conjunct(elements, result),
    ])
}

#[test]
fn concat_two_string_columns_with_literal_compiles_to_concat_statement() {
    let compiled = plan_bgp(&concat_query(
        vec![dvar("a"), dstr(" "), dvar("b")],
        "r",
    ))
    .unwrap();
    // The child operand columns are carried through; the result variable is a TERMINAL/revealed
    // value, so it is NOT added to the committed schema.
    assert!(compiled.relation.schema.iter().any(|v| v == "a"));
    assert!(compiled.relation.schema.iter().any(|v| v == "b"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Concat { .. }
    ));
}

#[test]
fn concat_all_literal_and_variable_elements_compiles() {
    // A leading + trailing literal around a single column operand.
    let compiled = plan_bgp(&concat_query(
        vec![dstr("<<"), dvar("a"), dstr(">>")],
        "r",
    ))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Concat { .. }
    ));
}

#[test]
fn concat_non_string_literal_element_rejects_closed() {
    // A numeric list element would be STRINGIFIED by SWI interpolate_string (a separate circuit).
    reject(
        concat_query(vec![dvar("a"), duint(42), dvar("b")], "r"),
        BgpPlanError::ConcatLiteralNotString,
    );
}

#[test]
fn concat_unbound_operand_rejects_closed() {
    // `?c` is not bound by any store read in the child.
    reject(
        concat_query(vec![dvar("a"), dvar("c")], "r"),
        BgpPlanError::ConcatOperandNotBound { var: "c".into() },
    );
}

#[test]
fn concat_operand_pinned_non_string_datatype_rejects_closed() {
    // Pinning `?a` to a numeric datatype via a range filter makes it a non-string operand.
    let query = and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        gte(dvar("a"), duint(0)),
        concat_conjunct(vec![dvar("a"), dvar("b")], "r"),
    ]);
    reject(query, BgpPlanError::ConcatOperandNotString { var: "a".into() });
}

#[test]
fn concat_result_already_bound_rejects_closed() {
    reject(
        concat_query(vec![dvar("a"), dvar("b")], "a"),
        BgpPlanError::ConcatResultAlreadyBound { var: "a".into() },
    );
}

#[test]
fn concat_result_literal_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Concatenate(Concatenate {
            list: ListOrVariable::List(vec![dvar("a"), dvar("b")]),
            result_string: dstr("fixed"),
        }),
    ]);
    reject(query, BgpPlanError::ConcatResultNotVariable);
}

#[test]
fn concat_runtime_list_variable_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        Query::Concatenate(Concatenate {
            list: ListOrVariable::Variable(dvar("a")),
            result_string: DataValue::Variable("r".into()),
        }),
    ]);
    reject(query, BgpPlanError::ConcatListNotConcrete);
}

#[test]
fn concat_empty_list_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        Query::Concatenate(Concatenate {
            list: ListOrVariable::List(vec![]),
            result_string: DataValue::Variable("r".into()),
        }),
    ]);
    reject(query, BgpPlanError::ConcatEmptyList);
}

#[test]
fn concat_nested_list_element_rejects_closed() {
    reject(
        concat_query(vec![dvar("a"), dlist(vec![dstr("x")])], "r"),
        BgpPlanError::ConcatNestedList,
    );
}

#[test]
fn concat_bare_without_operand_binding_rejects_closed() {
    // A lone concat conjunct with no store-binding child.
    let query = and_of(vec![concat_conjunct(vec![dstr("a"), dstr("b")], "r")]);
    reject(query, BgpPlanError::ConcatUnsupportedShape);
}

// -------------------------------------------------------------------------------------------
// WOQL `Join` (separator-interposed concat) — plan/shape guard.
// -------------------------------------------------------------------------------------------

/// `Query::Join(join(elements, sep, ?result))`.
fn join_conjunct(elements: Vec<DataValue>, separator: DataValue, result: &str) -> Query {
    Query::Join(Join {
        list: ListOrVariable::List(elements),
        separator,
        result_string: DataValue::Variable(result.to_string()),
    })
}

/// `t(?x, name, ?a) & t(?x, greet, ?b) & join(elements, sep, ?result)`.
fn join_query(elements: Vec<DataValue>, separator: DataValue, result: &str) -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        join_conjunct(elements, separator, result),
    ])
}

#[test]
fn join_two_string_columns_with_literal_separator_compiles_to_join_statement() {
    let compiled = plan_bgp(&join_query(vec![dvar("a"), dvar("b")], dstr(", "), "r")).unwrap();
    // Child operand columns carried through; the result variable is TERMINAL (not committed).
    assert!(compiled.relation.schema.iter().any(|v| v == "a"));
    assert!(compiled.relation.schema.iter().any(|v| v == "b"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Join { .. }
    ));
}

#[test]
fn join_authenticated_string_variable_separator_compiles() {
    // The separator is itself a bound store column (`?b`), not a literal.
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Join(Join {
            list: ListOrVariable::List(vec![dvar("a"), dstr("x"), dstr("y")]),
            separator: dvar("b"),
            result_string: DataValue::Variable("r".into()),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Join { .. }
    ));
}

#[test]
fn join_runtime_list_variable_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        Query::Join(Join {
            list: ListOrVariable::Variable(dvar("a")),
            separator: dstr(","),
            result_string: DataValue::Variable("r".into()),
        }),
    ]);
    reject(query, BgpPlanError::JoinListNotConcrete);
}

#[test]
fn join_non_string_element_rejects_closed() {
    // A numeric list element would be stringified by SWI interpolate_string (a separate circuit).
    reject(
        join_query(vec![dvar("a"), duint(42)], dstr(","), "r"),
        BgpPlanError::ConcatLiteralNotString,
    );
}

#[test]
fn join_non_string_separator_rejects_closed() {
    // A numeric separator would likewise be stringified. Reject closed.
    reject(
        join_query(vec![dvar("a"), dvar("b")], duint(9), "r"),
        BgpPlanError::ConcatLiteralNotString,
    );
}

#[test]
fn join_unbound_operand_rejects_closed() {
    reject(
        join_query(vec![dvar("a"), dvar("c")], dstr(","), "r"),
        BgpPlanError::ConcatOperandNotBound { var: "c".into() },
    );
}

#[test]
fn join_unbound_separator_variable_rejects_closed() {
    reject(
        join_query(vec![dvar("a"), dvar("b")], dvar("c"), "r"),
        BgpPlanError::ConcatOperandNotBound { var: "c".into() },
    );
}

#[test]
fn join_result_already_bound_rejects_closed() {
    reject(
        join_query(vec![dvar("a"), dvar("b")], dstr(","), "a"),
        BgpPlanError::JoinResultAlreadyBound { var: "a".into() },
    );
}

#[test]
fn join_result_literal_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::Join(Join {
            list: ListOrVariable::List(vec![dvar("a"), dvar("b")]),
            separator: dstr(","),
            result_string: dstr("fixed"),
        }),
    ]);
    reject(query, BgpPlanError::JoinResultNotVariable);
}

#[test]
fn join_empty_list_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        Query::Join(Join {
            list: ListOrVariable::List(vec![]),
            separator: dstr(","),
            result_string: DataValue::Variable("r".into()),
        }),
    ]);
    reject(query, BgpPlanError::JoinEmptyList);
}

#[test]
fn join_bare_without_operand_binding_rejects_closed() {
    let query = and_of(vec![join_conjunct(vec![dstr("a"), dstr("b")], dstr(","), "r")]);
    reject(query, BgpPlanError::JoinUnsupportedShape);
}

// -------------------------------------------------------------------------------------------
// WOQL `Substring` (per-row constant-index character substring) — plan/shape guard.
// -------------------------------------------------------------------------------------------

/// `t(?x, name, ?s) & sub_string(?s, before, length, ?after, ?sub)`.
fn substring_query(before: DataValue, length: DataValue, after: &str, sub: &str) -> Query {
    and_of(vec![
        tv("x", "name", "s"),
        Query::Substring(Substring {
            string: dvar("s"),
            before,
            length,
            after: DataValue::Variable(after.to_string()),
            substring: DataValue::Variable(sub.to_string()),
        }),
    ])
}

#[test]
fn substring_constant_index_over_authenticated_string_compiles() {
    let compiled = plan_bgp(&substring_query(duint(1), duint(3), "after", "sub")).unwrap();
    assert!(compiled.relation.schema.iter().any(|v| v == "s"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "sub"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "after"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Substring { .. }
    ));
}

#[test]
fn substring_literal_input_string_rejects_closed() {
    // A constant string input is not an authenticated store value.
    let query = and_of(vec![
        tv("x", "name", "s"),
        Query::Substring(Substring {
            string: dstr("hello"),
            before: duint(0),
            length: duint(2),
            after: DataValue::Variable("after".into()),
            substring: DataValue::Variable("sub".into()),
        }),
    ]);
    reject(query, BgpPlanError::SubstringStringNotVariable);
}

#[test]
fn substring_unbound_input_string_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "s"),
        Query::Substring(Substring {
            string: dvar("z"),
            before: duint(0),
            length: duint(2),
            after: DataValue::Variable("after".into()),
            substring: DataValue::Variable("sub".into()),
        }),
    ]);
    reject(query, BgpPlanError::SubstringStringNotBound { var: "z".into() });
}

#[test]
fn substring_variable_index_rejects_closed() {
    // A per-row store-authenticated index is a deferred follow-up.
    reject(
        substring_query(dvar("i"), duint(3), "after", "sub"),
        BgpPlanError::SubstringIndexNotLiteral,
    );
}

#[test]
fn substring_negative_index_rejects_closed() {
    // SWI sub_string requires non-negative offsets; a negative literal fails/throws — reject closed.
    reject(
        substring_query(dint(-1), duint(3), "after", "sub"),
        BgpPlanError::SubstringNegativeIndex,
    );
}

#[test]
fn substring_non_integer_index_rejects_closed() {
    reject(
        substring_query(dstr("x"), duint(3), "after", "sub"),
        BgpPlanError::SubstringIndexNotInteger,
    );
}

#[test]
fn substring_after_constant_rejects_closed() {
    // A bound After adds an unmodeled length-equality filter.
    let query = and_of(vec![
        tv("x", "name", "s"),
        Query::Substring(Substring {
            string: dvar("s"),
            before: duint(0),
            length: duint(2),
            after: duint(5),
            substring: DataValue::Variable("sub".into()),
        }),
    ]);
    reject(query, BgpPlanError::SubstringAfterNotVariable);
}

#[test]
fn substring_result_literal_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "s"),
        Query::Substring(Substring {
            string: dvar("s"),
            before: duint(0),
            length: duint(2),
            after: DataValue::Variable("after".into()),
            substring: dstr("fixed"),
        }),
    ]);
    reject(query, BgpPlanError::SubstringResultNotVariable);
}

#[test]
fn substring_result_already_bound_rejects_closed() {
    // The result var `?s` is already the (bound) input column.
    reject(
        substring_query(duint(0), duint(2), "after", "s"),
        BgpPlanError::SubstringResultAlreadyBound { var: "s".into() },
    );
}

#[test]
fn substring_result_aliases_after_rejects_closed() {
    reject(
        substring_query(duint(0), duint(2), "same", "same"),
        BgpPlanError::SubstringResultAliasesAfter,
    );
}

// -------------------------------------------------------------------------------------------
// WOQL `Pad` planning (`utils:pad/4`: constant single-char left-pad to a constant target length).
// -------------------------------------------------------------------------------------------

fn pad_conjunct(string: DataValue, char: DataValue, times: DataValue, result: &str) -> Query {
    Query::Pad(Pad {
        string,
        char,
        times,
        result_string: DataValue::Variable(result.to_string()),
    })
}

/// `t(?x, name, ?s) & pad(?s, char, times, ?result)`.
fn pad_query(char: DataValue, times: DataValue, result: &str) -> Query {
    and_of(vec![
        tv("x", "name", "s"),
        pad_conjunct(dvar("s"), char, times, result),
    ])
}

#[test]
fn pad_authenticated_string_single_char_constant_length_compiles() {
    let compiled = plan_bgp(&pad_query(dstr("0"), duint(5), "r")).unwrap();
    // The input operand column is carried through; the result is TERMINAL/revealed (NOT committed).
    assert!(compiled.relation.schema.iter().any(|v| v == "s"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "r"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::Pad { .. }
    ));
}

#[test]
fn pad_multi_char_pad_char_rejects_closed() {
    // utils:pad requires string_length(C) == 1 (throws pad_not_length_one otherwise).
    reject(
        pad_query(dstr("ab"), duint(5), "r"),
        BgpPlanError::PadCharNotSingleChar,
    );
}

#[test]
fn pad_variable_char_rejects_closed() {
    // A per-row store-authenticated pad char is a deferred follow-up.
    reject(
        pad_query(dvar("c"), duint(5), "r"),
        BgpPlanError::PadCharNotLiteral,
    );
}

#[test]
fn pad_non_string_char_rejects_closed() {
    reject(
        pad_query(duint(7), duint(5), "r"),
        BgpPlanError::PadCharNotString,
    );
}

#[test]
fn pad_variable_length_rejects_closed() {
    // A per-row store-authenticated target length is a deferred follow-up.
    reject(
        pad_query(dstr("0"), dvar("n"), "r"),
        BgpPlanError::PadLengthNotLiteral,
    );
}

#[test]
fn pad_negative_length_rejects_closed() {
    // A negative target length always throws bad_pad_length in the reference (L2 = L - L1 < 0).
    reject(
        pad_query(dstr("0"), dint(-1), "r"),
        BgpPlanError::PadNegativeLength,
    );
}

#[test]
fn pad_non_integer_length_rejects_closed() {
    reject(
        pad_query(dstr("0"), dstr("five"), "r"),
        BgpPlanError::PadLengthNotInteger,
    );
}

#[test]
fn pad_literal_input_string_rejects_closed() {
    // A constant string input is not an authenticated store value.
    let query = and_of(vec![
        tv("x", "name", "s"),
        pad_conjunct(dstr("hello"), dstr("0"), duint(5), "r"),
    ]);
    reject(query, BgpPlanError::PadStringNotVariable);
}

#[test]
fn pad_unbound_input_string_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "s"),
        pad_conjunct(dvar("z"), dstr("0"), duint(5), "r"),
    ]);
    reject(query, BgpPlanError::PadStringNotBound { var: "z".into() });
}

#[test]
fn pad_pinned_non_string_input_rejects_closed() {
    // Pinning `?s` to a numeric datatype via a range filter makes it a non-string operand.
    let query = and_of(vec![
        tv("x", "name", "s"),
        gte(dvar("s"), duint(0)),
        pad_conjunct(dvar("s"), dstr("0"), duint(5), "r"),
    ]);
    reject(query, BgpPlanError::PadStringNotAuthenticated { var: "s".into() });
}

#[test]
fn pad_result_literal_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "s"),
        Query::Pad(Pad {
            string: dvar("s"),
            char: dstr("0"),
            times: duint(5),
            result_string: dstr("fixed"),
        }),
    ]);
    reject(query, BgpPlanError::PadResultNotVariable);
}

#[test]
fn pad_result_already_bound_rejects_closed() {
    // The result var `?s` is already the (bound) input column.
    reject(
        pad_query(dstr("0"), duint(5), "s"),
        BgpPlanError::PadResultAlreadyBound { var: "s".into() },
    );
}

// -------------------------------------------------------------------------------------------
// WOQL `HashKey`/`hash` planning (`idgen_hash/3`: SHA-256 document-ID over base + args).
// -------------------------------------------------------------------------------------------

fn hashkey_conjunct(base: DataValue, args: Vec<DataValue>, uri: &str) -> Query {
    Query::HashKey(HashKey {
        base,
        key_list: args,
        uri: NodeValue::Variable(uri.to_string()),
    })
}

/// `t(?x, name, ?a) & t(?x, greet, ?b) & hash(base, args, ?uri)`.
fn hashkey_query(base: DataValue, args: Vec<DataValue>, uri: &str) -> Query {
    and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        hashkey_conjunct(base, args, uri),
    ])
}

#[test]
fn hashkey_literal_base_with_string_columns_compiles() {
    let compiled =
        plan_bgp(&hashkey_query(dstr("http://i/X/"), vec![dvar("a"), dvar("b")], "id")).unwrap();
    // The arg operand columns are carried through; the result URI is TERMINAL/revealed (NOT committed).
    assert!(compiled.relation.schema.iter().any(|v| v == "a"));
    assert!(compiled.relation.schema.iter().any(|v| v == "b"));
    assert!(!compiled.relation.schema.iter().any(|v| v == "id"));
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::HashKey { .. }
    ));
}

#[test]
fn hashkey_authenticated_base_variable_compiles() {
    // Base may itself be an authenticated string column (raw-prepended before the hash).
    let compiled = plan_bgp(&hashkey_query(dvar("a"), vec![dvar("b")], "id")).unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::HashKey { .. }
    ));
}

#[test]
fn hashkey_non_string_arg_rejects_closed() {
    // A numeric arg would be stringified by SWI ~w (a separate circuit) — reject closed.
    reject(
        hashkey_query(dstr("X/"), vec![dvar("a"), duint(42)], "id"),
        BgpPlanError::ConcatLiteralNotString,
    );
}

#[test]
fn hashkey_unbound_arg_rejects_closed() {
    reject(
        hashkey_query(dstr("X/"), vec![dvar("a"), dvar("c")], "id"),
        BgpPlanError::ConcatOperandNotBound { var: "c".into() },
    );
}

#[test]
fn hashkey_result_node_constant_rejects_closed() {
    let query = and_of(vec![
        tv("x", "name", "a"),
        tv("x", "greet", "b"),
        Query::HashKey(HashKey {
            base: dstr("X/"),
            key_list: vec![dvar("a")],
            uri: NodeValue::Node("http://i/fixed".into()),
        }),
    ]);
    reject(query, BgpPlanError::HashKeyResultNotVariable);
}

#[test]
fn hashkey_result_already_bound_rejects_closed() {
    // The result var `?a` is already a (bound) arg column.
    reject(
        hashkey_query(dstr("X/"), vec![dvar("a"), dvar("b")], "a"),
        BgpPlanError::HashKeyResultAlreadyBound { var: "a".into() },
    );
}

#[test]
fn upper_rejects_closed_locale_deferred() {
    // Deferred: SWI string_upper is locale-dependent (unpinned LC_CTYPE), so ASCII fidelity is
    // unprovable across deployments (Turkic 'i' -> U+0130). Reject closed with the documented error.
    let query = and_of(vec![
        tv("x", "name", "a"),
        Query::Upper(Upper {
            mixed: DataValue::Variable("a".into()),
            upper: DataValue::Variable("r".into()),
        }),
    ]);
    reject(query, BgpPlanError::StringCaseOpLocaleUnsupported);
}

// ============================================================================================
// WOQL single-input CALENDAR ops (Weekday/WeekdaySundayStart/IsoWeek + Day*/Month*)
// ============================================================================================

/// A child BGP that binds `?date` to an `xsd:date` store value (`t(?x, d, ?date)`).
fn cal_child_date() -> Query {
    tv("x", "d", "date")
}

#[test]
fn weekday_compiles_to_calendar_int_appending_result() {
    let compiled = plan_bgp(&and_of(vec![
        cal_child_date(),
        Query::Weekday(Weekday {
            date: dvar("date"),
            weekday: dvar("w"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarInt { .. }
    ));
    // The integer result is APPENDED to the committed schema (Select/Count over it works).
    assert_eq!(compiled.relation.schema.last().map(String::as_str), Some("w"));
    assert!(compiled.relation.schema.iter().any(|v| v == "date"));
}

#[test]
fn weekday_sunday_start_compiles_to_calendar_int() {
    let compiled = plan_bgp(&and_of(vec![
        cal_child_date(),
        Query::WeekdaySundayStart(WeekdaySundayStart {
            date: dvar("date"),
            weekday: dvar("w"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarInt { .. }
    ));
    assert_eq!(compiled.relation.schema.last().map(String::as_str), Some("w"));
}

#[test]
fn iso_week_compiles_to_calendar_int_appending_year_and_week() {
    let compiled = plan_bgp(&and_of(vec![
        cal_child_date(),
        Query::IsoWeek(IsoWeek {
            date: dvar("date"),
            week: dvar("wk"),
            year: dvar("yr"),
        }),
    ]))
    .unwrap();
    // TWO integer outputs are appended (year + week).
    assert!(compiled.relation.schema.iter().any(|v| v == "yr"));
    assert!(compiled.relation.schema.iter().any(|v| v == "wk"));
    if let CompiledRelationStatement::CalendarInt { outputs, .. } = &compiled.relation.statement {
        assert_eq!(outputs.len(), 2);
    } else {
        panic!("expected a CalendarInt statement");
    }
}

#[test]
fn day_after_compiles_to_calendar_date_terminal_result() {
    let compiled = plan_bgp(&and_of(vec![
        cal_child_date(),
        Query::DayAfter(DayAfter {
            date: dvar("date"),
            next: dvar("n"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarDate { .. }
    ));
    // Terminal/revealed: the result is NOT a committed schema column.
    assert!(!compiled.relation.schema.iter().any(|v| v == "n"));
    assert!(compiled.relation.schema.iter().any(|v| v == "date"));
}

#[test]
fn day_before_compiles_to_calendar_date() {
    let compiled = plan_bgp(&and_of(vec![
        cal_child_date(),
        Query::DayBefore(DayBefore {
            date: dvar("date"),
            previous: dvar("p"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarDate { .. }
    ));
    assert!(!compiled.relation.schema.iter().any(|v| v == "p"));
}

#[test]
fn month_start_date_compiles_to_calendar_date_over_yearmonth() {
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "ym", "date"),
        Query::MonthStartDate(MonthStartDate {
            year_month: dvar("date"),
            date: dvar("out"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarDate { .. }
    ));
    assert!(!compiled.relation.schema.iter().any(|v| v == "out"));
}

#[test]
fn month_end_date_compiles_to_calendar_date_over_yearmonth() {
    let compiled = plan_bgp(&and_of(vec![
        tv("x", "ym", "date"),
        Query::MonthEndDate(MonthEndDate {
            year_month: dvar("date"),
            date: dvar("out"),
        }),
    ]))
    .unwrap();
    assert!(matches!(
        compiled.relation.statement,
        CompiledRelationStatement::CalendarDate { .. }
    ));
}

// ---- reject-closed ----

#[test]
fn interval_op_rejects_closed_as_two_input() {
    reject(
        and_of(vec![
            tv("x", "s", "a"),
            Query::Interval(Interval {
                start: dvar("a"),
                end: dvar("b"),
                interval: dvar("i"),
            }),
        ]),
        BgpPlanError::CalendarTwoInputUnsupported { op: "Interval" },
    );
}

#[test]
fn weekday_over_numeric_range_pinned_operand_rejects_closed() {
    // `?a` is pinned to a NUMERIC datatype by the range filter, so it is not an xsd:date operand.
    reject(
        and_of(vec![
            tv("x", "p", "a"),
            gte(dvar("a"), duint(0)),
            Query::Weekday(Weekday {
                date: dvar("a"),
                weekday: dvar("w"),
            }),
        ]),
        BgpPlanError::CalendarOperandNotDate { var: "a".into() },
    );
}

#[test]
fn weekday_result_literal_rejects_closed_validate_mode() {
    reject(
        and_of(vec![
            cal_child_date(),
            Query::Weekday(Weekday {
                date: dvar("date"),
                weekday: duint(1),
            }),
        ]),
        BgpPlanError::CalendarResultNotVariable,
    );
}

#[test]
fn weekday_result_already_bound_rejects_closed() {
    reject(
        and_of(vec![
            cal_child_date(),
            Query::Weekday(Weekday {
                date: dvar("date"),
                weekday: dvar("date"),
            }),
        ]),
        BgpPlanError::CalendarResultAlreadyBound {
            var: "date".into(),
        },
    );
}

#[test]
fn iso_week_colliding_result_variables_reject_closed() {
    reject(
        and_of(vec![
            cal_child_date(),
            Query::IsoWeek(IsoWeek {
                date: dvar("date"),
                week: dvar("w"),
                year: dvar("w"),
            }),
        ]),
        BgpPlanError::CalendarResultAlreadyBound { var: "w".into() },
    );
}

#[test]
fn day_after_bare_without_operand_binding_rejects_closed() {
    // A bare calendar op (wrapped in a trivial And with only itself) has no store-authenticated
    // operand to recompute over.
    reject(
        and_of(vec![Query::DayAfter(DayAfter {
            date: dvar("date"),
            next: dvar("n"),
        })]),
        BgpPlanError::CalendarUnsupportedShape,
    );
}

// ---------------------------------------------------------------------------------------------------
// SCHEMA-GRAPH FOUNDATION — `Subsumption` plan shape + graph-selector digest binding.
// ---------------------------------------------------------------------------------------------------

fn class_nv(name: &str) -> NodeValue {
    NodeValue::Node(iri(name))
}

fn var_nv(name: &str) -> NodeValue {
    NodeValue::Variable(name.to_string())
}

fn subsumption(child: NodeValue, parent: NodeValue) -> Query {
    Query::Subsumption(Subsumption { child, parent })
}

#[test]
fn subsumption_child_bound_compiles_to_forward_schema_closure() {
    // `A << ?y` — forward reflexive sys:inherits closure over the SCHEMA graph, delivering ?y.
    let c = plan_bgp(&subsumption(class_nv("A"), var_nv("y"))).unwrap();
    assert_eq!(c.relation.schema, ["y"]);
    // The intrinsic distinct([child,parent]) gate must be present.
    assert!(c.full_row_distinct, "subsumption must be Distinct-gated (set semantics)");
    let CompiledRelationStatement::TransitiveClosure {
        reflexive,
        inverted,
        graph,
        edge,
        ..
    } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure, got {:?}", c.relation.statement);
    };
    assert!(*reflexive, "class_subsumed is reflexive-transitive");
    assert!(!*inverted, "child-bound is a forward walk");
    assert_eq!(*graph, ProofGraph::Schema, "subsumption reads the schema graph");
    // The edge predicate is the sys:inherits IRI (a single predicate-constant atom).
    assert_eq!(edge.atoms.len(), 1);
    assert_eq!(
        edge.atoms[0].predicate_term,
        SYS_INHERITS_IRI.as_bytes(),
        "the closure edge must be the sys:inherits predicate"
    );
}

#[test]
fn subsumption_parent_bound_inverts_over_schema() {
    // `?x << C` — inverted reflexive closure from the parent, delivering ?x.
    let c = plan_bgp(&subsumption(var_nv("x"), class_nv("C"))).unwrap();
    assert_eq!(c.relation.schema, ["x"]);
    let CompiledRelationStatement::TransitiveClosure {
        reflexive,
        inverted,
        graph,
        ..
    } = &c.relation.statement
    else {
        panic!("expected a TransitiveClosure, got {:?}", c.relation.statement);
    };
    assert!(*reflexive);
    assert!(*inverted, "parent-bound mirrors to the inverse sys:inherits edge");
    assert_eq!(*graph, ProofGraph::Schema);
}

#[test]
fn subsumption_both_bound_compiles_to_membership_gate() {
    // `A << B` — forward closure from A, gated to the constant B via a ColumnConstant equality.
    let c = plan_bgp(&subsumption(class_nv("A"), class_nv("B"))).unwrap();
    let CompiledRelationStatement::Equality { input, .. } = &c.relation.statement else {
        panic!("expected an Equality (membership gate), got {:?}", c.relation.statement);
    };
    let CompiledRelationStatement::TransitiveClosure { graph, .. } = &input.statement else {
        panic!("expected the gate to wrap a TransitiveClosure");
    };
    assert_eq!(*graph, ProofGraph::Schema);
}

#[test]
fn subsumption_both_free_rejects_closed() {
    assert_eq!(
        plan_bgp(&subsumption(var_nv("x"), var_nv("y"))).unwrap_err(),
        BgpPlanError::SubsumptionBothEndpointsFree,
    );
}

#[test]
fn subsumption_count_gated_rejects_closed() {
    // A `Count`-gated subsumption closure is a deferred follow-up (mirrors the path closure).
    let q = Query::Count(Count {
        query: Box::new(subsumption(class_nv("A"), var_nv("y"))),
        count: dvar("n"),
    });
    assert_eq!(
        plan_bgp(&q).unwrap_err(),
        BgpPlanError::ClosureCountGateDeferred,
    );
}

#[test]
fn graph_selector_is_digest_bound_instance_closure_stays_byte_identical() {
    // Isolate the graph binding: an INSTANCE path closure and a SCHEMA subsumption closure that agree
    // on edge (sys:inherits), seed (A), reflexive (star), and direction (forward) — differing ONLY in
    // the graph selector — must have DISTINCT query digests. This proves (a) the selector is bound and
    // (b) the instance closure's digest is unchanged by the additive `Schema` fold (byte-identity).
    let schema_closure = plan_bgp(&subsumption(class_nv("A"), var_nv("y"))).unwrap();

    // `Distinct([y], Path(A, star(<sys:inherits>), ?y))` — the SAME closure, but instance-graph.
    let instance_path = Query::Distinct(Distinct {
        variables: vec!["y".into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Node(iri("A")),
            pattern: PathPattern::Star(PathStar {
                star: Box::new(PathPattern::Predicate(PathPredicate {
                    predicate: Some(SYS_INHERITS_IRI.to_string()),
                })),
            }),
            object: Value::Variable("y".into()),
            path: None,
        })),
    });
    let instance_closure = plan_bgp(&instance_path).unwrap();

    // Same seed/edge/star/direction — but the instance closure reads graph=Instance.
    let CompiledRelationStatement::TransitiveClosure { graph, .. } = &instance_closure.relation.statement
    else {
        panic!("expected the path to compile to a TransitiveClosure");
    };
    assert_eq!(*graph, ProofGraph::Instance);

    let schema_digest = compiled_query_digest(&schema_closure).unwrap();
    let instance_digest = compiled_query_digest(&instance_closure).unwrap();
    assert_ne!(
        schema_digest, instance_digest,
        "the graph selector must change the query digest (anti-substitution binding)"
    );
}

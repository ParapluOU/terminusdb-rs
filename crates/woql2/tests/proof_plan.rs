//! CP2 — `plan_bgp` shape guard: accept the safe fragment, reject everything else
//! closed. No proving here (see `prove.rs` for the end-to-end faithfulness tests).

mod proof_common;
use proof_common::*;

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::control::{Distinct, From, Immediately, Pin, Select, Using};
use terminusdb_woql2::misc::{Comment, Count, InRange};
use terminusdb_woql2::path::{PathPattern, PathPredicate, PathStar};
use terminusdb_woql2::query::{Not, Or, Path, Query, True};
use terminusdb_woql2::triple::{Data, Link, TripleSlice};
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

use terminus_store::proof::query::{AtomTerm, ObjectRangeSpec};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::ValueTriple;

use tdb_succinct::{TdbDataType, TypedDictEntry};

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use decimal_rs::Decimal;
use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminusdb_woql2::proof::{plan_bgp, BgpPlanError, CompiledBgp};

/// Assert `plan_bgp` rejects `q` with exactly `want`. (`CompiledBgp` is not
/// `PartialEq` — the plan types are foreign — so compare only the error.)
#[track_caller]
fn reject(q: Query, want: BgpPlanError) {
    assert_eq!(plan_bgp(&q).unwrap_err(), want);
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
        .plan
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

    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.plan.atoms.len(), 3);
    assert_eq!(c.plan.joins.len(), 2);
    // Left-deep on the shared middle variable each step: matches the store's own
    // `layer_bgp_three_atom_path_end_to_end` plan (JoinStep{1,0}, {2,0}).
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (1, 0));
    assert_eq!((c.plan.joins[1].acc_key, c.plan.joins[1].rel_key), (2, 0));
    // Predicate terms are the IRI bytes, in placement order.
    assert_eq!(c.plan.atoms[0].predicate_term, iri("p").into_bytes());
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

    assert_eq!(c.schema, vec!["a", "b", "c", "d"]);
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (0, 0));
    assert_eq!((c.plan.joins[1].acc_key, c.plan.joins[1].rel_key), (1, 0));
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

    assert_eq!(c.schema, vec!["a", "b", "x", "c"]);
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (1, 0));
    assert_eq!((c.plan.joins[1].acc_key, c.plan.joins[1].rel_key), (1, 0));
}

#[test]
fn flattens_nested_ands() {
    // And[ And[t1, t2], t3 ] flattens to the same 3-atom path.
    let inner = and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")]);
    let q = and_of(vec![inner, tv("z", "p", "w")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.plan.atoms.len(), 3);
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
    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
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
    assert_eq!(c.schema, vec!["x", "y", "z"]);
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
fn accepts_link_and_data_as_typed_triple_patterns() {
    let q = and_of(vec![
        Query::Link(Link {
            subject: NodeValue::Variable("x".into()),
            predicate: NodeValue::Node(iri("knows")),
            object: NodeValue::Variable("y".into()),
            graph: GraphType::Instance,
        }),
        Query::Data(Data {
            subject: NodeValue::Variable("x".into()),
            predicate: NodeValue::Node(iri("active")),
            object: DataValue::Variable("active".into()),
            graph: GraphType::Instance,
        }),
    ]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["y", "x", "active"]);
    assert_eq!(c.plan.atoms.len(), 2);
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
    assert_eq!(c.schema, vec!["x", "y"]);
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
    assert_eq!(c.schema, vec!["x", "y", "z"]);
    assert_eq!(c.plan.atoms.len(), 2);
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
    assert_eq!(c.schema, vec!["x", "y"]);
    assert_eq!(c.plan.atoms.len(), 1);
}

#[test]
fn count_retains_the_output_binding_and_hides_inner_columns() {
    let q = Query::Count(Count {
        query: Box::new(and_of(vec![tv("x", "p", "y")])),
        count: dvar("count"),
    });
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y"]);
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
    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.plan.atoms.len(), 3);
    assert_eq!((c.plan.joins[1].acc_key, c.plan.joins[1].rel_key), (2, 0));
}

// ----------------------------------------------------------------------------
// REJECT-CLOSED — each returns the right BgpPlanError.
// ----------------------------------------------------------------------------

#[test]
fn rejects_or() {
    let q = Query::Or(Or {
        or: vec![tv("x", "p", "y"), tv("x", "q", "y")],
    });
    reject(q, BgpPlanError::UnsupportedConstruct { construct: "Or" });
}

#[test]
fn rejects_not() {
    let q = Query::Not(Not {
        query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
    });
    reject(q, BgpPlanError::UnsupportedConstruct { construct: "Not" });
}

#[test]
fn rejects_path_star() {
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
    reject(q, BgpPlanError::UnsupportedConstruct { construct: "Path" });
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
    assert_eq!(compiled.schema, vec!["x", "y", "z"]);
}

#[test]
fn rejects_distinct_combined_with_projection_until_dedup_is_proven() {
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into(), "y".into(), "z".into()],
        query: Box::new(Query::Select(Select {
            variables: vec!["x".into(), "y".into(), "z".into()],
            query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
        })),
    });
    reject(q, BgpPlanError::DistinctWithProjectionOrCount);
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
fn rejects_predicate_variable() {
    let q = and_of(vec![tv_pred_var("x", "pv", "y"), tv("y", "p", "z")]);
    reject(q, BgpPlanError::PredicateNotConstant);
}

#[test]
fn accepts_subject_node_constant() {
    // t(<s>, p, ?y), t(?y, q, ?z): the subject-const atom keeps only O = y (arity 1),
    // then joins on y with the all-variable atom's subject.
    let q = and_of(vec![tv_subj_const("s", "p", "y"), tv("y", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["y", "z"]);
    assert!(matches!(c.plan.atoms[0].subject, AtomTerm::NodeConstant(_)));
    assert!(matches!(c.plan.atoms[0].object, AtomTerm::Variable));
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (0, 0));
}

#[test]
fn accepts_object_node_constant() {
    // t(?x, p, <o>), t(?x, q, ?z): the object-const atom keeps only S = x (arity 1),
    // then joins on x with the all-variable atom's subject.
    let q = and_of(vec![tv_obj_node("x", "p", "o"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(c.plan.atoms[0].object, AtomTerm::NodeConstant(_)));
    assert!(matches!(c.plan.atoms[0].subject, AtomTerm::Variable));
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (0, 0));
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
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(
        c.plan.atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
    assert_value_constant(&c, "p", i64::make_entry(&42i64));
}

#[test]
fn accepts_object_boolean_literal() {
    // xsd:boolean object literal -> store Boolean value constant.
    let q = and_of(vec![tv_obj_bool("x", "p", true), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", bool::make_entry(&true));
}

#[test]
fn accepts_object_float_literal() {
    // xsd:float/double object literal -> store Float64 value constant.
    let q = and_of(vec![tv_obj_float("x", "p", 3.5), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", f64::make_entry(&3.5f64));
}

#[test]
fn accepts_object_unsigned_int_literal() {
    // xsd:unsignedInt object literal -> store UInt32 value constant.
    let q = and_of(vec![tv_obj_uint("x", "p", 7), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert_value_constant(&c, "p", u32::make_entry(&7u32));
}

#[test]
fn accepts_object_string_literal() {
    // t(?x, p, "hi"), t(?x, q, ?z): the string-literal atom keeps only S = x (arity 1).
    let q = and_of(vec![tv_obj_string("x", "p", "hi"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(
        c.plan.atoms[0].object,
        AtomTerm::ValueConstant(_, _)
    ));
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (0, 0));
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
fn rejects_fully_ground_atom() {
    // t(<s>, p, <o>): both positions are constants — no variables at all.
    let q = and_of(vec![tv_ground("s", "p", "o"), tv("y", "q", "z")]);
    reject(q, BgpPlanError::GroundAtom);
}

#[test]
fn accepts_repeated_variable_as_same_row_equality() {
    let q = and_of(vec![tv("x", "p", "x"), tv("x", "q", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y"]);
    assert!(c.plan.atoms[0].subject_object_equal);
    assert_eq!(c.plan.joins.len(), 1);
}

#[test]
fn accepts_composite_key() {
    // t(?x,p,?y), t(?x,q,?y) — the second atom shares BOTH x and y.
    let q = and_of(vec![tv("x", "p", "y"), tv("x", "q", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y"]);
    assert_eq!(c.plan.joins[0].additional_keys, vec![(1, 1)]);
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
    assert_eq!(c.schema, vec!["x", "y", "z"]);
    assert_eq!(c.plan.joins.len(), 2);
    assert_eq!(c.plan.joins[1].additional_keys, vec![(0, 1)]);
}

#[test]
fn rejects_disconnected() {
    // t(?x,p,?y), t(?a,q,?b) — two components, no shared variable.
    let q = and_of(vec![tv("x", "p", "y"), tv("a", "q", "b")]);
    reject(q, BgpPlanError::DisconnectedBgp);
}

#[test]
fn accepts_single_atom() {
    let q = and_of(vec![tv("x", "p", "y")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "y"]);
    assert_eq!(c.plan.atoms.len(), 1);
    assert!(c.plan.joins.is_empty());
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
    assert_eq!(c.schema, vec!["x", "y"]);
    assert_eq!(c.plan.atoms.len(), 1);
}

// ----------------------------------------------------------------------------
// Phase 7.4 — numeric object-value RANGE filters compiled from comparison conjuncts.
// A comparison `?a op k` attaches an ObjectRangeSpec to the atom binding ?a as its object.
// ----------------------------------------------------------------------------

/// The ObjectRangeSpec attached to the compiled atom whose predicate is `iri(pred)`.
fn range_of<'a>(c: &'a CompiledBgp, pred: &str) -> Option<&'a ObjectRangeSpec> {
    let term = iri(pred).into_bytes();
    c.plan
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
    assert!(c.schema.contains(&"a".to_string()));
    // schema is [a, x, n] (left-deep join of age then name on the shared x).
    assert_eq!(c.schema, vec!["a", "x", "n"]);
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

#[test]
fn triple_slice_rejects_until_result_order_is_authenticated() {
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
        BgpPlanError::UnsupportedConstruct {
            construct: "TripleSlice",
        },
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

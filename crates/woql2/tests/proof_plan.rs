//! CP2 — `plan_bgp` shape guard: accept the safe fragment, reject everything else
//! closed. No proving here (see `prove.rs` for the end-to-end faithfulness tests).

mod proof_common;
use proof_common::*;

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::control::{
    Distinct, From, Immediately, Once, Pin, Select, Using, When, WoqlOptional,
};
use terminusdb_woql2::misc::{Comment, Count, InRange, Limit, Start};
use terminusdb_woql2::order::{Order, OrderBy, OrderTemplate};
use terminusdb_woql2::path::{PathPattern, PathPredicate, PathStar};
use terminusdb_woql2::query::{Not, Or, Path, Query, True};
use terminusdb_woql2::triple::{Data, Link, TripleSlice, TripleSliceRev};
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

use terminus_store::proof::envelope::RelationEqualityPlan;
use terminus_store::proof::query::{AtomTerm, ObjectKindFilter, ObjectRangeSpec};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::ValueTriple;

use tdb_succinct::{TdbDataType, TypedDictEntry};

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use decimal_rs::Decimal;
use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use terminusdb_woql2::proof::{compiled_query_digest, plan_bgp, BgpPlanError, CompiledBgp};

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
    assert_eq!(compiled.relation.schema, vec!["x", "y", "z"]);
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
fn triple_slice_rev_rejects_until_result_order_is_authenticated() {
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
        BgpPlanError::UnsupportedConstruct {
            construct: "TripleSliceRev",
        },
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

    let two_columns = and_of(vec![tv("subject", "score", "score"), gte(dvar("score"), duint(0))]);
    reject(
        Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "score".into(),
                order: Order::Asc,
            }],
            query: Box::new(two_columns.clone()),
        }),
        BgpPlanError::OrderByUnsupportedShape,
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
        BgpPlanError::OrderByUnsupportedShape,
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
    for ordering in [
        vec![OrderTemplate { variable: "x".into(), order: Order::Asc }],
        vec![OrderTemplate { variable: "x".into(), order: Order::Asc }, OrderTemplate { variable: "x".into(), order: Order::Desc }],
    ] { assert_eq!(plan_bgp(&query(ordering)).unwrap_err(), BgpPlanError::OrderByUnsupportedShape); }

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

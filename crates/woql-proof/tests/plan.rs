//! CP2 — `plan_bgp` shape guard: accept the safe fragment, reject everything else
//! closed. No proving here (see `prove.rs` for the end-to-end faithfulness tests).

mod common;
use common::*;

use terminusdb_woql2::control::{Distinct, Select};
use terminusdb_woql2::path::{PathPattern, PathPredicate, PathStar};
use terminusdb_woql2::query::{Not, Or, Path, Query};
use terminusdb_woql2::value::Value;

use terminus_store::proof::query::{AtomTerm, ObjectRangeSpec};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::CanonicalDatatype;

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;

use woql_proof::{plan_bgp, BgpPlanError};

/// Assert `plan_bgp` rejects `q` with exactly `want`. (`CompiledBgp` is not
/// `PartialEq` — the plan types are foreign — so compare only the error.)
#[track_caller]
fn reject(q: Query, want: BgpPlanError) {
    assert_eq!(plan_bgp(&q).unwrap_err(), want);
}

// ----------------------------------------------------------------------------
// ACCEPT + correct plan structure (variable-blind).
// ----------------------------------------------------------------------------

#[test]
fn accepts_three_atom_path_with_correct_plan() {
    // t(?x,p,?y), t(?y,p,?z), t(?z,p,?w)
    let q = and_of(vec![tv("x", "p", "y"), tv("y", "p", "z"), tv("z", "p", "w")]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.schema, vec!["x", "y", "z", "w"]);
    assert_eq!(c.plan.atoms.len(), 3);
    assert_eq!(c.plan.joins.len(), 2);
    // Left-deep on the shared middle variable each step: matches the store's own
    // `layer_bgp_three_atom_path_end_to_end` plan (JoinStep{1,0}, {2,0}).
    assert_eq!(
        (c.plan.joins[0].acc_key, c.plan.joins[0].rel_key),
        (1, 0)
    );
    assert_eq!(
        (c.plan.joins[1].acc_key, c.plan.joins[1].rel_key),
        (2, 0)
    );
    // Predicate terms are the IRI bytes, in placement order.
    assert_eq!(c.plan.atoms[0].predicate_term, iri("p").into_bytes());
    assert!(c.projection.is_none());
}

#[test]
fn accepts_star_shared_subject_three_atoms() {
    // t(?c,p,?a), t(?c,q,?b), t(?c,r,?d) — c is the shared key in all three atoms.
    let q = and_of(vec![tv("c", "p", "a"), tv("c", "q", "b"), tv("c", "r", "d")]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.schema, vec!["a", "b", "c", "d"]);
    assert_eq!(
        (c.plan.joins[0].acc_key, c.plan.joins[0].rel_key),
        (0, 0)
    );
    assert_eq!(
        (c.plan.joins[1].acc_key, c.plan.joins[1].rel_key),
        (1, 0)
    );
}

#[test]
fn accepts_variable_shared_by_three_atoms_mixed_positions() {
    // t(?a,p,?x), t(?x,q,?b), t(?x,r,?c) — x appears in 3 atoms (object then subject).
    let q = and_of(vec![tv("a", "p", "x"), tv("x", "q", "b"), tv("x", "r", "c")]);
    let c = plan_bgp(&q).unwrap();

    assert_eq!(c.schema, vec!["a", "b", "x", "c"]);
    assert_eq!(
        (c.plan.joins[0].acc_key, c.plan.joins[0].rel_key),
        (1, 0)
    );
    assert_eq!(
        (c.plan.joins[1].acc_key, c.plan.joins[1].rel_key),
        (1, 0)
    );
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
    let bgp = and_of(vec![tv("x", "p", "y"), tv("y", "p", "z"), tv("z", "p", "w")]);
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
    assert_eq!(
        (c.plan.joins[1].acc_key, c.plan.joins[1].rel_key),
        (2, 0)
    );
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
fn rejects_distinct() {
    let q = Query::Distinct(Distinct {
        variables: vec!["x".into()],
        query: Box::new(and_of(vec![tv("x", "p", "y"), tv("y", "p", "z")])),
    });
    reject(q, BgpPlanError::UnsupportedConstruct { construct: "Distinct" });
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
fn rejects_object_data_unaligned_datatype() {
    // xsd:boolean object literal: not a round-trip-aligned datatype (CP3 aligns only string).
    let q = and_of(vec![tv_obj_data("x", "p"), tv("x", "q", "z")]);
    reject(
        q,
        BgpPlanError::UnsupportedValueDatatype { datatype: "boolean" },
    );
}

#[test]
fn rejects_object_integer_literal_unaligned_datatype() {
    // xsd:integer object literal: only xsd:string is round-trip-aligned in CP3.
    let q = and_of(vec![tv_obj_integer("x", "p", 42), tv("x", "q", "z")]);
    reject(
        q,
        BgpPlanError::UnsupportedValueDatatype { datatype: "integer" },
    );
}

#[test]
fn accepts_object_string_literal() {
    // t(?x, p, "hi"), t(?x, q, ?z): the string-literal atom keeps only S = x (arity 1).
    let q = and_of(vec![tv_obj_string("x", "p", "hi"), tv("x", "q", "z")]);
    let c = plan_bgp(&q).unwrap();
    assert_eq!(c.schema, vec!["x", "z"]);
    assert!(matches!(c.plan.atoms[0].object, AtomTerm::ValueConstant(_, _)));
    assert_eq!((c.plan.joins[0].acc_key, c.plan.joins[0].rel_key), (0, 0));
}

#[test]
fn rejects_fully_ground_atom() {
    // t(<s>, p, <o>): both positions are constants — no variables at all.
    let q = and_of(vec![tv_ground("s", "p", "o"), tv("y", "q", "z")]);
    reject(q, BgpPlanError::GroundAtom);
}

#[test]
fn rejects_repeated_variable_within_atom() {
    // t(?x, p, ?x) — a self-loop the single-key relation cannot express.
    let q = and_of(vec![tv("x", "p", "x"), tv("x", "q", "y")]);
    reject(q, BgpPlanError::RepeatedVariableWithinAtom { var: "x".into() });
}

#[test]
fn rejects_composite_key() {
    // t(?x,p,?y), t(?x,q,?y) — the second atom shares BOTH x and y.
    let q = and_of(vec![tv("x", "p", "y"), tv("x", "q", "y")]);
    reject(q, BgpPlanError::CompositeOrCyclicKey);
}

#[test]
fn rejects_triangle_cycle() {
    // t(?x,p,?y), t(?y,q,?z), t(?z,r,?x) — a 3-cycle.
    let q = and_of(vec![
        tv("x", "p", "y"),
        tv("y", "q", "z"),
        tv("z", "r", "x"),
    ]);
    reject(q, BgpPlanError::CompositeOrCyclicKey);
}

#[test]
fn rejects_disconnected() {
    // t(?x,p,?y), t(?a,q,?b) — two components, no shared variable.
    let q = and_of(vec![tv("x", "p", "y"), tv("a", "q", "b")]);
    reject(q, BgpPlanError::DisconnectedBgp);
}

#[test]
fn rejects_single_atom() {
    let q = and_of(vec![tv("x", "p", "y")]);
    reject(q, BgpPlanError::TooFewAtoms { count: 1 });
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
    reject(q, BgpPlanError::SelectedVariableNotInBgp { var: "nope".into() });
}

#[test]
fn rejects_bare_triple_body() {
    // A single Triple (not wrapped in And) is not a conjunction.
    let q = tv("x", "p", "y");
    reject(q, BgpPlanError::UnsupportedConstruct { construct: "Triple" });
}

// ----------------------------------------------------------------------------
// Phase 7.4 — numeric object-value RANGE filters compiled from comparison conjuncts.
// A comparison `?a op k` attaches an ObjectRangeSpec to the atom binding ?a as its object.
// ----------------------------------------------------------------------------

/// The ObjectRangeSpec attached to the compiled atom whose predicate is `iri(pred)`.
fn range_of<'a>(c: &'a woql_proof::CompiledBgp, pred: &str) -> Option<&'a ObjectRangeSpec> {
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

/// Compile `t(?x,age,?a), t(?x,name,?n)` plus one comparison conjunct.
fn age_name_range(cmp: Query) -> woql_proof::CompiledBgp {
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
    reject_range(greater(duint(1), duint(2)), BgpPlanError::RangeBothConstants);
}

#[test]
fn rejects_range_string_datatype() {
    reject_range(
        greater(dvar("a"), dstr("hi")),
        BgpPlanError::UnsupportedRangeDatatype { datatype: "string" },
    );
}

#[test]
fn rejects_range_integer_datatype() {
    // xsd:integer is not the round-trip-aligned unsignedInt; reject closed.
    reject_range(
        greater(dvar("a"), dint(5)),
        BgpPlanError::UnsupportedRangeDatatype { datatype: "integer" },
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
fn rejects_two_ranges_on_one_variable() {
    let q = and_of(vec![
        tv("x", "age", "a"),
        tv("x", "name", "n"),
        greater(dvar("a"), duint(5)),
        less(dvar("a"), duint(20)),
    ]);
    reject(q, BgpPlanError::DuplicateRangeOnVariable { var: "a".into() });
}

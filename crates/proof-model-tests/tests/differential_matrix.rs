//! Phase 2 differential matrix: the curated per-construct coverage matrix + the
//! adversarial datatype fidelity cases, all driven through the SAME oracle harness
//! proven in phase 1 (`tests/harness/mod.rs`). Every case either:
//!   * `assert_fidelity` — the SUPPORTED construct MUST return Prolog == Rust; or
//!   * `assert_compiler_rejects` — the OUT-OF-FRAGMENT construct MUST fail closed.
//!
//! DISCIPLINE (per the coordinator): a supported case that reveals Prolog != Rust is
//! a REAL FIDELITY BUG — the assertion stays correct and the divergence is reported,
//! never papered over. A reject case that is actually accepted (or vice versa) is a
//! reject-boundary surprise to report. `adv_mixed_int_decimal_range_boundary` is an
//! INTENTIONAL fail-closed coverage boundary (sound under-coverage), not a bug — it uses
//! `assert_prove_rejects` (a PROVE-time reject, which the variable-blind compiler does
//! not catch), and records Prolog's answer for the under-coverage log.
//!
//! Run (release-gated; each live case boots a real server child — keep threads low):
//!   RUST_MIN_STACK=134217728 \
//!   TERMINUSDB_SOURCE=/path/to/terminusdb-fork-checkout \
//!   cargo +nightly-2025-09-19 test -p terminusdb-proof-model-tests --release \
//!     --test differential_matrix -- --nocapture --test-threads=1

mod harness;

use harness::{assert_compiler_rejects, assert_fidelity, iri, run_differential, triple, Obj};

use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::compare::{Equals, Greater, Gte, Less, Lte, TypeOf};
use terminusdb_woql2::control::{Distinct, Select};
use terminusdb_woql2::misc::{Collect, Count, RangeMax, RangeMin};
use terminusdb_woql2::order::{GroupBy, Order, OrderBy, OrderTemplate};
use terminusdb_woql2::path::{PathPattern, PathPlus, PathPredicate, PathStar};
use terminusdb_woql2::query::{And, Path, Query};
use terminusdb_woql2::string::{Lower, Upper};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

// ---------------------------------------------------------------------------
// Query builders.
// ---------------------------------------------------------------------------

/// `?subj <pred> ?obj`.
fn tvv(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}
/// `<subj-IRI> <pred> ?obj` (keeps only `?obj`).
fn tcv(subj_iri: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subj_iri)),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.into()),
        graph: None,
    })
}
/// `?subj <pred> <obj-IRI>` (keeps only `?subj`).
fn tvc(subj: &str, pred: &str, obj_iri: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Node(iri(obj_iri)),
        graph: None,
    })
}
/// `?subj ?predvar <obj-IRI>` (keeps `?subj`, `?predvar`) — predicate-position variable.
fn tv_predvar_c(subj: &str, predvar: &str, obj_iri: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.into()),
        predicate: NodeValue::Variable(predvar.into()),
        object: Value::Node(iri(obj_iri)),
        graph: None,
    })
}
fn and_of(qs: Vec<Query>) -> Query {
    Query::And(And { and: qs })
}
fn dvar(n: &str) -> DataValue {
    DataValue::Variable(n.into())
}
fn duint(k: usize) -> DataValue {
    DataValue::Data(XSDAnySimpleType::UnsignedInt(k))
}
fn dint(v: i32) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Int(v))
}
fn dlong(v: i64) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Long(v))
}
fn ddec(s: &str) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Decimal(s.parse().expect("valid decimal")))
}
fn dfloat(f: f64) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Float(f))
}
fn greater(l: DataValue, r: DataValue) -> Query {
    Query::Greater(Greater { left: l, right: r })
}
fn less(l: DataValue, r: DataValue) -> Query {
    Query::Less(Less { left: l, right: r })
}
fn gte(l: DataValue, r: DataValue) -> Query {
    Query::Gte(Gte { left: l, right: r })
}
fn lte(l: DataValue, r: DataValue) -> Query {
    Query::Lte(Lte { left: l, right: r })
}
fn eq_val(l: Value, r: Value) -> Query {
    Query::Equals(Equals { left: l, right: r })
}

// ===========================================================================
// 1. BGP shapes.
// ===========================================================================

#[test]
fn bgp_bound_subject() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("a"), iri("p"), Obj::Node(iri("y"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("z"))),
    ];
    // `<a> <p> ?o` — constant subject, keeps only `?o`.
    let q = tcv("a", "p", "o");
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn bgp_bound_object() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("c"), iri("p"), Obj::Node(iri("y"))),
    ];
    // `?s <p> <x>` — constant object, keeps only `?s` -> {a, b}.
    let q = tvc("s", "p", "x");
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn bgp_variable_predicate() {
    let seed = vec![
        triple(iri("a"), iri("r1"), Obj::Node(iri("x"))),
        triple(iri("b"), iri("r2"), Obj::Node(iri("x"))),
        triple(iri("c"), iri("r3"), Obj::Node(iri("y"))),
    ];
    // `?s ?p <x>` — predicate-position variable + constant object -> {(a,r1),(b,r2)}.
    let q = tv_predvar_c("s", "p", "x");
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn bgp_three_atom_join_chain() {
    let seed = vec![
        triple(iri("a"), iri("p1"), Obj::Node(iri("b"))),
        triple(iri("b"), iri("p2"), Obj::Node(iri("c"))),
        triple(iri("c"), iri("p3"), Obj::Node(iri("d"))),
        // a decoy edge sharing p2 from an unrelated node (no chain)
        triple(iri("x"), iri("p2"), Obj::Node(iri("y"))),
    ];
    // `?a p1 ?b & ?b p2 ?c & ?c p3 ?d` -> exactly the one chain row.
    let q = and_of(vec![
        tvv("a", "p1", "b"),
        tvv("b", "p2", "c"),
        tvv("c", "p3", "d"),
    ]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn bgp_disconnected_cartesian_product() {
    let seed = vec![
        triple(iri("s1"), iri("p"), Obj::Node(iri("o1"))),
        triple(iri("s2"), iri("p"), Obj::Node(iri("o2"))),
        triple(iri("x1"), iri("q"), Obj::Node(iri("y1"))),
        triple(iri("x2"), iri("q"), Obj::Node(iri("y2"))),
    ];
    // Two disconnected atoms (no shared variable) -> verifier-known unit-key
    // Cartesian product: 2 x 2 = 4 rows over schema [s,o,x,y].
    let q = and_of(vec![tvv("s", "p", "o"), tvv("x", "q", "y")]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

// ===========================================================================
// 2. Filters: equality + numeric range over the integer datatypes.
// ===========================================================================

#[test]
fn filter_equality_on_subject_node() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("a"), iri("p"), Obj::Node(iri("y"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("z"))),
    ];
    // `?s <p> ?o & Equals(?s, <a>)` -> {(a,x),(a,y)}.
    let q = and_of(vec![
        tvv("s", "p", "o"),
        eq_val(Value::Variable("s".into()), Value::Node(iri("a"))),
    ]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_gt_unsigned() {
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), greater(dvar("v"), duint(30))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_lt_unsigned() {
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), less(dvar("v"), duint(30))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_gte_unsigned() {
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), gte(dvar("v"), duint(40))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_lte_unsigned() {
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), lte(dvar("v"), duint(20))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_over_signed_int() {
    // xsd:int (Int32) column with negatives; range crosses zero.
    let seed = vec![
        triple(iri("a"), iri("temp"), Obj::Int(-10)),
        triple(iri("b"), iri("temp"), Obj::Int(0)),
        triple(iri("c"), iri("temp"), Obj::Int(25)),
    ];
    // `?s temp ?v & Greater(?v, -5)` -> {b(0), c(25)}.
    let q = and_of(vec![tvv("s", "temp", "v"), greater(dvar("v"), dint(-5))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn filter_range_over_long() {
    let seed = vec![
        triple(iri("a"), iri("big"), Obj::Long(-1_000_000_000_000)),
        triple(iri("b"), iri("big"), Obj::Long(0)),
        triple(iri("c"), iri("big"), Obj::Long(5_000_000_000)),
    ];
    let q = and_of(vec![tvv("s", "big", "v"), gte(dvar("v"), dlong(0))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

// ===========================================================================
// 3. OrderBy: single-key asc + desc (positional); subset-key rejects.
// ===========================================================================

#[test]
fn order_by_asc() {
    let seed = list_items_uint(&[3, 1, 2]);
    let q = order_single("v", Order::Asc, list_v_ge0());
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // Ordered: proven order == Prolog sort order == [1,2,3].
}

#[test]
fn order_by_desc() {
    let seed = list_items_uint(&[3, 1, 2]);
    let q = order_single("v", Order::Desc, list_v_ge0());
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // [3,2,1].
}

#[test]
fn order_by_with_duplicate_keys_is_positional() {
    // Ties on the (single, full) key are IDENTICAL rows, so positional order is
    // unambiguous — the full-key requirement structurally avoids the F4 tie-break
    // attestation gap (a subset-key order rejects; see below).
    let seed = list_items_uint(&[3, 1, 1, 2, 3]);
    let q = order_single("v", Order::Asc, list_v_ge0());
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // [1,1,2,3,3].
}

#[test]
fn order_by_subset_key_rejects() {
    // A 2-int-column relation (constant subject with two integer properties) whose
    // OrderBy names only ONE key -> an un-attestable invented tie order over the
    // unnamed column -> reject closed (F4-max: MAXIMUM ORDER ATTESTATION).
    let child = and_of(vec![
        tcv("c", "a", "x"),
        tcv("c", "b", "y"),
        gte(dvar("x"), duint(0)),
        gte(dvar("y"), duint(0)),
    ]);
    let q = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "x".into(),
            order: Order::Asc,
        }],
        query: Box::new(child),
    });
    assert_compiler_rejects(&q);
}

// ===========================================================================
// 4. Aggregates: Count, GroupBy+Count, GroupSum, Group Min/Max, Collection Sum.
//
// REVIEW: these GroupBy/Collect + RangeMin/Sum/Length idioms are standard WOQL, but
// this is the first time the differential harness exercises the server's aggregate
// evaluation against our GroupExtrema/GroupSum/GroupCount recompute. A divergence
// here is a genuine finding (fidelity bug) OR a harness/aggregate-shape mismatch to
// triage — reported either way.
// ===========================================================================

#[test]
fn agg_count() {
    let seed = vec![
        triple(iri("a"), iri("type"), Obj::Node(iri("Class"))),
        triple(iri("b"), iri("type"), Obj::Node(iri("Class"))),
        triple(iri("c"), iri("type"), Obj::Node(iri("Class"))),
        triple(iri("d"), iri("type"), Obj::Node(iri("Other"))),
    ];
    let q = Query::Count(Count {
        query: Box::new(tvc("s", "type", "Class")),
        count: dvar("n"),
    });
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // 3.
}

#[test]
fn agg_group_count() {
    let seed = group_age_seed();
    // GroupBy Person; count Ages per Person -> {(p1,3),(p2,2)}.
    let q = and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("Age".into()),
            group_by: vec!["Person".into()],
            grouped_value: Value::Variable("Ages".into()),
            query: Box::new(group_age_child()),
        }),
        Query::Length(Length {
            list: dvar("Ages"),
            length: dvar("N"),
        }),
    ]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn agg_group_sum() {
    let seed = group_age_seed();
    // GroupBy Person; sum Ages per Person -> {(p1,60),(p2,20)}.
    let q = and_of(vec![
        Query::GroupBy(GroupBy {
            template: Value::Variable("Age".into()),
            group_by: vec!["Person".into()],
            grouped_value: Value::Variable("Ages".into()),
            query: Box::new(group_age_child()),
        }),
        Query::Sum(Sum {
            list: dvar("Ages"),
            result: dvar("Total"),
        }),
    ]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn agg_group_min() {
    let seed = group_age_seed();
    let q = group_extrema(false); // RangeMin -> {(p1,10),(p2,5)}.
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn agg_group_max() {
    let seed = group_age_seed();
    let q = group_extrema(true); // RangeMax -> {(p1,30),(p2,15)}.
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn agg_collection_sum() {
    let seed = group_age_seed();
    // Whole-collection sum of ALL ages -> {80}.
    let q = and_of(vec![
        Query::Collect(Collect {
            template: Value::Variable("Age".into()),
            into: Value::Variable("Ages".into()),
            query: Box::new(group_age_child()),
        }),
        Query::Sum(Sum {
            list: dvar("Ages"),
            result: dvar("Total"),
        }),
    ]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

// ===========================================================================
// 5. Distinct.
// ===========================================================================

#[test]
fn distinct_select_subject() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("a"), iri("p"), Obj::Node(iri("y"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("z"))),
    ];
    // `SELECT DISTINCT ?s WHERE ?s <p> ?o` -> {a, b} (a appears twice).
    let q = Query::Select(Select {
        variables: vec!["s".into()],
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["s".into()],
            query: Box::new(tvv("s", "p", "o")),
        })),
    });
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // Set semantics.
}

// ===========================================================================
// 6. Paths.
// ===========================================================================

#[test]
fn path_all_pairs_star_rejects() {
    // `path(?x, star(p), ?y)` — BOTH endpoints free over an unbounded reflexive
    // closure: the reachable-pairs domain is deferred (ClosureStarBothFreeDomainDeferred)
    // -> reject closed.
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
    assert_compiler_rejects(&q);
}

#[test]
fn path_reachable_set_from_bound_source() {
    // REVIEW (higher-risk path fidelity): a one-endpoint-bound `plus` closure delivers
    // the reachable SET from a fixed source; the free endpoint MUST be Distinct-gated.
    // Seed an acyclic chain a->b->c->d so edge-simple >=1-hop reachability == transitive
    // reachability (faithful to path.pl). A failure here needs triage: real fidelity bug
    // vs closure-semantics / harness mismatch.
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("b"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("c"))),
        triple(iri("c"), iri("p"), Obj::Node(iri("d"))),
    ];
    let q = Query::Distinct(Distinct {
        variables: vec!["y".into()],
        query: Box::new(Query::Path(Path {
            subject: Value::Node(iri("a")),
            pattern: PathPattern::Plus(PathPlus {
                plus: Box::new(PathPattern::Predicate(PathPredicate {
                    predicate: Some(iri("p")),
                })),
            }),
            object: Value::Variable("y".into()),
            path: None,
        })),
    });
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // {b, c, d}, set semantics.
}

// ===========================================================================
// 7. Out-of-fragment rejects (fail-closed boundary).
// ===========================================================================

#[test]
fn reject_type_of() {
    let q = Query::TypeOf(TypeOf {
        value: Value::Variable("x".into()),
        type_uri: NodeValue::Variable("t".into()),
    });
    assert_compiler_rejects(&q);
}

#[test]
fn reject_string_upper() {
    // `upper/2` string op is not a store-triple read circuit -> reject closed.
    let q = and_of(vec![
        tvv("s", "p", "o"),
        Query::Upper(Upper {
            mixed: dvar("o"),
            upper: dvar("u"),
        }),
    ]);
    assert_compiler_rejects(&q);
}

#[test]
fn reject_string_lower() {
    let q = and_of(vec![
        tvv("s", "p", "o"),
        Query::Lower(Lower {
            mixed: dvar("o"),
            lower: dvar("l"),
        }),
    ]);
    assert_compiler_rejects(&q);
}

// ===========================================================================
// 8. Adversarial datatype fidelity cases (highest value).
// ===========================================================================

#[test]
fn adv_decimal_resolution_renders_faithfully() {
    // Resolve an xsd:decimal COLUMN and check our BCD-lexical rendering matches
    // Prolog's, across terminating / whole / negative / long-fraction vectors
    // (mirrors the shapes captured in decimal_layer_render_tests).
    let seed = vec![
        triple(iri("s1"), iri("price"), Obj::Decimal("3.5".into())),
        triple(iri("s2"), iri("price"), Obj::Decimal("0.25".into())),
        triple(iri("s3"), iri("price"), Obj::Decimal("-0.000001".into())),
        triple(iri("s4"), iri("price"), Obj::Decimal("123.456".into())),
        triple(iri("s5"), iri("price"), Obj::Decimal("7".into())), // whole -> Int
    ];
    let q = tvv("s", "price", "v");
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn adv_decimal_range_rejects() {
    // A range over a non-order-preserving variable-width datatype (xsd:decimal)
    // reject-closes (would compare a 0 sentinel in the store).
    let seed = vec![
        triple(iri("s1"), iri("price"), Obj::Decimal("3.5".into())),
        triple(iri("s2"), iri("price"), Obj::Decimal("10.25".into())),
    ];
    let _ = &seed; // seed is irrelevant: rejection is at compile time.
    let q = and_of(vec![tvv("s", "price", "v"), greater(dvar("v"), ddec("5.0"))]);
    assert_compiler_rejects(&q);
}

#[test]
fn adv_mixed_int_decimal_range_boundary() {
    // INTENTIONAL FAIL-CLOSED BOUNDARY (sound UNDER-coverage), NOT a fidelity bug.
    //
    // The column mixes `xsd:long` (Int64) and `xsd:decimal` values. WOQL compares
    // numerically BY VALUE across datatypes, so Prolog returns BOTH s1 (5) and s2 (7.5)
    // for `> 4`. Our variable-blind compiler ACCEPTS the plan (it cannot see the data
    // heterogeneity), so this is NOT a compile-time reject; instead the PROVER fires the
    // D-fidelity strict-homogeneity guard at prove time (`RangeDatatypeNotHomogeneous`):
    // rather than under-report by matching only the Int64 rows (which WOULD be a fidelity
    // violation), it reject-closes. `assert_prove_rejects` captures + prints Prolog's
    // answer for the record (documenting the exact under-coverage) and asserts our
    // prove/verify errors with the boundary marker.
    let seed = vec![
        triple(iri("s1"), iri("val"), Obj::Long(5)),
        triple(iri("s2"), iri("val"), Obj::Decimal("7.5".into())),
    ];
    let q = and_of(vec![tvv("s", "val", "v"), greater(dvar("v"), dlong(4))]);
    harness::assert_prove_rejects(&seed, &q, "RangeDatatypeNotHomogeneous");
}

#[test]
fn adv_float_range_rejects() {
    // xsd:float/double range reject-closes: the store's IEEE total order splits
    // -0.0/+0.0 and orders NaN above +inf, diverging from WOQL numeric order (FIX-A).
    let seed = vec![
        triple(iri("s1"), iri("f"), Obj::Float(1.5)),
        triple(iri("s2"), iri("f"), Obj::Float(-2.0)),
    ];
    let _ = &seed;
    let q = and_of(vec![tvv("s", "f", "v"), greater(dvar("v"), dfloat(0.0))]);
    assert_compiler_rejects(&q);
}

#[test]
fn adv_float_equality_signed_zero_rejects() {
    // `Equals(0.0, -0.0)` is numerically TRUE in WOQL but the store gives -0.0 and +0.0
    // DISTINCT dictionary ids -> the id-based gadget would prove FALSE -> reject closed.
    let q = and_of(vec![
        tvv("s", "p", "o"),
        eq_val(
            Value::Data(XSDAnySimpleType::Float(0.0)),
            Value::Data(XSDAnySimpleType::Float(-0.0)),
        ),
    ]);
    assert_compiler_rejects(&q);
}

#[test]
fn adv_empty_result_relation() {
    // REVIEW: a SUPPORTED query that matches ZERO rows over a >=2-triple layer. Both
    // engines must return the empty relation. (Exercises the 0-row prove/resolve path.)
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), greater(dvar("v"), duint(1000))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q);
}

#[test]
fn adv_single_row_result() {
    // Exactly one matching row (layer has >=2 triples; result padded to the min domain).
    let seed = age_seed_uint();
    let q = and_of(vec![tvv("s", "age", "v"), greater(dvar("v"), duint(50))]);
    let r = run_differential(&seed, &q);
    assert_fidelity(&r, &q); // only the 55 row.
}

// ---------------------------------------------------------------------------
// Shared seeds / query fragments.
// ---------------------------------------------------------------------------

/// Three `xsd:unsignedInt` ages: 40, 20, 55 (subjects a/b/c).
fn age_seed_uint() -> Vec<harness::SeedTriple> {
    vec![
        triple(iri("a"), iri("age"), Obj::Uint(40)),
        triple(iri("b"), iri("age"), Obj::Uint(20)),
        triple(iri("c"), iri("age"), Obj::Uint(55)),
    ]
}

/// A constant-subject multi-valued `item` list of unsigned ints (for OrderBy).
fn list_items_uint(vals: &[usize]) -> Vec<harness::SeedTriple> {
    vals.iter()
        .map(|&v| triple(iri("list"), iri("item"), Obj::Uint(v)))
        .collect()
}

/// `<list> item ?v & Greater(?v, 0)` — pins `?v` to the UInt32 domain the order
/// gadget requires (an all-pass filter over positive seed values).
fn list_v_ge0() -> Query {
    and_of(vec![tcv("list", "item", "v"), gte(dvar("v"), duint(0))])
}

/// `OrderBy([var dir], child)`.
fn order_single(var: &str, dir: Order, child: Query) -> Query {
    Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: var.into(),
            order: dir,
        }],
        query: Box::new(child),
    })
}

/// p1 has ages 10/20/30, p2 has ages 5/15 (all `xsd:unsignedInt`).
fn group_age_seed() -> Vec<harness::SeedTriple> {
    vec![
        triple(iri("p1"), iri("age"), Obj::Uint(10)),
        triple(iri("p1"), iri("age"), Obj::Uint(20)),
        triple(iri("p1"), iri("age"), Obj::Uint(30)),
        triple(iri("p2"), iri("age"), Obj::Uint(5)),
        triple(iri("p2"), iri("age"), Obj::Uint(15)),
    ]
}

/// `?Person age ?Age & Gte(?Age, 0)` — the group child (pins Age to UInt32).
fn group_age_child() -> Query {
    and_of(vec![tvv("Person", "age", "Age"), gte(dvar("Age"), duint(0))])
}

/// `GroupBy Person (min|max of Ages) -> [Person, Youngest]`.
fn group_extrema(max: bool) -> Query {
    let group = Query::GroupBy(GroupBy {
        template: Value::Variable("Age".into()),
        group_by: vec!["Person".into()],
        grouped_value: Value::Variable("Ages".into()),
        query: Box::new(group_age_child()),
    });
    let range = if max {
        Query::RangeMax(RangeMax {
            list: dvar("Ages"),
            result: dvar("Youngest"),
        })
    } else {
        Query::RangeMin(RangeMin {
            list: dvar("Ages"),
            result: dvar("Youngest"),
        })
    };
    and_of(vec![group, range])
}

//! Phase 1 differential ("oracle") vertical slice for the Proof-of-WOQL fidelity
//! boundary.
//!
//! Each `#[test]` authors ONE woql2 [`Query`], seeds a small deterministic dataset,
//! and runs it through BOTH engines via [`harness::run_differential`]: the real
//! SWI-Prolog TerminusDB server (the ORACLE) and our Rust prover/verifier (the
//! system under test). [`harness::assert_fidelity`] then canonicalizes both raw
//! answers and asserts they agree under the mode the query fixes.
//!
//! The slice proves the whole loop end to end over five SUPPORTED shapes
//! (single-atom BGP, equality/range filter, 2-atom join, `OrderBy`, `Count`) plus
//! one genuinely OUT-OF-FRAGMENT query the compiler must reject closed. The
//! fidelity-critical comparator itself is unit-tested (server-free) inside
//! `tests/harness/mod.rs`.
//!
//! Run (release-gated; the debug proof build is far too slow). The coordinator
//! supplies the m4 / LD_LIBRARY_PATH / TERMINUSDB_SOURCE / RUST_MIN_STACK env; the
//! server tests each boot a real child process, so `--test-threads=1` keeps the
//! resource use bounded:
//!
//!   export PATH=/nix/store/.../gnum4-1.4.21/bin:$PATH
//!   RUST_MIN_STACK=134217728 \
//!   TERMINUSDB_SOURCE=/path/to/terminusdb-fork-checkout \
//!   cargo +nightly-2025-09-19 test -p terminusdb-proof-model-tests --release \
//!     --test differential -- --nocapture --test-threads=1

mod harness;

use harness::{assert_compiler_rejects, assert_fidelity, iri, run_differential, triple, Obj};

use terminusdb_woql2::compare::{Greater, TypeOf};
use terminusdb_woql2::misc::Count;
use terminusdb_woql2::order::{Order, OrderBy, OrderTemplate};
use terminusdb_woql2::query::{And, Query};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

// --- Small query builders (shared by the slice) -----------------------------

/// `?subj <pred-IRI> ?obj` — an all-variable single atom.
fn triple_vv(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.to_string()),
        graph: None,
    })
}

/// `<subj-IRI> <pred-IRI> ?obj` — a constant-subject atom (keeps only `?obj`).
fn triple_cv(subj_iri: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subj_iri)),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.to_string()),
        graph: None,
    })
}

/// `?subj <pred-IRI> <obj-IRI>` — a node-constant object atom (keeps only `?subj`).
fn triple_vc(subj: &str, pred: &str, obj_iri: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Node(iri(obj_iri)),
        graph: None,
    })
}

fn and_of(qs: Vec<Query>) -> Query {
    Query::And(And { and: qs })
}

// ===========================================================================
// (a) Single-atom BGP: `?s <p> ?o`.
// ===========================================================================
#[test]
fn single_atom_bgp_matches_oracle() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("b"), iri("p"), Obj::Node(iri("y"))),
        // A different-predicate edge, invisible to `?s <p> ?o`, so the layer holds
        // more than the two matched rows.
        triple(iri("c"), iri("q"), Obj::Node(iri("z"))),
    ];
    let query = triple_vv("s", "p", "o");
    let result = run_differential(&seed, &query);
    assert_fidelity(&result, &query);
}

// ===========================================================================
// (b) Equality/range FILTER: `?s <age> ?v  &  Greater(?v, 30)`.
// Exercises the numeric range gadget; output is [s (node), v (int)].
// REVIEW: depends on the server committing `xsd:unsignedInt` as the store `UInt32`
// datatype the range gadget proves over — confirm against a live run.
// ===========================================================================
#[test]
fn range_filter_matches_oracle() {
    let seed = vec![
        triple(iri("a"), iri("age"), Obj::Uint(40)),
        triple(iri("b"), iri("age"), Obj::Uint(20)),
        triple(iri("c"), iri("age"), Obj::Uint(55)),
    ];
    let query = and_of(vec![
        triple_vv("s", "age", "v"),
        Query::Greater(Greater {
            left: DataValue::Variable("v".to_string()),
            right: DataValue::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(30)),
        }),
    ]);
    let result = run_differential(&seed, &query);
    assert_fidelity(&result, &query);
}

// ===========================================================================
// (c) 2-atom JOIN on a shared variable: `?rec <album> ?alb & ?alb <artist> ?art`.
// ===========================================================================
#[test]
fn two_atom_join_matches_oracle() {
    let seed = vec![
        triple(iri("rec1"), iri("album"), Obj::Node(iri("alb1"))),
        triple(iri("rec2"), iri("album"), Obj::Node(iri("alb1"))),
        triple(iri("alb1"), iri("artist"), Obj::Node(iri("art1"))),
        triple(iri("alb1"), iri("artist"), Obj::Node(iri("art2"))),
    ];
    let query = and_of(vec![
        triple_vv("rec", "album", "alb"),
        triple_vv("alb", "artist", "art"),
    ]);
    let result = run_differential(&seed, &query);
    assert_fidelity(&result, &query);
}

// ===========================================================================
// (d) OrderBy: order a multi-valued integer property ASCending. The inner atom
// has a CONSTANT subject, so the ordered relation is a single column `[v]`. The
// store order gadget requires every column to carry a KNOWN integer/bool datatype
// domain, and a bare object variable has domain `None` (could be a node or any
// value) — so we pin `v`'s datatype with an ALL-PASS `Greater(v, 0)` range filter
// (values are 1,2,3). The filter changes no rows; it only makes the column a
// proven `UInt32` the order gadget accepts.
// REVIEW: like the filter, depends on `xsd:unsignedInt` -> store `UInt32`; and on
// `verify_and_resolve_values` returning the ordered relation's rows in the PROVEN
// (sorted) order (asserted positionally here).
// ===========================================================================
#[test]
fn order_by_matches_oracle_in_order() {
    let seed = vec![
        triple(iri("list"), iri("item"), Obj::Uint(3)),
        triple(iri("list"), iri("item"), Obj::Uint(1)),
        triple(iri("list"), iri("item"), Obj::Uint(2)),
    ];
    let query = Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate {
            variable: "v".to_string(),
            order: Order::Asc,
        }],
        query: Box::new(and_of(vec![
            triple_cv("list", "item", "v"),
            Query::Greater(Greater {
                left: DataValue::Variable("v".to_string()),
                right: DataValue::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(0)),
            }),
        ])),
    });
    let result = run_differential(&seed, &query);
    // assert_fidelity compares this ORDERED (positional): oracle's Prolog sort order
    // must equal the proof's proven order [1,2,3].
    assert_fidelity(&result, &query);
}

// ===========================================================================
// (e) COUNT aggregate: `Count(?s <type> <Class>)` — the count is the proof's
// authenticated `result_len`; the oracle returns a single count binding.
// ===========================================================================
#[test]
fn count_matches_oracle() {
    let seed = vec![
        triple(iri("a"), iri("type"), Obj::Node(iri("Class"))),
        triple(iri("b"), iri("type"), Obj::Node(iri("Class"))),
        triple(iri("c"), iri("type"), Obj::Node(iri("Class"))),
        // A non-matching type edge, so the count is a genuine filter (3, not 4).
        triple(iri("d"), iri("type"), Obj::Node(iri("Other"))),
    ];
    let query = Query::Count(Count {
        query: Box::new(triple_vc("s", "type", "Class")),
        count: DataValue::Variable("n".to_string()),
    });
    let result = run_differential(&seed, &query);
    assert_fidelity(&result, &query);
}

// ===========================================================================
// (f) OUT-OF-FRAGMENT: `TypeOf` is a value-shape construct, NOT a store-triple
// read (see the codebase memory note: TypeOf/Dot/IsA/Member are not triple reads).
// Our compiler MUST reject it closed rather than mis-execute it — the other half
// of fidelity. Needs no server.
// ===========================================================================
#[test]
fn type_of_is_rejected_closed() {
    let query = Query::TypeOf(TypeOf {
        value: Value::Variable("x".to_string()),
        type_uri: NodeValue::Variable("t".to_string()),
    });
    assert_compiler_rejects(&query);
}

// ===========================================================================
// (g) ZERO-ARITY EXISTENCE: a fully-ground `Triple` with NO output variables.
// Prolog answers a ground goal with exactly ONE empty binding when the triple
// exists and NONE when it does not; the prover mirrors this via the proof-
// authenticated `result_len` (1 / 0) over a hidden zero-arity UNIT column. This
// locks the harness reading `result_len` as the row count when there are no output
// columns — the case a column-length row count silently reported as 0 rows
// (differential fuzzer seed 0xf0fac0de12345623).
// ===========================================================================
#[test]
fn ground_triple_existence_matches_oracle() {
    let seed = vec![
        triple(iri("a"), iri("p"), Obj::Node(iri("x"))),
        triple(iri("b"), iri("q"), Obj::Node(iri("y"))),
    ];

    // PRESENT: (a,p,x) is a triple -> exactly one (empty) solution in BOTH engines.
    let present = Query::Triple(Triple {
        subject: NodeValue::Node(iri("a")),
        predicate: NodeValue::Node(iri("p")),
        object: Value::Node(iri("x")),
        graph: None,
    });
    let result = run_differential(&seed, &present);
    assert_fidelity(&result, &present);

    // ABSENT: (a,p,y) is not a triple -> zero solutions in BOTH engines.
    let absent = Query::Triple(Triple {
        subject: NodeValue::Node(iri("a")),
        predicate: NodeValue::Node(iri("p")),
        object: Value::Node(iri("y")),
        graph: None,
    });
    let result = run_differential(&seed, &absent);
    assert_fidelity(&result, &absent);
}

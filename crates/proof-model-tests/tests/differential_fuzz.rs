//! Phase 3: a grammar-constrained GENERATIVE differential fuzzer.
//!
//! Boots ONE embedded TerminusDB server and, for N seeded iterations, generates a
//! random (dataset, query) pair INSIDE the provable fragment, runs BOTH engines
//! (Prolog oracle vs our prover) over a fresh per-iteration db on the shared server,
//! and asserts Prolog == Rust whenever the query compiles AND proves. Compile-rejects
//! and prove-rejects (e.g. the D-fidelity strict-homogeneity guard) are LOGGED as
//! coverage boundaries, not failures. A genuine Prolog != Rust mismatch on a
//! compiling+proving query is the ONE thing that fails the test — loudly, with a
//! deterministic reproducing seed + dataset + query.
//!
//! Determinism: a fixed `BASE_SEED` constant; per-iteration seed = `BASE_SEED ^ i`;
//! `StdRng::seed_from_u64`. No clocks. To reproduce a reported failure, re-run with
//! the same code (the printed seed derives the exact `(data, query)`).
//!
//! Run (release-gated; ONE server for the whole run):
//!   DIFF_FUZZ_ITERS=100 RUST_MIN_STACK=134217728 \
//!   TERMINUSDB_SOURCE=/path/to/terminusdb-fork-checkout \
//!   cargo +nightly-2025-09-19 test -p terminusdb-proof-model-tests --release \
//!     --test differential_fuzz -- --nocapture
//! (`DIFF_FUZZ_ITERS` default 100; `DIFF_FUZZ_VERBOSE=1` logs each prove-reject.)

mod harness;

use harness::{
    check_fidelity, iri, run_differential_on, triple, FuzzOutcome, Obj, SeedTriple, SharedServer,
};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::compare::{Equals, Greater, Gte, Less, Lte};
use terminusdb_woql2::control::{Distinct, Select};
use terminusdb_woql2::misc::{Count, RangeMax, RangeMin};
use terminusdb_woql2::order::{GroupBy, Order, OrderBy, OrderTemplate};
use terminusdb_woql2::query::{And, Query};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

// ---------------------------------------------------------------------------
// Fixed vocabulary (kept small so generated queries hit committed data — results
// are non-trivial, not perpetually empty).
// ---------------------------------------------------------------------------
const SUBJECTS: [&str; 3] = ["a", "b", "c"];
const NODE_TARGETS: [&str; 4] = ["a", "b", "c", "d"]; // link targets (subjects + a leaf)
const VARS: [&str; 3] = ["u", "v", "w"];
const STRINGS: [&str; 3] = ["x", "y", "z"];
const DECIMALS: [&str; 5] = ["3.5", "0.25", "7", "10.5", "-2.25"];

// Predicates, one per datatype family (homogeneous per predicate by default).
const P_LINK: &str = "link"; // Node
const P_NUM: &str = "num"; // xsd:unsignedInt (UInt32)
const P_SINT: &str = "sint"; // xsd:int (Int32)
const P_BIG: &str = "big"; // xsd:long (Int64)
const P_TAG: &str = "tag"; // xsd:string
const P_FLAG: &str = "flag"; // xsd:boolean
const P_PRICE: &str = "price"; // xsd:decimal
const PREDS: [&str; 7] = [P_LINK, P_NUM, P_SINT, P_BIG, P_TAG, P_FLAG, P_PRICE];

fn choose<'a>(rng: &mut StdRng, arr: &[&'a str]) -> &'a str {
    arr[rng.gen_range(0..arr.len())]
}

// ---------------------------------------------------------------------------
// Data generation. Guarantees one triple per predicate family (so any generated
// query predicate has committed data) plus a few random extras, and occasionally
// injects datatype heterogeneity on the numeric `num` column (a decimal in a
// UInt32 column) so a range/order over it prove-rejects — which we LOG.
// ---------------------------------------------------------------------------
fn random_object(rng: &mut StdRng, pred: &str) -> Obj {
    match pred {
        P_LINK => Obj::Node(iri(choose(rng, &NODE_TARGETS))),
        P_NUM => Obj::Uint(rng.gen_range(0usize..100)),
        P_SINT => Obj::Int(rng.gen_range(-50i32..50)),
        P_BIG => Obj::Long(rng.gen_range(-1_000_000_000i64..1_000_000_000)),
        P_TAG => Obj::Str(choose(rng, &STRINGS).to_string()),
        P_FLAG => Obj::Bool(rng.gen_bool(0.5)),
        P_PRICE => Obj::Decimal(choose(rng, &DECIMALS).to_string()),
        _ => Obj::Node(iri("a")),
    }
}

fn gen_data(rng: &mut StdRng) -> Vec<SeedTriple> {
    let mut out = Vec::new();
    for &pred in &PREDS {
        let subj = choose(rng, &SUBJECTS);
        out.push(triple(iri(subj), iri(pred), random_object(rng, pred)));
    }
    let extra = rng.gen_range(0usize..=6);
    for _ in 0..extra {
        let pred = choose(rng, &PREDS);
        let subj = choose(rng, &SUBJECTS);
        out.push(triple(iri(subj), iri(pred), random_object(rng, pred)));
    }
    if rng.gen_bool(0.15) {
        // Heterogeneous numeric column: a decimal in the UInt32 `num` column. A
        // range/order over `num` will now prove-reject (RangeDatatypeNotHomogeneous).
        let subj = choose(rng, &SUBJECTS);
        out.push(triple(
            iri(subj),
            iri(P_NUM),
            Obj::Decimal(choose(rng, &DECIMALS).to_string()),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Query generation (grammar over the provable fragment).
// ---------------------------------------------------------------------------
fn dvar(n: &str) -> DataValue {
    DataValue::Variable(n.to_string())
}

/// A data-value CONSTANT of a predicate's family (for object constants in atoms).
fn random_object_value(rng: &mut StdRng, pred: &str) -> Value {
    match pred {
        P_LINK => Value::Node(iri(choose(rng, &NODE_TARGETS))),
        P_NUM => Value::Data(XSDAnySimpleType::UnsignedInt(rng.gen_range(0usize..100))),
        P_SINT => Value::Data(XSDAnySimpleType::Int(rng.gen_range(-50i32..50))),
        P_BIG => Value::Data(XSDAnySimpleType::Long(rng.gen_range(-1_000_000_000i64..1_000_000_000))),
        P_TAG => Value::Data(XSDAnySimpleType::String(choose(rng, &STRINGS).to_string())),
        P_FLAG => Value::Data(XSDAnySimpleType::Boolean(rng.gen_bool(0.5))),
        P_PRICE => Value::Data(XSDAnySimpleType::Decimal(
            choose(rng, &DECIMALS).parse().expect("valid decimal"),
        )),
        _ => Value::Node(iri("a")),
    }
}

/// One random triple atom: var/const at each position (constants drawn from the same
/// vocab so results are non-trivial). Reusing the small VAR pool across atoms yields
/// natural joins.
fn gen_atom(rng: &mut StdRng) -> Query {
    let subject = if rng.gen_bool(0.55) {
        NodeValue::Variable(choose(rng, &VARS).to_string())
    } else {
        NodeValue::Node(iri(choose(rng, &SUBJECTS)))
    };
    if rng.gen_bool(0.15) {
        // Predicate-position VARIABLE: keep the object open (var or node const).
        let predicate = NodeValue::Variable(choose(rng, &VARS).to_string());
        let object = if rng.gen_bool(0.5) {
            Value::Variable(choose(rng, &VARS).to_string())
        } else {
            Value::Node(iri(choose(rng, &NODE_TARGETS)))
        };
        Query::Triple(Triple { subject, predicate, object, graph: None })
    } else {
        let pred = choose(rng, &PREDS);
        let predicate = NodeValue::Node(iri(pred));
        let object = if rng.gen_bool(0.55) {
            Value::Variable(choose(rng, &VARS).to_string())
        } else {
            random_object_value(rng, pred)
        };
        Query::Triple(Triple { subject, predicate, object, graph: None })
    }
}

fn gen_bgp(rng: &mut StdRng) -> Query {
    let k = rng.gen_range(1usize..=3);
    let mut atoms: Vec<Query> = (0..k).map(|_| gen_atom(rng)).collect();
    if atoms.len() == 1 {
        atoms.pop().unwrap()
    } else {
        Query::And(And { and: atoms })
    }
}

/// A numeric predicate family + a matching range constant of that datatype.
fn pick_numeric(rng: &mut StdRng) -> (&'static str, DataValue) {
    match rng.gen_range(0u32..3) {
        0 => (P_NUM, DataValue::Data(XSDAnySimpleType::UnsignedInt(rng.gen_range(0usize..100)))),
        1 => (P_SINT, DataValue::Data(XSDAnySimpleType::Int(rng.gen_range(-50i32..50)))),
        _ => (P_BIG, DataValue::Data(XSDAnySimpleType::Long(rng.gen_range(-1_000_000_000i64..1_000_000_000)))),
    }
}

/// `?s <num_pred> ?v & <op>(?v, k)` — a numeric range filter that compiles+proves
/// (when the column is homogeneous).
fn gen_filtered(rng: &mut StdRng) -> Query {
    let (pred, konst) = pick_numeric(rng);
    let atom = Query::Triple(Triple {
        subject: NodeValue::Variable("s".to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable("v".to_string()),
        graph: None,
    });
    let filter = match rng.gen_range(0u32..4) {
        0 => Query::Greater(Greater { left: dvar("v"), right: konst }),
        1 => Query::Less(Less { left: dvar("v"), right: konst }),
        2 => Query::Gte(Gte { left: dvar("v"), right: konst }),
        _ => Query::Lte(Lte { left: dvar("v"), right: konst }),
    };
    Query::And(And { and: vec![atom, filter] })
}

/// `?s <pred> ?o & Equals(?s, <subj>)` — subject-node equality (domain Node).
fn gen_equality(rng: &mut StdRng) -> Query {
    let pred = choose(rng, &PREDS);
    let atom = Query::Triple(Triple {
        subject: NodeValue::Variable("s".to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable("o".to_string()),
        graph: None,
    });
    let eq = Query::Equals(Equals {
        left: Value::Variable("s".to_string()),
        right: Value::Node(iri(choose(rng, &SUBJECTS))),
    });
    Query::And(And { and: vec![atom, eq] })
}

/// `SELECT DISTINCT ?s WHERE ?s <pred> ?o`.
fn gen_distinct(rng: &mut StdRng) -> Query {
    let pred = choose(rng, &PREDS);
    let atom = Query::Triple(Triple {
        subject: NodeValue::Variable("s".to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable("o".to_string()),
        graph: None,
    });
    Query::Select(Select {
        variables: vec!["s".to_string()],
        query: Box::new(Query::Distinct(Distinct {
            variables: vec!["s".to_string()],
            query: Box::new(atom),
        })),
    })
}

/// `OrderBy([v dir], <subj> num ?v & Gte(?v, 0))` — a full-key single-column order
/// over the UInt32 `num` values of one subject.
fn gen_order(rng: &mut StdRng) -> Query {
    let subj = choose(rng, &SUBJECTS);
    let child = Query::And(And {
        and: vec![
            Query::Triple(Triple {
                subject: NodeValue::Node(iri(subj)),
                predicate: NodeValue::Node(iri(P_NUM)),
                object: Value::Variable("v".to_string()),
                graph: None,
            }),
            Query::Gte(Gte {
                left: dvar("v"),
                right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
            }),
        ],
    });
    let dir = if rng.gen_bool(0.5) { Order::Asc } else { Order::Desc };
    Query::OrderBy(OrderBy {
        ordering: vec![OrderTemplate { variable: "v".to_string(), order: dir }],
        query: Box::new(child),
    })
}

/// A GroupBy aggregate over the UInt32 `num` column grouped by subject: min / max /
/// sum / count, chosen at random.
fn gen_group(rng: &mut StdRng) -> Query {
    let child = Query::And(And {
        and: vec![
            Query::Triple(Triple {
                subject: NodeValue::Variable("Person".to_string()),
                predicate: NodeValue::Node(iri(P_NUM)),
                object: Value::Variable("Age".to_string()),
                graph: None,
            }),
            Query::Gte(Gte {
                left: dvar("Age"),
                right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
            }),
        ],
    });
    let group = Query::GroupBy(GroupBy {
        template: Value::Variable("Age".to_string()),
        group_by: vec!["Person".to_string()],
        grouped_value: Value::Variable("Ages".to_string()),
        query: Box::new(child),
    });
    let combinator = match rng.gen_range(0u32..4) {
        0 => Query::RangeMin(RangeMin { list: dvar("Ages"), result: dvar("R") }),
        1 => Query::RangeMax(RangeMax { list: dvar("Ages"), result: dvar("R") }),
        2 => Query::Sum(Sum { list: dvar("Ages"), result: dvar("R") }),
        _ => Query::Length(Length { list: dvar("Ages"), length: dvar("R") }),
    };
    Query::And(And { and: vec![group, combinator] })
}

fn gen_query(rng: &mut StdRng) -> Query {
    match rng.gen_range(0u32..100) {
        0..=39 => gen_bgp(rng),
        40..=53 => gen_filtered(rng),
        54..=63 => gen_equality(rng),
        64..=73 => Query::Count(Count {
            query: Box::new(gen_bgp(rng)),
            count: dvar("cnt"),
        }),
        74..=83 => gen_distinct(rng),
        84..=91 => gen_order(rng),
        _ => gen_group(rng),
    }
}

// ---------------------------------------------------------------------------
// The one fuzz test: boots the server ONCE, runs N iterations, asserts fidelity on
// every compiling+proving case, logs the coverage profile.
// ---------------------------------------------------------------------------
#[test]
fn fuzz_differential() {
    // Fixed base seed — deterministic across runs; per-iteration seed = BASE ^ i.
    const BASE_SEED: u64 = 0xF0FA_C0DE_1234_5678;

    let iters: u64 = std::env::var("DIFF_FUZZ_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let verbose = std::env::var("DIFF_FUZZ_VERBOSE").is_ok();

    // The terminusdb-bin Prolog server occasionally stops responding under sustained fuzz
    // load; the per-query oracle timeout turns that into a skip, but a wedged shared server
    // stays wedged, so EVERY later iteration would time out. Reboot it after this many
    // CONSECUTIVE oracle errors so the sweep recovers instead of wasting the rest of the run.
    const MAX_CONSECUTIVE_ORACLE_ERRORS: u64 = 3;
    let mut shared = SharedServer::boot().expect("boot shared fuzz server");

    let mut matched = 0u64;
    let mut compile_rejected = 0u64;
    let mut prove_rejected = 0u64;
    let mut oracle_error = 0u64;
    let mut consecutive_oracle_errors = 0u64;
    let mut server_reboots = 0u64;

    for i in 0..iters {
        let seed = BASE_SEED ^ i;
        let mut rng = StdRng::seed_from_u64(seed);
        let data = gen_data(&mut rng);
        let query = gen_query(&mut rng);
        let db = format!("fuzz_{i}");

        match run_differential_on(&shared, &db, &data, &query) {
            Ok(outcome) => {
                // A real answer came back — the server is healthy; clear the wedge counter.
                consecutive_oracle_errors = 0;
                match outcome {
                    FuzzOutcome::CompileRejected => compile_rejected += 1,
                    FuzzOutcome::ProveRejected(msg) => {
                        prove_rejected += 1;
                        if verbose {
                            eprintln!("[fuzz {i}] prove-rejected (seed {seed:#018x}): {msg}");
                        }
                    }
                    FuzzOutcome::Proved(result) => {
                        if let Err(reason) = check_fidelity(&result, &query) {
                            panic!(
                                "FIDELITY BUG at iter {i} (seed {seed:#018x})\n  data = {data:?}\n  query = {query:?}\n  {reason}"
                            );
                        }
                        matched += 1;
                    }
                }
            }
            Err(e) => {
                // Not a fidelity bug: an infra/oracle error (e.g. the oracle timed out
                // because the shared Prolog server wedged). Count + log with the repro.
                oracle_error += 1;
                consecutive_oracle_errors += 1;
                eprintln!("[fuzz {i}] oracle/infra error (seed {seed:#018x}): {e}\n  query = {query:?}");
                // Recover a wedged server so the rest of the sweep still runs.
                if consecutive_oracle_errors >= MAX_CONSECUTIVE_ORACLE_ERRORS {
                    eprintln!(
                        "[fuzz {i}] {consecutive_oracle_errors} consecutive oracle errors — rebooting the shared server"
                    );
                    shared = SharedServer::boot().expect("reboot shared fuzz server after wedge");
                    server_reboots += 1;
                    consecutive_oracle_errors = 0;
                }
            }
        }
    }

    eprintln!(
        "[fuzz] iters={iters} matched={matched} compile_rejected={compile_rejected} prove_rejected={prove_rejected} oracle_error={oracle_error} server_reboots={server_reboots}"
    );

    // Sanity floor: with ~40% of shapes dedicated to reliably-provable forms, a
    // reasonably-sized run that proves+matches NOTHING means the generator or the
    // server is broken. (Skip the floor for tiny smoke runs.)
    if iters >= 20 {
        assert!(
            matched > 0,
            "fuzzer proved+matched ZERO cases in {iters} iterations — generator/server likely broken"
        );
    }
}

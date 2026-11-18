//! Proof-cost scaling across a 20-query complexity ladder (CPU backend).
//!
//! Runs twenty WOQL queries ordered from trivial (a ground existence check) to
//! deeply composed (count over a gated recursive Or/Optional/Not pipeline),
//! proving and verifying each against the same committed layer, and reports
//! prove/verify wall time, envelope bytes, and verifier-artifact bytes per
//! query. Emits one CSV line per query on stdout (plus a human-readable
//! header on stderr) so the results can be plotted directly.
//!
//! Stock CPU:
//! `RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql-proof \
//!    --example proof_scaling -- --rows 1024 > scaling.csv`

use std::sync::Arc;
use std::time::{Duration, Instant};

use tdb_succinct::TdbDataType;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::{open_sync_memory_store, ValueTriple};
use terminusdb_woql2::compare::{Gte, Lte};
use terminusdb_woql2::control::{Select, WoqlOptional};
use terminusdb_woql2::misc::{Limit, Start};
use terminusdb_woql2::collection::{Length, Sum};
use terminusdb_woql2::misc::{Count, RangeMin};
use terminusdb_woql2::order::{GroupBy, Order, OrderBy, OrderTemplate};
use terminusdb_woql2::query::{And, Not, Or, Query};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};
use terminusdb_woql_proof::proof::{
    decode_verifier_commitment_and_verify_envelope, plan_bgp,
};

fn iri(value: &str) -> String {
    format!("http://perf/{value}")
}

fn tv(subject: &str, predicate: &str, object: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subject.into()),
        predicate: NodeValue::Node(iri(predicate)),
        object: Value::Variable(object.into()),
        graph: None,
    })
}

/// All-ground existence check: every position a constant.
fn ground(subject: &str, predicate: &str, object: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subject)),
        predicate: NodeValue::Node(iri(predicate)),
        object: Value::Node(iri(object)),
        graph: None,
    })
}

/// Ground subject, variable object: a one-column read.
fn tvc(subject: &str, predicate: &str, object: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subject)),
        predicate: NodeValue::Node(iri(predicate)),
        object: Value::Variable(object.into()),
        graph: None,
    })
}

fn and(queries: Vec<Query>) -> Query {
    Query::And(And { and: queries })
}

fn dvar(name: &str) -> DataValue {
    DataValue::Variable(name.into())
}

fn unsigned(value: u32) -> DataValue {
    DataValue::Data(terminusdb_schema::XSDAnySimpleType::UnsignedInt(value as usize))
}

/// The shared dataset (see proof_performance's build_layer): persons with p/r
/// edges, friends with q/s edges (even rows), ban gates (i % 4 == 0), and a
/// numeric `score` per category.
fn build_layer(rows: usize, groups: usize) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for i in 0..rows {
        let person = iri(&format!("person-{i}"));
        let friend = iri(&format!("friend-{i}"));
        let value = iri(&format!("value-{i}"));
        for (predicate, object) in [("p", &friend), ("r", &friend)] {
            builder
                .add_value_triple(ValueTriple::new_node(&person, &iri(predicate), object))
                .unwrap();
        }
        if i % 2 == 0 {
            for predicate in ["q", "s"] {
                builder
                    .add_value_triple(ValueTriple::new_node(&friend, &iri(predicate), &value))
                    .unwrap();
            }
        }
        if i % 4 == 0 {
            for predicate in ["ban", "ban2"] {
                builder
                    .add_value_triple(ValueTriple::new_node(
                        &person,
                        &iri(predicate),
                        &iri("blocked"),
                    ))
                    .unwrap();
            }
        }
        builder
            .add_value_triple(ValueTriple::new_value(
                &iri(&format!("category-{}", i % groups)),
                &iri("score"),
                u32::make_entry(&(i as u32)),
            ))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// The recursive Or / Optional / Not pipeline from proof_performance.
fn recursive() -> Query {
    let branch = |p, q, blocked| {
        and(vec![
            tv("x", p, "y"),
            Query::WoqlOptional(WoqlOptional {
                query: Box::new(tv("y", q, "z")),
            }),
            Query::Not(Not {
                query: Box::new(tv("x", blocked, "w")),
            }),
        ])
    };
    Query::Or(Or {
        or: vec![branch("p", "q", "ban"), branch("r", "s", "ban2")],
    })
}

fn score_child(gate: u32) -> Query {
    and(vec![
        Query::Triple(Triple {
            subject: NodeValue::Variable("category".into()),
            predicate: NodeValue::Node(iri("score")),
            object: Value::Variable("score".into()),
            graph: None,
        }),
        Query::Gte(Gte {
            left: dvar("score"),
            right: unsigned(gate),
        }),
    ])
}

/// The 20-query ladder: simple to complex. Names are stable for plotting.
fn ladder() -> Vec<(&'static str, Query)> {
    vec![
        ("01-ground-existence", ground("person-0", "p", "friend-0")),
        ("02-one-var-object", tvc("person-3", "p", "y")),
        ("03-one-var-subject", tv("x", "p", "friend-7")),
        ("04-two-var-bgp", tv("x", "p", "y")),
        (
            "05-bgp-join-2",
            and(vec![tv("x", "p", "y"), tv("y", "q", "z")]),
        ),
        (
            "06-bgp-join-3",
            and(vec![tv("x", "p", "y"), tv("y", "q", "z"), tv("y", "s", "w")]),
        ),
        (
            "07-select-projection",
            Query::Select(Select {
                variables: vec!["x".into()],
                query: Box::new(and(vec![tv("x", "p", "y"), tv("x", "r", "z")])),
            }),
        ),
        (
            "08-count",
            Query::Count(Count {
                query: Box::new(and(vec![tv("x", "p", "y"), tv("y", "q", "z")])),
                count: dvar("Rows"),
            }),
        ),
        ("09-range-filter", score_child(128)),
        (
            "10-conj-filters",
            and(vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("category".into()),
                    predicate: NodeValue::Node(iri("score")),
                    object: Value::Variable("score".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: dvar("score"),
                    right: unsigned(64),
                }),
                Query::Lte(Lte {
                    left: dvar("score"),
                    right: unsigned(192),
                }),
            ]),
        ),
        (
            // Disconnected components: a 1-row relation composed with a
            // 128-row relation (a full 256x256 join exceeds the 256 MiB
            // envelope budget and is rejected fail-closed at decode).
            "11-cartesian-product",
            and(vec![
                ground("person-0", "p", "friend-0"),
                tvc("category-0", "score", "s"),
            ]),
        ),
        (
            "12-optional-join",
            and(vec![
                tv("x", "p", "y"),
                Query::WoqlOptional(WoqlOptional {
                    query: Box::new(tv("y", "q", "z")),
                }),
            ]),
        ),
        (
            "13-antijoin-not",
            and(vec![
                tv("x", "p", "y"),
                Query::Not(Not {
                    query: Box::new(tv("x", "ban", "w")),
                }),
            ]),
        ),
        (
            "14-or-union",
            Query::Or(Or {
                or: vec![tv("x", "p", "y"), tv("x", "r", "y")],
            }),
        ),
        ("15-or-optional-not", recursive()),
        (
            "16-order-scalar",
            Query::OrderBy(OrderBy {
                ordering: vec![OrderTemplate {
                    variable: "score".into(),
                    order: Order::Asc,
                }],
                query: Box::new(tvc("category-0", "score", "score")),
            }),
        ),
        (
            "17-order-slice",
            Query::Limit(Limit {
                limit: 5,
                query: Box::new(Query::Start(Start {
                    start: 2,
                    query: Box::new(Query::OrderBy(OrderBy {
                        ordering: vec![OrderTemplate {
                            variable: "score".into(),
                            order: Order::Asc,
                        }],
                        query: Box::new(and(vec![
                            tvc("category-0", "score", "score"),
                            Query::Gte(Gte { left: dvar("score"), right: unsigned(0) }),
                        ])),
                    })),
                })),
            }),
        ),
        (
            "18-group-sum",
            Query::Select(Select {
                variables: vec!["category".into(), "total".into()],
                query: Box::new(and(vec![
                    Query::GroupBy(GroupBy {
                        template: Value::Variable("score".into()),
                        group_by: vec!["category".into()],
                        grouped_value: Value::Variable("scores".into()),
                        query: Box::new(score_child(0)),
                    }),
                    Query::Sum(Sum {
                        list: dvar("scores"),
                        result: dvar("total"),
                    }),
                ])),
            }),
        ),
        (
            "19-group-min",
            Query::Select(Select {
                variables: vec!["category".into(), "minimum".into()],
                query: Box::new(and(vec![
                    Query::GroupBy(GroupBy {
                        template: Value::Variable("score".into()),
                        group_by: vec!["category".into()],
                        grouped_value: Value::Variable("scores".into()),
                        query: Box::new(score_child(0)),
                    }),
                    Query::RangeMin(RangeMin {
                        list: dvar("scores"),
                        result: dvar("minimum"),
                    }),
                ])),
            }),
        ),
        (
            "20-count-gated-pipeline",
            Query::Count(Count {
                query: Box::new(and(vec![
                    ground("person-0", "ban", "blocked"),
                    recursive(),
                ])),
                count: dvar("Rows"),
            }),
        ),
    ]
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values[values.len() / 2]
}

fn timed<T>(runs: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut times = Vec::with_capacity(runs);
    let mut last = None;
    for _ in 0..runs {
        let started = Instant::now();
        last = Some(f());
        times.push(started.elapsed());
    }
    (median(times), last.unwrap())
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn main() {
    // Deep recursive envelopes need far more than the default main-thread stack.
    let child = std::thread::Builder::new()
        .stack_size(1024 * 1024 * 1024)
        .spawn(run)
        .unwrap();
    child.join().unwrap();
}

fn run() {
    assert!(!cfg!(debug_assertions), "release build required");
    let mut rows = 1024usize;
    let mut groups = 8usize;
    let mut prove_runs = 3usize;
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    while !args.is_empty() {
        match args[0].as_str() {
            "--rows" => rows = args[1].parse().unwrap(),
            "--groups" => groups = args[1].parse().unwrap(),
            "--prove-runs" => prove_runs = args[1].parse().unwrap(),
            other => panic!("unknown argument {other}; expected --rows/--groups/--prove-runs"),
        }
        args.drain(..2);
    }

    eprintln!(
        "proof-scaling: cpu backend, rows={rows}, groups={groups}, prove_runs={prove_runs}"
    );
    let layer = build_layer(rows, groups);
    println!("id,name,result_rows,prove_ms,verify_ms,envelope_bytes,artifact_bytes,compact_verify_ms");

    for (name, query) in ladder() {
        let compiled = plan_bgp(&query)
            .unwrap_or_else(|error| panic!("plan failed for {name}: {error}"));
        let (prove, proved) = timed(prove_runs, || {
            compiled
                .prove_with_context(&terminus_store::proof::argument::StoreProverContext::cpu(), &layer)
                .unwrap_or_else(|error| panic!("prove failed for {name}: {error}"))
        });
        let root = layer.resolution_root();
        let (verify, ()) = timed(3, || proved.verify(&compiled, &layer, root).unwrap());
        let (_, envelope) = timed(1, || {
            proved.encode_envelope(&compiled, &layer, root).unwrap()
        });
        let (_, artifact) = timed(1, || layer.verifier_commitment().unwrap().encode().unwrap());
        let (compact_verify, _) = timed(3, || {
            decode_verifier_commitment_and_verify_envelope(
                &envelope,
                &compiled,
                &artifact,
                root,
            )
            .unwrap()
        });
        println!(
            "{},{},{},{:.2},{:.2},{},{},{:.2}",
            &name[..2],
            name[3..].to_owned(),
            proved.result_len,
            ms(prove),
            ms(verify),
            envelope.len(),
            artifact.len(),
            ms(compact_verify),
        );
    }
}

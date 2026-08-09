//! Reproducible release-only phase timings for Proof-of-WOQL.
//!
//! `RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 --features proof --example proof_performance -- 64 128`

use std::sync::Arc;
use std::time::{Duration, Instant};

use tdb_succinct::TdbDataType;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::{open_sync_memory_store, ValueTriple};
use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::compare::Gte;
use terminusdb_woql2::control::{Select, WoqlOptional};
use terminusdb_woql2::misc::RangeMin;
use terminusdb_woql2::order::GroupBy;
use terminusdb_woql2::proof::{decode_verifier_commitment_and_verify_envelope, plan_bgp};
use terminusdb_woql2::query::{And, Not, Or, Query};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

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

fn and(queries: Vec<Query>) -> Query {
    Query::And(And { and: queries })
}

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

fn small_bgp() -> Query {
    and(vec![tv("x", "p", "y"), tv("y", "q", "z")])
}

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

fn grouped_min() -> Query {
    let child = and(vec![
        Query::Triple(Triple {
            subject: NodeValue::Variable("category".into()),
            predicate: NodeValue::Node(iri("score")),
            object: Value::Variable("score".into()),
            graph: Some(GraphType::Instance),
        }),
        Query::Gte(Gte {
            left: DataValue::Variable("score".into()),
            right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
        }),
    ]);
    let group = Query::GroupBy(GroupBy {
        template: Value::Variable("score".into()),
        group_by: vec!["category".into()],
        grouped_value: Value::Variable("scores".into()),
        query: Box::new(child),
    });
    Query::Select(Select {
        variables: vec!["category".into(), "minimum".into()],
        query: Box::new(and(vec![
            group,
            Query::RangeMin(RangeMin {
                list: DataValue::Variable("scores".into()),
                result: DataValue::Variable("minimum".into()),
            }),
        ])),
    })
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

fn scenario(name: &str, query: Query, layer: &LayerCommitment, prove_runs: usize) {
    let compiled = plan_bgp(&query).unwrap();
    let (prove, proved) = timed(prove_runs, || compiled.prove(layer).unwrap());
    let root = layer.state.commitment_root;
    let (verify, ()) = timed(3, || proved.verify(&compiled, layer, root).unwrap());
    let (encode, envelope) = timed(3, || {
        proved.encode_envelope(&compiled, layer, root).unwrap()
    });
    let (catalog, artifact) = timed(3, || layer.verifier_commitment().unwrap().encode().unwrap());
    let (compact_verify, _) = timed(3, || {
        decode_verifier_commitment_and_verify_envelope(&envelope, &compiled, &artifact, root)
            .unwrap()
    });
    println!(
        "{name:18} rows={:5} prove={:9.2} verify={:9.2} encode_checked={:9.2} artifact={:8.2} compact_verify={:9.2} envelope={:8} artifact_bytes={:8}",
        proved.result_len,
        ms(prove),
        ms(verify),
        ms(encode),
        ms(catalog),
        ms(compact_verify),
        envelope.len(),
        artifact.len(),
    );
}

fn main() {
    assert!(!cfg!(debug_assertions), "release build required");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let rows = args.first().map_or(64, |v| v.parse().unwrap());
    let group_rows = args.get(1).map_or(128, |v| v.parse().unwrap());
    let groups = 8.min(group_rows.max(1));
    println!("release proof-performance rows={rows} group_rows={group_rows} groups={groups}");
    let graph_layer = build_layer(rows, groups);
    let group_layer = build_layer(group_rows, groups);
    scenario("small-bgp", small_bgp(), &graph_layer, 3);
    scenario("or-optional-not", recursive(), &graph_layer, 1);
    scenario("grouped-min", grouped_min(), &group_layer, 1);
}

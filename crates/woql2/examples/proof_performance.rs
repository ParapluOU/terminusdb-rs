//! Reproducible release-only phase timings for Proof-of-WOQL.
//!
//! Stock CPU:
//! `RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 --features proof --example proof_performance -- --backend cpu --tier log20 --rows 64 --group-rows 128 --precompute 0`
//!
//! Blitzar CPU:
//! `RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 --features blitzar-cpu --example proof_performance -- --backend blitzar-cpu --tier log20 --rows 64 --group-rows 128 --precompute 0`
//!
//! Blitzar GPU (actual validation requires Linux x86_64 with an NVIDIA GPU and CUDA):
//! `RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 --features blitzar-gpu --example proof_performance -- --backend blitzar-gpu --tier log20 --rows 64 --group-rows 128 --precompute 0`
//!
//! CPU and GPU Blitzar features are intentionally mutually exclusive. This runner
//! performs no automatic backend selection and owns exactly one proving context.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tdb_succinct::TdbDataType;
#[cfg(any(
    all(feature = "blitzar-cpu", not(feature = "blitzar-gpu")),
    all(feature = "blitzar-gpu", not(feature = "blitzar-cpu"))
))]
use terminus_store::proof::argument::BlitzarRuntimeConfig;
use terminus_store::proof::argument::{ProverSetupTier, StoreProverContext};
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

const USAGE: &str = "usage: proof_performance [--backend cpu|blitzar-cpu|blitzar-gpu] [--tier log20|log22|log24] [--rows N] [--group-rows N] [--precompute N]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Cpu,
    BlitzarCpu,
    BlitzarGpu,
}

impl Backend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "blitzar-cpu" => Ok(Self::BlitzarCpu),
            "blitzar-gpu" => Ok(Self::BlitzarGpu),
            _ => Err(format!(
                "invalid backend {value:?}; expected cpu, blitzar-cpu, or blitzar-gpu"
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::BlitzarCpu => "blitzar-cpu",
            Self::BlitzarGpu => "blitzar-gpu",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Config {
    backend: Backend,
    tier: ProverSetupTier,
    tier_name: &'static str,
    rows: usize,
    group_rows: usize,
    precompute: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: Backend::Cpu,
            tier: ProverSetupTier::Log20,
            tier_name: "log20",
            rows: 64,
            group_rows: 128,
            precompute: 0,
        }
    }
}

fn tier(value: &str) -> Result<(ProverSetupTier, &'static str), String> {
    match value {
        "log20" => Ok((ProverSetupTier::Log20, "log20")),
        "log22" => Ok((ProverSetupTier::Log22, "log22")),
        "log24" => Ok((ProverSetupTier::Log24, "log24")),
        _ => Err(format!(
            "invalid tier {value:?}; expected log20, log22, or log24"
        )),
    }
}

fn value<'a>(args: &'a [String], index: &mut usize, option: &str) -> Result<&'a str, String> {
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .ok_or_else(|| format!("missing value for {option}"))
}

fn positive_usize(value: &str, option: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{option} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{option} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_config() -> Result<Option<Config>, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut config = Config::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Ok(None),
            "--backend" => config.backend = Backend::parse(value(&args, &mut index, "--backend")?)?,
            "--tier" => {
                let parsed = tier(value(&args, &mut index, "--tier")?)?;
                config.tier = parsed.0;
                config.tier_name = parsed.1;
            }
            "--rows" => {
                config.rows = positive_usize(value(&args, &mut index, "--rows")?, "--rows")?
            }
            "--group-rows" => {
                config.group_rows =
                    positive_usize(value(&args, &mut index, "--group-rows")?, "--group-rows")?;
            }
            "--precompute" => {
                config.precompute = value(&args, &mut index, "--precompute")?
                    .parse::<u64>()
                    .map_err(|_| "--precompute must be a non-negative integer".to_owned())?;
            }
            unknown => return Err(format!("unknown option {unknown:?}")),
        }
        index += 1;
    }
    Ok(Some(config))
}

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

fn initialize_context(
    config: Config,
) -> Result<(StoreProverContext<'static>, Duration, Duration), String> {
    match config.backend {
        Backend::Cpu => {
            let backend_initialization = Duration::ZERO;
            let started = Instant::now();
            let context = StoreProverContext::cpu();
            Ok((context, backend_initialization, started.elapsed()))
        }
        Backend::BlitzarCpu => initialize_blitzar_cpu(config),
        Backend::BlitzarGpu => initialize_blitzar_gpu(config),
    }
}

fn initialize_blitzar_cpu(
    config: Config,
) -> Result<(StoreProverContext<'static>, Duration, Duration), String> {
    #[cfg(all(feature = "blitzar-cpu", not(feature = "blitzar-gpu")))]
    {
        initialize_blitzar(config)
    }
    #[cfg(all(feature = "blitzar-cpu", feature = "blitzar-gpu"))]
    {
        let _ = config;
        Err("blitzar-cpu and blitzar-gpu features conflict; enable exactly one".to_owned())
    }
    #[cfg(not(feature = "blitzar-cpu"))]
    {
        let _ = config;
        Err("backend blitzar-cpu requires the blitzar-cpu Cargo feature".to_owned())
    }
}

fn initialize_blitzar_gpu(
    config: Config,
) -> Result<(StoreProverContext<'static>, Duration, Duration), String> {
    #[cfg(all(feature = "blitzar-gpu", not(feature = "blitzar-cpu")))]
    {
        initialize_blitzar(config)
    }
    #[cfg(all(feature = "blitzar-cpu", feature = "blitzar-gpu"))]
    {
        let _ = config;
        Err("blitzar-cpu and blitzar-gpu features conflict; enable exactly one".to_owned())
    }
    #[cfg(not(feature = "blitzar-gpu"))]
    {
        let _ = config;
        Err("backend blitzar-gpu requires the blitzar-gpu Cargo feature".to_owned())
    }
}

#[cfg(any(
    all(feature = "blitzar-cpu", not(feature = "blitzar-gpu")),
    all(feature = "blitzar-gpu", not(feature = "blitzar-cpu"))
))]
fn initialize_blitzar(
    config: Config,
) -> Result<(StoreProverContext<'static>, Duration, Duration), String> {
    let started = Instant::now();
    let runtime = StoreProverContext::initialize_blitzar(BlitzarRuntimeConfig {
        num_precomputed_generators: config.precompute,
    });
    let backend_initialization = started.elapsed();

    let started = Instant::now();
    let context = StoreProverContext::blitzar(&runtime, config.tier)
        .map_err(|error| format!("failed to construct {} setup: {error}", config.tier_name))?;
    let setup_initialization = started.elapsed();
    Ok((context, backend_initialization, setup_initialization))
}

fn scenario(
    config: Config,
    context: &StoreProverContext<'_>,
    name: &str,
    query: Query,
    layer: &LayerCommitment,
    prove_runs: usize,
) {
    let compiled = plan_bgp(&query).unwrap();
    let (prove, proved) = timed(prove_runs, || {
        compiled.prove_with_context(context, layer).unwrap()
    });
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
        "backend={:12} tier={} precompute={:<8} {name:18} result_rows={:5} prove_ms={:9.2} verify_ms={:9.2} encode_checked_ms={:9.2} artifact_ms={:8.2} compact_verify_ms={:9.2} envelope_bytes={:8} artifact_bytes={:8}",
        config.backend.as_str(),
        config.tier_name,
        config.precompute,
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
    let config = match parse_config() {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("{USAGE}");
            return;
        }
        Err(error) => {
            eprintln!("error: {error}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let (context, backend_initialization, setup_initialization) = initialize_context(config)
        .unwrap_or_else(|error| {
            eprintln!("error: {error}");
            std::process::exit(2);
        });

    let groups = 8.min(config.group_rows);
    println!(
        "release proof-performance backend={} tier={} precompute={} rows={} group_rows={} groups={}",
        config.backend.as_str(),
        config.tier_name,
        config.precompute,
        config.rows,
        config.group_rows,
        groups,
    );
    println!(
        "backend_init_ms={:.2} setup_init_ms={:.2}",
        ms(backend_initialization),
        ms(setup_initialization),
    );

    let graph_layer = build_layer(config.rows, groups);
    let group_layer = build_layer(config.group_rows, groups);
    if config.backend == Backend::BlitzarGpu {
        let compiled = plan_bgp(&small_bgp()).unwrap();
        let started = Instant::now();
        compiled
            .prove_with_context(&context, &graph_layer)
            .expect("GPU warmup proof failed");
        println!("gpu_warmup_ms={:.2}", ms(started.elapsed()));
    } else {
        println!("gpu_warmup_ms=n/a");
    }

    scenario(config, &context, "small-bgp", small_bgp(), &graph_layer, 3);
    scenario(
        config,
        &context,
        "or-optional-not",
        recursive(),
        &graph_layer,
        1,
    );
    scenario(
        config,
        &context,
        "grouped-min",
        grouped_min(),
        &group_layer,
        1,
    );
}

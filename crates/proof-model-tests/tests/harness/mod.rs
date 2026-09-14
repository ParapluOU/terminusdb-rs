//! Differential ("oracle") test harness for the Proof-of-WOQL fidelity boundary.
//!
//! The **real SWI-Prolog TerminusDB server** (booted via `terminusdb-bin`) is the
//! ORACLE — ground truth for WOQL semantics. Our **Rust prover/verifier**
//! (`plan_bgp` -> `prove` -> `verify_and_resolve_values`) is the system under test.
//!
//! THE KEY TRICK: every test authors its query ONCE as a woql2 [`Query`] value.
//! [`run_differential`] then:
//!   (a) serializes it to JSON-LD WOQL and runs it through the live Prolog HTTP
//!       server, collecting Prolog's raw bindings (the ORACLE answer);
//!   (b) hands the SAME [`Query`] AST to our Rust prover over the committed layer
//!       replicated out of that same server (the SYSTEM-UNDER-TEST answer).
//! Both engines answer the identical query, so [`assert_fidelity`] compares like
//! for like.
//!
//! The comparator ([`assert_fidelity`] + its pure internals [`cell_from_rust`],
//! [`cell_from_prolog`], [`compare_relations`]) is the FIDELITY-CRITICAL piece: a
//! bug here silently hides real divergence, so it carries its own unit tests (see
//! the `comparator_unit_tests` module at the bottom of this file — they run in the
//! same test binary, need NO server, and MUST stay green).
//!
//! Phases 2 (curated matrix + adversarial datatypes) and 3 (generative fuzzer)
//! reuse `run_differential` / `assert_fidelity` UNCHANGED — only the seed/query
//! generators differ.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;

use serde_json::Value as Json;

use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::query::CanonicalResultValue;
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::storage::pack_layer_parents;
use terminus_store::store::sync::{open_sync_memory_store, SyncStore};
use terminus_store::Layer;

use terminusdb_woql_proof::proof::{plan_bgp, CompiledBgp};
use terminusdb_woql2::query::{And, Query};
use terminusdb_woql2::triple::{AddTriple, Triple};
use terminusdb_woql2::value::{NodeValue, Value};

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{BranchSpec, TerminusDBHttpClient};

use terminusdb_schema::XSDAnySimpleType;

// The store's own xsd:decimal storage decoder — the exact inverse of the BCD
// encoding a committed `CanonicalObject::Value{Decimal, lexical}` carries. Lets us
// render a resolved decimal to its canonical lexical, matching Prolog's rendering.
use tdb_succinct::tfc::decimal::storage_to_decimal;

// ---------------------------------------------------------------------------
// Seed model.
//
// A seed triple's object is either a graph node (IRI) or a typed literal. Every
// test seed builds its IRIs under a shared, distinctive host so the harness can
// locate the committed instance-graph layer inside the replicated pack (see
// [`MARKER`] and [`iri`]).
// ---------------------------------------------------------------------------

/// The distinctive host substring every differential seed shares. Used to
/// discover the committed instance-graph head layer inside the replicated pack
/// (robust to any canonicalization the server applies to full IRIs).
pub const MARKER: &str = "d.terminusdb.test";

/// Build a full IRI under the shared differential-test host. Tests use this for
/// BOTH the seed operands AND the query constants, so the committed strings and
/// the query constants are byte-identical.
pub fn iri(name: &str) -> String {
    format!("http://{MARKER}/{name}")
}

/// A seed triple's object. Phase 2 extends the phase-1 set (Node/Uint/Str/Bool)
/// with signed ints, `xsd:integer`, `xsd:decimal`, and `xsd:float` so the
/// adversarial datatype cases can commit real heterogeneous columns.
#[derive(Clone, Debug)]
pub enum Obj {
    /// A graph node / IRI (a link edge).
    Node(String),
    /// `xsd:unsignedInt` — the round-trip-aligned small-int datatype the range /
    /// order gadgets prove over (stored as `UInt32`).
    Uint(usize),
    /// `xsd:string`.
    Str(String),
    /// `xsd:boolean`.
    Bool(bool),
    /// `xsd:int` (signed 32-bit → store `Int32`).
    Int(i32),
    /// `xsd:long` (signed 64-bit → store `Int64`). `xsd:integer` also maps to
    /// `Int64` in `xsd_scalar_entry`.
    Long(i64),
    /// `xsd:integer` (bounded i64 → store `Int64`).
    Integer(i64),
    /// `xsd:decimal` — a NON-order-preserving variable-width datatype. Ranges over
    /// it reject closed; resolution renders it via `tdb-succinct`'s decimal decoder.
    /// Carries the canonical decimal LEXICAL (e.g. `"3.5"`, `"-0.000001"`).
    Decimal(String),
    /// `xsd:float`/`double` (store `Float64`). Reachable in seeds only to build the
    /// float REJECT cases; never resolved in a fidelity comparison.
    Float(f64),
}

impl Obj {
    fn into_woql(self) -> Value {
        match self {
            Obj::Node(n) => Value::Node(n),
            Obj::Uint(k) => Value::Data(XSDAnySimpleType::UnsignedInt(k)),
            Obj::Str(s) => Value::Data(XSDAnySimpleType::String(s)),
            Obj::Bool(b) => Value::Data(XSDAnySimpleType::Boolean(b)),
            Obj::Int(v) => Value::Data(XSDAnySimpleType::Int(v)),
            Obj::Long(v) => Value::Data(XSDAnySimpleType::Long(v)),
            Obj::Integer(v) => Value::Data(XSDAnySimpleType::Integer(v)),
            Obj::Decimal(s) => Value::Data(XSDAnySimpleType::Decimal(
                s.parse().expect("seed decimal lexical must parse"),
            )),
            Obj::Float(f) => Value::Data(XSDAnySimpleType::Float(f)),
        }
    }
}

/// One seed triple `(subject IRI, predicate IRI, object)`.
#[derive(Clone, Debug)]
pub struct SeedTriple {
    pub subject: String,
    pub predicate: String,
    pub object: Obj,
}

/// Convenience seed-triple constructor.
pub fn triple(subject: impl Into<String>, predicate: impl Into<String>, object: Obj) -> SeedTriple {
    SeedTriple {
        subject: subject.into(),
        predicate: predicate.into(),
        object,
    }
}

// ---------------------------------------------------------------------------
// Differential result — the RAW answer from both engines.
//
// Intentionally raw: canonicalization + comparison happens in `assert_fidelity`
// (via the pure, unit-tested `cell_from_*` / `compare_relations`), so the tests
// exercise the very same fidelity-critical code the harness relies on.
// ---------------------------------------------------------------------------

/// Both engines' raw answers to the same query over the same committed data.
pub struct DifferentialResult {
    /// The Prolog server's raw bindings: one JSON object per solution row, keyed
    /// by (bare, un-prefixed) variable name. Unbound variables are absent / `null`.
    pub oracle_bindings: Vec<Json>,
    /// The server's `api:variable_names` (for diagnostics).
    pub oracle_var_names: Vec<String>,
    /// The Rust prover's resolved answer, COLUMN-MAJOR in the compiled relation's
    /// full schema order (`rust_columns[col][row]`), as returned by
    /// `verify_and_resolve_values`. Empty when the query is a `Count`.
    pub rust_columns: Vec<Vec<CanonicalResultValue>>,
    /// `Some((count_variable, count))` when the query is an outer `Count`; the
    /// count is the proof-authenticated `result_len`. `None` otherwise.
    pub rust_count: Option<(String, i128)>,
    /// The compiled relation's full output column names (`compiled.relation.schema`).
    pub schema: Vec<String>,
    /// The `Select` projection (column indices into `schema`), if any. `None`
    /// means every schema column is an output column.
    pub projection: Option<Vec<usize>>,
    /// The proof-authenticated number of result rows (`proved.result_len`). This is
    /// the AUTHORITATIVE row count — column data is column-major, so for a
    /// zero-output-column ("zero-arity existence") query (e.g. a fully-ground
    /// `Triple`) `rust_columns` is empty and the row count is NOT recoverable from a
    /// column length; it lives here. Present ground triple => 1 (one empty-tuple row,
    /// matching Prolog's single empty binding); absent => 0. Unused on the `Count`
    /// path (that answer is `rust_count`).
    pub rust_result_len: usize,
}

// ---------------------------------------------------------------------------
// Phase A (async): boot server, commit seed, run the ORACLE query, replicate.
// Everything async (server/client/reqwest) lives inside ONE runtime; the sync
// terminus-store phase runs afterwards on the plain test thread (terminus-store's
// `task_sync` uses its OWN global runtime, so store calls must NOT be inside a
// `block_on`). Mirrors `http_replication_proof.rs`.
// ---------------------------------------------------------------------------
struct OraclePhase {
    oracle_bindings: Vec<Json>,
    oracle_var_names: Vec<String>,
    pack: Vec<u8>,
}

/// Wall-clock cap on ALL per-call oracle work (server/db create + seed commit + query +
/// pack). A flaky/hung TerminusDB request would otherwise block the runtime forever — a
/// single hung request once wedged a fuzz run for ~18h. On elapse we return an error the
/// fuzzer classifies as `oracle_error` and SKIPS, instead of hanging indefinitely.
const ORACLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn oracle_phase(db: &str, seed: &[SeedTriple], query: &Query) -> anyhow::Result<OraclePhase> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let out = rt.block_on(async {
        let work = async {
        // (1) Boot the real embedded proof-capable server (in-memory).
        let server = TerminusDBServer::test().await?;
        let port = server.port();
        let client = server.client().await?;
        let base = format!("http://localhost:{port}");
        let http = reqwest::Client::new();

        // (2a) Create a SCHEMALESS database so arbitrary ground triples are accepted.
        let created = http
            .post(format!("{base}/api/db/admin/{db}"))
            .basic_auth("admin", Some("root"))
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!({
                    "label": db,
                    "comment": "proof-of-woql differential oracle",
                    "schema": false,
                    "public": true
                })
                .to_string(),
            )
            .send()
            .await?;
        anyhow::ensure!(
            created.status().is_success() || created.status().as_u16() == 400,
            "database creation failed: {} {}",
            created.status(),
            created.text().await.unwrap_or_default()
        );

        // (2b) COMMIT the seed via a single WOQL `And` of `AddTriple`s over `/api/woql`.
        let spec = BranchSpec::with_branch(db, "main");
        let insert = Query::And(And {
            and: seed
                .iter()
                .cloned()
                .map(|t| {
                    Query::AddTriple(AddTriple {
                        subject: NodeValue::Node(t.subject),
                        predicate: NodeValue::Node(t.predicate),
                        object: t.object.into_woql(),
                        graph: None,
                    })
                })
                .collect(),
        });
        let _committed = client
            .query_mut::<serde_json::Value>(spec.clone(), insert, "differential", "seed commit")
            .await?;

        // (3) Run the ORACLE query through the live server and collect its bindings.
        // Each binding is a raw JSON object (var name -> URI string | {@type,@value} | null).
        let oracle = client
            .query::<serde_json::Value>(Some(spec), query.clone())
            .await?;
        let oracle_bindings = oracle.bindings.clone();
        let oracle_var_names = oracle.api_variable_names.clone();

        // (4) REPLICATE over real HTTP: raw `POST /api/pack/admin/<db>` (empty body =>
        // full pack). Body is `<40-byte ascii repository head><terminus-store pack>`.
        let packed = http
            .post(format!("{base}/api/pack/admin/{db}"))
            .basic_auth("admin", Some("root"))
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await?;
        anyhow::ensure!(
            packed.status().is_success(),
            "pack request failed: {} {}",
            packed.status(),
            packed.text().await.unwrap_or_default()
        );
        let bytes = packed.bytes().await?.to_vec();
        anyhow::ensure!(
            bytes.len() > 40,
            "pack response too short ({} bytes) to hold a head + pack",
            bytes.len()
        );

        Ok::<OraclePhase, anyhow::Error>(OraclePhase {
            oracle_bindings,
            oracle_var_names,
            // `/api/pack` returns `<40-byte ascii repository head><pack bytes>`;
            // strip the head (like http_replication_proof.rs) before import.
            pack: bytes[40..].to_vec(),
        })
        // `server` dropped here (child killed); the bytes + bindings are already ours.
        };
        match tokio::time::timeout(ORACLE_TIMEOUT, work).await {
            Ok(res) => res,
            Err(_) => Err(anyhow::anyhow!(
                "oracle_phase timed out after {}s (TerminusDB server likely wedged)",
                ORACLE_TIMEOUT.as_secs()
            )),
        }
    })?;

    drop(rt);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Pack import (fork store) — verbatim from `http_replication_proof.rs`.
// ---------------------------------------------------------------------------
fn import_pack_into(store: &SyncStore, pack: &[u8]) -> HashSet<[u32; 5]> {
    let parents: HashMap<[u32; 5], Option<[u32; 5]>> =
        pack_layer_parents(Cursor::new(pack)).unwrap_or_else(|e| panic!("pack is unreadable: {e}"));
    let all: Vec<[u32; 5]> = parents.keys().copied().collect();

    if store
        .import_layers(pack, Box::new(all.clone().into_iter()))
        .is_ok()
    {
        return all.into_iter().collect();
    }

    let mut available: HashSet<[u32; 5]> = HashSet::new();
    let mut failed: HashSet<[u32; 5]> = HashSet::new();
    loop {
        let mut changed = false;
        for &id in &all {
            if available.contains(&id) || failed.contains(&id) {
                continue;
            }
            if let Some(Some(parent)) = parents.get(&id) {
                if parents.contains_key(parent) && !available.contains(parent) {
                    if failed.contains(parent) {
                        failed.insert(id);
                        changed = true;
                    }
                    continue;
                }
            }
            match store.import_layers(pack, Box::new(std::iter::once(id))) {
                Ok(()) => {
                    available.insert(id);
                    changed = true;
                }
                Err(_) => {
                    failed.insert(id);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    available
}

/// Discover the committed instance-graph head layer inside the imported set: the
/// imported layer with the MOST visible triples that carries at least one triple
/// under the differential-test [`MARKER`] host (the instance-graph head sees all
/// ancestors). Returns its verified [`LayerCommitment`] (migrating the proof chain
/// if the replicated layer needs a format bump first).
fn discover_commitment(store: &SyncStore, imported: &HashSet<[u32; 5]>) -> Arc<LayerCommitment> {
    let mut best: Option<([u32; 5], usize)> = None;
    for &id in imported {
        let Some(layer) = store.get_layer_from_id(id).expect("open imported layer") else {
            continue;
        };
        let mut total = 0usize;
        let mut has_marker = false;
        for t in layer.triples() {
            total += 1;
            if !has_marker {
                if let Some(vt) = layer.id_triple_to_string(&t) {
                    if vt.subject.contains(MARKER) {
                        has_marker = true;
                    }
                }
            }
        }
        if has_marker {
            match &best {
                Some((_, best_total)) if *best_total >= total => {}
                _ => best = Some((id, total)),
            }
        }
    }

    let (instance_id, _) = best.expect(
        "the replicated pack contained no instance-graph layer carrying a seed triple \
         under the differential-test marker host",
    );

    let layer = store
        .get_layer_from_id(instance_id)
        .expect("re-open instance layer")
        .expect("instance layer present");
    match layer.proof_commitment() {
        Ok(c) => c,
        Err(_) => {
            store
                .migrate_proof_chain(instance_id)
                .expect("proof-chain migration of the replicated layer");
            store
                .get_layer_from_id(instance_id)
                .expect("re-open after migrate")
                .expect("layer present after migrate")
                .proof_commitment()
                .expect("commitment after migrate")
        }
    }
}

// ---------------------------------------------------------------------------
// The harness entry point.
// ---------------------------------------------------------------------------

static DB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Run one differential test: commit `seed`, answer `query` through BOTH the live
/// Prolog server (ORACLE) and our Rust prover over the replicated committed layer
/// (SYSTEM UNDER TEST), and return BOTH raw answers for [`assert_fidelity`].
///
/// PANICS on any infrastructure failure (server boot, HTTP, import, prove) — this
/// is test-only code and a broken harness must fail loudly. A query that our
/// compiler rejects is a PROGRAMMER error here (use [`assert_compiler_rejects`]
/// for the out-of-fragment boundary); `run_differential` is for supported queries.
pub fn run_differential(seed: &[SeedTriple], query: &Query) -> DifferentialResult {
    // Unique DB name per call keeps diagnostics readable; each `TerminusDBServer::test()`
    // is a fresh in-memory server, so isolation does not depend on the name.
    let n = DB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let db = format!("diff_{n}");

    // Compile up front so an accidentally-out-of-fragment query fails with a clear
    // message instead of a confusing oracle/replication error.
    let compiled: CompiledBgp = plan_bgp(query).unwrap_or_else(|e| {
        panic!("run_differential requires a SUPPORTED query; plan_bgp rejected it: {e:?}")
    });

    // Phase A (async): oracle answer + replicated pack.
    let phase = oracle_phase(&db, seed, query).expect("oracle/replication phase failed");

    // Phase B (sync): prove + resolve the SAME query over the replicated layer.
    let store = open_sync_memory_store();
    let imported = import_pack_into(&store, &phase.pack);
    assert!(
        !imported.is_empty(),
        "no layer from the replicated pack could be imported"
    );
    let commitment = discover_commitment(&store, &imported);
    let root = commitment.resolution_root();

    let proved = compiled
        .prove(&commitment)
        .expect("prove the compiled query over the replicated layer");

    let (rust_columns, rust_count) = if let Some(count_variable) = compiled.count_variable.clone() {
        // Count: the proof-authenticated `result_len` IS the count (no dictionary
        // resolution). `verify` binds it to the committed root.
        proved
            .verify(&compiled, &commitment, root)
            .expect("verify Count proof against the committed root");
        let (_name, count) = proved
            .count_binding(&compiled)
            .expect("count binding present for a Count query");
        (Vec::new(), Some((count_variable, count as i128)))
    } else {
        let columns = proved
            .verify_and_resolve_values(&compiled, &commitment, root)
            .expect("verify + resolve the resolved answer against the committed root");
        (columns, None)
    };

    DifferentialResult {
        oracle_bindings: phase.oracle_bindings,
        oracle_var_names: phase.oracle_var_names,
        rust_columns,
        rust_count,
        schema: compiled.relation.schema.clone(),
        projection: compiled.projection.clone(),
        rust_result_len: proved.result_len,
    }
}

/// The out-of-fragment half of fidelity: assert our compiler REJECTS `query`
/// (fails closed) rather than silently mis-executing what it cannot faithfully
/// reproduce. Needs no server.
pub fn assert_compiler_rejects(query: &Query) {
    match plan_bgp(query) {
        Ok(_) => panic!(
            "compiler ACCEPTED an out-of-fragment query it should reject closed: {query:?}"
        ),
        Err(_e) => { /* fail-closed, as required */ }
    }
}

/// The outcome of a PROVE-TIME reject probe: the oracle's answer (captured for the
/// record — a fail-closed boundary is sound UNDER-coverage, so we want to see what
/// TerminusDB actually returns) plus the prover's error, if any.
pub struct ProveProbe {
    /// The Prolog server's raw bindings for the query (what TerminusDB returns for the
    /// case our prover declines to prove).
    pub oracle_bindings: Vec<Json>,
    /// The server's `api:variable_names`.
    pub oracle_var_names: Vec<String>,
    /// `Some(error message)` if `prove` (or the following verify/resolve) failed —
    /// i.e. the prover fail-closed. `None` if the prover ACCEPTED and produced a result.
    pub prove_error: Option<String>,
}

/// Run the oracle + prove steps but CAPTURE the prover's error instead of panicking.
///
/// The query MUST compile (`plan_bgp` Ok) — this probe is for constructs the
/// variable-blind compiler ACCEPTS but the prover fail-closes at prove/verify time
/// (e.g. the D-fidelity strict-homogeneity guard over a datatype-heterogeneous numeric
/// column). Use [`assert_compiler_rejects`] for compile-time rejects.
pub fn probe_prove(seed: &[SeedTriple], query: &Query) -> ProveProbe {
    let n = DB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let db = format!("probe_{n}");

    let compiled: CompiledBgp = plan_bgp(query).unwrap_or_else(|e| {
        panic!("probe_prove requires a query that COMPILES (use assert_compiler_rejects for compile-time rejects); plan_bgp said: {e:?}")
    });

    let phase = oracle_phase(&db, seed, query).expect("oracle/replication phase failed");

    let store = open_sync_memory_store();
    let imported = import_pack_into(&store, &phase.pack);
    assert!(
        !imported.is_empty(),
        "no layer from the replicated pack could be imported"
    );
    let commitment = discover_commitment(&store, &imported);
    let root = commitment.resolution_root();

    // Capture any error from prove, or (if prove succeeds) from the verify/resolve step —
    // some fail-closed guards fire at verify time, not prove time.
    let prove_error = match compiled.prove(&commitment) {
        Err(e) => Some(e.to_string()),
        Ok(proved) => {
            if compiled.count_variable.is_some() {
                proved
                    .verify(&compiled, &commitment, root)
                    .err()
                    .map(|e| e.to_string())
            } else {
                proved
                    .verify_and_resolve_values(&compiled, &commitment, root)
                    .err()
                    .map(|e| e.to_string())
            }
        }
    };

    ProveProbe {
        oracle_bindings: phase.oracle_bindings,
        oracle_var_names: phase.oracle_var_names,
        prove_error,
    }
}

/// Assert that our prover FAIL-CLOSES at prove/verify time on `query` with an error
/// containing `expected_substring` — an intentional coverage boundary, NOT a bug. The
/// oracle's answer is captured and PRINTED (visible under `--nocapture`) so the exact
/// under-coverage is recorded (what TerminusDB returns that we decline to prove).
pub fn assert_prove_rejects(seed: &[SeedTriple], query: &Query, expected_substring: &str) {
    let probe = probe_prove(seed, query);
    eprintln!(
        "[assert_prove_rejects] oracle returned {} row(s) over vars {:?}: {:?}",
        probe.oracle_bindings.len(),
        probe.oracle_var_names,
        probe.oracle_bindings
    );
    match &probe.prove_error {
        Some(msg) => {
            eprintln!("[assert_prove_rejects] prover fail-closed: {msg}");
            assert!(
                msg.contains(expected_substring),
                "prover fail-closed, but the error {msg:?} does not contain the expected                  boundary marker {expected_substring:?}"
            );
        }
        None => panic!(
            "prover ACCEPTED a query it must fail-close on (expected a prove/verify reject              containing {expected_substring:?}); oracle bindings = {:?}",
            probe.oracle_bindings
        ),
    }
}

// ===========================================================================
// THE FIDELITY-CRITICAL COMPARATOR.
//
// Both answers are normalized to a canonical relation (Vec of rows; each row a
// Vec of `Cell`), then compared under the mode the query fixes. `Cell` is a
// LOGICAL value (an IRI, a string, an integer, a bool, ...), decoded from each
// engine's OWN representation — so we never depend on byte-identical encodings
// across the Prolog/Rust boundary.
// ===========================================================================

/// A canonical, representation-independent cell value that BOTH engines map into.
///
/// The whole point: a Prolog IRI and the Rust `CanonicalObject::Node` for the same
/// resource both become `Cell::Node(iri_bytes)`; a stored `xsd:unsignedInt` (Rust
/// `CanonicalObject::Value{UInt32,..}`) and Prolog's `{@type: xsd:unsignedInt,
/// @value: 55}` both become `Cell::Int(55)`; a proven `Count` (`GeneratedDecimalU64`)
/// and Prolog's count literal both become `Cell::Int(n)`. Decoding to the LOGICAL
/// value on each side avoids the store's order-preserving byte encodings entirely.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cell {
    /// A graph node / IRI, by its raw IRI bytes.
    Node(Vec<u8>),
    /// An `xsd:string`(-family) literal, by its UTF-8 lexical bytes.
    Str(Vec<u8>),
    /// Any INTEGER-valued literal or generated (count/eval) scalar, by its value.
    /// An integral-valued `xsd:decimal` also normalizes here (so a whole decimal and
    /// the same integer compare equal, matching WOQL numeric value semantics).
    Int(i128),
    /// A NON-integral `xsd:decimal`, by its canonical trimmed decimal string (e.g.
    /// `"3.5"`, `"-0.000001"`). Both engines normalize to the SAME string via
    /// [`normalize_decimal`], so the store's BCD lexical and Prolog's rendering agree.
    Dec(String),
    /// An `xsd:boolean` literal.
    Bool(bool),
    /// A value neither engine's canonicalizer resolved to a logical form above.
    /// Carries a stable `(datatype-tag, repr)` so an EQUAL pair still matches and
    /// an UNEQUAL pair still diverges — but see the `// REVIEW:` note on
    /// `cell_from_prolog`: cross-engine `Unknown` matching is best-effort and NOT
    /// relied on by the phase-1 slice.
    Unknown { tag: String, repr: Vec<u8> },
    /// An absent binding (unbound / Optional `null`).
    Null,
}

/// Decode the local name of an xsd type tag: the part after the last `:` or `#`
/// (`"xsd:unsignedInt"` -> `"unsignedInt"`, `"http://.../XMLSchema#string"` ->
/// `"string"`), lowercased for robust matching.
fn xsd_local_name(ty: &str) -> String {
    let after = ty.rsplit(|c| c == ':' || c == '#').next().unwrap_or(ty);
    after.to_ascii_lowercase()
}

/// The set of xsd local names we treat as integer-valued (mapped to `Cell::Int`).
fn is_integer_type(local: &str) -> bool {
    matches!(
        local,
        "integer"
            | "int"
            | "long"
            | "short"
            | "byte"
            | "unsignedint"
            | "unsignedlong"
            | "unsignedshort"
            | "unsignedbyte"
            | "nonnegativeinteger"
            | "positiveinteger"
            | "negativeinteger"
            | "nonpositiveinteger"
    )
}

/// Number-carrying xsd local names. `decimal`/`double`/`float` are included so an
/// INTEGRAL-valued count returned as `xsd:decimal` (a common TerminusDB Count
/// shape) still canonicalizes to `Cell::Int`. A genuinely fractional value yields
/// `None` from [`value_as_int`] and falls through to `Cell::Unknown` — see the
/// REVIEW note: real fractional decimal/float COLUMNS are phase-2 territory and
/// need their own canonical form on BOTH engines.
fn is_number_type(local: &str) -> bool {
    is_integer_type(local) || matches!(local, "decimal" | "double" | "float")
}

/// Interpret a JSON `@value` (or bare binding) as an integer, if it is exactly
/// integral. Handles JSON numbers (incl. `arbitrary_precision`), integral floats,
/// and arbitrary-precision integers delivered as strings.
fn value_as_int(v: &Json) -> Option<i128> {
    match v {
        Json::Number(n) => n
            .as_i64()
            .map(|x| x as i128)
            .or_else(|| n.as_u64().map(|x| x as i128))
            .or_else(|| {
                // arbitrary_precision keeps the token as a string; try that first
                // so a big integer is not lost to an f64 round-trip.
                n.to_string()
                    .parse::<i128>()
                    .ok()
                    .or_else(|| n.as_f64().and_then(float_as_int))
            }),
        Json::String(s) => s.parse::<i128>().ok(),
        _ => None,
    }
}

fn float_as_int(f: f64) -> Option<i128> {
    if f.is_finite() && f.fract() == 0.0 {
        Some(f as i128)
    } else {
        None
    }
}

/// Canonicalize a decimal LEXICAL string to a stable trimmed FIXED-POINT form so the
/// store's BCD rendering and Prolog's rendering of the SAME value agree.
///
/// IMPORTANT: TerminusDB's WOQL HTTP API renders an `xsd:decimal` binding as a JSON
/// NUMBER — and may use SCIENTIFIC/EXPONENT form (e.g. `-1e-6` for `-0.000001`), NOT the
/// canonical fixed-point lexical the store commits. So the oracle-side decimal path MUST
/// canonicalize exponent form. This is done with EXACT integer/string arithmetic (shift
/// the decimal point by the exponent) — never via `f64`, which would lose precision for
/// high-precision decimals.
///
/// Normalization: parse an optional sign, an optional `[eE][+-]?digits` exponent, and a
/// `int.frac` mantissa; shift the point by the exponent; then strip leading integer zeros
/// (keep one), strip trailing fractional zeros, drop an empty fraction, and normalize a
/// signed zero to `"0"`. Returns `None` for any non-decimal (multiple points, garbage,
/// or an absurd exponent) form.
fn normalize_decimal(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    // Split off an optional exponent (`e`/`E`); the exponent itself parses its own sign.
    let (mantissa, exp) = match rest.split_once(|c| c == 'e' || c == 'E') {
        Some((m, e)) => (m, e.parse::<i64>().ok()?),
        None => (rest, 0i64),
    };
    // Guard an absurd exponent so a pathological input cannot blow up the shifted string.
    if exp.abs() > 1000 {
        return None;
    }
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => (i, f),
        None => (mantissa, ""),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return None; // "", ".", "e5" are not numbers
    }
    // Mantissa digits with an implied point after `int_part`; value = digits * 10^shift.
    let digits = format!("{int_part}{frac_part}");
    let shift = exp - frac_part.len() as i64;
    let (int_str, frac_str) = if shift >= 0 {
        (format!("{digits}{}", "0".repeat(shift as usize)), String::new())
    } else {
        let k = (-shift) as usize;
        if digits.len() > k {
            let cut = digits.len() - k;
            (digits[..cut].to_string(), digits[cut..].to_string())
        } else {
            ("0".to_string(), format!("{}{digits}", "0".repeat(k - digits.len())))
        }
    };
    let mut int_norm = int_str.trim_start_matches('0').to_string();
    if int_norm.is_empty() {
        int_norm = "0".to_string();
    }
    let frac_norm = frac_str.trim_end_matches('0').to_string();
    let is_zero = int_norm == "0" && frac_norm.is_empty();
    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(&int_norm);
    if !frac_norm.is_empty() {
        out.push('.');
        out.push_str(&frac_norm);
    }
    Some(out)
}

/// Map a decimal lexical to a numeric [`Cell`]: an INTEGRAL value (no fractional part)
/// normalizes to `Cell::Int` (so a whole `xsd:decimal` and the same integer compare
/// equal — WOQL numeric value semantics); a fractional value becomes `Cell::Dec` on its
/// canonical trimmed string. `None` for a non-decimal string.
fn decimal_string_to_cell(s: &str) -> Option<Cell> {
    let norm = normalize_decimal(s)?;
    if norm.contains('.') {
        Some(Cell::Dec(norm))
    } else {
        match norm.parse::<i128>() {
            Ok(i) => Some(Cell::Int(i)),
            // A whole number too large for i128 keeps its exact decimal string.
            Err(_) => Some(Cell::Dec(norm)),
        }
    }
}

/// Extract a decimal lexical string from a JSON `@value` (a number token — preserved
/// losslessly under serde_json `arbitrary_precision` — or a string), for numeric
/// canonicalization.
fn json_numeric_lexical(v: &Json) -> Option<String> {
    match v {
        Json::Number(n) => Some(n.to_string()),
        Json::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Canonicalize ONE Prolog binding value into a [`Cell`].
///
/// Prolog result shapes (see `terminusdb-client`'s `WOQLResult` fixture):
///   * a graph node / URI is a BARE JSON string -> `Cell::Node`;
///   * a typed literal is `{"@type": "xsd:...", "@value": v}`;
///   * an unbound variable is absent / JSON `null` -> `Cell::Null`.
///
/// REVIEW (verify against a live run): (1) that URIs really arrive as bare strings
/// and literals always as `{@type,@value}` (assumed here); (2) the exact `@type`
/// tag a stored `xsd:unsignedInt` / `xsd:string` / `xsd:boolean` comes back as,
/// and what tag a `Count` result carries (we accept any integer-family tag AND a
/// bare number); (3) decimal/float/date land in `Unknown` for now (phase 2).
pub fn cell_from_prolog(value: Option<&Json>) -> Cell {
    let value = match value {
        None => return Cell::Null,
        Some(Json::Null) => return Cell::Null,
        Some(v) => v,
    };
    match value {
        // A bare string binding is a graph node / URI.
        Json::String(s) => Cell::Node(s.clone().into_bytes()),
        // A bare number can appear for generated scalars (e.g. a Count) depending on
        // server version; canonicalize by numeric value (integral -> Int, else Dec).
        Json::Number(n) => decimal_string_to_cell(&n.to_string()).unwrap_or_else(|| Cell::Unknown {
            tag: "num".to_string(),
            repr: value.to_string().into_bytes(),
        }),
        Json::Bool(b) => Cell::Bool(*b),
        Json::Object(map) => {
            let ty = map.get("@type").and_then(Json::as_str).unwrap_or("");
            let local = xsd_local_name(ty);
            let val = match map.get("@value") {
                Some(v) => v,
                None => {
                    return Cell::Unknown {
                        tag: local,
                        repr: Vec::new(),
                    }
                }
            };
            if local == "string" {
                if let Json::String(s) = val {
                    return Cell::Str(s.clone().into_bytes());
                }
            }
            if local == "boolean" {
                return match val {
                    Json::Bool(b) => Cell::Bool(*b),
                    Json::String(s) => Cell::Bool(s == "true"),
                    other => Cell::Unknown {
                        tag: local,
                        repr: other.to_string().into_bytes(),
                    },
                };
            }
            // Numeric literal: integer / xsd:integer / xsd:decimal (integral -> Int,
            // fractional -> Dec). Covers the Count result (integral decimal) and the
            // adversarial decimal-rendering / mixed-numeric columns.
            if is_number_type(&local) || matches!(val, Json::Number(_)) {
                if let Some(cell) = json_numeric_lexical(val).and_then(|s| decimal_string_to_cell(&s))
                {
                    return cell;
                }
            }
            // REVIEW: xsd:float/double, langString, date-family, and other typed
            // literals land here — a stable (tag, repr) so an EQUAL pair still matches.
            // Floats appear only in reject cases (never resolved); langString/date are
            // not yet seedable via WOQL `Value::Data` (see the matrix report).
            Cell::Unknown {
                tag: local,
                repr: val.to_string().into_bytes(),
            }
        }
        // Arrays (list bindings) are out of the phase-1 fragment.
        other => Cell::Unknown {
            tag: "json".to_string(),
            repr: other.to_string().into_bytes(),
        },
    }
}

/// Canonicalize ONE resolved Rust value ([`CanonicalResultValue`]) into a [`Cell`].
///
/// The store's typed-literal `lexical` bytes are its ORDER-PRESERVING encoding, so
/// integers are decoded back to their logical value (unsigned = big-endian; signed
/// = big-endian with the sign bit flipped, per `tdb-succinct`'s `ToLexical`). Node
/// IRIs and `xsd:string` lexicals are already their logical bytes.
pub fn cell_from_rust(value: &CanonicalResultValue) -> Cell {
    match value {
        CanonicalResultValue::Null => Cell::Null,
        CanonicalResultValue::GeneratedDecimalU64(n) => Cell::Int(*n as i128),
        CanonicalResultValue::GeneratedDecimalI64(n) => Cell::Int(*n as i128),
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => Cell::Node(bytes.clone()),
        CanonicalResultValue::Object(CanonicalObject::Value { datatype, lexical }) => {
            cell_from_typed_lexical(*datatype, lexical)
        }
    }
}

fn cell_from_typed_lexical(datatype: CanonicalDatatype, lexical: &[u8]) -> Cell {
    use CanonicalDatatype::*;
    match datatype {
        String => Cell::Str(lexical.to_vec()),
        Boolean => Cell::Bool(lexical.first().copied().unwrap_or(0) != 0),
        UInt8 | UInt16 | UInt32 | UInt64 => Cell::Int(decode_unsigned_be(lexical)),
        Int8 | Int16 | Int32 | Int64 => Cell::Int(decode_signed_orderpreserving(lexical)),
        // xsd:decimal: decode the committed BCD storage lexical via the store's OWN
        // decoder (`storage_to_decimal`), then normalize to the same trimmed form the
        // Prolog side produces. Integral -> Int, fractional -> Dec.
        Decimal => {
            let mut buf: &[u8] = lexical;
            let lexical_str = storage_to_decimal(&mut buf);
            decimal_string_to_cell(&lexical_str).unwrap_or_else(|| Cell::Unknown {
                tag: "Decimal".to_string(),
                repr: lexical.to_vec(),
            })
        }
        // REVIEW: bigint (huge xsd:integer) / float / date-family are not exercised in
        // the phase-2 fidelity cases (float appears only in rejects); keep raw so an
        // equal pair still matches and an unequal one still diverges.
        other => Cell::Unknown {
            tag: format!("{other:?}"),
            repr: lexical.to_vec(),
        },
    }
}

/// Decode a big-endian UNSIGNED integer lexical (`u8/u16/u32/u64` `to_lexical`).
fn decode_unsigned_be(lexical: &[u8]) -> i128 {
    let mut acc: u128 = 0;
    for &b in lexical {
        acc = (acc << 8) | b as u128;
    }
    acc as i128
}

/// Decode a signed integer lexical in `tdb-succinct`'s order-preserving encoding:
/// big-endian of width `w` with the top (sign) bit XOR-flipped. We read the stored
/// unsigned value, flip the sign bit back, then sign-extend from `w` bytes.
fn decode_signed_orderpreserving(lexical: &[u8]) -> i128 {
    let w = lexical.len();
    if w == 0 || w > 16 {
        // Out of the expected 1/2/4/8-byte signed widths; fall back to unsigned view.
        return decode_unsigned_be(lexical);
    }
    let mut stored: u128 = 0;
    for &b in lexical {
        stored = (stored << 8) | b as u128;
    }
    let bits = (w as u32) * 8;
    let sign_bit = 1u128 << (bits - 1);
    let flipped = stored ^ sign_bit; // undo the ToLexical sign flip
    if bits < 128 && flipped >= sign_bit {
        // Two's-complement negative within `w` bytes.
        (flipped as i128) - (1i128 << bits)
    } else {
        flipped as i128
    }
}

/// How two answers must be compared, fixed by the query shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareMode {
    /// Bag equality (row order irrelevant) — the default for a plain BGP / filter /
    /// join / count.
    Multiset,
    /// Exact positional equality — asserted ONLY where the query fixes order
    /// (`OrderBy`).
    Ordered,
    /// Set equality (duplicates collapsed) — a `Distinct` query.
    Set,
}

/// Derive the comparison mode from the query the caller authored: order is asserted
/// only under an `OrderBy`; set semantics only under a `Distinct`; everything else
/// is a multiset. Proof-transparent wrappers (`Select`/`Using`/`Pin`/`From`/
/// `Limit`/`Start`/`Immediately`) are peeled; the first semantic operator decides.
pub fn mode_of_query(query: &Query) -> CompareMode {
    let mut cur = query;
    loop {
        cur = match cur {
            Query::Select(s) => s.query.as_ref(),
            Query::Using(u) => u.query.as_ref(),
            Query::Pin(p) => p.query.as_ref(),
            Query::Immediately(i) => i.query.as_ref(),
            Query::From(f) => f.query.as_ref(),
            Query::Limit(l) => l.query.as_ref(),
            Query::Start(s) => s.query.as_ref(),
            Query::OrderBy(_) => return CompareMode::Ordered,
            Query::Distinct(_) => return CompareMode::Set,
            // A Count collapses to a single count row; bag comparison suffices.
            Query::Count(_) => return CompareMode::Multiset,
            _ => return CompareMode::Multiset,
        };
    }
}

/// Compare two canonical relations under `mode`. Pure and total (never panics) so
/// the comparator can be unit-tested directly. Returns `Ok(())` when the answers
/// agree, `Err(reason)` (a human-readable divergence) when they do not.
pub fn compare_relations(
    oracle: &[Vec<Cell>],
    rust: &[Vec<Cell>],
    mode: CompareMode,
) -> Result<(), String> {
    match mode {
        CompareMode::Ordered => {
            if oracle.len() != rust.len() {
                return Err(format!(
                    "row count differs: oracle {} vs rust {} (ordered)\n oracle={oracle:?}\n rust={rust:?}",
                    oracle.len(),
                    rust.len()
                ));
            }
            for (i, (o, r)) in oracle.iter().zip(rust.iter()).enumerate() {
                if o != r {
                    return Err(format!(
                        "ordered row {i} differs: oracle {o:?} vs rust {r:?}"
                    ));
                }
            }
            Ok(())
        }
        CompareMode::Multiset => {
            let mut o = oracle.to_vec();
            let mut r = rust.to_vec();
            o.sort();
            r.sort();
            if o == r {
                Ok(())
            } else {
                Err(format!(
                    "multiset mismatch:\n oracle(sorted)={o:?}\n rust(sorted)={r:?}"
                ))
            }
        }
        CompareMode::Set => {
            let mut o = oracle.to_vec();
            let mut r = rust.to_vec();
            o.sort();
            o.dedup();
            r.sort();
            r.dedup();
            if o == r {
                Ok(())
            } else {
                Err(format!(
                    "set mismatch:\n oracle(set)={o:?}\n rust(set)={r:?}"
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Assembling the two relations from a `DifferentialResult`, then comparing.
// ---------------------------------------------------------------------------

/// The output column NAMES for the answer, in the order both relations align by:
/// the count variable for a `Count`, else the `Select`-projected (or full) schema.
fn output_vars(result: &DifferentialResult) -> Vec<String> {
    if let Some((var, _)) = &result.rust_count {
        return vec![var.clone()];
    }
    match &result.projection {
        Some(indices) => indices.iter().map(|&i| result.schema[i].clone()).collect(),
        None => result.schema.clone(),
    }
}

/// The Rust answer as canonical ROWS (row-major), projected to the output columns.
fn rust_rows(result: &DifferentialResult) -> Vec<Vec<Cell>> {
    if let Some((_, count)) = &result.rust_count {
        return vec![vec![Cell::Int(*count)]];
    }
    let indices: Vec<usize> = match &result.projection {
        Some(p) => p.clone(),
        None => (0..result.schema.len()).collect(),
    };
    // Row count comes from the authoritative proof-authenticated `result_len`, not a
    // column length: a zero-output-column existence query (fully-ground `Triple`) has
    // NO columns to measure, yet still yields `result_len` empty-tuple rows. When there
    // ARE columns the two agree by construction (each column has `result_len` entries).
    let nrows = if result.rust_columns.is_empty() {
        result.rust_result_len
    } else {
        result.rust_columns[0].len()
    };
    (0..nrows)
        .map(|r| {
            indices
                .iter()
                .map(|&c| cell_from_rust(&result.rust_columns[c][r]))
                .collect()
        })
        .collect()
}

/// The oracle answer as canonical ROWS (row-major), aligned to `vars` by name.
fn oracle_rows(result: &DifferentialResult, vars: &[String]) -> Vec<Vec<Cell>> {
    result
        .oracle_bindings
        .iter()
        .map(|binding| {
            vars.iter()
                .map(|v| cell_from_prolog(binding.get(v.as_str())))
                .collect()
        })
        .collect()
}

/// THE fidelity assertion: canonicalize BOTH engines' raw answers to a relation
/// aligned by output-variable name, and assert they agree under the mode the query
/// fixes (multiset by default; ordered under `OrderBy`; set under `Distinct`).
/// PANICS with a diagnostic on divergence.
/// Non-panicking core of [`assert_fidelity`]: canonicalize BOTH engines' answers and
/// compare under the mode the query fixes. Returns `Err(diagnostic)` on divergence, so
/// the fuzzer can attach a reproducing seed/dataset/query to the panic.
pub fn check_fidelity(result: &DifferentialResult, query: &Query) -> Result<(), String> {
    let vars = output_vars(result);
    let rust = rust_rows(result);
    let oracle = oracle_rows(result, &vars);
    let mode = mode_of_query(query);
    compare_relations(&oracle, &rust, mode).map_err(|reason| {
        format!(
            "FIDELITY DIVERGENCE ({mode:?}) on output vars {vars:?}: {reason} | oracle var names = {:?} | raw oracle bindings = {:?}",
            result.oracle_var_names, result.oracle_bindings
        )
    })
}

// ===========================================================================
// SHARED-SERVER harness (phase 3 fuzzer). Booting a `TerminusDBServer` costs
// seconds, so the generative fuzzer boots ONE server and reuses it across every
// iteration; each iteration only creates a fresh schemaless db, commits, queries,
// and packs. The self-booting `run_differential` above is UNCHANGED.
// ===========================================================================

/// One long-lived embedded server + client + runtime, reused across fuzz iterations.
/// Holds the `TerminusDBServer` so the child process stays alive for the whole run.
pub struct SharedServer {
    rt: tokio::runtime::Runtime,
    _server: TerminusDBServer,
    client: TerminusDBHttpClient,
    base: String,
    http: reqwest::Client,
}

impl SharedServer {
    /// Boot the embedded proof-capable server ONCE.
    pub fn boot() -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let (server, client, base, http) = rt.block_on(async {
            let server = TerminusDBServer::test().await?;
            let port = server.port();
            let client = server.client().await?;
            let base = format!("http://localhost:{port}");
            let http = reqwest::Client::new();
            Ok::<_, anyhow::Error>((server, client, base, http))
        })?;
        Ok(Self { rt, _server: server, client, base, http })
    }

    /// Per-iteration async work on the SHARED server: create a fresh schemaless `db`,
    /// commit `seed`, run the ORACLE `query`, and pull that db's pack. Returns
    /// `(oracle_bindings, oracle_var_names, pack_bytes_without_head)`.
    fn oracle_once(
        &self,
        db: &str,
        seed: &[SeedTriple],
        query: &Query,
    ) -> anyhow::Result<(Vec<Json>, Vec<String>, Vec<u8>)> {
        let base = &self.base;
        let http = &self.http;
        let client = &self.client;
        self.rt.block_on(async {
            let work = async {
            let created = http
                .post(format!("{base}/api/db/admin/{db}"))
                .basic_auth("admin", Some("root"))
                .header("Content-Type", "application/json")
                .body(
                    serde_json::json!({
                        "label": db,
                        "comment": "proof-of-woql differential fuzz",
                        "schema": false,
                        "public": true
                    })
                    .to_string(),
                )
                .send()
                .await?;
            anyhow::ensure!(
                created.status().is_success() || created.status().as_u16() == 400,
                "database creation failed: {} {}",
                created.status(),
                created.text().await.unwrap_or_default()
            );

            let spec = BranchSpec::with_branch(db, "main");
            let insert = Query::And(And {
                and: seed
                    .iter()
                    .cloned()
                    .map(|t| {
                        Query::AddTriple(AddTriple {
                            subject: NodeValue::Node(t.subject),
                            predicate: NodeValue::Node(t.predicate),
                            object: t.object.into_woql(),
                            graph: None,
                        })
                    })
                    .collect(),
            });
            let _committed = client
                .query_mut::<serde_json::Value>(spec.clone(), insert, "fuzz", "seed commit")
                .await?;

            let oracle = client
                .query::<serde_json::Value>(Some(spec), query.clone())
                .await?;
            let oracle_bindings = oracle.bindings.clone();
            let oracle_var_names = oracle.api_variable_names.clone();

            let packed = http
                .post(format!("{base}/api/pack/admin/{db}"))
                .basic_auth("admin", Some("root"))
                .header("Content-Type", "application/json")
                .body("{}")
                .send()
                .await?;
            anyhow::ensure!(
                packed.status().is_success(),
                "pack request failed: {} {}",
                packed.status(),
                packed.text().await.unwrap_or_default()
            );
            let bytes = packed.bytes().await?.to_vec();
            anyhow::ensure!(bytes.len() > 40, "pack response too short ({} bytes)", bytes.len());
            Ok::<_, anyhow::Error>((oracle_bindings, oracle_var_names, bytes[40..].to_vec()))
            };
            match tokio::time::timeout(ORACLE_TIMEOUT, work).await {
                Ok(res) => res,
                Err(_) => Err(anyhow::anyhow!(
                    "oracle_once timed out after {}s (TerminusDB server likely wedged); skipping iteration",
                    ORACLE_TIMEOUT.as_secs()
                )),
            }
        })
    }
}

/// The per-iteration outcome the fuzzer classifies.
pub enum FuzzOutcome {
    /// `plan_bgp` refused the query (coverage boundary, not a failure).
    CompileRejected,
    /// Compiled, but the prover/verifier fail-closed (carries the error for the log).
    ProveRejected(String),
    /// Compiled AND proved: both engines' raw answers, ready for `check_fidelity`.
    Proved(DifferentialResult),
}

/// Fuzzer per-iteration driver on the SHARED server. Compiles first (`CompileRejected`
/// on Err, with NO server round-trip), then runs the oracle + prove/resolve and
/// classifies. Infrastructure/oracle errors surface as `Err` (counted+logged by the
/// fuzzer, distinct from a fidelity mismatch).
pub fn run_differential_on(
    shared: &SharedServer,
    db: &str,
    seed: &[SeedTriple],
    query: &Query,
) -> anyhow::Result<FuzzOutcome> {
    let compiled: CompiledBgp = match plan_bgp(query) {
        Ok(c) => c,
        Err(_) => return Ok(FuzzOutcome::CompileRejected),
    };

    let (oracle_bindings, oracle_var_names, pack) = shared.oracle_once(db, seed, query)?;

    let store = open_sync_memory_store();
    let imported = import_pack_into(&store, &pack);
    anyhow::ensure!(!imported.is_empty(), "no layer imported from replicated pack");
    let commitment = discover_commitment(&store, &imported);
    let root = commitment.resolution_root();

    let (rust_columns, rust_count, rust_result_len) = match compiled.prove(&commitment) {
        Err(e) => return Ok(FuzzOutcome::ProveRejected(e.to_string())),
        Ok(proved) => {
            // The proof-authenticated row count — captured BEFORE resolving columns so
            // a zero-output-column (zero-arity existence) result still carries its true
            // cardinality (see `DifferentialResult::rust_result_len`).
            let rust_result_len = proved.result_len;
            if let Some(count_variable) = compiled.count_variable.clone() {
                match proved.verify(&compiled, &commitment, root) {
                    Err(e) => return Ok(FuzzOutcome::ProveRejected(e.to_string())),
                    Ok(()) => {
                        let (_name, count) = proved
                            .count_binding(&compiled)
                            .expect("count binding present for a Count query");
                        (Vec::new(), Some((count_variable, count as i128)), rust_result_len)
                    }
                }
            } else {
                match proved.verify_and_resolve_values(&compiled, &commitment, root) {
                    Err(e) => return Ok(FuzzOutcome::ProveRejected(e.to_string())),
                    Ok(columns) => (columns, None, rust_result_len),
                }
            }
        }
    };

    Ok(FuzzOutcome::Proved(DifferentialResult {
        oracle_bindings,
        oracle_var_names,
        rust_columns,
        rust_count,
        schema: compiled.relation.schema.clone(),
        projection: compiled.projection.clone(),
        rust_result_len,
    }))
}

pub fn assert_fidelity(result: &DifferentialResult, query: &Query) {
    let vars = output_vars(result);
    let rust = rust_rows(result);
    let oracle = oracle_rows(result, &vars);
    let mode = mode_of_query(query);
    if let Err(reason) = compare_relations(&oracle, &rust, mode) {
        panic!(
            "FIDELITY DIVERGENCE ({mode:?}) on output vars {vars:?}:\n{reason}\n\
             oracle var names = {:?}\n raw oracle bindings = {:?}",
            result.oracle_var_names, result.oracle_bindings
        );
    }
}

// ===========================================================================
// Comparator unit tests — SERVER-FREE. These guard the fidelity-critical
// canonicalizers + comparator: a bug here makes the whole suite worthless, so we
// assert it ACCEPTS matching answers and REJECTS mismatched ones (value diff,
// node-vs-string kind confusion, and a reordering where order matters).
//
// NOT gated behind `#[cfg(test)]`: this module is only ever compiled as part of an
// integration test binary, and dropping the gate guarantees the `#[test]`s run
// regardless of how `cfg(test)` resolves for the containing test crate.
// ===========================================================================
mod comparator_unit_tests {
    use super::*;
    use serde_json::json;
    use tdb_succinct::TdbDataType;

    // --- Rust-side value constructors (mirroring how the store commits) ---------
    fn rust_node(iri: &str) -> CanonicalResultValue {
        CanonicalResultValue::Object(CanonicalObject::Node(iri.as_bytes().to_vec()))
    }
    fn rust_uint(v: u32) -> CanonicalResultValue {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::UInt32,
            lexical: u32::make_entry(&v).to_bytes().to_vec(),
        })
    }
    fn rust_string(s: &str) -> CanonicalResultValue {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: String::make_entry(&s.to_string()).to_bytes().to_vec(),
        })
    }
    fn rust_bool(b: bool) -> CanonicalResultValue {
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::Boolean,
            lexical: bool::make_entry(&b).to_bytes().to_vec(),
        })
    }
    fn rust_decimal(lexical: &str) -> CanonicalResultValue {
        // Commit exactly as the store does: build the tdb-succinct Decimal typed entry
        // and take its BCD storage bytes.
        let d = tdb_succinct::Decimal::new(lexical.to_string()).expect("valid decimal");
        CanonicalResultValue::Object(CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical: tdb_succinct::Decimal::make_entry(&d).to_bytes().to_vec(),
        })
    }

    // --- Value canonicalization: same logical value, both engines -> same Cell --
    #[test]
    fn node_iri_matches_across_engines() {
        let iri = "http://d.terminusdb.test/a";
        assert_eq!(cell_from_rust(&rust_node(iri)), Cell::Node(iri.as_bytes().to_vec()));
        assert_eq!(cell_from_prolog(Some(&json!(iri))), Cell::Node(iri.as_bytes().to_vec()));
        assert_eq!(cell_from_rust(&rust_node(iri)), cell_from_prolog(Some(&json!(iri))));
    }

    #[test]
    fn unsigned_int_matches_across_engines_despite_encoding() {
        // Rust holds the order-preserving big-endian lexical; Prolog holds the
        // decimal value. Both must canonicalize to the SAME integer.
        let rust = cell_from_rust(&rust_uint(55));
        let prolog = cell_from_prolog(Some(&json!({"@type": "xsd:unsignedInt", "@value": 55})));
        assert_eq!(rust, Cell::Int(55));
        assert_eq!(prolog, Cell::Int(55));
        assert_eq!(rust, prolog);
    }

    #[test]
    fn string_literal_matches_across_engines() {
        let rust = cell_from_rust(&rust_string("hello"));
        let prolog = cell_from_prolog(Some(&json!({"@type": "xsd:string", "@value": "hello"})));
        assert_eq!(rust, Cell::Str(b"hello".to_vec()));
        assert_eq!(rust, prolog);
    }

    #[test]
    fn boolean_literal_matches_across_engines() {
        assert_eq!(cell_from_rust(&rust_bool(true)), Cell::Bool(true));
        assert_eq!(
            cell_from_prolog(Some(&json!({"@type": "xsd:boolean", "@value": true}))),
            Cell::Bool(true)
        );
        assert_eq!(cell_from_rust(&rust_bool(false)), Cell::Bool(false));
    }

    #[test]
    fn generated_count_matches_prolog_count_literal() {
        // A proven Count is a generated scalar on the Rust side; Prolog returns a
        // typed decimal/integer literal. Both -> the same integer.
        assert_eq!(cell_from_rust(&CanonicalResultValue::GeneratedDecimalU64(3)), Cell::Int(3));
        assert_eq!(
            cell_from_prolog(Some(&json!({"@type": "xsd:decimal", "@value": 3}))),
            Cell::Int(3)
        );
        assert_eq!(cell_from_prolog(Some(&json!(3))), Cell::Int(3));
    }

    #[test]
    fn signed_int_roundtrips_through_orderpreserving_decode() {
        // Guard the sign-bit-flip decode against tdb-succinct's ToLexical.
        for v in [-128i32, -1, 0, 1, 127, 30_000, -30_000] {
            let lex = i32::make_entry(&v).to_bytes().to_vec();
            let cell = cell_from_rust(&CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::Int32,
                lexical: lex,
            }));
            assert_eq!(cell, Cell::Int(v as i128), "signed decode of {v}");
        }
    }

    // --- xsd:decimal canonicalization (adversarial datatype) --------------------
    #[test]
    fn fractional_decimal_matches_across_engines() {
        for lit in ["3.5", "-0.000001", "0.25", "123.456", "-3.14285714285714285714"] {
            let rust = cell_from_rust(&rust_decimal(lit));
            // Prolog may render the @value as a JSON number OR a string; both must agree.
            // Build the number arm by parsing real JSON so the numeric token is preserved
            // losslessly (serde_json arbitrary_precision).
            let num_json: Json =
                serde_json::from_str(&format!(r#"{{"@type":"xsd:decimal","@value":{lit}}}"#))
                    .expect("valid json");
            let p_num = cell_from_prolog(Some(&num_json));
            let p_str = cell_from_prolog(Some(&json!({"@type": "xsd:decimal", "@value": lit})));
            assert_eq!(rust, Cell::Dec(lit.to_string()), "rust decode {lit}");
            assert_eq!(rust, p_num, "rust vs prolog-number {lit}");
            assert_eq!(rust, p_str, "rust vs prolog-string {lit}");
        }
    }

    #[test]
    fn scientific_notation_decimal_matches_fixed_point_across_engines() {
        // TerminusDB's HTTP API can serialize an xsd:decimal as a JSON number in EXPONENT
        // form (e.g. -1e-6). The oracle path must canonicalize it to the same fixed-point
        // lexical the store commits (-0.000001) — WITHOUT an f64 round-trip.
        let rust = cell_from_rust(&rust_decimal("-0.000001"));
        assert_eq!(rust, Cell::Dec("-0.000001".to_string()));
        // Number token in exponent form (parsed as real JSON, arbitrary_precision preserved).
        let num_json: Json =
            serde_json::from_str(r#"{"@type":"xsd:decimal","@value":-1e-6}"#).expect("json");
        assert_eq!(cell_from_prolog(Some(&num_json)), rust, "exponent number arm");
        // String token in exponent form.
        assert_eq!(
            cell_from_prolog(Some(&json!({"@type": "xsd:decimal", "@value": "-1e-6"}))),
            rust,
            "exponent string arm"
        );
        // A genuinely different decimal (in exponent form) must NOT match.
        let other: Json =
            serde_json::from_str(r#"{"@type":"xsd:decimal","@value":-2e-6}"#).expect("json");
        assert_ne!(cell_from_prolog(Some(&other)), rust);
    }

    #[test]
    fn whole_decimal_and_integer_collapse_to_same_int() {
        // A whole xsd:decimal and the same integer are the SAME logical number in WOQL,
        // so both engines must land on Cell::Int (not two different tokens).
        assert_eq!(cell_from_rust(&rust_decimal("5")), Cell::Int(5));
        assert_eq!(cell_from_rust(&rust_uint(5)), Cell::Int(5));
        assert_eq!(
            cell_from_prolog(Some(&json!({"@type": "xsd:decimal", "@value": 5}))),
            Cell::Int(5)
        );
        assert_eq!(
            cell_from_prolog(Some(&json!({"@type": "xsd:decimal", "@value": "5.0"}))),
            Cell::Int(5)
        );
    }

    #[test]
    fn decimal_mismatch_and_kind_confusion_rejected() {
        // Different decimal values must NOT match.
        assert_ne!(cell_from_rust(&rust_decimal("3.5")), cell_from_rust(&rust_decimal("3.6")));
        // A fractional decimal must not collapse into an integer.
        assert_ne!(cell_from_rust(&rust_decimal("3.5")), Cell::Int(3));
        assert_ne!(cell_from_rust(&rust_decimal("3.5")), Cell::Int(4));
        // A decimal is not a string.
        assert_ne!(
            cell_from_rust(&rust_decimal("3.5")),
            cell_from_prolog(Some(&json!({"@type": "xsd:string", "@value": "3.5"})))
        );
    }

    #[test]
    fn normalize_decimal_canonicalizes_and_rejects_nondecimal() {
        assert_eq!(normalize_decimal("3.50").as_deref(), Some("3.5"));
        assert_eq!(normalize_decimal("05").as_deref(), Some("5"));
        assert_eq!(normalize_decimal("-0").as_deref(), Some("0"));
        assert_eq!(normalize_decimal("-0.0").as_deref(), Some("0"));
        assert_eq!(normalize_decimal(".5").as_deref(), Some("0.5"));
        assert_eq!(normalize_decimal("5.").as_deref(), Some("5"));
        assert_eq!(normalize_decimal("+42").as_deref(), Some("42"));
        assert_eq!(normalize_decimal("-3.140").as_deref(), Some("-3.14"));
        // SCIENTIFIC/EXPONENT forms (as TerminusDB's HTTP API may render xsd:decimal)
        // canonicalize to the SAME fixed-point lexical the store commits.
        assert_eq!(normalize_decimal("-1e-6").as_deref(), Some("-0.000001"));
        assert_eq!(normalize_decimal("1E-6").as_deref(), Some("0.000001"));
        assert_eq!(normalize_decimal("1.5e2").as_deref(), Some("150"));
        assert_eq!(normalize_decimal("1e3").as_deref(), Some("1000"));
        assert_eq!(normalize_decimal(".5e1").as_deref(), Some("5"));
        assert_eq!(normalize_decimal("2.5E-1").as_deref(), Some("0.25"));
        assert_eq!(normalize_decimal("0e5").as_deref(), Some("0"));
        // Genuinely non-decimal forms still reject.
        for bad in ["1.2.3", "abc", "", "-", " 1x", "0x5", "1e", "e5", "1e2e3"] {
            assert!(normalize_decimal(bad).is_none(), "should reject {bad:?}");
        }
    }

    #[test]
    fn unbound_and_null_map_to_null() {
        assert_eq!(cell_from_prolog(None), Cell::Null);
        assert_eq!(cell_from_prolog(Some(&json!(null))), Cell::Null);
        assert_eq!(cell_from_rust(&CanonicalResultValue::Null), Cell::Null);
    }

    // --- Value-canonicalization MISMATCHES must be rejected ---------------------
    #[test]
    fn node_and_string_of_same_text_do_not_collapse() {
        // A graph node "x" and an xsd:string literal "x" are DIFFERENT logical
        // values; the comparator must not confuse them.
        let node = cell_from_prolog(Some(&json!("x")));
        let string = cell_from_prolog(Some(&json!({"@type": "xsd:string", "@value": "x"})));
        assert_ne!(node, string);
        assert_eq!(node, Cell::Node(b"x".to_vec()));
        assert_eq!(string, Cell::Str(b"x".to_vec()));
    }

    #[test]
    fn different_integers_do_not_match() {
        assert_ne!(cell_from_rust(&rust_uint(55)), cell_from_prolog(Some(&json!({"@type":"xsd:unsignedInt","@value":56}))));
    }

    // --- Zero-arity existence: row count comes from `result_len`, not columns ----
    #[test]
    fn rust_rows_zero_arity_existence_uses_result_len() {
        // A fully-ground BGP has ZERO output columns, so the row count cannot come
        // from a column length (there is none) — it is the proof-authenticated
        // `result_len`. Present => one empty-tuple row (matches Prolog's single empty
        // binding); absent => no rows. Regression lock for the fuzzer-found harness
        // gap (seed 0xf0fac0de12345623) where a column-length count reported 0 rows.
        let present = DifferentialResult {
            oracle_bindings: Vec::new(),
            oracle_var_names: Vec::new(),
            rust_columns: Vec::new(),
            rust_count: None,
            schema: Vec::new(),
            projection: None,
            rust_result_len: 1,
        };
        assert_eq!(rust_rows(&present), vec![Vec::<Cell>::new()]);

        let absent = DifferentialResult {
            oracle_bindings: Vec::new(),
            oracle_var_names: Vec::new(),
            rust_columns: Vec::new(),
            rust_count: None,
            schema: Vec::new(),
            projection: None,
            rust_result_len: 0,
        };
        assert_eq!(rust_rows(&absent), Vec::<Vec<Cell>>::new());
    }

    // --- Relation comparison: accept equal, reject unequal ----------------------
    fn relation(rows: &[&[Cell]]) -> Vec<Vec<Cell>> {
        rows.iter().map(|r| r.to_vec()).collect()
    }

    #[test]
    fn multiset_accepts_reordered_equal_relations() {
        let a = relation(&[&[Cell::Int(1)], &[Cell::Int(2)], &[Cell::Int(2)]]);
        let b = relation(&[&[Cell::Int(2)], &[Cell::Int(2)], &[Cell::Int(1)]]);
        assert!(compare_relations(&a, &b, CompareMode::Multiset).is_ok());
    }

    #[test]
    fn multiset_rejects_multiplicity_difference() {
        // Same set, different multiplicity — a real divergence a set-compare would
        // WRONGLY accept, so the default must be a multiset.
        let a = relation(&[&[Cell::Int(1)], &[Cell::Int(1)]]);
        let b = relation(&[&[Cell::Int(1)]]);
        assert!(compare_relations(&a, &b, CompareMode::Multiset).is_err());
    }

    #[test]
    fn multiset_rejects_value_canonicalization_mismatch() {
        // Prolog-shaped vs Rust-shaped inputs for the SAME positions but a WRONG
        // value on one side: must be rejected.
        let oracle = relation(&[&[cell_from_prolog(Some(&json!({"@type":"xsd:unsignedInt","@value":80})))]]);
        let rust = relation(&[&[cell_from_rust(&rust_uint(81))]]);
        assert!(compare_relations(&oracle, &rust, CompareMode::Multiset).is_err());
    }

    #[test]
    fn ordered_rejects_a_reordering_that_multiset_accepts() {
        let sorted = relation(&[&[Cell::Int(1)], &[Cell::Int(2)], &[Cell::Int(3)]]);
        let shuffled = relation(&[&[Cell::Int(2)], &[Cell::Int(1)], &[Cell::Int(3)]]);
        // Order matters here: the reordering is a divergence...
        assert!(compare_relations(&sorted, &shuffled, CompareMode::Ordered).is_err());
        // ...but as a multiset the very same pair agrees, proving order is only
        // asserted where the mode demands it.
        assert!(compare_relations(&sorted, &shuffled, CompareMode::Multiset).is_ok());
    }

    #[test]
    fn ordered_accepts_identical_order() {
        let a = relation(&[&[Cell::Int(1)], &[Cell::Int(2)], &[Cell::Int(3)]]);
        assert!(compare_relations(&a, &a.clone(), CompareMode::Ordered).is_ok());
    }

    #[test]
    fn set_collapses_duplicates() {
        let a = relation(&[&[Cell::Node(b"x".to_vec())], &[Cell::Node(b"x".to_vec())]]);
        let b = relation(&[&[Cell::Node(b"x".to_vec())]]);
        assert!(compare_relations(&a, &b, CompareMode::Set).is_ok());
        // But the multiset compare (the default) still distinguishes them.
        assert!(compare_relations(&a, &b, CompareMode::Multiset).is_err());
    }

    // --- Mode derivation from the query AST -------------------------------------
    #[test]
    fn mode_is_derived_from_query_shape() {
        use terminusdb_woql2::control::{Distinct, Select};
        use terminusdb_woql2::misc::Count;
        use terminusdb_woql2::order::{Order, OrderBy, OrderTemplate};
        use terminusdb_woql2::value::DataValue;

        let base = Query::Triple(Triple {
            subject: NodeValue::Variable("s".into()),
            predicate: NodeValue::Node(iri("p")),
            object: Value::Variable("o".into()),
            graph: None,
        });
        assert_eq!(mode_of_query(&base), CompareMode::Multiset);

        let ordered = Query::OrderBy(OrderBy {
            ordering: vec![OrderTemplate {
                variable: "o".into(),
                order: Order::Asc,
            }],
            query: Box::new(base.clone()),
        });
        assert_eq!(mode_of_query(&ordered), CompareMode::Ordered);
        // A Select wrapper is proof-transparent: peel it, the OrderBy still decides.
        let selected_ordered = Query::Select(Select {
            variables: vec!["o".into()],
            query: Box::new(ordered),
        });
        assert_eq!(mode_of_query(&selected_ordered), CompareMode::Ordered);

        let distinct = Query::Distinct(Distinct {
            variables: vec!["s".into(), "o".into()],
            query: Box::new(base.clone()),
        });
        assert_eq!(mode_of_query(&distinct), CompareMode::Set);

        let counted = Query::Count(Count {
            query: Box::new(base),
            count: DataValue::Variable("n".into()),
        });
        assert_eq!(mode_of_query(&counted), CompareMode::Multiset);
    }
}

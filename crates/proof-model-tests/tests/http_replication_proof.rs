//! Full-stack end-to-end proof of the "replicate over real HTTP, then prove
//! offline" story for the Proof-of-WOQL system.
//!
//! What this test exercises, in order:
//!   1. Boots the REAL embedded, proof-capable TerminusDB server
//!      (`terminusdb-bin` -> a SWI-Prolog HTTP server child process whose store
//!      IS the ParapluOU fork, so every committed layer carries an intrinsic PF4
//!      commitment).
//!   2. Creates a (schemaless) database and COMMITS a single ground triple
//!      `http://example.org/s  http://example.org/p  http://example.org/o`
//!      through the HTTP client (a WOQL `AddTriple` over `/api/woql/...`).
//!   3. A "side process" step REPLICATES the committed data over ACTUAL HTTP:
//!      it does a raw `POST /api/pack/admin/<db>` (a genuine TCP socket call via
//!      `reqwest`) and receives the terminus-store pack. The pack is imported into
//!      a LOCAL fork terminus-store that this test fully controls.
//!   4. Over the REPLICATED instance-graph layer it produces a single-triple BGP
//!      read proof and VERIFIES it, asserting (a) the resolved answer equals the
//!      server-committed data and (b) the proof is BOUND to the layer's committed
//!      `commitment_root` (the trust anchor). Fail-closed negatives assert that a
//!      tampered root, a corrupted envelope, and a false (wrong-object) query all
//!      fail to verify / cannot be proven.
//!
//! REPLICATION APPROACH — raw `/api/pack` GET (the task's "Alternative").
//! Chosen over the two-server clone because it cleanly yields BOTH a store we can
//! open AND lets us discover the instance-graph head layer by content-scan
//! (matching the distinctive `example.org` triple), with no dependency on
//! terminusdb's internal commit/repository-graph head resolution. The pack the
//! server emits is produced by the fork's `pack_export`, so it is a v2
//! (`proof-state=pf4`) pack and `import_layers` validates each layer's intrinsic
//! commitment during import (fail-closed) before it becomes visible.
//!
//! STORE-VERSION UNIFICATION: the import + prove handles link the FORK
//! terminus-store (the `terminus-store` path dep = `terminusdb-store-paraplu`,
//! 0.21.8). `terminusdb-client` carries no terminus-store dep at all, so there is
//! a single terminus-store copy and the `LayerCommitment` handed to `woql2`'s
//! prover is the same crate identity.
//!
//! Run (release-gated; the debug proof build is far too slow):
//!   export PATH=/nix/store/hiknqamwkn01i55z0jwmlvprdd2191ml-gnum4-1.4.21/bin:$PATH
//!   RUST_MIN_STACK=134217728 \
//!   TERMINUSDB_SOURCE=/path/to/terminusdb-fork-checkout \
//!   cargo +nightly-2025-09-19 test -p terminusdb-proof-model-tests --release \
//!     --test http_replication_proof -- --nocapture

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use terminus_store::proof::query::CanonicalResultValue;
use terminus_store::proof::CanonicalObject;
use terminus_store::storage::pack_layer_parents;
use terminus_store::store::sync::{open_sync_memory_store, SyncStore};
use terminus_store::{Layer, ObjectType, ValueTriple};

use terminusdb_woql_proof::proof::{decode_and_verify_envelope, plan_bgp};
use terminusdb_woql2::query::{And, Query};
use terminusdb_woql2::triple::{AddTriple, Triple};
use terminusdb_woql2::value::{NodeValue, Value};

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::BranchSpec;

// The distinctive ground triple we commit through the server and later prove over
// the replicated layer. Full IRIs are stored verbatim (no prefix expansion), and
// the `example.org` marker lets us locate the instance-graph layer inside the
// pack regardless of any canonicalization terminusdb applies.
const SUBJ: &str = "http://example.org/s";
const PRED: &str = "http://example.org/p";
const OBJ: &str = "http://example.org/o";
// A second, DIFFERENT-predicate triple committed in the SAME layer. The proof's
// padded hypercube needs the committed table to have >= 2 rows
// (`next_power_of_two(real_rows) >= 2`), so a single-triple layer is rejected.
// This extra `p2` edge is invisible to the single-predicate `?s <PRED> ?o` BGP
// below, which still matches exactly the one `PRED` triple.
const PRED2: &str = "http://example.org/p2";
const OBJ2: &str = "http://example.org/o2";
const DB: &str = "e2e_http_proof";

// ---------------------------------------------------------------------------
// Phase 1-3: boot server, commit over HTTP, replicate the pack over HTTP.
// Everything async (server/client/reqwest) lives inside ONE runtime; the pack
// bytes are returned out so the sync store phase runs on the plain test thread.
// ---------------------------------------------------------------------------
fn commit_and_replicate_over_http() -> anyhow::Result<Vec<u8>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let response = rt.block_on(async {
        // (1) Boot the real embedded proof-capable server (in-memory mode: the
        // pack travels over HTTP, so no on-disk store is needed).
        let server = TerminusDBServer::test().await?;
        let port = server.port();
        let client = server.client().await?;
        let base = format!("http://localhost:{port}");
        let http = reqwest::Client::new();

        // (2a) Create a SCHEMALESS database so an arbitrary ground triple is
        // accepted (a schema-checking DB would reject unknown predicates).
        let created = http
            .post(format!("{base}/api/db/admin/{DB}"))
            .basic_auth("admin", Some("root"))
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!({
                    "label": DB,
                    "comment": "proof-of-woql http e2e",
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

        // (2b) COMMIT the ground triple via a WOQL AddTriple over `/api/woql`.
        let spec = BranchSpec::with_branch(DB, "main");
        let insert = Query::And(And {
            and: vec![
                Query::AddTriple(AddTriple {
                    subject: NodeValue::Node(SUBJ.to_string()),
                    predicate: NodeValue::Node(PRED.to_string()),
                    object: Value::Node(OBJ.to_string()),
                    graph: None,
                }),
                Query::AddTriple(AddTriple {
                    subject: NodeValue::Node(SUBJ.to_string()),
                    predicate: NodeValue::Node(PRED2.to_string()),
                    object: Value::Node(OBJ2.to_string()),
                    graph: None,
                }),
            ],
        });
        let _res = client
            .query_mut::<serde_json::Value>(spec, insert, "e2e", "insert ex:s ex:p ex:o + ex:s ex:p2 ex:o2")
            .await?;

        // (3) REPLICATE over real HTTP: raw `POST /api/pack/admin/<db>` with an
        // empty body (no `repository_head`) asks for the FULL pack. The response
        // body is `<40-byte ascii repository head><terminus-store pack bytes>`.
        let packed = http
            .post(format!("{base}/api/pack/admin/{DB}"))
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
        Ok::<Vec<u8>, anyhow::Error>(bytes)
        // `server` is dropped here (child killed) — the pack bytes are already ours.
    })?;

    drop(rt);
    Ok(response)
}

// ---------------------------------------------------------------------------
// Pack import (fork store). Tries a single bulk import; on failure (e.g. a
// system layer without an intrinsic commitment) falls back to a tolerant,
// parent-first per-layer import and returns the set that landed.
// ---------------------------------------------------------------------------
fn import_pack_into(store: &SyncStore, pack: &[u8]) -> HashSet<[u32; 5]> {
    let parents: HashMap<[u32; 5], Option<[u32; 5]>> =
        pack_layer_parents(Cursor::new(pack)).unwrap_or_else(|e| panic!("pack is unreadable: {e}"));
    let all: Vec<[u32; 5]> = parents.keys().copied().collect();

    // Fast path: import every layer at once (ancestry handled internally, and the
    // whole import is atomic — nothing is published if any layer is rejected).
    if store
        .import_layers(pack, Box::new(all.clone().into_iter()))
        .is_ok()
    {
        return all.into_iter().collect();
    }

    // Fallback: parent-first, tolerating individually-unimportable layers.
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
                        // Parent can never land -> neither can this child.
                        failed.insert(id);
                        changed = true;
                    }
                    continue; // wait for the parent to import first
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

// ---------------------------------------------------------------------------
// Result helpers.
// ---------------------------------------------------------------------------
fn as_node(v: &CanonicalResultValue) -> String {
    match v {
        CanonicalResultValue::Object(CanonicalObject::Node(bytes)) => {
            String::from_utf8(bytes.clone()).expect("node IRI is valid utf-8")
        }
        other => panic!("expected a node result, got {other:?}"),
    }
}

fn nodes(col: &[CanonicalResultValue]) -> Vec<String> {
    col.iter().map(as_node).collect()
}

// ---------------------------------------------------------------------------
// The one end-to-end test.
// ---------------------------------------------------------------------------
#[test]
fn replicate_over_real_http_then_prove_offline() {
    // Phases 1-3: real server, real HTTP commit, real HTTP pack replication.
    let pack_response =
        commit_and_replicate_over_http().expect("http commit + replication phase failed");

    // `<40-byte ascii repository head>` ++ `<pack bytes>`.
    let (_repository_head, pack) = pack_response.split_at(40);

    // Import the replicated layers into a LOCAL fork terminus-store we control.
    let store = open_sync_memory_store();
    let imported = import_pack_into(&store, pack);
    assert!(
        !imported.is_empty(),
        "no layer from the replicated pack could be imported"
    );

    // Discover the instance-graph head: the imported layer carrying our
    // distinctive `example.org` triple. Among any matches, take the one with the
    // most visible triples (the head sees all ancestors).
    let mut best: Option<([u32; 5], ValueTriple, usize)> = None;
    for &id in &imported {
        let Some(layer) = store.get_layer_from_id(id).expect("open imported layer") else {
            continue;
        };
        let mut marker: Option<ValueTriple> = None;
        let mut total = 0usize;
        for t in layer.triples() {
            total += 1;
            if marker.is_none() {
                if let Some(vt) = layer.id_triple_to_string(&t) {
                    let object_matches =
                        matches!(&vt.object, ObjectType::Node(n) if n.contains("example.org/o"));
                    if object_matches
                        && vt.predicate.contains("example.org/p")
                        && vt.subject.contains("example.org/s")
                    {
                        marker = Some(vt);
                    }
                }
            }
        }
        if let Some(vt) = marker {
            match &best {
                Some((_, _, best_total)) if *best_total >= total => {}
                _ => best = Some((id, vt, total)),
            }
        }
    }

    let (instance_id, marker, _) = best.expect(
        "the replicated pack contained no instance-graph layer with the committed \
         `http://example.org/s ex:p ex:o` triple",
    );
    // The exact stored strings (robust to any canonicalization the server applied).
    let stored_pred = marker.predicate.clone();
    let stored_subj = marker.subject.clone();
    let stored_obj = match &marker.object {
        ObjectType::Node(n) => n.clone(),
        other => panic!("committed object was not a node: {other:?}"),
    };

    // The replicated layer's intrinsic commitment IS the server-committed trust
    // anchor (import validated it fail-closed on the way in). Defensive: if the
    // chain needs a format migration, migrate then re-open before proving.
    let layer = store
        .get_layer_from_id(instance_id)
        .expect("re-open instance layer")
        .expect("instance layer present");
    let commitment = match layer.proof_commitment() {
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
    };
    // The verifier's external trust anchor is the layer's `resolution_root`
    // (= H(commitment_root ‖ nv_merkle_root ‖ pred_merkle_root)), not the bare
    // `commitment_root` — it binds the dictionary Merkle roots used for term
    // resolution as well as the committed columns.
    let root = commitment.resolution_root();

    // Prove a single-triple BGP read `?s <stored_pred> ?o` over the replicated
    // layer, and VERIFY it against the committed root.
    let query = Query::Triple(Triple {
        subject: NodeValue::Variable("s".to_string()),
        predicate: NodeValue::Node(stored_pred.clone()),
        object: Value::Variable("o".to_string()),
        graph: None,
    });
    let compiled = plan_bgp(&query).expect("compile single-triple BGP");
    assert_eq!(
        compiled.relation.schema,
        vec!["s".to_string(), "o".to_string()]
    );

    let proved = compiled.prove(&commitment).expect("prove BGP over replicated layer");
    assert_eq!(
        proved.result_len, 1,
        "exactly the one committed `ex:p` triple should match"
    );

    // (4a) The resolved answer equals the SERVER-COMMITTED data.
    let values = proved
        .verify_and_resolve_values(&compiled, &commitment, root)
        .expect("verify + resolve against the committed root");
    assert_eq!(nodes(&values[0]), vec![stored_subj.clone()]);
    assert_eq!(nodes(&values[1]), vec![stored_obj.clone()]);
    // And it is genuinely OUR committed triple.
    assert!(stored_subj.contains("example.org/s"));
    assert!(stored_obj.contains("example.org/o"));

    // (4b) The proof is BOUND to the committed root — a tampered root is rejected.
    let mut tampered_root = root;
    tampered_root[0] ^= 0x01;
    assert_ne!(root, tampered_root);
    assert!(
        proved
            .verify_and_resolve_values(&compiled, &commitment, tampered_root)
            .is_err(),
        "verifying against a tampered trust anchor must fail closed"
    );

    // Envelope round-trip: verifies under the real root, rejected under a tampered
    // root and when the proof bytes are corrupted.
    let envelope = proved
        .encode_envelope(&compiled, &commitment, root)
        .expect("encode envelope");
    decode_and_verify_envelope(&envelope, &compiled, &commitment, root)
        .expect("decode + verify envelope under the committed root");
    assert!(
        decode_and_verify_envelope(&envelope, &compiled, &commitment, tampered_root).is_err(),
        "envelope must reject a tampered root"
    );
    let mut corrupt = envelope.clone();
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0x01;
    assert!(
        decode_and_verify_envelope(&corrupt, &compiled, &commitment, root).is_err(),
        "a corrupted envelope must fail closed"
    );

    // (4c) A FALSE query (wrong object constant) cannot be proven to hold — either
    // the constant is not in the committed dictionary (compile/prove rejects) or
    // it proves zero rows. Never a false positive.
    let false_query = Query::Triple(Triple {
        subject: NodeValue::Variable("s".to_string()),
        predicate: NodeValue::Node(stored_pred.clone()),
        object: Value::Node("http://example.org/WRONG".to_string()),
        graph: None,
    });
    let false_rows = match plan_bgp(&false_query) {
        Ok(compiled_false) => compiled_false
            .prove(&commitment)
            .map(|p| p.result_len)
            .unwrap_or(0),
        Err(_) => 0,
    };
    assert_eq!(
        false_rows, 0,
        "a triple with the wrong object must not be provable against the committed root"
    );
}

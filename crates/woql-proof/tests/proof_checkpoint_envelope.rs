//! Single-layer reads over PROVEN CHECKPOINTS through the woql2 compile -> prove -> encode ->
//! byte-in verify pipeline: `ProvedBgp::encode_checkpoint_envelope` with
//! `decode_checkpoint_and_verify_envelope` (no deltas since) and
//! `decode_checkpoint_deltas_and_verify_envelope` (a checkpoint plus the deltas since). Results equal
//! the term-bearing envelope over a base layer of the same state; another checkpoint's root or
//! view, another query, a stripped witness and a missing delta reject.

mod proof_common;
use proof_common::*;

use std::sync::Arc;

use terminus_store::proof::argument::StoreProverContext;
use terminus_store::proof::checkpoint::{genesis_checkpoint, prove_checkpoint, CheckpointQueryLayer};
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::envelope::CheckpointRelationEnvelope;
use terminus_store::proof::ResolutionOpenings;
use terminus_store::{open_sync_memory_store, ValueTriple};
use terminusdb_woql_proof::proof::{
    decode_checkpoint_and_verify_envelope, decode_checkpoint_deltas_and_verify_envelope,
    decode_verifier_commitment_and_verify_envelope, plan_bgp,
};

use tdb_succinct::TdbDataType;

fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 * 1024 * 1024).spawn(f).unwrap().join().unwrap();
}

fn node(s: &str, p: &str, o: &str) -> ValueTriple {
    ValueTriple::new_node(&iri(s), &iri(p), &iri(o))
}
fn score(s: &str, v: u32) -> ValueTriple {
    ValueTriple::new_value(&iri(s), &iri("score"), u32::make_entry(&v))
}

/// A base with junk, two deltas (removals + new terms), and a base layer of the head's state.
fn chain() -> (Vec<Arc<LayerCommitment>>, LayerCommitment) {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for t in [node("a", "knows", "b"), node("x", "knows", "y"), score("c1", 30), score("c1", 99)] {
        builder.add_value_triple(t).unwrap();
    }
    let base = builder.commit().unwrap();
    let d1 = base.open_write().unwrap();
    d1.add_value_triple(node("b", "knows", "c")).unwrap();
    d1.remove_value_triple(node("x", "knows", "y")).unwrap();
    d1.add_value_triple(score("c2", 5)).unwrap();
    let d1 = d1.commit().unwrap();
    let d2 = d1.open_write().unwrap();
    d2.remove_value_triple(score("c1", 99)).unwrap();
    d2.add_value_triple(score("c2", 20)).unwrap();
    d2.add_value_triple(node("a", "likes", "c")).unwrap();
    let d2 = d2.commit().unwrap();
    let reference = open_sync_memory_store().create_base_layer().unwrap();
    for t in [node("a", "knows", "b"), node("b", "knows", "c"), node("a", "likes", "c"), score("c1", 30), score("c2", 5), score("c2", 20)] {
        reference.add_value_triple(t).unwrap();
    }
    let reference = (*reference.commit().unwrap().proof_commitment().unwrap()).clone();
    (
        vec![base.proof_commitment().unwrap(), d1.proof_commitment().unwrap(), d2.proof_commitment().unwrap()],
        reference,
    )
}

#[test]
fn checkpoint_envelope_verifies_byte_in_with_and_without_deltas_since() {
    big_stack(|| {
        let (layers, reference) = chain();
        let deltas: Vec<&LayerCommitment> = layers[1..].iter().map(|l| l.as_ref()).collect();
        let c0 = genesis_checkpoint(&layers[0]).unwrap().0;
        let (head, head_proof) = prove_checkpoint(&c0, &deltas).unwrap();
        let query = CheckpointQueryLayer::new(&StoreProverContext::cpu(), Arc::new(head)).unwrap();
        let artifact = query.artifact().encode().unwrap();
        let root = query.artifact().checkpoint_root;
        let view = query.view_proof().encode().unwrap();
        let views: Vec<Vec<u8>> = deltas.iter().map(|d| d.verifier_commitment().unwrap().encode().unwrap()).collect();
        let view_refs: Vec<&[u8]> = views.iter().map(Vec::as_slice).collect();
        let head_root = layers[2].resolution_root();
        let c0_bytes = c0.artifact.encode().unwrap();
        let c0_root = c0.artifact.checkpoint_root;
        let head_proof = head_proof.encode().unwrap();
        let bearing = reference.verifier_commitment().unwrap().encode().unwrap();
        let bearing_root = reference.resolution_root();
        for query_ast in [
            tv("s", "knows", "o"),
            tv_obj_node("s", "knows", "c"),
            tv_obj_node("s", "knows", "y"),
            and_of(vec![tv("c", "score", "v"), greater(dvar("v"), duint(10))]),
        ] {
            let compiled = plan_bgp(&query_ast).unwrap();
            let want = decode_verifier_commitment_and_verify_envelope(
                &compiled.prove(&reference).unwrap().encode_envelope(&compiled, &reference, bearing_root).unwrap(),
                &compiled,
                &bearing,
                bearing_root,
            )
            .unwrap();
            let bytes = compiled.prove(query.layer()).unwrap().encode_checkpoint_envelope(&compiled, &query).unwrap();
            let got = decode_checkpoint_and_verify_envelope(&bytes, &compiled, &artifact, &view, root).unwrap();
            let sorted = |v: &Vec<Vec<terminus_store::proof::CanonicalObject>>| {
                let mut rows: Vec<String> = (0..v.first().map_or(0, Vec::len))
                    .map(|r| v.iter().map(|c| format!("{:?}", c[r])).collect::<Vec<_>>().join("|"))
                    .collect();
                rows.sort();
                rows
            };
            assert_eq!(sorted(&got.result_objects), sorted(&want.result_objects));
            assert_eq!(got.proved.result_len, want.proved.result_len);
            let via_deltas = decode_checkpoint_deltas_and_verify_envelope(
                &bytes, &compiled, &c0_bytes, c0_root, &view_refs, head_root, &artifact, &head_proof, &view,
            )
            .unwrap();
            assert_eq!(sorted(&via_deltas.result_objects), sorted(&want.result_objects));

            // Another checkpoint's root; a missing delta; a stripped witness; another query.
            assert!(decode_checkpoint_and_verify_envelope(&bytes, &compiled, &artifact, &view, c0_root).is_err());
            assert!(decode_checkpoint_deltas_and_verify_envelope(
                &bytes, &compiled, &c0_bytes, c0_root, &view_refs[..1], head_root, &artifact, &head_proof, &view,
            )
            .is_err());
            let mut stripped = CheckpointRelationEnvelope::decode(&bytes).unwrap();
            stripped.openings = ResolutionOpenings::default();
            assert!(decode_checkpoint_and_verify_envelope(&stripped.encode().unwrap(), &compiled, &artifact, &view, root).is_err());
            let likes = plan_bgp(&tv("s", "likes", "o")).unwrap();
            assert!(decode_checkpoint_and_verify_envelope(&bytes, &likes, &artifact, &view, root).is_err());
        }
    });
}

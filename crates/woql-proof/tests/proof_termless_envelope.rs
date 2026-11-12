//! The single-layer TERM-LESS envelope through the woql2 compile -> prove -> encode -> byte-in
//! verify pipeline: `ProvedBgp::encode_termless_envelope` + `decode_termless_and_verify_envelope`
//! against the O(1) term-less `VerifierCommitment`. Results equal the term-bearing envelope's; the
//! wrong commitment/root, a stripped witness and out-of-fragment queries reject.

mod proof_common;
use proof_common::*;

use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::envelope::TermlessRelationEnvelope;
use terminus_store::proof::ResolutionOpenings;
use terminus_store::{open_sync_memory_store, ValueTriple};
use terminusdb_woql_proof::proof::{
    decode_termless_and_verify_envelope, decode_verifier_commitment_and_verify_envelope, plan_bgp,
};

use tdb_succinct::TdbDataType;

fn node_layer(triples: &[(&str, &str, &str)]) -> LayerCommitment {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (s, p, o) in triples {
        builder.add_value_triple(ValueTriple::new_node(&iri(s), &iri(p), &iri(o))).unwrap();
    }
    (*builder.commit().unwrap().proof_commitment().unwrap()).clone()
}

fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 * 1024 * 1024).spawn(f).unwrap().join().unwrap();
}

#[test]
fn termless_envelope_verifies_byte_in_and_matches_term_bearing() {
    big_stack(|| {
        let lc = node_layer(&[("a", "knows", "b"), ("b", "knows", "c"), ("a", "likes", "c")]);
        let root = lc.resolution_root();
        let termless = lc.verifier_commitment_termless().unwrap().encode().unwrap();
        let bearing = lc.verifier_commitment().unwrap().encode().unwrap();
        for query in [
            tv("s", "knows", "o"),
            tv_obj_node("s", "knows", "b"),
            and_of(vec![tv("x", "knows", "y"), tv("x", "likes", "z")]),
        ] {
            let compiled = plan_bgp(&query).unwrap();
            let proved = compiled.prove(&lc).unwrap();
            let reference_bytes = proved.encode_envelope(&compiled, &lc, root).unwrap();
            let reference =
                decode_verifier_commitment_and_verify_envelope(&reference_bytes, &compiled, &bearing, root)
                    .unwrap();
            let bytes = proved.encode_termless_envelope(&compiled, &lc, root).unwrap();
            let verified = decode_termless_and_verify_envelope(&bytes, &compiled, &termless, root).unwrap();
            assert_eq!(verified.result_objects, reference.result_objects);
            assert_eq!(verified.proved.result_len, reference.proved.result_len);
            assert!(verified.proved.result_len > 0);

            // A term-BEARING commitment is not accepted on the term-less path.
            assert!(decode_termless_and_verify_envelope(&bytes, &compiled, &bearing, root).is_err());
            // The term-less envelope is not a term-bearing envelope, and vice versa.
            assert!(decode_verifier_commitment_and_verify_envelope(&bytes, &compiled, &bearing, root).is_err());
            assert!(decode_termless_and_verify_envelope(&reference_bytes, &compiled, &termless, root).is_err());
            // A term-less commitment through the TERM-BEARING envelope path fails closed.
            assert!(decode_verifier_commitment_and_verify_envelope(&reference_bytes, &compiled, &termless, root).is_err());

            // Witness stripped (re-framed honestly, so only the missing openings differ).
            let mut stripped = TermlessRelationEnvelope::decode(&bytes).unwrap();
            stripped.openings = ResolutionOpenings::default();
            assert!(decode_termless_and_verify_envelope(&stripped.encode().unwrap(), &compiled, &termless, root).is_err());
        }

        // Another catalog's commitment + root, and the right commitment under another root.
        let other = node_layer(&[("p", "knows", "q"), ("q", "knows", "r")]);
        let other_termless = other.verifier_commitment_termless().unwrap().encode().unwrap();
        let compiled = plan_bgp(&tv("s", "knows", "o")).unwrap();
        let bytes = compiled.prove(&lc).unwrap().encode_termless_envelope(&compiled, &lc, root).unwrap();
        assert!(decode_termless_and_verify_envelope(&bytes, &compiled, &other_termless, other.resolution_root()).is_err());
        assert!(decode_termless_and_verify_envelope(&bytes, &compiled, &termless, other.resolution_root()).is_err());
        // A different query (anti-substitution).
        let likes = plan_bgp(&tv("s", "likes", "o")).unwrap();
        assert!(decode_termless_and_verify_envelope(&bytes, &likes, &termless, root).is_err());
    });
}

#[test]
fn termless_envelope_fails_closed_for_value_reading_queries() {
    big_stack(|| {
        let store = open_sync_memory_store();
        let builder = store.create_base_layer().unwrap();
        for score in [10u32, 20, 30, 40] {
            builder
                .add_value_triple(ValueTriple::new_value(&iri("metric"), &iri("score"), u32::make_entry(&score)))
                .unwrap();
        }
        let lc = (*builder.commit().unwrap().proof_commitment().unwrap()).clone();
        let compiled = plan_bgp(&and_of(vec![tv("m", "score", "v"), greater(dvar("v"), duint(15))])).unwrap();
        let proved = compiled.prove(&lc).unwrap();
        assert!(proved.encode_termless_envelope(&compiled, &lc, lc.resolution_root()).is_err());
    });
}

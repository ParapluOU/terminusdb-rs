//! Reconciled delta-stack reads — the compiler capstone.
//!
//! Routes a supported read-only WOQL query over an **N-layer delta stack** to the store's
//! reconcile + present-BGP + resolve path, delivering the end goal: *prove a read-only WOQL
//! query over the live delta stack, no re-run*. The chain (base + deltas, head last) is passed
//! in because a `LayerCommitment` only carries `parent_state_root`, not the parent object.
//!
//! Supported shapes (mirroring the store present-path this crate already proves):
//!   1. **Single-atom filter** `Triple(...)` with EXACTLY ONE constant position (predicate,
//!      subject, or object node IRI) and two output variables → `prove_read_single_atom`.
//!   2. **2-atom object→subject join** `Triple(?x, p, ?y) & Triple(?y, q, ?z)` (both predicates
//!      constant, the shared variable is atom-1's object and atom-2's subject) →
//!      `prove_read_two_atom_join`.
//!
//! Everything else (general n-atom BGP, numeric range, object VALUE/list/dict constants, the
//! schema graph) REJECT-CLOSES here; base / single-materialized-layer queries keep using the
//! existing [`crate::proof::CompiledBgp::prove`] path unchanged.

use dory_pcs::backends::arkworks::ArkFr;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::dict::{hash_object, hash_term};
use terminus_store::proof::reconcile::{
    prove_read_single_atom, prove_read_two_atom_join, verify_and_resolve_single_atom,
    verify_and_resolve_two_atom_join, FilterPosition, PresentFilterProof, PresentJoinProof,
    PresentReadResult, ReconcileProof,
};
use terminus_store::proof::{CanonicalObject, ProofHash};

use terminusdb_schema::GraphType;

use crate::proof::plan_bgp;
use crate::query::Query;
use crate::triple::Triple;
use crate::value::{NodeValue, Value};

/// Every reason the reconciled read path refuses a query.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReconciledReadError {
    /// The query is not one of the two reconciled shapes (single-atom filter / 2-atom join).
    #[error("unsupported reconciled shape: {0}")]
    UnsupportedShape(&'static str),
    /// The query does not compile as a supported BGP.
    #[error("query does not compile as a BGP: {0}")]
    NotBgp(String),
    /// The underlying store reconcile/present-BGP/resolve path errored.
    #[error("store proof error: {0}")]
    Store(String),
}

/// The detected supported read shape (with the store-facing constants).
#[derive(Debug, Clone, Copy)]
enum Shape {
    SingleAtom { position: FilterPosition, constant_hash: ArkFr },
    Join { const_p: ArkFr, const_q: ArkFr },
}

/// A proved reconciled read: the store proof bundle plus the shape it was proved for (so the
/// verifier routes identically).
pub enum ReconciledReadProof {
    SingleAtom {
        reconcile: ReconcileProof,
        filter: PresentFilterProof,
        result: PresentReadResult,
        position: FilterPosition,
        constant_hash: ArkFr,
    },
    Join {
        reconcile: ReconcileProof,
        join: PresentJoinProof,
        result: PresentReadResult,
        const_p: ArkFr,
        const_q: ArkFr,
    },
}

impl ReconciledReadProof {
    /// The prover's (unverified) result columns, for convenience/inspection. The trusted,
    /// resolved answer comes from [`verify_and_resolve_reconciled_read`].
    pub fn result_columns(&self) -> &[Vec<ArkFr>] {
        match self {
            ReconciledReadProof::SingleAtom { result, .. } => &result.columns,
            ReconciledReadProof::Join { result, .. } => &result.columns,
        }
    }
}

fn nv_node_bytes(nv: &NodeValue) -> Option<Vec<u8>> {
    match nv {
        NodeValue::Node(iri) => Some(iri.clone().into_bytes()),
        NodeValue::Variable(_) => None,
    }
}
fn nv_is_var(nv: &NodeValue) -> bool {
    matches!(nv, NodeValue::Variable(_))
}
fn nv_var<'a>(nv: &'a NodeValue) -> Option<&'a str> {
    match nv {
        NodeValue::Variable(v) => Some(v.as_str()),
        NodeValue::Node(_) => None,
    }
}
fn val_node_bytes(v: &Value) -> Option<Vec<u8>> {
    match v {
        Value::Node(iri) => Some(iri.clone().into_bytes()),
        _ => None,
    }
}
fn val_is_var(v: &Value) -> bool {
    matches!(v, Value::Variable(_))
}
fn val_var<'a>(v: &'a Value) -> Option<&'a str> {
    match v {
        Value::Variable(name) => Some(name.as_str()),
        _ => None,
    }
}

fn require_instance_graph(t: &Triple) -> Result<(), ReconciledReadError> {
    match &t.graph {
        None | Some(GraphType::Instance) => Ok(()),
        Some(GraphType::Schema) => {
            Err(ReconciledReadError::UnsupportedShape("only the instance graph is supported"))
        }
    }
}

fn detect_single_atom(t: &Triple) -> Result<Shape, ReconciledReadError> {
    require_instance_graph(t)?;
    // Object VALUE / list / dict constants are deferred; only node-IRI constants are supported.
    if matches!(t.object, Value::Data(_) | Value::Dictionary(_) | Value::List(_)) {
        return Err(ReconciledReadError::UnsupportedShape(
            "object value/list/dict constants are deferred (node-IRI constants only)",
        ));
    }
    let subj_c = nv_node_bytes(&t.subject);
    let pred_c = nv_node_bytes(&t.predicate);
    let obj_c = val_node_bytes(&t.object);
    let count = subj_c.is_some() as u8 + pred_c.is_some() as u8 + obj_c.is_some() as u8;
    if count != 1 {
        return Err(ReconciledReadError::UnsupportedShape(
            "single-atom read needs exactly one constant position (the other two are output vars)",
        ));
    }
    if let Some(pred) = pred_c {
        if !nv_is_var(&t.subject) || !val_is_var(&t.object) {
            return Err(ReconciledReadError::UnsupportedShape("non-constant positions must be variables"));
        }
        Ok(Shape::SingleAtom { position: FilterPosition::Predicate, constant_hash: hash_term(&pred) })
    } else if let Some(subj) = subj_c {
        if !nv_is_var(&t.predicate) || !val_is_var(&t.object) {
            return Err(ReconciledReadError::UnsupportedShape("non-constant positions must be variables"));
        }
        Ok(Shape::SingleAtom {
            position: FilterPosition::Subject,
            constant_hash: hash_object(&CanonicalObject::Node(subj)),
        })
    } else {
        let obj = obj_c.expect("exactly one constant, and it is the object");
        if !nv_is_var(&t.subject) || !nv_is_var(&t.predicate) {
            return Err(ReconciledReadError::UnsupportedShape("non-constant positions must be variables"));
        }
        Ok(Shape::SingleAtom {
            position: FilterPosition::Object,
            constant_hash: hash_object(&CanonicalObject::Node(obj)),
        })
    }
}

fn detect_join(t1: &Triple, t2: &Triple) -> Result<Shape, ReconciledReadError> {
    require_instance_graph(t1)?;
    require_instance_graph(t2)?;
    let p = nv_node_bytes(&t1.predicate)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-1 predicate must be a constant"))?;
    let q = nv_node_bytes(&t2.predicate)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-2 predicate must be a constant"))?;
    let _x = nv_var(&t1.subject)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-1 subject must be a variable"))?;
    let y1 = val_var(&t1.object)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-1 object must be a variable"))?;
    let y2 = nv_var(&t2.subject)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-2 subject must be a variable"))?;
    let _z = val_var(&t2.object)
        .ok_or(ReconciledReadError::UnsupportedShape("atom-2 object must be a variable"))?;
    if y1 != y2 {
        return Err(ReconciledReadError::UnsupportedShape(
            "join must share atom-1 object with atom-2 subject",
        ));
    }
    Ok(Shape::Join { const_p: hash_term(&p), const_q: hash_term(&q) })
}

/// Detect the supported reconciled shape. Validates the query compiles as a BGP first (rejecting
/// unsupported constructs), then matches the AST — the AST carries variable identity that the
/// join needs.
fn detect_shape(query: &Query) -> Result<Shape, ReconciledReadError> {
    plan_bgp(query).map_err(|e| ReconciledReadError::NotBgp(e.to_string()))?;
    match query {
        Query::Triple(t) => detect_single_atom(t),
        Query::And(and) if and.and.len() == 2 => match (&and.and[0], &and.and[1]) {
            (Query::Triple(t1), Query::Triple(t2)) => detect_join(t1, t2),
            _ => Err(ReconciledReadError::UnsupportedShape("both And conjuncts must be triples")),
        },
        Query::And(_) => Err(ReconciledReadError::UnsupportedShape(
            "only a 2-atom And is supported (general n-atom BGP is deferred)",
        )),
        _ => Err(ReconciledReadError::UnsupportedShape(
            "only a single triple or a 2-triple And is supported",
        )),
    }
}

/// Returns `true` iff this WOQL query is routable to the reconciled delta-stack path.
pub fn is_reconciled_read(query: &Query) -> bool {
    detect_shape(query).is_ok()
}

/// Prove a supported read-only WOQL query over an N-layer delta stack (base + deltas, head last)
/// via the store reconcile + present-BGP path. The returned proof carries the raw
/// authenticated result columns; [`verify_and_resolve_reconciled_read`] delivers the resolved
/// canonical answer.
pub fn prove_reconciled_read(
    query: &Query,
    layers: &[&LayerCommitment],
) -> Result<ReconciledReadProof, ReconciledReadError> {
    match detect_shape(query)? {
        Shape::SingleAtom { position, constant_hash } => {
            let (reconcile, filter, result) = prove_read_single_atom(layers, position, constant_hash)
                .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
            Ok(ReconciledReadProof::SingleAtom { reconcile, filter, result, position, constant_hash })
        }
        Shape::Join { const_p, const_q } => {
            let (reconcile, join, result) = prove_read_two_atom_join(layers, const_p, const_q)
                .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
            Ok(ReconciledReadProof::Join { reconcile, join, result, const_p, const_q })
        }
    }
}

/// Verify a reconciled read proof against the trusted head root AND resolve its bindings to
/// canonical `CanonicalObject`s (real IRIs / typed literals) — the readable answer, no re-run.
pub fn verify_and_resolve_reconciled_read(
    layers: &[&LayerCommitment],
    trusted_head_root: ProofHash,
    proof: &ReconciledReadProof,
) -> Result<Vec<Vec<CanonicalObject>>, ReconciledReadError> {
    match proof {
        ReconciledReadProof::SingleAtom { reconcile, filter, result, position, constant_hash } => {
            verify_and_resolve_single_atom(
                layers, trusted_head_root, *position, *constant_hash, reconcile, filter, &result.columns,
            )
            .map_err(|e| ReconciledReadError::Store(e.to_string()))
        }
        ReconciledReadProof::Join { reconcile, join, result, const_p, const_q } => {
            verify_and_resolve_two_atom_join(
                layers, trusted_head_root, *const_p, *const_q, reconcile, join, &result.columns,
            )
            .map_err(|e| ReconciledReadError::Store(e.to_string()))
        }
    }
}

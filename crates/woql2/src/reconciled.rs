//! Reconciled delta-stack reads — the compiler capstone.
//!
//! Routes a supported read-only WOQL query over an **N-layer delta stack** to the store's
//! reconcile + present-BGP + resolve path, delivering the end goal: *prove a read-only WOQL
//! query over the live delta stack, no re-run*. The chain (base + deltas, head last) is passed
//! in because a `LayerCommitment` only carries `parent_state_root`, not the parent object.
//!
//! The lowering is GENERAL: any conjunctive BGP the base path compiles ([`plan_bgp`]) — an
//! n-atom left-deep join over shared variables, constants in any position, repeated-variable
//! same-row equality, and numeric object ranges — is mapped onto the store's `present` BGP
//! ([`prove_read_bgp`] / [`verify_and_resolve_bgp`]) and proved over the reconciled delta head.
//! The compiled [`LayerBgpPlan`] is translated ~1:1 to `Vec<PresentBgpAtom>` + `Vec<JoinStep>`
//! (the store `present` atom type MIRRORS the base [`LayerBgpAtom`], restricted to a constant
//! predicate; the [`JoinStep`] plan is shared verbatim), so the reconciled path matches the base
//! path's coverage over a delta stack instead of a single materialized layer.
//!
//! What still REJECT-CLOSES here (matching the base `present` path's limits):
//!   * anything [`plan_bgp`] refuses (non-BGP constructs, the schema graph, ranges on
//!     unsupported datatypes) — surfaced as [`ReconciledReadError::NotBgp`];
//!   * a `Select` projection or a `Count` wrapper (the reconciled path delivers the full BGP
//!     relation's rows, not a projection or a cardinality);
//!   * a non-leaf compiled relation (Distinct/GroupBy/Or/join/order/… — anything the store
//!     `present` BGP cannot express);
//!   * a predicate VARIABLE atom (`present` authenticates a CONSTANT predicate hash);
//!   * a delta-source atom (`AddedTriple`/`DeletedTriple`) — `present` IS the reconciled
//!     materialized head, so additions/removals column sources do not map onto it;
//!   * an object-KIND-restricted variable object (`Link`/`Data`) — `present` has no
//!     object-kind channel;
//!   * a fully-ground atom — the `present` path keeps at least one variable per atom.

use dory_pcs::backends::arkworks::ArkFr;
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::dict::{hash_object, hash_term};
use terminus_store::proof::query::{
    AtomTerm, ColumnSource, LayerBgpAtom, LayerBgpPlan, ObjectKindFilter, ObjectRangeSpec,
};
use terminus_store::proof::reconcile::{
    prove_read_bgp, verify_and_resolve_bgp, PresentBgpAtom, PresentBgpProof, PresentReadResult,
    PresentTerm, ReconcileProof,
};
use terminus_store::proof::{CanonicalObject, ProofHash};

use crate::proof::plan_bgp;
use crate::query::Query;

/// Every reason the reconciled read path refuses a query.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReconciledReadError {
    /// The compiled BGP uses a shape the store `present` path cannot express (predicate
    /// variable, delta source, object-kind filter, fully-ground atom, projection/count, or a
    /// non-leaf compiled relation).
    #[error("unsupported reconciled shape: {0}")]
    UnsupportedShape(&'static str),
    /// The proof's embedded atom/join spec does not match the verifier's OWN re-compiled query
    /// (anti-substitution: a proof for a different supported query was presented).
    #[error("proof shape does not match the verifier's query")]
    QueryShapeMismatch,
    /// The query does not compile as a supported BGP (`plan_bgp` rejected it: a non-BGP
    /// construct, the schema graph, an unsupported range datatype, …).
    #[error("query does not compile as a BGP: {0}")]
    NotBgp(String),
    /// The underlying store reconcile/present-BGP/resolve path errored.
    #[error("store proof error: {0}")]
    Store(String),
}

/// The lowered store-facing statement for a supported reconciled read: the mapped `present`
/// BGP atoms plus the left-deep join plan. Re-derivable from a `&Query` by both prover and
/// verifier, so it is the anti-substitution binding.
#[derive(Clone, Debug)]
struct LoweredBgp {
    atoms: Vec<PresentBgpAtom>,
    joins: Vec<JoinStep>,
}

/// A proved reconciled read: the store proof bundle plus the lowered atom/join spec it was
/// proved for (so [`verify_and_resolve_reconciled_read`] can bind it to the verifier's query).
pub struct ReconciledReadProof {
    pub reconcile: ReconcileProof,
    pub bgp: PresentBgpProof,
    pub result: PresentReadResult,
    /// The lowered `present` atom spec the proof was produced for. Bound to the verifier's OWN
    /// re-compiled query at verify time (anti-substitution); the verifier NEVER trusts these for
    /// the store call, only compares them to its own re-derivation.
    pub atoms: Vec<PresentBgpAtom>,
    pub joins: Vec<JoinStep>,
}

impl ReconciledReadProof {
    /// The prover's (unverified) result columns, for convenience/inspection. The trusted,
    /// resolved answer comes from [`verify_and_resolve_reconciled_read`].
    pub fn result_columns(&self) -> &[Vec<ArkFr>] {
        &self.result.columns
    }
}

/// Map one base [`AtomTerm`] to a store `present` [`PresentTerm`]: a query variable stays a
/// variable; a node or (object) value constant becomes its public `hash_object(...)`.
fn map_term(term: &AtomTerm) -> PresentTerm {
    match term {
        AtomTerm::Variable => PresentTerm::Variable,
        AtomTerm::NodeConstant(iri) => {
            PresentTerm::Constant(hash_object(&CanonicalObject::Node(iri.clone())))
        }
        AtomTerm::ValueConstant(datatype, lexical) => PresentTerm::Constant(hash_object(
            &CanonicalObject::Value { datatype: *datatype, lexical: lexical.clone() },
        )),
    }
}

/// Map one compiled [`LayerBgpAtom`] to a store `present` [`PresentBgpAtom`], reject-closing
/// every shape `present` cannot express (see the module docs).
fn map_atom(atom: &LayerBgpAtom) -> Result<PresentBgpAtom, ReconciledReadError> {
    // `present` IS the reconciled MATERIALIZED head; delta-file column sources
    // (AddedTriple/DeletedTriple) do not map onto it.
    if atom.source != ColumnSource::Materialized {
        return Err(ReconciledReadError::UnsupportedShape(
            "delta-source atoms (AddedTriple/DeletedTriple) are unsupported on the reconciled present path",
        ));
    }
    // The `present` path authenticates a CONSTANT predicate hash; a predicate variable is not
    // supported (its ids live in an independent Store namespace).
    if atom.predicate_variable {
        return Err(ReconciledReadError::UnsupportedShape(
            "predicate-variable atoms are unsupported on the reconciled present path",
        ));
    }
    // `present` has no object-KIND channel: a Node/Literal-restricted VARIABLE object (`Link`
    // / `Data`) cannot be enforced. A constant object already pins its kind through the
    // authenticated dictionary entry, and `attach_ranges` clears a ranged object's kind to Any,
    // so a residual non-Any kind here is a genuine restriction we cannot honor.
    if atom.object_kind != ObjectKindFilter::Any {
        return Err(ReconciledReadError::UnsupportedShape(
            "object-kind-restricted variable objects (Link/Data) are unsupported on the reconciled present path",
        ));
    }
    // A value-literal SUBJECT is unrepresentable in woql2 (the subject is a NodeValue); guard it
    // anyway so the mapping never silently hashes a nonsensical subject literal.
    if matches!(atom.subject, AtomTerm::ValueConstant(..)) {
        return Err(ReconciledReadError::UnsupportedShape(
            "value-literal subject is unsupported on the reconciled present path",
        ));
    }
    let subject = map_term(&atom.subject);
    let object = map_term(&atom.object);
    // The `present` path keeps >= 1 variable per atom; a fully-ground atom (both positions
    // constant, no same-row aliasing) has no kept column.
    if matches!(subject, PresentTerm::Constant(_))
        && matches!(object, PresentTerm::Constant(_))
        && !atom.subject_object_equal
    {
        return Err(ReconciledReadError::UnsupportedShape(
            "fully-ground atoms keep no variable and are unsupported on the reconciled present path",
        ));
    }
    Ok(PresentBgpAtom {
        subject,
        predicate_hash: hash_term(&atom.predicate_term),
        object,
        subject_object_equal: atom.subject_object_equal,
        // Both the base `LayerBgpAtom` and the store `PresentBgpAtom` carry the SAME
        // `terminus_store::proof::query::ObjectRangeSpec`, so the numeric-range filter maps by a
        // direct clone (op/datatype/constant/bit-width/second-bound preserved).
        object_range: atom.object_range.clone(),
    })
}

/// Translate a compiled leaf [`LayerBgpPlan`] to the store `present` inputs. The join plan is
/// shared verbatim (`JoinStep` is the same store type); only the atoms are re-classified.
fn lower_plan(plan: &LayerBgpPlan) -> Result<LoweredBgp, ReconciledReadError> {
    let atoms = plan.atoms.iter().map(map_atom).collect::<Result<Vec<_>, _>>()?;
    Ok(LoweredBgp { atoms, joins: plan.joins.clone() })
}

/// Compile `query` with the base [`plan_bgp`] (reusing its full BGP classification: constants /
/// variables / repeated vars / ranges, and its rejections), then lower the resulting leaf BGP to
/// the store `present` inputs. Reject-closes anything the `present` path cannot deliver.
fn lower_query(query: &Query) -> Result<LoweredBgp, ReconciledReadError> {
    let compiled = plan_bgp(query).map_err(|e| ReconciledReadError::NotBgp(e.to_string()))?;
    // The reconciled path delivers the FULL BGP relation's rows. A Select projection changes
    // which columns are delivered and a Count delivers a cardinality, not rows — reject both so
    // the delivered columns are exactly the compiled BGP schema.
    if compiled.projection.is_some() {
        return Err(ReconciledReadError::UnsupportedShape(
            "Select projection is unsupported on the reconciled present path (delivers the full BGP relation)",
        ));
    }
    if compiled.count_variable.is_some() {
        return Err(ReconciledReadError::UnsupportedShape(
            "Count is unsupported on the reconciled present path (it delivers rows, not a cardinality)",
        ));
    }
    // Only a plain leaf BGP maps to the store `present` BGP; every composite relation
    // (Distinct/GroupBy/join/order/anti/union/…) is out of scope for this path.
    let plan = compiled.relation.as_bgp().ok_or(ReconciledReadError::UnsupportedShape(
        "only a plain conjunctive BGP is supported on the reconciled present path (no distinct/group/join/order/etc.)",
    ))?;
    lower_plan(plan)
}

/// Returns `true` iff this WOQL query is routable to the reconciled delta-stack path.
pub fn is_reconciled_read(query: &Query) -> bool {
    lower_query(query).is_ok()
}

/// Prove a supported read-only WOQL query over an N-layer delta stack (base + deltas, head last)
/// via the store reconcile + present-BGP path. The returned proof carries the raw
/// authenticated result columns; [`verify_and_resolve_reconciled_read`] delivers the resolved
/// canonical answer.
pub fn prove_reconciled_read(
    query: &Query,
    layers: &[&LayerCommitment],
) -> Result<ReconciledReadProof, ReconciledReadError> {
    let LoweredBgp { atoms, joins } = lower_query(query)?;
    let (reconcile, bgp, result) = prove_read_bgp(layers, &atoms, &joins)
        .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
    Ok(ReconciledReadProof { reconcile, bgp, result, atoms, joins })
}

/// Verify a reconciled read proof against the trusted head root AND resolve its bindings to
/// canonical `CanonicalObject`s (real IRIs / typed literals) — the readable answer, no re-run.
pub fn verify_and_resolve_reconciled_read(
    query: &Query,
    layers: &[&LayerCommitment],
    trusted_head_root: ProofHash,
    proof: &ReconciledReadProof,
) -> Result<Vec<Vec<CanonicalObject>>, ReconciledReadError> {
    // Anti-substitution: bind the proof to the verifier's OWN compiled query. Re-derive the
    // lowered atom/join spec from `query` and require the proof to encode EXACTLY that spec, so a
    // valid proof for a DIFFERENT supported query (different constant/predicate/join plan) cannot
    // be accepted in this query's name. The verifier then feeds its OWN re-derived spec to the
    // store verifier (never the prover's), so the cryptographic statement is likewise bound to
    // the verifier's query.
    let expected = lower_query(query)?;
    if !bgp_spec_eq(&expected, proof) {
        return Err(ReconciledReadError::QueryShapeMismatch);
    }
    verify_and_resolve_bgp(
        layers,
        trusted_head_root,
        &expected.atoms,
        &expected.joins,
        &proof.reconcile,
        &proof.bgp,
        &proof.result.columns,
    )
    .map_err(|e| ReconciledReadError::Store(e.to_string()))
}

// --- Anti-substitution spec equality (field-wise; the store atom/join types are not `PartialEq`) ---

fn object_range_eq(a: &Option<ObjectRangeSpec>, b: &Option<ObjectRangeSpec>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.op == y.op
                && x.datatype == y.datatype
                && x.c == y.c
                && x.b_bits == y.b_bits
                && match (&x.second, &y.second) {
                    (None, None) => true,
                    (Some(bx), Some(by)) => bx.op == by.op && bx.c == by.c,
                    _ => false,
                }
        }
        _ => false,
    }
}

fn atom_eq(a: &PresentBgpAtom, b: &PresentBgpAtom) -> bool {
    a.subject == b.subject
        && a.predicate_hash == b.predicate_hash
        && a.object == b.object
        && a.subject_object_equal == b.subject_object_equal
        && object_range_eq(&a.object_range, &b.object_range)
}

fn join_eq(a: &JoinStep, b: &JoinStep) -> bool {
    a.acc_key == b.acc_key && a.rel_key == b.rel_key && a.additional_keys == b.additional_keys
}

fn bgp_spec_eq(expected: &LoweredBgp, proof: &ReconciledReadProof) -> bool {
    expected.atoms.len() == proof.atoms.len()
        && expected.joins.len() == proof.joins.len()
        && expected
            .atoms
            .iter()
            .zip(proof.atoms.iter())
            .all(|(x, y)| atom_eq(x, y))
        && expected
            .joins
            .iter()
            .zip(proof.joins.iter())
            .all(|(x, y)| join_eq(x, y))
}

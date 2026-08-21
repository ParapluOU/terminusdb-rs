//! Reconciled delta-stack reads — the compiler capstone.
//!
//! Routes a supported read-only WOQL query over an **N-layer delta stack** to the store's
//! reconcile + present-BGP + resolve path, delivering the end goal: *prove a read-only WOQL
//! query over the live delta stack, no re-run*. The chain (base + deltas, head last) is passed
//! in because a `LayerCommitment` only carries `parent_state_root`, not the parent object.
//!
//! Two store read paths sit behind one entry point, chosen by the compiled plan's **column
//! source** ([`ColumnSource`]):
//!   * **Materialized present** (`Triple`/`Link`/`Data`) — the query reads the reconciled head
//!     state; lowered to the store `present` BGP ([`prove_read_bgp`] / [`verify_and_resolve_bgp`]).
//!   * **Delta reads** (`AddedTriple` / `DeletedTriple`) — the query reads THIS commit's
//!     additions (resp. removals) file "no deeper"; lowered to the store delta-read BGP
//!     ([`prove_delta_read`] / [`verify_and_resolve_delta_read`]), which authenticates the head
//!     group's columns + constants against the trusted head root via the SAME cross-layer chain
//!     dictionaries the reconciliation uses (so an ancestor-defined constant still resolves).
//!
//! The BGP shape lowering is GENERAL and SHARED across both paths: any conjunctive BGP the base
//! path compiles ([`plan_bgp`]) — an n-atom left-deep join over shared variables, constants in any
//! position, repeated-variable same-row equality, and numeric object ranges — is mapped onto the
//! store `present`/`delta` BGP over the delta head. The compiled [`LayerBgpPlan`] is translated
//! ~1:1 to `Vec<PresentBgpAtom>` + `Vec<JoinStep>` (the store `PresentBgpAtom` MIRRORS the base
//! [`LayerBgpAtom`], restricted to a constant predicate; the [`JoinStep`] plan is shared verbatim).
//! The atom mapping is source-AGNOSTIC; the *route* (materialized vs additions vs removals) is
//! fixed by the plan's uniform column source and carried in the proof for anti-substitution.
//!
//! What still REJECT-CLOSES here (matching the store present/delta paths' limits):
//!   * anything [`plan_bgp`] refuses (non-BGP constructs, the schema graph, ranges on
//!     unsupported datatypes) — surfaced as [`ReconciledReadError::NotBgp`];
//!   * a `Select` projection or a `Count` wrapper (the reconciled path delivers the full BGP
//!     relation's rows, not a projection or a cardinality);
//!   * a non-leaf compiled relation (Distinct/GroupBy/Or/join/order/… — anything the store
//!     BGP cannot express);
//!   * a predicate VARIABLE atom (the store authenticates a CONSTANT predicate hash);
//!   * a MIXED-source conjunction (e.g. `AddedTriple(?x,p,?y) & Triple(?y,q,?z)`, or additions
//!     joined with removals) — each store read path reads a SINGLE committed group;
//!   * an object-KIND-restricted variable object (`Link`/`Data`) — no object-kind channel;
//!   * a fully-ground atom — the store BGP keeps at least one variable per atom.

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::dict::{hash_object, hash_term};
use terminus_store::proof::order::OrderDirection;
use terminus_store::proof::query::{
    AtomTerm, ColumnSource, LayerBgpAtom, LayerBgpPlan, ObjectKindFilter, ObjectRangeSpec,
};
use terminus_store::proof::reconcile::{
    decode_and_verify_delta_read, decode_and_verify_read_bgp, prove_delta_read,
    prove_read_group_count_bgp, prove_read_bgp, prove_read_order_bgp, verify_and_resolve_bgp,
    verify_and_resolve_delta_read, verify_and_resolve_group_count_bgp, verify_and_resolve_order_bgp,
    verify_read_bgp, DeltaReadProof, DeltaSource, PresentBgpAtom, PresentBgpProof, PresentReadResult,
    PresentScalarOrderProof, PresentTerm, ReconcileProof,
};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject, ProofHash};

use crate::proof::{plan_bgp, CompiledRelationStatement};
use crate::query::Query;

/// Every reason the reconciled read path refuses a query.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReconciledReadError {
    /// The compiled BGP uses a shape the store path cannot express (predicate variable, mixed
    /// column sources, object-kind filter, fully-ground atom, projection/count, or a non-leaf
    /// compiled relation).
    #[error("unsupported reconciled shape: {0}")]
    UnsupportedShape(&'static str),
    /// The proof's embedded atom/join/source spec does not match the verifier's OWN re-compiled
    /// query (anti-substitution: a proof for a different supported query was presented).
    #[error("proof shape does not match the verifier's query")]
    QueryShapeMismatch,
    /// The query does not compile as a supported BGP (`plan_bgp` rejected it: a non-BGP
    /// construct, the schema graph, an unsupported range datatype, …).
    #[error("query does not compile as a BGP: {0}")]
    NotBgp(String),
    /// The underlying store reconcile/present-BGP/delta-read/resolve path errored.
    #[error("store proof error: {0}")]
    Store(String),
}

/// Which store read path a lowered BGP routes to, fixed by the plan's uniform column source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadSource {
    /// All atoms read the reconciled materialized head (`Triple`/`Link`/`Data`).
    Materialized,
    /// All atoms read the head commit's additions file (`AddedTriple`).
    Additions,
    /// All atoms read the head commit's removals file (`DeletedTriple`).
    Removals,
}

/// A single-column, full-key, numeric `OrderBy` wrapping the reconciled materialized present BGP.
/// The datatype + direction are bound into the anti-substitution spec so a proof for an ascending
/// order (or a different numeric datatype) cannot be accepted in a descending order's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OrderSpec {
    datatype: CanonicalDatatype,
    direction: OrderDirection,
}

/// The lowered store-facing statement for a supported reconciled read: the mapped BGP atoms, the
/// left-deep join plan, the route (uniform column source), and an optional single-column full-key
/// numeric `OrderBy` over the materialized present. Re-derivable from a `&Query` by both prover and
/// verifier, so it is the anti-substitution binding.
/// Which per-group aggregate a reconciled `GroupBy` computes over the materialized present. This
/// slice ships `Count` (per-group cardinality, no value-authentication); `Sum`/`Min`/`Max` extend
/// it with a numeric value column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupAgg {
    /// `GroupBy(keys, ..) + Length` — the per-group solution count.
    Count,
}

/// A single-/multi-key `GroupBy` aggregate over the reconciled materialized present, recomputed by
/// the verifier over the root-authenticated child rows. `key_columns` index the child BGP schema
/// (== the store result column order). Bound into the anti-substitution spec so a proof for a
/// different grouping (or a different aggregate) cannot be accepted in this query's name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GroupSpec {
    key_columns: Vec<usize>,
    agg: GroupAgg,
}

#[derive(Clone, Debug)]
struct LoweredBgp {
    atoms: Vec<PresentBgpAtom>,
    joins: Vec<JoinStep>,
    source: ReadSource,
    /// `Some` iff an outer `OrderBy([v ASC|DESC], BGP)` wraps a single-column numeric materialized
    /// present BGP. `None` for a plain (unordered) read.
    order: Option<OrderSpec>,
    /// `true` iff an outer `Count(BGP)` wraps a plain leaf BGP over the materialized present: the
    /// delivered answer is the cardinality (`result_len`), already chain-authenticated by the
    /// present-BGP proof — no value-authentication, no new gadget. Mutually exclusive with `order`
    /// (a `Count` reject-closes an ordered/grouped/distinct/delta child; see `lower_query`).
    count: bool,
    /// `Some` iff a `GroupBy` aggregate over a plain leaf BGP over the materialized present. The
    /// per-group answer is a deterministic verifier-recompute over the root-authenticated child
    /// rows. Mutually exclusive with `order`/`count`.
    group: Option<GroupSpec>,
}

/// The store proof bundle for whichever read path the plan routed to.
enum ReadProofInner {
    /// Materialized present BGP over the reconciled head.
    Present { reconcile: ReconcileProof, bgp: PresentBgpProof },
    /// `Count(BGP)` over the reconciled materialized present. The store bundle is the SAME
    /// reconcile + present-BGP pair as [`ReadProofInner::Present`]; the cardinality is the
    /// present-BGP proof's chain-authenticated `result_len` (delivered via [`verify_read_bgp`]),
    /// so no additional aggregate gadget is carried.
    Count { reconcile: ReconcileProof, bgp: PresentBgpProof },
    /// `GroupCount(BGP)` over the reconciled materialized present. The store bundle is the SAME
    /// reconcile + present-BGP pair as [`ReadProofInner::Present`] (the child rows are delivered in
    /// `result` for authentication); the per-group counts are recomputed by the verifier over the
    /// root-authenticated resolved child rows, so no additional aggregate gadget is carried.
    GroupCount { reconcile: ReconcileProof, bgp: PresentBgpProof },
    /// Single-column full-key numeric `OrderBy` over the reconciled materialized present.
    PresentOrder {
        reconcile: ReconcileProof,
        bgp: PresentBgpProof,
        order: PresentScalarOrderProof,
    },
    /// Delta read over the head commit's additions/removals file.
    Delta { proof: DeltaReadProof },
}

/// A proved reconciled read: the store proof bundle plus the lowered atom/join/source spec it was
/// proved for (so [`verify_and_resolve_reconciled_read`] can bind it to the verifier's query).
pub struct ReconciledReadProof {
    inner: ReadProofInner,
    pub result: PresentReadResult,
    /// The lowered BGP atom spec the proof was produced for. Bound to the verifier's OWN
    /// re-compiled query at verify time (anti-substitution); the verifier NEVER trusts these for
    /// the store call, only compares them to its own re-derivation.
    pub atoms: Vec<PresentBgpAtom>,
    pub joins: Vec<JoinStep>,
    source: ReadSource,
    /// The lowered `OrderBy` spec the proof was produced for (bound to the verifier's re-compiled
    /// query for anti-substitution), or `None` for a plain unordered read.
    order: Option<OrderSpec>,
    /// `true` iff this proof was produced for a `Count(BGP)` (bound to the verifier's re-compiled
    /// query for anti-substitution): a count proof must not be accepted in a plain read's name and
    /// vice versa.
    count: bool,
    /// The `GroupBy` aggregate spec this proof was produced for (bound to the verifier's re-compiled
    /// query for anti-substitution): a proof for a different grouping/aggregate cannot be accepted
    /// in this query's name.
    group: Option<GroupSpec>,
}

impl ReconciledReadProof {
    /// The prover's (unverified) result columns, for convenience/inspection. The trusted,
    /// resolved answer comes from [`verify_and_resolve_reconciled_read`].
    pub fn result_columns(&self) -> &[Vec<ArkFr>] {
        &self.result.columns
    }
}

/// Map one base [`AtomTerm`] to a store `PresentTerm`: a query variable stays a variable; a node
/// or (object) value constant becomes its public `hash_object(...)`.
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

/// Render a nonneg cardinality as the reconciled `Count`'s single delivered answer cell. The value
/// is the exact solution count — numerically identical to TerminusDB's `aggregate_all(count, …)`
/// — delivered as a `UInt64` value object (an internal integer rendering of the count; the count
/// itself is a cardinality, not a dictionary-resolved binding).
fn count_object(cardinality: usize) -> CanonicalObject {
    CanonicalObject::Value {
        datatype: CanonicalDatatype::UInt64,
        lexical: (cardinality as u64).to_be_bytes().to_vec(),
    }
}

/// The route for one compiled atom's column source. Delta atoms map onto the delta-read path;
/// materialized atoms onto the present path. Mixed sources are rejected at the plan level.
fn classify_source(source: ColumnSource) -> ReadSource {
    match source {
        ColumnSource::Materialized => ReadSource::Materialized,
        ColumnSource::Additions => ReadSource::Additions,
        ColumnSource::Removals => ReadSource::Removals,
    }
}

/// Map one compiled [`LayerBgpAtom`] to a store [`PresentBgpAtom`], reject-closing every SHAPE the
/// store BGP cannot express (see the module docs). Source-AGNOSTIC: the atom's column source is
/// classified separately ([`uniform_source`]) so a single mapping serves both the materialized and
/// delta-read routes.
fn map_atom(atom: &LayerBgpAtom) -> Result<PresentBgpAtom, ReconciledReadError> {
    // The store path authenticates a CONSTANT predicate hash; a predicate variable is not
    // supported (its ids live in an independent Store namespace).
    if atom.predicate_variable {
        return Err(ReconciledReadError::UnsupportedShape(
            "predicate-variable atoms are unsupported on the reconciled path",
        ));
    }
    // The store BGP has no object-KIND channel: a Node/Literal-restricted VARIABLE object (`Link`
    // / `Data`) cannot be enforced. A constant object already pins its kind through the
    // authenticated dictionary entry, and `attach_ranges` clears a ranged object's kind to Any,
    // so a residual non-Any kind here is a genuine restriction we cannot honor.
    if atom.object_kind != ObjectKindFilter::Any {
        return Err(ReconciledReadError::UnsupportedShape(
            "object-kind-restricted variable objects (Link/Data) are unsupported on the reconciled path",
        ));
    }
    // A value-literal SUBJECT is unrepresentable in woql2 (the subject is a NodeValue); guard it
    // anyway so the mapping never silently hashes a nonsensical subject literal.
    if matches!(atom.subject, AtomTerm::ValueConstant(..)) {
        return Err(ReconciledReadError::UnsupportedShape(
            "value-literal subject is unsupported on the reconciled path",
        ));
    }
    let subject = map_term(&atom.subject);
    let object = map_term(&atom.object);
    // The store BGP keeps >= 1 variable per atom; a fully-ground atom (both positions constant, no
    // same-row aliasing) has no kept column.
    if matches!(subject, PresentTerm::Constant(_))
        && matches!(object, PresentTerm::Constant(_))
        && !atom.subject_object_equal
    {
        return Err(ReconciledReadError::UnsupportedShape(
            "fully-ground atoms keep no variable and are unsupported on the reconciled path",
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

/// The plan's uniform column source, rejecting a mixed-source conjunction (each store read path
/// reads a SINGLE committed group, so all atoms must share one source).
fn uniform_source(plan: &LayerBgpPlan) -> Result<ReadSource, ReconciledReadError> {
    let mut atoms = plan.atoms.iter();
    let first = atoms
        .next()
        .ok_or(ReconciledReadError::UnsupportedShape("empty BGP has no atoms"))?;
    let source = classify_source(first.source);
    for atom in atoms {
        if classify_source(atom.source) != source {
            return Err(ReconciledReadError::UnsupportedShape(
                "mixed column sources (e.g. AddedTriple joined with Triple, or additions joined with removals) are unsupported: each store read path reads a single committed group",
            ));
        }
    }
    Ok(source)
}

/// Translate a compiled leaf [`LayerBgpPlan`] to the store BGP inputs (atoms + join plan + route).
/// The join plan is shared verbatim (`JoinStep` is the same store type); only the atoms are
/// re-classified and the uniform source is captured.
fn lower_plan(plan: &LayerBgpPlan) -> Result<LoweredBgp, ReconciledReadError> {
    let source = uniform_source(plan)?;
    let atoms = plan.atoms.iter().map(map_atom).collect::<Result<Vec<_>, _>>()?;
    Ok(LoweredBgp {
        atoms,
        joins: plan.joins.clone(),
        source,
        order: None,
        count: false,
        group: None,
    })
}

/// Compile `query` with the base [`plan_bgp`] (reusing its full BGP classification: constants /
/// variables / repeated vars / ranges / column source, and its rejections), then lower the
/// resulting leaf BGP to the store inputs. Reject-closes anything the store paths cannot deliver.
fn lower_query(query: &Query) -> Result<LoweredBgp, ReconciledReadError> {
    let compiled = plan_bgp(query).map_err(|e| ReconciledReadError::NotBgp(e.to_string()))?;
    // COUNT — `Count(BGP)` over the reconciled materialized present. The count wrapper sets
    // `count_variable = Some(_)` AND `projection = Some(vec![])`, so it MUST be handled BEFORE the
    // projection reject below. The cardinality is the present-BGP result-relation row count
    // (`result_len`), already chain-authenticated — the join-count identity inside the BGP chain
    // (resp. the single-atom filter count) proves it — so no value-authentication and no new
    // aggregate gadget is required (full natural-join assignments over the set-valued present are
    // already unique, so the row count IS the WOQL `aggregate_all(count, …)` solution count). Only
    // a plain leaf BGP over the materialized present is accepted; a Distinct-collapsed, ordered,
    // grouped, joined-composite, or delta-source count reject-closes (fail-closed over coverage).
    if compiled.count_variable.is_some() {
        // `Count(Distinct(...))` dedups before counting — a cardinality this slice does not prove.
        if compiled.full_row_distinct {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled Count over a Distinct relation is unsupported (dedup changes the cardinality)",
            ));
        }
        let plan = compiled.relation.as_bgp().ok_or(ReconciledReadError::UnsupportedShape(
            "reconciled Count requires a plain leaf BGP input (no distinct/group/order/join composite)",
        ))?;
        let mut lowered = lower_plan(plan)?;
        // A count is proven ONLY over the reconciled materialized present; a delta-source count
        // (Count over AddedTriple/DeletedTriple) is out of scope this slice.
        if lowered.source != ReadSource::Materialized {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled Count is supported only over the materialized present (not delta additions/removals)",
            ));
        }
        lowered.count = true;
        return Ok(lowered);
    }
    // The reconciled path delivers the FULL BGP relation's rows. A Select projection changes
    // which columns are delivered — reject it so the delivered columns are exactly the compiled
    // BGP schema. (A bare `Count` is handled above; anything still carrying a projection here is a
    // genuine `Select` whose projected rows this path does not deliver.)
    if compiled.projection.is_some() {
        return Err(ReconciledReadError::UnsupportedShape(
            "Select projection is unsupported on the reconciled path (delivers the full BGP relation)",
        ));
    }
    // A single-column, full-key, numeric `OrderBy` over a plain leaf BGP is supported over the
    // reconciled MATERIALIZED present: the compiler's `plan_exact_scalar_order` already reject-closes
    // subset-key / multi-key / non-numeric / float / `-0.0` orders (F4-max), so an `ExactScalarOrder`
    // reaching here is guaranteed full-key numeric. Multi-key (`ExactMultiOrder`), ordered slices,
    // group-by, and every other composite stay reject-closed below. The ordered input must itself be
    // a plain leaf BGP with a single column (the store gadget sorts one authenticated value column).
    if let Some((input, datatype, direction)) = compiled.relation.as_exact_scalar_order_with_direction() {
        if input.schema.len() != 1 {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled OrderBy supports only a single-column (full-key) numeric order",
            ));
        }
        let plan = input.as_bgp().ok_or(ReconciledReadError::UnsupportedShape(
            "reconciled OrderBy requires a plain leaf BGP input (no distinct/group/join/order/etc.)",
        ))?;
        let mut lowered = lower_plan(plan)?;
        // Order is proven ONLY over the reconciled materialized present; a delta-source ordered read
        // (OrderBy over AddedTriple/DeletedTriple) is out of scope.
        if lowered.source != ReadSource::Materialized {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled OrderBy is supported only over the materialized present (not delta additions/removals)",
            ));
        }
        lowered.order = Some(OrderSpec { datatype, direction });
        return Ok(lowered);
    }
    // GROUP-COUNT — `GroupBy(keys, ..) + Length` over the reconciled materialized present. The
    // per-group solution count is a DETERMINISTIC recompute over the root-authenticated child rows
    // (verifier-recompute; the child rows are authenticated + resolved by the store, then grouped by
    // the authenticated key IRIs and counted). No value-authentication (a count is cardinality-based;
    // the group KEYS are authenticated node ids). Only the BARE GroupCount over a plain leaf BGP over
    // the materialized present is accepted; a comparison-filtered group count (a `GeneratedU64Filter`
    // wrapper), a grouped value aggregate (sum/extrema), a composite child, or a delta source
    // reject-close (fail-closed over coverage). The base `plan_group_count_form` already restricts
    // the group keys to the Node domain, so a value-keyed grouping never reaches here.
    if let CompiledRelationStatement::GroupCount { input, key_columns } = &compiled.relation.statement {
        let plan = input.as_bgp().ok_or(ReconciledReadError::UnsupportedShape(
            "reconciled GroupCount requires a plain leaf BGP child (no distinct/group/order/join composite)",
        ))?;
        let mut lowered = lower_plan(plan)?;
        // A group aggregate is proven ONLY over the reconciled materialized present.
        if lowered.source != ReadSource::Materialized {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled GroupCount is supported only over the materialized present (not delta additions/removals)",
            ));
        }
        if key_columns.is_empty() {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled GroupCount requires at least one group key",
            ));
        }
        lowered.group = Some(GroupSpec { key_columns: key_columns.clone(), agg: GroupAgg::Count });
        return Ok(lowered);
    }
    // Only a plain leaf BGP maps to the store BGP; every OTHER composite relation
    // (Distinct/GroupBy/join/multi-key-order/…) is out of scope for this path.
    let plan = compiled.relation.as_bgp().ok_or(ReconciledReadError::UnsupportedShape(
        "only a plain conjunctive BGP (optionally wrapped by a single-column numeric OrderBy) is supported on the reconciled path (no distinct/group/join/multi-key-order/etc.)",
    ))?;
    lower_plan(plan)
}

/// Returns `true` iff this WOQL query is routable to the reconciled delta-stack path (materialized
/// present OR a delta read).
pub fn is_reconciled_read(query: &Query) -> bool {
    lower_query(query).is_ok()
}

/// Prove a supported read-only WOQL query over an N-layer delta stack (base + deltas, head last).
/// A `Triple`/`Link`/`Data` query proves over the reconciled materialized head; an
/// `AddedTriple`/`DeletedTriple` query proves over the head commit's additions/removals file. The
/// returned proof carries the raw authenticated result columns;
/// [`verify_and_resolve_reconciled_read`] delivers the resolved canonical answer.
pub fn prove_reconciled_read(
    query: &Query,
    layers: &[&LayerCommitment],
) -> Result<ReconciledReadProof, ReconciledReadError> {
    let LoweredBgp { atoms, joins, source, order, count, group } = lower_query(query)?;
    if count {
        // COUNT over the reconciled materialized present. `lower_query` guarantees
        // `source == Materialized` and `order == None`. The store bundle is the SAME reconcile +
        // present-BGP pair a plain read produces; the delivered answer is the single-cell
        // cardinality (`result_len`), re-derived and bound at verify via [`verify_read_bgp`].
        let (reconcile, bgp, _rows) = prove_read_bgp(layers, &atoms, &joins)
            .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
        let cardinality = bgp.result_len;
        let result = PresentReadResult {
            columns: vec![vec![ArkFr::from_u64(cardinality as u64)]],
            len: 1,
            id_hints: Vec::new(),
        };
        return Ok(ReconciledReadProof {
            inner: ReadProofInner::Count { reconcile, bgp },
            result,
            atoms,
            joins,
            source,
            order,
            count,
            group,
        });
    }
    // GROUP aggregate over the reconciled materialized present. `lower_query` guarantees
    // `source == Materialized`, `order == None`, `count == false`. The store bundle is the SAME
    // reconcile + present-BGP pair a plain read produces; the CHILD rows are delivered in `result`
    // for authentication, and the per-group aggregate is recomputed by the verifier.
    if let Some(agg) = group.as_ref().map(|spec| spec.agg) {
        match agg {
            GroupAgg::Count => {
                let (reconcile, bgp, result) = prove_read_group_count_bgp(layers, &atoms, &joins)
                    .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
                return Ok(ReconciledReadProof {
                    inner: ReadProofInner::GroupCount { reconcile, bgp },
                    result,
                    atoms,
                    joins,
                    source,
                    order,
                    count,
                    group,
                });
            }
        }
    }
    let (inner, result) = match (source, order) {
        // Single-column full-key numeric OrderBy over the reconciled materialized present.
        (ReadSource::Materialized, Some(spec)) => {
            let (reconcile, bgp, order_proof, result) =
                prove_read_order_bgp(layers, &atoms, &joins, spec.datatype, spec.direction)
                    .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
            (ReadProofInner::PresentOrder { reconcile, bgp, order: order_proof }, result)
        }
        (ReadSource::Materialized, None) => {
            let (reconcile, bgp, result) = prove_read_bgp(layers, &atoms, &joins)
                .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
            (ReadProofInner::Present { reconcile, bgp }, result)
        }
        (ReadSource::Additions | ReadSource::Removals, None) => {
            let delta_source = match source {
                ReadSource::Additions => DeltaSource::Additions,
                ReadSource::Removals => DeltaSource::Removals,
                ReadSource::Materialized => unreachable!(),
            };
            let (proof, result) = prove_delta_read(layers, delta_source, &atoms, &joins)
                .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
            (ReadProofInner::Delta { proof }, result)
        }
        // OrderBy over a delta source is reject-closed by `lower_query`; unreachable here.
        (ReadSource::Additions | ReadSource::Removals, Some(_)) => {
            return Err(ReconciledReadError::UnsupportedShape(
                "reconciled OrderBy is supported only over the materialized present (not delta additions/removals)",
            ));
        }
    };
    Ok(ReconciledReadProof { inner, result, atoms, joins, source, order, count, group })
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
    // lowered atom/join/source spec from `query` and require the proof to encode EXACTLY that spec,
    // so a valid proof for a DIFFERENT supported query (different constant/predicate/join plan/
    // source) cannot be accepted in this query's name. The verifier then feeds its OWN re-derived
    // spec to the store verifier (never the prover's), so the cryptographic statement is likewise
    // bound to the verifier's query.
    let expected = lower_query(query)?;
    if !bgp_spec_eq(&expected, proof) {
        return Err(ReconciledReadError::QueryShapeMismatch);
    }
    // COUNT — the delivered answer is the single-cell cardinality, not resolved rows. The
    // present-BGP proof's chain-authenticated result-row count is re-derived here ([`verify_read_bgp`]
    // returns it); the delivered cell is then bound to it so a tampered count rejects.
    if expected.count {
        let ReadProofInner::Count { reconcile, bgp } = &proof.inner else {
            return Err(ReconciledReadError::QueryShapeMismatch);
        };
        let cardinality = verify_read_bgp(
            layers,
            trusted_head_root,
            &expected.atoms,
            &expected.joins,
            reconcile,
            bgp,
        )
        .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
        if proof.result.columns.len() != 1
            || proof.result.columns[0].len() != 1
            || proof.result.columns[0][0] != ArkFr::from_u64(cardinality as u64)
        {
            return Err(ReconciledReadError::Store(
                "reconciled count: delivered count cell does not match the chain-authenticated cardinality".to_string(),
            ));
        }
        return Ok(vec![vec![count_object(cardinality)]]);
    }
    // GROUP aggregate — the per-group answer is a deterministic verifier-recompute over the
    // root-authenticated resolved child rows (the store authenticates + resolves the child relation,
    // then groups by the authenticated key IRIs). The result is returned COLUMN-major (keys...,
    // aggregate), matching the plain-read resolver convention.
    if let Some(spec) = expected.group.as_ref() {
        match spec.agg {
            GroupAgg::Count => {
                let ReadProofInner::GroupCount { reconcile, bgp } = &proof.inner else {
                    return Err(ReconciledReadError::QueryShapeMismatch);
                };
                let groups = verify_and_resolve_group_count_bgp(
                    layers,
                    trusted_head_root,
                    &expected.atoms,
                    &expected.joins,
                    &spec.key_columns,
                    reconcile,
                    bgp,
                    &proof.result.columns,
                )
                .map_err(|e| ReconciledReadError::Store(e.to_string()))?;
                // Column-major output: one key column per group key, plus the per-group count as a
                // trailing UInt64 cell (the same rendering `Count` uses).
                let ncols = spec.key_columns.len() + 1;
                let mut columns: Vec<Vec<CanonicalObject>> = vec![Vec::with_capacity(groups.len()); ncols];
                for (keys, group_count) in groups {
                    if keys.len() != spec.key_columns.len() {
                        return Err(ReconciledReadError::Store(
                            "reconciled group-count: recomputed group key arity mismatch".to_string(),
                        ));
                    }
                    for (column, key) in columns.iter_mut().zip(keys) {
                        column.push(key);
                    }
                    columns[ncols - 1].push(count_object(group_count as usize));
                }
                return Ok(columns);
            }
        }
    }
    match (expected.source, expected.order, &proof.inner) {
        (ReadSource::Materialized, None, ReadProofInner::Present { reconcile, bgp }) => {
            verify_and_resolve_bgp(
                layers,
                trusted_head_root,
                &expected.atoms,
                &expected.joins,
                reconcile,
                bgp,
                &proof.result.columns,
            )
            .map_err(|e| ReconciledReadError::Store(e.to_string()))
        }
        (
            ReadSource::Materialized,
            Some(spec),
            ReadProofInner::PresentOrder { reconcile, bgp, order },
        ) => verify_and_resolve_order_bgp(
            layers,
            trusted_head_root,
            &expected.atoms,
            &expected.joins,
            spec.datatype,
            spec.direction,
            reconcile,
            bgp,
            order,
            &proof.result.columns,
        )
        .map_err(|e| ReconciledReadError::Store(e.to_string())),
        (ReadSource::Additions | ReadSource::Removals, None, ReadProofInner::Delta { proof: delta }) => {
            let delta_source = match expected.source {
                ReadSource::Additions => DeltaSource::Additions,
                ReadSource::Removals => DeltaSource::Removals,
                ReadSource::Materialized => unreachable!(),
            };
            verify_and_resolve_delta_read(
                layers,
                trusted_head_root,
                delta_source,
                &expected.atoms,
                &expected.joins,
                delta,
                &proof.result.columns,
            )
            .map_err(|e| ReconciledReadError::Store(e.to_string()))
        }
        // The proof's stored route disagrees with the verifier's re-compiled query's route.
        _ => Err(ReconciledReadError::QueryShapeMismatch),
    }
}

/// C4 byte-in DB-free FFI: verify a delta-read plain-BGP proof shipped as BYTES against the trusted
/// head root, driven by the VERIFIER's OWN WOQL `query`. Re-lowers the query to `(atoms, joins,
/// source)` — the spec comes from the verifier's query, NEVER from the proof bytes
/// (anti-substitution) — maps the route to a [`DeltaSource`] (a materialized route is not a delta
/// read and is rejected as a shape mismatch), then hands the encoded proof + the encoded term-less
/// `VerifierCommitment` chain to the store's byte-in [`decode_and_verify_delta_read`]. Mirrors
/// [`crate::proof::prove_entry::decode_verifier_commitment_and_verify_envelope`]. Returns the
/// authenticated result row count.
pub fn decode_and_verify_reconciled_delta_read(
    query: &Query,
    delta_proof_bytes: &[u8],
    verifier_commitment_bytes: &[&[u8]],
    trusted_head_root: ProofHash,
) -> Result<usize, ReconciledReadError> {
    // A materialized OrderBy also lowers to `source == Materialized`, so it is refused by the
    // `Materialized` arm below (this byte-in path ships a `DeltaReadProof`, never an order proof).
    let LoweredBgp { atoms, joins, source, order: _, count: _, group: _ } = lower_query(query)?;
    let delta_source = match source {
        ReadSource::Additions => DeltaSource::Additions,
        ReadSource::Removals => DeltaSource::Removals,
        // A materialized (reconciled-present) read is NOT a delta read; refuse rather than guess.
        ReadSource::Materialized => {
            return Err(ReconciledReadError::UnsupportedShape(
                "delta-read byte-in path requires an AddedTriple/DeletedTriple query (a materialized read is not a delta read)",
            ));
        }
    };
    decode_and_verify_delta_read(
        delta_proof_bytes,
        verifier_commitment_bytes,
        trusted_head_root,
        delta_source,
        &atoms,
        &joins,
    )
    .map_err(|e| ReconciledReadError::Store(e.to_string()))
}

/// C4-slice-2 byte-in DB-free FFI: verify a reconciled MATERIALIZED-present plain-BGP read shipped
/// as BYTES against the trusted head root, driven by the VERIFIER's OWN WOQL `query`. Re-lowers the
/// query to `(atoms, joins, source)` — the spec comes from the verifier's query, NEVER from the
/// proof bytes (anti-substitution) — and requires the route to be `Materialized`: an
/// `AddedTriple`/`DeletedTriple` (delta) query is rejected (a delta read ships a `DeltaReadProof`,
/// not a reconciled present read — use [`decode_and_verify_reconciled_delta_read`]). Then hands the
/// encoded proof + the encoded term-less `VerifierCommitment` chain to the store's byte-in
/// [`decode_and_verify_read_bgp`]. Returns the authenticated result row count.
pub fn decode_and_verify_reconciled_read_bgp(
    query: &Query,
    read_proof_bytes: &[u8],
    verifier_commitment_bytes: &[&[u8]],
    trusted_head_root: ProofHash,
) -> Result<usize, ReconciledReadError> {
    let LoweredBgp { atoms, joins, source, order, count, group } = lower_query(query)?;
    // This is the MATERIALIZED-present read; a delta-source query is a different proof shape.
    if source != ReadSource::Materialized {
        return Err(ReconciledReadError::UnsupportedShape(
            "reconciled-present byte-in path requires a materialized Triple/Link/Data query (an AddedTriple/DeletedTriple read is a delta read: use decode_and_verify_reconciled_delta_read)",
        ));
    }
    // An OrderBy ships a distinct (reconcile + BGP + order) proof bundle, not a plain present read;
    // the ordered byte-in path is out of scope for this slice, so reject rather than mis-decode.
    if order.is_some() {
        return Err(ReconciledReadError::UnsupportedShape(
            "reconciled-present byte-in path does not support OrderBy (an ordered read ships a distinct proof bundle)",
        ));
    }
    // A Count delivers a cardinality, not resolved rows; the byte-in count path is out of scope for
    // this slice (use the in-process verify_and_resolve_reconciled_read for counts).
    if count {
        return Err(ReconciledReadError::UnsupportedShape(
            "reconciled-present byte-in path does not support Count (a count delivers a cardinality, not rows)",
        ));
    }
    // A GroupBy aggregate delivers recomputed group rows, not the plain BGP relation; the byte-in
    // group path is out of scope for this slice.
    if group.is_some() {
        return Err(ReconciledReadError::UnsupportedShape(
            "reconciled-present byte-in path does not support GroupBy (a group aggregate delivers recomputed group rows)",
        ));
    }
    decode_and_verify_read_bgp(
        read_proof_bytes,
        verifier_commitment_bytes,
        trusted_head_root,
        &atoms,
        &joins,
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
    expected.source == proof.source
        // The OrderBy spec (datatype + direction) is part of the anti-substitution binding: an
        // ascending proof (or a different numeric datatype) cannot be accepted in a descending
        // order's name, nor an ordered proof for an unordered query (or vice versa).
        && expected.order == proof.order
        // The Count flag is part of the anti-substitution binding: a count proof cannot be
        // accepted in a plain read's name (or vice versa).
        && expected.count == proof.count
        // The GroupBy spec (key columns + aggregate kind) is part of the anti-substitution binding:
        // a proof for a different grouping/aggregate cannot be accepted in this query's name, nor a
        // grouped proof for an ungrouped query (or vice versa).
        && expected.group == proof.group
        && expected.atoms.len() == proof.atoms.len()
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

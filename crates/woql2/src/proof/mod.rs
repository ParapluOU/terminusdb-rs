//! Compile a WOQL conjunctive BGP AST into a safe, variable-blind [`LayerBgpPlan`].
//!
//! See the crate docs for why the PLAN is the sole safety line. This module accepts
//! only the fragment the authenticated left-deep join chain can encode faithfully and
//! rejects everything else closed:
//!
//! * zero or more nested outer `Select`s (projection is read-side only — the proof
//!   still proves the FULL BGP), wrapping
//! * an [`And`] (nested `And`s flattened) of `k >= 1` [`Triple`]s, each a
//!   `<subj> <predicate> <obj>` atom where the predicate is either a constant IRI or a
//!   query variable retained from the authenticated P column, and each of subject/object
//!   is EITHER a query VARIABLE (a kept output column) OR an
//!   aligned CONSTANT — a subject/object node IRI ([`AtomTerm::NodeConstant`]) or an
//!   object typed literal of a round-trip-aligned xsd datatype ([`AtomTerm::ValueConstant`]:
//!   all scalar `XSDAnySimpleType` variants, routed through the Store's
//!   typed-value construction). Each atom keeps one output column per variable (S,P,O order);
//!   the atoms join into a left-deep chain. Shared variables use ordinary natural joins;
//!   disconnected components use the Store's verifier-known UNIT-key Cartesian product.
//!   Two-variable graph keys use the Store's injective two-u64 packing and remain bound
//!   to the original columns.
//!
//! Fully-ground atoms are authenticated zero-arity existence relations and gate the
//! relational result. Every emitted constant is one the store
//! can authenticate against its committed node/value dictionary, canonicalized IDENTICALLY
//! to how the store stored it (node IRIs by their IRI bytes; `xsd:string` literals by
//! routing through the store's own value construction).
//!
//! ## Where this fits in compile → prove → verify
//!
//! This module is the **compile** step. It lowers a trusted WOQL query into a variable-blind
//! [`LayerBgpPlan`] of store gadgets; the store then proves that plan and a verifier
//! RE-DERIVES the SAME plan from its OWN copy of the query — never from the proof — and checks
//! it. That re-derivation is the *anti-substitution* property: a proof for one query cannot
//! masquerade as the answer to another. The plan is where semantic **fidelity** is enforced
//! (anything it cannot encode faithfully is rejected closed, rather than proved approximately);
//! the store gadgets it targets provide the cryptographic **soundness**. Both are required and
//! they are NOT the same property — see the store-side developer primer,
//! [`terminus_store::proof::primer`] (esp. §10 prove→verify + anti-substitution, §11
//! soundness-vs-fidelity, §6 gadgets), for the cryptography vocabulary used throughout this
//! module (commitment, sumcheck, gadget, Fiat–Shamir, `resolution_root`).

use chrono::Datelike;
use dory_pcs::backends::arkworks::{ArkFr, ArkGT};
use dory_pcs::primitives::arithmetic::Field;
use dory_pcs::primitives::transcript::Transcript;
use sha2::{Digest, Sha256};
use tdb_succinct::{AnyURI, Date, HexBinary, TdbDataType, TypedDictEntry};
use terminus_store::proof::anti::{
    verify_global_left_outer_join, verify_global_not, verify_left_anti_join_with_keys,
    verify_inner_join_with_keys, verify_left_outer_join_with_keys, InnerJoinProof,
    LeftAntiJoinProof, LeftOuterJoinProof,
};
use terminus_store::proof::argument::{commit_column, StoreProverContext, Transcript254};
use terminus_store::proof::bgp::{JoinStep, RelationCommit};
use terminus_store::proof::commitment::{
    LayerCommitment, VerifierCommitment, VerifierLayer,
};
use terminus_store::proof::envelope::{
    plan_digest, prove_ordered_array_walk_reconciled_with_context,
    prove_ordered_list_walk_reconciled_with_context,
    prove_reconciled_transitive_closure_with_context, resolve_reconciled_node_ids,
    resolve_relation_equality_constraint, resolve_tuple_disequality_pairs,
    verify_ordered_array_walk_reconciled, verify_ordered_list_walk_reconciled,
    verify_reconciled_transitive_closure, CollectionSumTemplate,
    EqualityValueDomain,
    GeneratedU64ComparisonOp, LayerBgpEnvelope, OrderedArrayWalkReconciledProof,
    OrderedListWalkReconciledProof, ReconciledClosureProof,
    RecursiveRelationEnvelope, RecursiveRelationProof, RelationEqualityPlan, RelationProofPlan,
    RelationProofPlanKind, TransitiveClosureFixpoint, TransitiveClosureRound, TransitiveClosureSeed,
    TupleDisequalityNode, TupleDisequalityPairPlan,
    MAX_RELATION_PROOF_DEPTH, MAX_RELATION_PROOF_NODES,
};
use terminus_store::proof::disequality::{verify_tuple_disequality, TupleDisequalityProof};
use terminus_store::proof::equality::{
    verify_relation_equality, RelationEqualityProof,
};
use terminus_store::proof::distinct::{verify_relation_distinct, RelationDistinctProof};
use terminus_store::proof::group::GroupExtremaKind;
use terminus_store::proof::order::{verify_exact_scalar_order, ExactScalarOrderProof, OrderDirection};
use terminus_store::proof::multiorder::{verify_exact_multi_order, ExactMultiOrderProof};
use terminus_store::proof::query::{
    build_termless_bgp_openings, resolve_result_values, resolve_result_values_with_openings,
    resolve_typed_result_objects, resolve_typed_result_objects_with_openings, result_id_namespaces,
    verify_layer_bgp, verify_layer_bgp_result_log_domain,
    verify_result_columns, generated_decimal_i64_field, generated_decimal_i64_value,
    generated_decimal_u64_value, AtomTerm, CanonicalResultValue, ColumnSource, LayerBgpAtom,
    LayerBgpPlan,
    LayerBgpProof, ObjectKindFilter, ObjectRangeBound, ObjectRangeSpec,
    ResultColumnDescriptor, ResultIdNamespace,
};
use terminus_store::proof::ResolutionOpenings;
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::union::{verify_bag_union, BagUnionProof};
use terminus_store::proof::{node_value_op_value, CanonicalDatatype, CanonicalObject, ProofHash};
use terminus_store::store::sync::SyncStoreLayer;
use terminus_store::ValueTriple;

// Exact PURE-RUST big rationals + the store's decimal storage decoder for the xsd:decimal/rational
// value layer (`Floor`, `Divide`, decimal operands). `num-bigint`/`num-rational` are GMP-free (the
// same backend the workspace's `rug` shim uses) so the proof compiler stays cross-compilable. The
// reference's `casting:rational_to_decimal_string/3` is itself exact integer arithmetic, so the
// `BigInt` reproduction in `rational_to_decimal_string_20` matches it bit-for-bit.
// `storage_to_decimal` is the store's own `xsd:decimal` lexical decoder (BCD storage bytes ->
// canonical decimal string), the exact inverse of the encoding a stored decimal's
// `CanonicalObject::Value` lexical carries.
use num_bigint::{BigInt, Sign};
use num_integer::Integer as _;
use num_rational::BigRational;
use num_traits::{Num as _, One as _, Pow as _, Signed as _, ToPrimitive as _, Zero as _};
use tdb_succinct::tfc::decimal::storage_to_decimal;
// The store's own arbitrary-precision integer decoder (exact inverse of `bigint_to_storage`) — used
// to render a root-authenticated `xsd:integer` (`CanonicalDatatype::BigInt`) source to base-10 for
// the number→string cast. `rug::Integer` Display is base-10, matching SWI `format ~w`.
use tdb_succinct::tfc::integer::storage_to_bigint;

use crate::expression::{ArithmeticExpression, ArithmeticValue};
use crate::misc::InRange;
use crate::query::{And, Query};
use crate::triple::Triple;
use crate::value::{DataValue, ListOrVariable, NodeValue, Value};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

/// Prover-context selector re-exports for the high-level prove API.
///
/// The `CompiledBgp::prove_*_with_context` entries take a
/// [`StoreProverContext`](terminus_store::proof::argument::StoreProverContext).
/// Re-exporting the context (and, when a `blitzar-*` feature is enabled, the
/// Blitzar backend selector types) here lets downstream callers (node / SDK)
/// pick the prover backend WITHOUT taking a direct `terminus-store` dependency.
///
/// The default backend is `StoreProverContext::cpu()`; `StoreProverContext::blitzar(&rt, tier)`
/// selects Blitzar MSM acceleration (GPU under `blitzar-gpu`, CPU-MSM under `blitzar-cpu`).
pub mod prover {
    /// The prover-context handle passed to the `CompiledBgp::prove_*_with_context` entries.
    pub use terminus_store::proof::argument::StoreProverContext;

    /// Blitzar (GPU / CPU-MSM) backend selector types. Only available when a
    /// `blitzar-cpu` / `blitzar-gpu` feature is compiled in.
    ///
    /// Usage:
    /// ```ignore
    /// use terminusdb_woql2::prover::{BlitzarRuntime, BlitzarRuntimeConfig, ProverSetupTier, StoreProverContext};
    /// let rt = StoreProverContext::initialize_blitzar(BlitzarRuntimeConfig { num_precomputed_generators: 0 });
    /// let ctx = StoreProverContext::blitzar(&rt, ProverSetupTier::Log20)?;
    /// let proved = compiled.prove_with_context(&ctx, &layer)?;
    /// ```
    #[cfg(any(feature = "blitzar-cpu", feature = "blitzar-gpu"))]
    pub use terminus_store::proof::argument::{BlitzarRuntime, BlitzarRuntimeConfig, ProverSetupTier};
}

// ---------------------------------------------------------------------------
// Module layout.
//
// This file was `proof.rs`, a single ~17k-line module; it is now split into
// cohesive submodules by concern. The split is purely structural — no behavior,
// logic, signatures, or external visibility changed. Every prior
// `crate::proof::*` path still resolves: the `pub use <submodule>::*`
// re-exports below hoist each submodule's public items back to this module, so
// the public surface (`CompiledBgp`, `plan_bgp`, `BgpPlanError`, the prove /
// verify entries, the `prover` selector module, ...) is unchanged.
//
// Each submodule opens with `use super::*;` to see the shared imports above and
// its sibling items. Internal helpers that are now referenced across file
// boundaries were widened from private to `pub(crate)` (a crate-internal detail;
// the crate's external `pub` API is untouched).
//
//   * `types`       — the core output data types (CompiledBgp, CompiledRelation,
//                     the plan-statement enum, the value/eval/calendar expr trees,
//                     the result-readout types).
//   * `digest`      — compiled_query_digest: the stable per-query fingerprint.
//   * `columns`     — verifier-recompute helpers (the `expected_*` routines) for
//                     the eval / typecast / string / aggregate / calendar / decimal forms.
//   * `proved`      — `impl ProvedBgp`: walk the plan tree and prove each node.
//   * `prove_entry` — the prove/verify entry points + portable envelope + errors.
//   * `errors`      — the `BgpPlanError` rejection taxonomy.
//   * `bgp`         — the core left-deep BGP join planner (`plan_bgp`, `Atom`, ...).
//
// The per-construct `plan_*_form` recognizers (all `pub(crate)`, dispatched by
// `bgp::plan_bgp`) are grouped by the WOQL family they lower:
//
//   * `eval`        — `Eval` arithmetic + decimal/rational value layer + `Typecast`.
//   * `string_ops`  — concat / join / substring / pad / hashkey string circuits.
//   * `aggregate`   — group / collection sum, count, and min/max extrema.
//   * `order`       — `OrderBy`, exact ordered slices, triple-count comparisons.
//   * `paths`       — fixed-length + transitive-closure property paths.
//   * `subsumption` — schema-graph reads: Subsumption and `IsA`.
//   * `calendar`    — calendar / date functions.
// ---------------------------------------------------------------------------

mod types;
mod digest;
mod columns;
mod proved;
mod prove_entry;
mod errors;
mod bgp;
// WOQL `ReadDocument` — the compiler-only document bundle (increment 2b).
mod document;

// The per-construct lowering families (see the layout note above).
mod eval;
mod string_ops;
mod aggregate;
mod order;
mod paths;
mod subsumption;
mod calendar;

pub use types::*;
pub use digest::*;
pub use columns::*;
// `proved` contributes only `impl ProvedBgp` methods (no re-exportable items).
pub use prove_entry::*;
pub use errors::*;
pub use bgp::*;
pub use document::*;

// The `plan_*_form` families expose only `pub(crate)` planners (no external `pub`
// items) — hoist them crate-internally so `bgp::plan_bgp` can dispatch to them.
pub(crate) use eval::*;
pub(crate) use string_ops::*;
pub(crate) use aggregate::*;
pub(crate) use order::*;
pub(crate) use paths::*;
pub(crate) use subsumption::*;
pub(crate) use calendar::*;

#[cfg(test)]
mod group_extrema_planner_tests;
#[cfg(test)]
mod collection_extrema_planner_tests;
#[cfg(test)]
mod composite_join_key_namespace_tests;
#[cfg(test)]
mod decimal_layer_render_tests;

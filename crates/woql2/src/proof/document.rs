//! WOQL `ReadDocument` — prove a whole document by id (increment 2b: flat + list documents).
//!
//! A document is NOT a single relation, so `ReadDocument` does not become a
//! [`CompiledRelationStatement`] variant. It is **pure compiler-side orchestration over
//! already-audited leaves** — no store change, no new cryptography. A proven `ReadDocument` is a
//! BUNDLE of ordinary sub-proofs:
//!
//!   * ONE instance-graph bound-subject BGP `Triple(id, ?p, ?o)` — the COMPLETE, root-authenticated
//!     direct triples of the document (this is the floor; a flat document needs nothing more);
//!   * a FRAME read on the schema graph — a bound-subject BGP `Triple(C, ?p, ?r)` that enumerates
//!     the class `C`'s direct properties, plus one bound-subject BGP per non-primitive range/element
//!     node to classify each property (scalar / link / set / list, or reject-close);
//!   * for an INHERITING class — the reflexive forward `sys:inherits` closure from `C` over the
//!     SCHEMA graph (the audited subsumption-closure gadget), proving the complete ancestor SET
//!     `{C} ∪ ancestors(C)`, plus one bound-subject frame BGP per STRICT ancestor, so the frame is
//!     the UNION of the directly-declared properties across the whole super-chain;
//!   * one authenticated `OrderedListWalk` per PRESENT list property — delivering its exact ordered
//!     element sequence (the store leaf shipped in increment 1/2a).
//!
//! ## The two soundness pillars (get these exactly right — §3/§7 of `readdocument-proving.md`)
//!
//! 1. **The plan is a PURE FUNCTION of the query.** [`plan_read_document`] takes only the `id`; it
//!    NEVER reads the database. The class `C`, the property set, and the list count are DATA:
//!    discovered by the prover (which reads the layers), and RE-AUTHENTICATED by the verifier from
//!    the root-bound frame — never baked into the plan. The verifier reconstructs every sub-proof's
//!    plan from `id` + the *authenticated* frame and verifies each against the trusted root, so a
//!    prover has no latitude to inject the shape.
//!
//! 2. **The instance-`C` == schema-`C` tie is a verify-time IRI equality.** A `ReadDocument` reads
//!    ONE bound subject → ONE class `C`, so the cross-root "same class on both graphs" tie degenerates
//!    from a bulk hash join to: the verifier derives `C` ONCE from the root-authenticated instance
//!    `rdf:type` row, and reuses that same `C` IRI to reconstruct the schema frame plan. Sound because
//!    both roots are pinned to the trusted combined `database_root` and there is a single subject.
//!
//! 3. **The inherited frame is COMPLETE via the AUTHENTICATED ancestor closure.** When `C` inherits,
//!    the frame is the union of the directly-declared properties across `{C} ∪ ancestors(C)`. That
//!    ancestor SET is NOT a prover-supplied list: it is the proven output of the reflexive forward
//!    `sys:inherits` closure from `C`, whose EMPTY-DELTA FIXPOINT proves no ancestor is omitted. The
//!    verifier RE-BUILDS the closure `CompiledBgp` from the derived `C` and re-verifies it against the
//!    schema root, then re-reads EVERY ancestor's properties — so a prover cannot hide an inherited
//!    property by dropping an ancestor (a dropped ancestor would either fail the closure re-verify or,
//!    if its property is used by the instance, trip the no-injection cross-check as an un-framed
//!    predicate). Whether to prove the closure at all is itself decided from AUTHENTICATED data: `C`'s
//!    complete, root-authenticated frame BGP either carries a `sys:inherits` edge (⇒ prove the closure)
//!    or provably does not (⇒ `C` has no ancestors, the reflexive closure is exactly `{C}`), so a
//!    prover cannot skip the closure for a class that actually inherits.
//!
//! ## Completeness / no-injection (the bundle cross-checks in verify)
//!
//! The bound-subject BGP `D` is COMPLETE + root-authenticated, so a prover cannot omit a property.
//! Every instance predicate in `D` (minus `rdf:type`) MUST be frame-classified, else reject
//! ([`BgpPlanError::ReadDocumentUnknownInstanceProperty`]) — no injection. A list property is rendered
//! from its `OrderedListWalk` (NOT the raw cons head); list-ness is authenticated from the schema, so
//! a prover cannot mislabel a list as a scalar. Exactly one head edge is required per present list
//! property, and the set of list-walk sub-proofs must match the present list properties EXACTLY (no
//! injected or missing walk). Cons-internal triples have subject `!= id`, so they never enter `D`.
//!
//! ## Scope / reject boundary
//!
//! Accept a bound-node `id` whose class is a plain `sys:Class` — WITH OR WITHOUT single- or
//! multi-level `sys:inherits` — and whose effective (inherited-union) properties are scalar (INCLUDING
//! `rdf:langString`), link (non-subdocument), `Set` (with OR without cardinality bounds), `Cardinality`,
//! `Optional` (of a base type or plain simple class), `List` (non-subdocument element), or 1-D dense
//! `Array`. Increment 5 made `Optional`, `Cardinality`/bounded-`Set`, and `rdf:langString` PROVABLE:
//!   * `Optional<T>` renders as its element T's descriptor (Scalar for a base type, Link for a plain
//!     class) — present ⇒ render, absent ⇒ omit (render-what's-stored);
//!   * `sys:Cardinality` and a cardinality-bounded `sys:Set` render exactly as the existing `Set` (the
//!     bounds are insert-time validation, never rendered);
//!   * `rdf:langString` renders as a scalar — the language tag survives in the authenticated value's
//!     glued `"lang@text"` lexical.
//! Reject-close: variable id; subdocument / table range; `sys:oneOf` / tagged-union / enum (a choice
//! class's alternative properties live outside the direct frame BGP — refuse rather than omit them);
//! multi-dimensional or sparse array; an `Optional`/collection element that is subdocument / unfoldable /
//! inheriting / a nested collection; an instance property absent from the (inherited) frame; a list
//! property with != 1 head edge; a conflicting inherited range (the same predicate declared with two
//! different ranges up the chain); ANY inheritance-closed subdocument nature or ANY
//! tagged-union/`sys:oneOf`/enum nature anywhere in the ancestor chain (fail-closed). A LINK/ELEMENT
//! target class that itself inherits still reject-closes (its inherited subdocument-ness is not ruled out
//! — a separate, larger effort). Compiler-only; nothing under `terminusdb-store-paraplu` changes.

use super::*;

use std::collections::{BTreeMap, BTreeSet};

use crate::document::ReadDocument;
use terminus_store::proof::commitment::{database_root, validate_database_root, VerifierCommitmentRef};

// The reconciled (delta-stack) instance-read machinery reused VERBATIM for increment 6a. A bound-subject
// read over a NON-materialized delta head routes to the store's single-atom present FILTER
// (`FilterPosition::Subject`, constant `hash_object(Node(id))`), which delivers the subject's
// `[predicate, object]` rows exactly like a materialized bound-subject BGP. NO new store crypto — this is
// pure compiler-side routing over already-audited store entries.
use dory_pcs::backends::arkworks::ArkFr;
use terminus_store::proof::dict::hash_object;
use terminus_store::proof::reconcile::{
    prove_read_single_atom, verify_and_resolve_single_atom, FilterPosition, PresentFilterProof,
    ReconcileProof,
};

// ---------------------------------------------------------------------------------------------------
// Well-known IRIs — the schema-frame vocabulary. Mirrors the server Prolog `schema.pl`
// (`schema_type_descriptor_compute`, `is_built_in`, `is_direct_subdocument`, `base_type/1`). Read,
// not guessed: every constant below is copied from the authoritative encoder.
// ---------------------------------------------------------------------------------------------------

// `RDF_TYPE_IRI` and `SYS_INHERITS_IRI` are already defined (and re-exported) by the subsumption
// submodule; reuse them rather than redefining.
const RDF_LANGSTRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

const SYS_CLASS_TYPE: &str = "http://terminusdb.com/schema/sys#Class";
const SYS_TAGGED_UNION: &str = "http://terminusdb.com/schema/sys#TaggedUnion";
const SYS_ENUM: &str = "http://terminusdb.com/schema/sys#Enum";
const SYS_UNIT: &str = "http://terminusdb.com/schema/sys#Unit";
const SYS_FOREIGN: &str = "http://terminusdb.com/schema/sys#Foreign";
const SYS_JSON: &str = "http://terminusdb.com/schema/sys#JSON";
const SYS_JSONDOCUMENT: &str = "http://terminusdb.com/schema/sys#JSONDocument";
const SYS_SET: &str = "http://terminusdb.com/schema/sys#Set";
const SYS_LIST: &str = "http://terminusdb.com/schema/sys#List";
const SYS_ARRAY: &str = "http://terminusdb.com/schema/sys#Array";
const SYS_TABLE: &str = "http://terminusdb.com/schema/sys#Table";
const SYS_CARDINALITY: &str = "http://terminusdb.com/schema/sys#Cardinality";
const SYS_OPTIONAL: &str = "http://terminusdb.com/schema/sys#Optional";
const SYS_CLASS_PRED: &str = "http://terminusdb.com/schema/sys#class";
/// The `sys:dimensions` descriptor on a `sys:Array` range node (`schema.pl` array clause): its integer
/// value is the array's dimensionality (default 1 when ABSENT). v1 renders 1-D DENSE arrays only, so a
/// present `sys:dimensions` value other than `1` (a multi-dimensional array, carrying `sys:index2…`
/// columns the store composite does not read) reject-closes.
const SYS_DIMENSIONS: &str = "http://terminusdb.com/schema/sys#dimensions";
const SYS_SUBDOCUMENT: &str = "http://terminusdb.com/schema/sys#subdocument";
const SYS_UNFOLDABLE: &str = "http://terminusdb.com/schema/sys#unfoldable";
const SYS_UNFOLD: &str = "http://terminusdb.com/schema/sys#unfold";
const SYS_ONEOF: &str = "http://terminusdb.com/schema/sys#oneOf";

/// The `is_built_in/1` predicate exclusion list (`schema.pl:611-631`). Properties with these
/// predicates are schema plumbing, never document body fields; they are dropped from the frame's
/// property enumeration (F1). `rdf:type` is here too — it is used to bind `C` (F0) but is never a body
/// property.
const BUILT_IN_PREDICATES: &[&str] = &[
    RDF_TYPE_IRI,
    "http://terminusdb.com/schema/sys#comment",
    "http://terminusdb.com/schema/sys#properties",
    "http://terminusdb.com/schema/sys#documentation",
    SYS_INHERITS_IRI,
    "http://terminusdb.com/schema/sys#key",
    SYS_ONEOF,
    "http://terminusdb.com/schema/sys#base",
    SYS_CLASS_PRED,
    "http://terminusdb.com/schema/sys#abstract",
    SYS_SUBDOCUMENT,
    "http://terminusdb.com/schema/sys#unfoldable",
    "http://terminusdb.com/schema/sys#unfold",
    "http://terminusdb.com/schema/sys#shared",
    "http://terminusdb.com/schema/sys#metadata",
];

/// A curated subset of the server's `base_type/1` list (`core/triple/base_type.pl`). A range whose IRI
/// is a base type is a SCALAR (a primitive with no schema triples of its own); the classification needs
/// no schema read for it.
///
/// SOUNDNESS INVARIANT — this list MUST remain a strict SUBSET of the server's `base_type/1`. A subset
/// is SAFE: the only effect of omitting a genuine base type is that an honest scalar document
/// reject-closes (a liveness loss, never an unsound accept). A SUPERSET is a SOUNDNESS RISK — adding a
/// datatype the server does NOT treat as a base type would make the verifier render, as a scalar, a
/// value the reference resolves as a link/collection (a shape v1 cannot faithfully reproduce). NEVER
/// add a datatype here that `base_type/1` (base_type.pl) does not list. (The server list is a strict
/// superset: it additionally has `xdd#html`, `sys#JSON`, `sys#JSONDocument`, and `sys#Dictionary`,
/// deliberately omitted here; omitting them is safe per the invariant above. The
/// `base_types_subset_guard` unit test mechanically enforces the subset.)
const BASE_TYPES: &[&str] = &[
    "http://www.w3.org/2001/XMLSchema#anySimpleType",
    "http://www.w3.org/2001/XMLSchema#string",
    "http://www.w3.org/2001/XMLSchema#boolean",
    "http://www.w3.org/2001/XMLSchema#decimal",
    "http://www.w3.org/2001/XMLSchema#double",
    "http://www.w3.org/2001/XMLSchema#float",
    "http://www.w3.org/2001/XMLSchema#time",
    "http://www.w3.org/2001/XMLSchema#date",
    "http://www.w3.org/2001/XMLSchema#dateTime",
    "http://www.w3.org/2001/XMLSchema#dateTimeStamp",
    "http://www.w3.org/2001/XMLSchema#gYear",
    "http://www.w3.org/2001/XMLSchema#gMonth",
    "http://www.w3.org/2001/XMLSchema#gDay",
    "http://www.w3.org/2001/XMLSchema#gYearMonth",
    "http://www.w3.org/2001/XMLSchema#gMonthDay",
    "http://www.w3.org/2001/XMLSchema#duration",
    "http://www.w3.org/2001/XMLSchema#yearMonthDuration",
    "http://www.w3.org/2001/XMLSchema#dayTimeDuration",
    "http://www.w3.org/2001/XMLSchema#byte",
    "http://www.w3.org/2001/XMLSchema#short",
    "http://www.w3.org/2001/XMLSchema#int",
    "http://www.w3.org/2001/XMLSchema#long",
    "http://www.w3.org/2001/XMLSchema#unsignedByte",
    "http://www.w3.org/2001/XMLSchema#unsignedShort",
    "http://www.w3.org/2001/XMLSchema#unsignedInt",
    "http://www.w3.org/2001/XMLSchema#unsignedLong",
    "http://www.w3.org/2001/XMLSchema#integer",
    "http://www.w3.org/2001/XMLSchema#positiveInteger",
    "http://www.w3.org/2001/XMLSchema#nonNegativeInteger",
    "http://www.w3.org/2001/XMLSchema#negativeInteger",
    "http://www.w3.org/2001/XMLSchema#nonPositiveInteger",
    "http://www.w3.org/2001/XMLSchema#base64Binary",
    "http://www.w3.org/2001/XMLSchema#hexBinary",
    "http://www.w3.org/2001/XMLSchema#anyURI",
    "http://www.w3.org/2001/XMLSchema#language",
    "http://www.w3.org/2001/XMLSchema#normalizedString",
    "http://www.w3.org/2001/XMLSchema#token",
    "http://www.w3.org/2001/XMLSchema#NMTOKEN",
    "http://www.w3.org/2001/XMLSchema#Name",
    "http://www.w3.org/2001/XMLSchema#NCName",
    "http://www.w3.org/2001/XMLSchema#NOTATION",
    "http://www.w3.org/2001/XMLSchema#QName",
    "http://www.w3.org/2001/XMLSchema#ID",
    "http://www.w3.org/2001/XMLSchema#IDREF",
    "http://www.w3.org/2001/XMLSchema#ENTITY",
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#XMLLiteral",
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#PlainLiteral",
    // `rdf:langString` is a base type and (increment 5) IS accepted as a SCALAR: a language-tagged
    // literal round-trips through the authenticated value dictionary as a single
    // `CanonicalObject::Value { datatype: LangString, lexical }` whose lexical is the store's glued
    // `"lang@text"` (`make_entry_from_lang_term`; `LangString` is a `stringy_type` storing raw bytes like
    // `xsd:string`), so BOTH the text AND the language tag are recoverable — see `classify_range`.
    RDF_LANGSTRING,
    "http://www.w3.org/2000/01/rdf-schema#Literal",
    "http://terminusdb.com/schema/xdd#coordinate",
    "http://terminusdb.com/schema/xdd#coordinatePolygon",
    "http://terminusdb.com/schema/xdd#coordinatePolyline",
    "http://terminusdb.com/schema/xdd#dateRange",
    "http://terminusdb.com/schema/xdd#dateTimeInterval",
    "http://terminusdb.com/schema/xdd#gYearRange",
    "http://terminusdb.com/schema/xdd#integerRange",
    "http://terminusdb.com/schema/xdd#decimalRange",
    "http://terminusdb.com/schema/xdd#json",
    "http://terminusdb.com/schema/xdd#url",
    "http://terminusdb.com/schema/xdd#email",
];

fn is_built_in(predicate: &[u8]) -> bool {
    BUILT_IN_PREDICATES.iter().any(|iri| iri.as_bytes() == predicate)
}

fn is_base_type(iri: &[u8]) -> bool {
    BASE_TYPES.iter().any(|base| base.as_bytes() == iri)
}

// ---------------------------------------------------------------------------------------------------
// Public bundle types.
// ---------------------------------------------------------------------------------------------------

/// A compiled `ReadDocument` — a PURE FUNCTION of the query (`id` only; see the module invariant #1).
/// It carries NO discovered data: the class `C`, the property set, and the list count are all
/// authenticated in the proof and re-derived by the verifier from the root-bound frame.
#[derive(Clone, Debug)]
pub struct CompiledReadDocument {
    /// The bound document IRI (from the query identifier). The single subject the whole bundle binds.
    pub id: String,
    /// Query-routing metadata retained from an outer `Using`; the proof is anchored to the layers the
    /// caller supplies, and an integration uses this to resolve them.
    pub collection: Option<String>,
}

impl CompiledReadDocument {
    /// Bundle-identity digest (domain-separated `id` fold). Each sub-proof already carries its own
    /// query digest; this is a stable handle for the whole bundle. Not consulted in verify — the id is
    /// already bound through the subject BGP's own digest.
    pub fn digest(&self) -> ProofHash {
        read_document_digest(self.id.as_bytes())
    }
}

/// A source-agnostic instance-graph bound-subject read (increment 6a). EVERY instance bound-subject
/// read in a `ReadDocument` bundle — the node's own direct triples, each child type-check read, each
/// collection-element read — is one of these. Both variants deliver the SAME logical answer (the
/// complete, deduplicated `(predicate, object)` rows of one bound subject), so everything downstream
/// (`extract_class`, `classify_frame`, the completeness/no-injection cross-checks) is source-agnostic
/// and reused VERBATIM.
///
/// SOUNDNESS: the route is chosen by the ENTRY POINT (materialized vs reconciled), never by this
/// prover-supplied variant. [`InstanceSubjectProof::resolve`] pairs the leaf against the entry's
/// [`InstanceSource`]; a leaf whose variant disagrees reject-closes. On BOTH paths the delivered rows
/// are re-verified against the trusted root before use (`present` is a complete duplicate-free SET by
/// the reconcile invariant, and the `FilterPosition::Subject` filter selects EXACTLY `id`'s rows).
#[derive(Debug)]
pub enum InstanceSubjectProof {
    /// A materialized bound-subject BGP `Triple(id, ?p, ?o)` over a single instance `LayerCommitment`
    /// (the base path — byte-identical to increments 2b–5).
    Materialized(ProvedBgp),
    /// A reconciled single-atom present filter over a base..head delta stack (the 6a delta path): the
    /// reconcile proof, the `FilterPosition::Subject` present-filter proof, the prover's published
    /// result columns (re-committed + resolved against the trusted head root at verify), and the
    /// prover's UNTRUSTED per-column dictionary id-hints (I2a — see `id_hints`).
    Reconciled {
        reconcile: ReconcileProof,
        filter: PresentFilterProof,
        columns: Vec<Vec<ArkFr>>,
        /// I2a: the prover's UNTRUSTED per-column dictionary id-hints (one hint column per output
        /// `columns` entry, aligned row-for-row). `verify_and_resolve_single_atom` resolves the two
        /// output columns via the `NodeValueHash`/`PredicateHash` (filter hash→term) path, whose
        /// MANDATORY resolution belt-hash binds each hint to the authenticated result hash
        /// (`hash_object`/`hash_term(opened_term) == value`). NOT committed and NOT part of
        /// `commitment_root` — the belt-hash is the sole binding, so a forged/absent hint fails closed.
        id_hints: Vec<Vec<u64>>,
    },
}

impl InstanceSubjectProof {
    /// Test/inspection helper: the materialized BGP (panics on a reconciled leaf). Lets the
    /// materialized suite's tamper cases reach the committed result columns after the field-type change.
    pub fn expect_materialized_mut(&mut self) -> &mut ProvedBgp {
        match self {
            InstanceSubjectProof::Materialized(bgp) => bgp,
            InstanceSubjectProof::Reconciled { .. } => {
                panic!("expected a materialized instance subject proof, found a reconciled one")
            }
        }
    }
}

/// A source-agnostic SCHEMA-graph bound-subject read (increment 6e) — the EXACT schema analogue of
/// [`InstanceSubjectProof`]. EVERY schema-graph bound-subject read in a `ReadDocument` bundle — the
/// class `C`'s own frame `Triple(C, ?p, ?r)`, each property range/element node `Triple(R, ?p, ?o)`,
/// and each strict ancestor's frame — is one of these. Both variants deliver the SAME logical answer
/// (the complete, deduplicated `(predicate, object)` rows of one bound schema subject), so the
/// classification logic ([`classify_range`] / [`collect_class_frame`] / [`closure_natures`]) is
/// source-agnostic and reused VERBATIM.
///
/// SOUNDNESS: the route (materialized vs reconciled) is chosen by the ENTRY POINT's [`SchemaSource`],
/// never by this prover-supplied variant. [`SchemaSubjectProof::resolve`] pairs the leaf against the
/// entry's source; a leaf whose variant disagrees reject-closes. On BOTH paths the delivered rows are
/// re-verified against the trusted SCHEMA root (pinned to the combined `database_root`) before use — a
/// materialized single-layer BGP, or a reconciled single-atom present filter that selects EXACTLY
/// `node`'s rows over the delta schema head's complete duplicate-free `present`.
#[derive(Debug)]
pub enum SchemaSubjectProof {
    /// A materialized bound-subject BGP `Triple(node, ?p, ?o)` over a single schema `LayerCommitment`
    /// (the base path — byte-identical to increments 2b–6d).
    Materialized(ProvedBgp),
    /// A reconciled single-atom present filter over a base..head delta SCHEMA stack (the 6e delta
    /// path): the reconcile proof, the `FilterPosition::Subject` present-filter proof, the
    /// prover's published result columns (re-committed + resolved against the trusted schema-head root
    /// at verify), and the prover's UNTRUSTED per-column dictionary id-hints (I2a — see `id_hints`).
    /// Uses the SAME store entry as the instance side (`prove_read_single_atom` /
    /// `verify_and_resolve_single_atom`), just over the schema layers.
    Reconciled {
        reconcile: ReconcileProof,
        filter: PresentFilterProof,
        columns: Vec<Vec<ArkFr>>,
        /// I2a: the prover's UNTRUSTED per-column dictionary id-hints, bound to the authenticated
        /// result hashes by `verify_and_resolve_single_atom`'s MANDATORY resolution belt-hash — the
        /// schema-side twin of [`InstanceSubjectProof::Reconciled::id_hints`]. NOT committed.
        id_hints: Vec<Vec<u64>>,
    },
}

impl SchemaSubjectProof {
    /// Test/inspection helper mirroring [`InstanceSubjectProof::expect_materialized_mut`]: the
    /// materialized BGP (panics on a reconciled leaf), so the materialized suite's schema-read tamper
    /// cases reach the committed result columns after the field-type change.
    pub fn expect_materialized_mut(&mut self) -> &mut ProvedBgp {
        match self {
            SchemaSubjectProof::Materialized(bgp) => bgp,
            SchemaSubjectProof::Reconciled { .. } => {
                panic!("expected a materialized schema subject proof, found a reconciled one")
            }
        }
    }
}

/// A source-agnostic SCHEMA-graph reflexive `sys:inherits` ancestor closure (increment 6e) — the
/// schema analogue of [`InstanceSubjectProof`], for the ONE closure sub-proof (not a bound-subject
/// read). The materialized arm is byte-identical to increments 2b–6d (the audited subsumption closure
/// as a `CompiledBgp`/`ProvedBgp`); the reconciled arm carries the store's standalone
/// `TransitiveClosure`-over-`present` proof (FOUNDATION-B, 6c-i) run on the SCHEMA chain.
/// [`SchemaClosureProof`] pairs the carried variant against the entry's [`SchemaSource`]; a variant
/// that disagrees reject-closes. On BOTH paths the reachable ancestor SET is the closure's proven
/// output — COMPLETE (empty-delta fixpoint) and deduplicated — never trusted from a prover-supplied
/// list, so no inherited property can be hidden by dropping an ancestor.
#[derive(Debug)]
pub enum SchemaClosureProof {
    /// The materialized reflexive `sys:inherits` closure over a single schema layer (the base path).
    Materialized(ProvedBgp),
    /// The reconciled reflexive `sys:inherits` closure over the delta schema head's `present` (6e) —
    /// carried WHOLESALE (a store-private composite), re-verified by the store's own
    /// `verify_reconciled_transitive_closure`; its `Vec<u64>` reachable-id output is rendered to class
    /// IRIs via the store's root-bound `resolve_reconciled_node_ids`.
    Reconciled(Box<ReconciledClosureProof>),
}

impl SchemaClosureProof {
    /// Test/inspection helper mirroring [`SchemaSubjectProof::expect_materialized_mut`]: the
    /// materialized closure BGP (panics on a reconciled closure), so the materialized suite's
    /// ancestor-closure tamper case reaches the committed result columns after the field-type change.
    pub fn expect_materialized_mut(&mut self) -> &mut ProvedBgp {
        match self {
            SchemaClosureProof::Materialized(bgp) => bgp,
            SchemaClosureProof::Reconciled(_) => {
                panic!("expected a materialized schema closure proof, found a reconciled one")
            }
        }
    }
}

/// One range/element schema node's classification sub-proof: a bound-subject BGP `Triple(node, ?p, ?o)`
/// on the SCHEMA graph, delivering the node's direct schema triples. The verifier reconstructs the
/// plan from `node` (a pure function of the node IRI) and verifies against the schema root, so the
/// delivered rows are root-authenticated.
#[derive(Debug)]
pub struct ProvedRangeClassification {
    /// The schema node IRI (a range descriptor node OR a class) whose direct triples this proves.
    pub node: Vec<u8>,
    /// `Triple(node, ?p, ?o)` proved over the schema graph — a MATERIALIZED bound-subject BGP (base
    /// path) OR a RECONCILED single-atom present filter over a delta schema head (6e). Source-agnostic:
    /// the route is fixed by the entry point's [`SchemaSource`].
    pub schema_read: SchemaSubjectProof,
}

/// The authenticated class frame of the document: the class `C`, the schema BGP enumerating `C`'s
/// direct properties, the reflexive `sys:inherits` ancestor closure (for an inheriting `C`), and one
/// classification sub-proof per non-primitive range/element node AND per strict ancestor class.
#[derive(Debug)]
pub struct ProvedFrame {
    /// The prover's `C` (informational). The verifier DERIVES `C` from the root-authenticated instance
    /// `rdf:type` row and uses ITS derived `C` to reconstruct the frame plan — it never trusts this.
    pub class: Vec<u8>,
    /// `Triple(C, ?p, ?r)` proved over the schema graph — `C`'s OWN direct properties. Source-agnostic
    /// (materialized BGP or reconciled present filter over a delta schema head, 6e).
    pub class_schema: SchemaSubjectProof,
    /// The reflexive forward `sys:inherits` closure from `C` over the schema graph — the authenticated
    /// ancestor SET `{C} ∪ ancestors(C)`, delivered as ONE node column. Present ONLY when `C`'s
    /// complete, root-authenticated frame bears a `sys:inherits` edge; a non-inheriting `C` has NO
    /// ancestors (so the set is exactly `{C}` and no closure is proved). The verifier RE-BUILDS this
    /// closure plan from the derived `C` and re-verifies it, so the set is the closure's proven output
    /// (the empty-delta fixpoint proves it is COMPLETE — no ancestor omitted), never trusted from here.
    /// Source-agnostic (materialized subsumption BGP or reconciled `TransitiveClosure` over the delta
    /// schema head's `present`, 6e).
    pub ancestor_closure: Option<SchemaClosureProof>,
    /// One entry per distinct schema node the classification reads — property ranges + element classes
    /// (excluding primitives) AND every strict ancestor class of `C`.
    pub ranges: Vec<ProvedRangeClassification>,
}

/// The ordered-list-walk proof carried for one PRESENT list property, ROUTED by the entry point (the
/// materialized-vs-reconciled analogue of [`InstanceSubjectProof`], added for increment 6c-iii). The
/// materialized arm is byte-identical to increments 2b–5; the reconciled arm carries the store's
/// standalone `OrderedListWalk`-over-`present` proof. [`ListWalkProof::resolve`] pairs the carried
/// variant against the entry's [`InstanceSource`]; a variant that disagrees reject-closes.
#[derive(Debug)]
pub enum ListWalkProof {
    /// The materialized `OrderedListWalk` envelope over a single instance layer (the base path).
    Materialized(ProvedBgp),
    /// The reconciled ordered-list walk over the delta stack's `present` (the 6c-iii delta path) —
    /// carried WHOLESALE (a store-private composite), re-verified by the store's own verify entry.
    Reconciled(Box<OrderedListWalkReconciledProof>),
}

impl ListWalkProof {
    /// Test/inspection helper mirroring [`InstanceSubjectProof::expect_materialized_mut`]: the
    /// materialized envelope (panics on a reconciled walk), so the materialized suite's tamper cases
    /// reach the committed walk result columns after the field-type change.
    pub fn expect_materialized_mut(&mut self) -> &mut ProvedBgp {
        match self {
            ListWalkProof::Materialized(bgp) => bgp,
            ListWalkProof::Reconciled(_) => {
                panic!("expected a materialized list walk proof, found a reconciled one")
            }
        }
    }
}

/// One PRESENT list property's ordered-walk sub-proof.
#[derive(Debug)]
pub struct ProvedListProperty {
    /// The list-valued predicate IRI.
    pub predicate: Vec<u8>,
    /// The `OrderedListWalk` proof for `(id, predicate)` — the exact ordered element sequence — over
    /// the materialized layer OR the reconciled `present`, routed by the entry point.
    pub walk: ListWalkProof,
}

/// The ordered-array-walk proof carried for one PRESENT array property, ROUTED by the entry point —
/// the exact `sys:Array` analogue of [`ListWalkProof`] (added for increment 6d). The materialized arm
/// is byte-identical to increments 4–5; the reconciled arm carries the store's standalone
/// `OrderedArrayWalk`-over-`present` proof. A variant that disagrees with the entry's [`InstanceSource`]
/// reject-closes.
#[derive(Debug)]
pub enum ArrayWalkProof {
    /// The materialized `OrderedArrayWalk` envelope over a single instance layer (the base path).
    Materialized(ProvedBgp),
    /// The reconciled ordered-array walk over the delta stack's `present` (the 6d delta path) — carried
    /// WHOLESALE (a store-private composite), re-verified by the store's own verify entry.
    Reconciled(Box<OrderedArrayWalkReconciledProof>),
}

impl ArrayWalkProof {
    /// Test/inspection helper mirroring [`ListWalkProof::expect_materialized_mut`]: the materialized
    /// envelope (panics on a reconciled walk), so the materialized suite's tamper cases reach the
    /// committed walk result columns after the field-type change.
    pub fn expect_materialized_mut(&mut self) -> &mut ProvedBgp {
        match self {
            ArrayWalkProof::Materialized(bgp) => bgp,
            ArrayWalkProof::Reconciled(_) => {
                panic!("expected a materialized array walk proof, found a reconciled one")
            }
        }
    }
}

/// One PRESENT 1-D dense array property's ordered-walk sub-proof (increment 4). The exact analogue of
/// [`ProvedListProperty`] for `sys:Array`: the store's `OrderedArrayWalk` proof for `(id, predicate)`
/// delivers the `sys:index`-ordered `sys:value` column, over the materialized layer OR the reconciled
/// `present` (6d), routed by the entry point.
#[derive(Debug)]
pub struct ProvedArrayProperty {
    /// The array-valued predicate IRI.
    pub predicate: Vec<u8>,
    /// The store's `OrderedArrayWalk` proof for `(id, predicate)` — the exact `sys:index`-ordered
    /// element sequence.
    pub walk: ArrayWalkProof,
}

/// One PRESENT `Set`/`List` collection property's per-element ACTUAL-TYPE witnesses (increment-3 audit
/// fix). v1 does NOT inline collection elements — it renders each as a bare id — so it must PROVE that
/// the reference would NOT unfold the collection. For each NODE-valued element the witness authenticates
/// the element's ACTUAL `rdf:type` + reflexive `sys:inherits` closure (the SAME machinery a single-`Link`
/// STOP uses), and verify rejects if any class in that closure bears `sys:subdocument` / `sys:unfoldable`
/// (the reference would unfold such an element). LITERAL/scalar elements carry no witness — only
/// node-valued elements can unfold. The `@unfold`-collection-property reject is enforced separately (it
/// needs no per-element witness).
#[derive(Debug)]
pub struct ProvedCollectionProperty {
    /// The `Set`/`List`-valued predicate IRI.
    pub predicate: Vec<u8>,
    /// One entry per DISTINCT node-valued element object (deduplicated by element id — an element's type
    /// depends only on its IRI, so one witness suffices for repeated elements).
    pub elements: Vec<ProvedCollectionElement>,
}

/// One `Set`/`List` element's ACTUAL-type witness (increment-3 audit fix).
#[derive(Debug)]
pub struct ProvedCollectionElement {
    /// The element object IRI.
    pub child_id: Vec<u8>,
    /// The element's ACTUAL-type witness (identical shape + verification to a STOP link witness).
    pub witness: ProvedChildTypeWitness,
}

/// A single-valued class LINK child's SUBDOCUMENT-DECISION witness (increment 3): everything needed to
/// authenticate the child's `rdf:type` and decide whether that class is a subdocument — WITHOUT
/// classifying the child's own property frame. A STOP link (child is NOT a subdocument) carries ONLY
/// this witness; the child's own (possibly unsupported-shape) properties are never read, so a plain
/// link to a document with, say, an `Optional` field does not reject the parent.
#[derive(Debug)]
pub struct ProvedChildTypeWitness {
    /// The child's instance bound-subject read → its `rdf:type` = `C_child` (+ its direct triples,
    /// unused for a stop link). Source-AGNOSTIC (materialized BGP or reconciled present filter), so a
    /// child type-check over a delta head routes to the same single-atom present filter. Reconstructed
    /// at verify from `child_id`.
    pub subject_bgp: InstanceSubjectProof,
    /// `C_child`'s OWN schema frame read `Triple(C_child, ?p, ?r)`. `None` iff the child has NO
    /// `rdf:type` row (a typeless / dangling link) — the reference renders such a target as a plain
    /// link id, and so do we (a typeless target is trivially non-subdocument). Source-agnostic
    /// (materialized BGP or reconciled present filter over a delta schema head, 6e).
    pub class_schema: Option<SchemaSubjectProof>,
    /// `C_child`'s reflexive `sys:inherits` closure over the SCHEMA graph (the authenticated
    /// `{C_child} ∪ ancestors`). `None` iff there is no class, or `C_child` bears no `sys:inherits`
    /// edge (then the set is exactly `{C_child}`). Source-agnostic (materialized subsumption BGP or
    /// reconciled `TransitiveClosure` over the delta schema head's `present`, 6e).
    pub ancestor_closure: Option<SchemaClosureProof>,
    /// One schema read per STRICT ancestor class of `C_child` — the natures (`sys:subdocument` /
    /// `sys:unfoldable`) that make subdocument-ness / unfoldable-ness inheritance-closed.
    pub ancestor_reads: Vec<ProvedRangeClassification>,
}

/// One PRESENT single-valued class LINK property object (increment 3). Verify RE-DERIVES the child's
/// subdocument-ness from root-authenticated data (never the prover's choice of `target` variant) and
/// FORCES the render mode: a subdocument child MUST carry an `Inline` body (else omission reject); a
/// non-subdocument child MUST carry only a `Stop` witness (else injection reject).
#[derive(Debug)]
pub struct ProvedLink {
    /// The class-LINK predicate IRI.
    pub predicate: Vec<u8>,
    /// The child object IRI this edge points at.
    pub child_id: Vec<u8>,
    /// The child's proof — a stop witness or an inlined subdocument node.
    pub target: ProvedLinkTarget,
}

/// A LINK child's proof: the child is either a plain link (stop) or an inlined subdocument.
#[derive(Debug)]
pub enum ProvedLinkTarget {
    /// The child's class is NOT a subdocument → render the child id (today's link behavior). Carries
    /// only the child-type witness (no body).
    Stop(ProvedChildTypeWitness),
    /// The child's class IS a subdocument → the fully inlined child node (recursion). Its OWN
    /// authenticated frame re-proves the subdocument-ness the parent verifies against.
    Inline(Box<ProvedReadDocument>),
}

/// A proven `ReadDocument` bundle — ONE document node (the root, or an inlined subdocument). The
/// verifier reconstructs every sub-plan from the node's `id` + the authenticated frame and verifies
/// each sub-proof against the trusted combined `database_root`, then runs the completeness /
/// no-injection cross-checks (including the recursive per-link demand/reject of increment 3).
#[derive(Debug)]
pub struct ProvedReadDocument {
    /// The instance bound-subject read — the COMPLETE direct triples of this node. Source-AGNOSTIC: a
    /// MATERIALIZED bound-subject BGP (base path) OR a RECONCILED single-atom present filter over a
    /// delta stack (6a). Verify reconstructs the plan from the node `id` and never trusts a prover-
    /// supplied compiled plan; the route (materialized vs reconciled) is fixed by the ENTRY POINT, so a
    /// leaf whose variant disagrees with the entry's instance source reject-closes.
    pub subject_bgp: InstanceSubjectProof,
    /// The authenticated class frame.
    pub frame: ProvedFrame,
    /// One walk per PRESENT list property.
    pub lists: Vec<ProvedListProperty>,
    /// One walk per PRESENT 1-D dense array property (increment 4).
    pub arrays: Vec<ProvedArrayProperty>,
    /// One entry per PRESENT single-valued class LINK property object (stop link or inlined subdoc).
    pub links: Vec<ProvedLink>,
    /// One entry per PRESENT `Set`/`List` property — the per-element ACTUAL-TYPE witnesses that prove no
    /// element would be unfolded by the reference (increment-3 audit fix; v1 renders collection elements
    /// as bare ids, so it reject-closes any collection the reference would unfold).
    pub collections: Vec<ProvedCollectionProperty>,
}

/// A rendered property value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedValue {
    /// A scalar / link / set property: the resolved objects the bound-subject BGP delivered for this
    /// predicate (a scalar is one literal, a STOP link is one node id, a set is many — all
    /// root-authenticated, unordered, correct-multiplicity).
    Direct(Vec<CanonicalObject>),
    /// A list property: the ordered element sequence from the authenticated `OrderedListWalk` (order
    /// AND duplicate multiplicity preserved).
    List(Vec<CanonicalObject>),
    /// A 1-D DENSE array property: the `sys:index`-ordered `sys:value` sequence from the authenticated
    /// `OrderedArrayWalk` (ascending-index order AND duplicate multiplicity preserved; density —
    /// `{indices} == {0,…,n-1}` — certified by the store composite). DISTINCT from [`Self::List`]: an
    /// array is `@container:@array`, ordered by an authenticated numeric key rather than chain topology.
    Array(Vec<CanonicalObject>),
    /// A single-valued subdocument property: the INLINED child document (recursively resolved). The
    /// child's class is a subdocument (`sys:subdocument`, inheritance-closed), authenticated from the
    /// child's OWN root-verified `rdf:type` — so its full contents are part of THIS document, exactly
    /// as `get_document(id, unfold(true))` materializes them.
    Subdocument(Box<ResolvedDocument>),
}

/// The rendered, root-authenticated document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedDocument {
    /// The document IRI bytes (from the query).
    pub id: Vec<u8>,
    /// The class `C` IRI bytes (derived from the authenticated `rdf:type` row).
    pub class: Vec<u8>,
    /// The rendered body — one entry per PRESENT frame property (deterministically ordered by
    /// predicate IRI; `rdf:type` excluded).
    pub properties: Vec<(Vec<u8>, ResolvedValue)>,
}

impl ResolvedDocument {
    /// Look up a rendered property by its full predicate IRI (test/inspection convenience).
    pub fn get(&self, predicate: &str) -> Option<&ResolvedValue> {
        self.properties
            .iter()
            .find(|(p, _)| p.as_slice() == predicate.as_bytes())
            .map(|(_, v)| v)
    }
}

// ---------------------------------------------------------------------------------------------------
// Planning (PURE — reads only the query).
// ---------------------------------------------------------------------------------------------------

/// Compile a bare `ReadDocument` to its bundle plan. PURE: it reads only `read.identifier`. A variable
/// identifier reject-closes ([`BgpPlanError::ReadDocumentUnboundIdentifier`]) — a document proof binds
/// exactly one stored subject.
pub fn plan_read_document(read: &ReadDocument) -> Result<CompiledReadDocument, BgpPlanError> {
    let id = match &read.identifier {
        crate::value::NodeValue::Node(iri) => iri.clone(),
        crate::value::NodeValue::Variable(_) => {
            return Err(BgpPlanError::ReadDocumentUnboundIdentifier)
        }
    };
    Ok(CompiledReadDocument { id, collection: None })
}

/// Route a top-level `Query::ReadDocument` (optionally under an outer `Using`) to [`plan_read_document`].
/// `plan_bgp`'s own `ReadDocument` reject arm is unchanged — a document is not a relation, so the
/// relation path still refuses it; this is the document-bundle entry.
pub fn compile_read_document(query: &Query) -> Result<CompiledReadDocument, BgpPlanError> {
    let mut collection: Option<String> = None;
    let mut body = query;
    loop {
        body = match body {
            Query::Using(using) => {
                collection = Some(using.collection.clone());
                using.query.as_ref()
            }
            Query::ReadDocument(read) => {
                let mut compiled = plan_read_document(read)?;
                compiled.collection = collection;
                return Ok(compiled);
            }
            other => {
                return Err(BgpPlanError::UnsupportedConstruct {
                    construct: query_variant_name(other),
                })
            }
        };
    }
}

// ---------------------------------------------------------------------------------------------------
// Sub-plan builders (each a PURE function of its inputs — used identically by prove and verify).
// ---------------------------------------------------------------------------------------------------

/// The bound-subject BGP `Triple(subject, ?p, ?o)` — the complete direct triples of `subject`. Built
/// through the audited `plan_bgp` path so its schema/domains/digest match a compiler-produced BGP
/// exactly. Used for the instance subject read AND every schema-graph frame read (the graph is chosen
/// by WHICH commitment prove/verify anchors it to, not by the plan).
fn bound_subject_bgp(subject_iri: &str) -> Result<CompiledBgp, BgpPlanError> {
    plan_bgp(&Query::Triple(crate::triple::Triple {
        subject: crate::value::NodeValue::Node(subject_iri.to_string()),
        predicate: crate::value::NodeValue::Variable("p".to_string()),
        object: crate::value::Value::Variable("o".to_string()),
        graph: None,
    }))
}

/// The internal endpoint variable the ancestor closure delivers its class set under. Never
/// user-visible (a `ReadDocument` has no query variables); it only names the closure's one output
/// column.
const ANCESTOR_ENDPOINT_VAR: &str = "__rd_ancestor";

/// The reflexive FORWARD `sys:inherits` closure from `class_iri` over the SCHEMA graph — the ancestor
/// SET `{class} ∪ ancestors(class)`, delivered as one node column. Built through the AUDITED
/// subsumption-closure gadget ([`build_subsumption_closure`], `inverted = false` = walk `sys:inherits`
/// object-ward to gather superclasses; `reflexive = true` = the seed subsumes itself) so its proof /
/// verify / digest machinery is exactly the shipped Subsumption closure. A PURE function of `class_iri`
/// (used identically by prove and verify), like [`bound_subject_bgp`]. Empty `selections`/`distincts`
/// deliver the full endpoint column; the intrinsic set-collapse `Distinct` is synthesized inside.
fn ancestor_closure_bgp(class_iri: &str) -> Result<CompiledBgp, BgpPlanError> {
    build_subsumption_closure(
        class_iri.to_string(),
        ANCESTOR_ENDPOINT_VAR.to_string(),
        /* inverted */ false,
        /* selections */ &[],
        /* collection */ None,
        /* count_variable */ None,
        /* distincts */ &[],
    )
}

/// The `OrderedListWalk` leaf for `(subject, predicate)` — the exact ordered element sequence of the
/// stored `rdf:List`. Direct construction (there is no WOQL surface for the leaf), mirroring the
/// increment-2a test helper. The store carries all of the walk proof wholesale.
fn ordered_list_walk_bgp(subject_iri: &[u8], predicate_iri: &[u8]) -> CompiledBgp {
    CompiledBgp {
        relation: CompiledRelation {
            schema: vec!["elem".to_string()],
            domains: vec![None],
            statement: CompiledRelationStatement::OrderedListWalk {
                subject: CanonicalObject::Node(subject_iri.to_vec()),
                predicate: predicate_iri.to_vec(),
                graph: ProofGraph::Instance,
            },
        },
        projection: None,
        collection: None,
        count_variable: None,
        full_row_distinct: false,
    }
}

/// The `OrderedArrayWalk` leaf for `(subject, predicate)` — the exact `sys:index`-ordered `sys:value`
/// sequence of a stored 1-D DENSE `sys:Array` (increment 4). Direct construction (there is no WOQL
/// surface for the leaf), the exact analogue of [`ordered_list_walk_bgp`]. The store carries the whole
/// array-walk composite (the cell-set BGP, functional binds, type filter, dictionary index lookup, and
/// density+order permutation) wholesale.
fn ordered_array_walk_bgp(subject_iri: &[u8], predicate_iri: &[u8]) -> CompiledBgp {
    CompiledBgp {
        relation: CompiledRelation {
            schema: vec!["elem".to_string()],
            domains: vec![None],
            statement: CompiledRelationStatement::OrderedArrayWalk {
                subject: CanonicalObject::Node(subject_iri.to_vec()),
                predicate: predicate_iri.to_vec(),
                graph: ProofGraph::Instance,
            },
        },
        projection: None,
        collection: None,
        count_variable: None,
        full_row_distinct: false,
    }
}

// ---------------------------------------------------------------------------------------------------
// Shared helpers.
// ---------------------------------------------------------------------------------------------------

/// A resolved bound-subject BGP result as `(predicate IRI bytes, object)` rows.
type SubjectRows = Vec<(Vec<u8>, CanonicalObject)>;

fn invalid(message: impl Into<String>) -> QueryProofError {
    QueryProofError::Store(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    ))
}

fn shape_reject(shape: &'static str) -> QueryProofError {
    BgpPlanError::ReadDocumentUnsupportedPropertyShape { shape }.into()
}

fn iri_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Verify + resolve a bound-subject BGP against `root` and pair its two columns into
/// `(predicate, object)` rows. (`verify_and_resolve` re-verifies the proof against the trusted root,
/// so the rows are root-authenticated.)
fn resolve_subject_rows(
    proved: &ProvedBgp,
    compiled: &CompiledBgp,
    layer: &impl VerifierLayer,
    root: ProofHash,
) -> Result<SubjectRows, QueryProofError> {
    let columns = proved.verify_and_resolve(compiled, layer, root)?;
    if columns.len() != 2 {
        return Err(invalid(
            "bound-subject BGP must resolve to exactly two columns (predicate, object)",
        ));
    }
    let predicates = &columns[0];
    let objects = &columns[1];
    if predicates.len() != objects.len() {
        return Err(invalid(
            "bound-subject BGP predicate/object columns differ in length",
        ));
    }
    let mut rows = Vec::with_capacity(predicates.len());
    for (predicate, object) in predicates.iter().zip(objects.iter()) {
        let predicate = match predicate {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => return Err(invalid("bound-subject BGP predicate column is not a node")),
        };
        rows.push((predicate, object.clone()));
    }
    Ok(rows)
}

/// Reshape a reconciled single-atom `FilterPosition::Subject` result — two columns `[predicate, object]`
/// (mirroring the materialized bound-subject BGP; see `filter_output_kinds`) — into [`SubjectRows`],
/// EXACTLY as [`resolve_subject_rows`] does. `verify_and_resolve_single_atom` already re-committed the
/// columns to the filter proof AND resolved them against the trusted head-root-bound chain catalogs, so
/// the rows are root-authenticated.
fn reshape_present_subject_rows(
    columns: Vec<Vec<CanonicalObject>>,
) -> Result<SubjectRows, QueryProofError> {
    if columns.len() != 2 {
        return Err(invalid(
            "reconciled single-atom subject read must resolve to two columns (predicate, object)",
        ));
    }
    let predicates = &columns[0];
    let objects = &columns[1];
    if predicates.len() != objects.len() {
        return Err(invalid(
            "reconciled subject read predicate/object columns differ in length",
        ));
    }
    let mut rows = Vec::with_capacity(predicates.len());
    for (predicate, object) in predicates.iter().zip(objects.iter()) {
        let predicate = match predicate {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => return Err(invalid("reconciled subject read predicate column is not a node")),
        };
        rows.push((predicate, object.clone()));
    }
    Ok(rows)
}

/// The `FilterPosition::Subject` constant for a document/child IRI: `hash_object(Node(id))` — a PURE
/// function of `id`, used identically by prove and verify (so the verifier's re-derived filter constant
/// binds the read to exactly this subject).
fn subject_filter_constant(id: &str) -> ArkFr {
    hash_object(&CanonicalObject::Node(id.as_bytes().to_vec()))
}

// ---------------------------------------------------------------------------------------------------
// Instance-graph SOURCE / PROVER routing (increment 6a). The route — a single MATERIALIZED layer vs a
// RECONCILED delta stack — is chosen by the ENTRY POINT and threaded through the whole node tree, so it
// is a PURE FUNCTION of the query (never prover-supplied data). Every instance bound-subject read in the
// tree resolves through this one source; everything else (schema frame reads + the `sys:inherits`
// closure) stays on the materialized schema on BOTH paths.
// ---------------------------------------------------------------------------------------------------

/// The instance-graph VERIFY source for a `ReadDocument` node tree.
enum InstanceSource<'a> {
    /// A single materialized instance layer (the base path).
    Materialized(VerifierCommitmentRef<'a>),
    /// A reconciled delta stack (base..head, head last) — the 6a delta path.
    Reconciled(&'a [&'a LayerCommitment]),
}

impl InstanceSource<'_> {
    /// The trusted instance-graph root: a materialized layer's own `resolution_root`, or the delta
    /// stack's head `resolution_root` (== the trusted delta-head anchor the reconcile verifier pins to
    /// `trusted_head_root`). β re-anchor: the external trust anchor is `resolution_root` (binds
    /// `commitment_root` AND both dictionary Merkle roots), consistent with the store's
    /// `verify_and_resolve_single_atom` / `verify_and_resolve`, which now gate on `resolution_root`.
    fn instance_root(&self) -> ProofHash {
        match self {
            InstanceSource::Materialized(view) => view.resolution_root(),
            InstanceSource::Reconciled(layers) => layers
                .last()
                .expect("delta instance stack is non-empty")
                .resolution_root(),
        }
    }

    /// The verifier view used ONLY to anchor a SCHEMA-graph read (the `sys:inherits` closure) into the
    /// combined `database_root`. A schema read never touches instance data — it needs the instance
    /// solely for the root binding — so the delta stack's HEAD layer view (whose commitment root is the
    /// trusted head anchor) is the correct, sound instance anchor.
    fn root_binding_view(&self) -> VerifierCommitmentRef<'_> {
        match self {
            InstanceSource::Materialized(view) => *view,
            InstanceSource::Reconciled(layers) => layers
                .last()
                .expect("delta instance stack is non-empty")
                .verifier_view(),
        }
    }

    fn is_reconciled(&self) -> bool {
        matches!(self, InstanceSource::Reconciled(_))
    }
}

impl InstanceSubjectProof {
    /// Verify + resolve this bound-subject read against the entry-chosen [`InstanceSource`], returning
    /// the `(predicate, object)` rows. The route is fixed by `source` (the ENTRY POINT), NOT by this
    /// proof's own variant: a leaf whose variant disagrees with `source` reject-closes (the
    /// pure-function-of-the-query invariant — a prover cannot pick the route). On both arms the rows are
    /// re-verified against the trusted root before return.
    fn resolve(
        &self,
        id: &str,
        source: &InstanceSource<'_>,
        instance_root: ProofHash,
    ) -> Result<SubjectRows, QueryProofError> {
        match (self, source) {
            (InstanceSubjectProof::Materialized(bgp), InstanceSource::Materialized(view)) => {
                let compiled = bound_subject_bgp(id)?;
                resolve_subject_rows(bgp, &compiled, view, instance_root)
            }
            (
                InstanceSubjectProof::Reconciled { reconcile, filter, columns, id_hints },
                InstanceSource::Reconciled(layers),
            ) => {
                // The single-atom present filter re-verifies `present` against the trusted head root,
                // pins the selection to EXACTLY `id`'s rows (constant `hash_object(Node(id))` in the
                // Subject position), and resolves the two output columns [predicate, object] against the
                // chain catalogs by an id-routed Merkle opening under the MANDATORY belt-hash bound to
                // the carried `id_hints` (I2a) — the reconciled analogue of `resolve_subject_rows`.
                let resolved = verify_and_resolve_single_atom(
                    layers,
                    instance_root,
                    FilterPosition::Subject,
                    subject_filter_constant(id),
                    reconcile,
                    filter,
                    columns,
                    id_hints,
                )?;
                reshape_present_subject_rows(resolved)
            }
            // The leaf variant (prover data) disagrees with the entry-chosen route → reject.
            _ => Err(invalid(
                "instance subject proof route does not match the entry point's instance source",
            )),
        }
    }
}

/// The instance-graph PROVE routing — the prove-side twin of [`InstanceSource`], likewise chosen by the
/// prove ENTRY POINT. It produces a source-agnostic [`InstanceSubjectProof`] plus the resolved rows for
/// any bound subject.
enum InstanceProver<'a> {
    /// Prove bound-subject reads as materialized BGPs over a single instance layer.
    Materialized { layer: &'a LayerCommitment, root: ProofHash },
    /// Prove bound-subject reads as reconciled single-atom present filters over a delta stack.
    Reconciled { layers: &'a [&'a LayerCommitment], head_root: ProofHash },
}

impl InstanceProver<'_> {
    fn instance_root(&self) -> ProofHash {
        match self {
            InstanceProver::Materialized { root, .. } => *root,
            InstanceProver::Reconciled { head_root, .. } => *head_root,
        }
    }

    /// The `LayerCommitment` used to anchor a SCHEMA-graph read (the `sys:inherits` closure) into the
    /// combined `database_root`, and to prove a materialized-path ordered walk. Always a
    /// `&LayerCommitment` (the materialized layer, or the delta stack's head), so the schema closure
    /// prove/verify monomorphizes identically on both paths.
    fn root_binding_layer(&self) -> &LayerCommitment {
        match self {
            InstanceProver::Materialized { layer, .. } => layer,
            InstanceProver::Reconciled { layers, .. } => {
                *layers.last().expect("delta instance stack is non-empty")
            }
        }
    }

    fn is_reconciled(&self) -> bool {
        matches!(self, InstanceProver::Reconciled { .. })
    }

    /// Prove ONE bound-subject read and resolve its rows. Materialized → a bound-subject BGP; delta →
    /// a reconciled single-atom present filter (`FilterPosition::Subject`). The reconciled arm proves
    /// AND re-verifies (to recover the resolved rows the prover needs to discover the frame), exactly as
    /// the materialized arm proves-then-resolves.
    fn read_subject(
        &self,
        ctx: &StoreProverContext<'_>,
        id: &str,
    ) -> Result<(InstanceSubjectProof, SubjectRows), QueryProofError> {
        match self {
            InstanceProver::Materialized { layer, root } => {
                let compiled = bound_subject_bgp(id)?;
                let proved = compiled.prove_with_context(ctx, layer)?;
                let rows = resolve_subject_rows(&proved, &compiled, *layer, *root)?;
                Ok((InstanceSubjectProof::Materialized(proved), rows))
            }
            InstanceProver::Reconciled { layers, head_root } => {
                let constant = subject_filter_constant(id);
                let (reconcile, filter, read) =
                    prove_read_single_atom(layers, FilterPosition::Subject, constant)?;
                let columns = read.columns;
                // I2a: carry the prover's per-column id-hints; the belt-hash inside
                // `verify_and_resolve_single_atom` binds them to the authenticated result hashes.
                let id_hints = read.id_hints;
                let resolved = verify_and_resolve_single_atom(
                    layers,
                    *head_root,
                    FilterPosition::Subject,
                    constant,
                    &reconcile,
                    &filter,
                    &columns,
                    &id_hints,
                )?;
                let rows = reshape_present_subject_rows(resolved)?;
                Ok((InstanceSubjectProof::Reconciled { reconcile, filter, columns, id_hints }, rows))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Schema-graph SOURCE / PROVER routing (increment 6e — the EXACT schema analogue of the 6a instance
// routing above). The route — a single MATERIALIZED schema layer vs a RECONCILED delta schema stack —
// is chosen by the ENTRY POINT and threaded through the whole node tree, so it is a PURE FUNCTION of
// the query (never prover-supplied data). Every schema bound-subject read (`C`'s frame, each range /
// element node, each strict ancestor) resolves through this one source; the `sys:inherits` closure is
// likewise routed. A MATERIALIZED schema read stays exactly as increments 2b–6d (byte-identical); a
// RECONCILED schema read routes to the SAME store entries the 6a instance side uses
// (`prove_read_single_atom` / `verify_and_resolve_single_atom`) plus, for the closure, the 6c-i
// FOUNDATION-B `TransitiveClosure`-over-`present` — all on the SCHEMA chain. NO new store crypto.
// ---------------------------------------------------------------------------------------------------

/// The schema-graph VERIFY source for a `ReadDocument` node tree.
enum SchemaSource<'a> {
    /// A single materialized schema layer (the base path).
    Materialized(VerifierCommitmentRef<'a>),
    /// A reconciled delta schema stack (base..head, head last) — the 6e delta path.
    Reconciled(&'a [&'a LayerCommitment]),
}

impl SchemaSource<'_> {
    /// The trusted schema-graph root: a materialized layer's own `resolution_root`, or the delta
    /// stack's HEAD `resolution_root` (== the trusted delta-head anchor the reconcile verifier pins to
    /// `trusted_head_root`). β re-anchor: the external trust anchor is `resolution_root` (binds
    /// `commitment_root` AND both dictionary Merkle roots). Pinned into the combined `database_root` by
    /// the entry's `validate_database_root` (which itself composes the two per-graph `resolution_root`s),
    /// so every reconciled schema read/closure bound to this root is transitively bound to `database_root`.
    fn schema_root(&self) -> ProofHash {
        match self {
            SchemaSource::Materialized(view) => view.resolution_root(),
            SchemaSource::Reconciled(layers) => layers
                .last()
                .expect("delta schema stack is non-empty")
                .resolution_root(),
        }
    }
}

impl SchemaSubjectProof {
    /// Verify + resolve this schema bound-subject read against the entry-chosen [`SchemaSource`],
    /// returning the `(predicate, object)` rows. The route is fixed by `source` (the ENTRY POINT), NOT
    /// by this proof's own variant: a leaf whose variant disagrees with `source` reject-closes (the
    /// pure-function-of-the-query invariant). On both arms the rows are re-verified against the trusted
    /// schema root before return — the schema mirror of [`InstanceSubjectProof::resolve`].
    fn resolve(
        &self,
        node_iri: &str,
        source: &SchemaSource<'_>,
        schema_root: ProofHash,
    ) -> Result<SubjectRows, QueryProofError> {
        match (self, source) {
            (SchemaSubjectProof::Materialized(bgp), SchemaSource::Materialized(view)) => {
                let compiled = bound_subject_bgp(node_iri)?;
                resolve_subject_rows(bgp, &compiled, view, schema_root)
            }
            (
                SchemaSubjectProof::Reconciled { reconcile, filter, columns, id_hints },
                SchemaSource::Reconciled(layers),
            ) => {
                // The single-atom present filter over the SCHEMA chain re-verifies `present` against
                // the trusted schema-head root, pins the selection to EXACTLY `node`'s rows (constant
                // `hash_object(Node(node))` in the Subject position), and resolves the two output
                // columns [predicate, object] against the chain catalogs by an id-routed Merkle opening
                // under the MANDATORY belt-hash bound to the carried `id_hints` (I2a) — the reconciled
                // analogue of a materialized bound-subject schema BGP.
                let resolved = verify_and_resolve_single_atom(
                    layers,
                    schema_root,
                    FilterPosition::Subject,
                    subject_filter_constant(node_iri),
                    reconcile,
                    filter,
                    columns,
                    id_hints,
                )?;
                reshape_present_subject_rows(resolved)
            }
            // The leaf variant (prover data) disagrees with the entry-chosen route → reject.
            _ => Err(invalid(
                "schema subject proof route does not match the entry point's schema source",
            )),
        }
    }
}

/// Verify + resolve the ancestor-closure sub-proof against the entry-chosen sources, returning the
/// authenticated class SET as node-IRI byte vectors. Routed by the entry's [`SchemaSource`] (never the
/// carried variant): a MATERIALIZED closure re-verifies the audited subsumption BGP through
/// `verify_over_graphs_and_resolve` (anchored to the SCHEMA root under the trusted combined
/// `database_root`); a RECONCILED closure re-verifies the store's `TransitiveClosure`-over-`present`
/// against the trusted schema-HEAD root (pinned to `database_root` by the entry's
/// `validate_database_root`) and renders its `Vec<u64>` reachable-id output to class IRIs via the
/// root-bound `resolve_reconciled_node_ids`. In BOTH cases the set is the closure's proven output:
/// complete (empty-delta fixpoint) and de-duplicated. A variant / source mismatch reject-closes.
fn resolve_ancestor_closure(
    closure: &SchemaClosureProof,
    class_iri: &str,
    instance: &InstanceSource<'_>,
    schema: &SchemaSource<'_>,
    database_root: ProofHash,
) -> Result<Vec<Vec<u8>>, QueryProofError> {
    match (closure, schema) {
        (SchemaClosureProof::Materialized(proved), SchemaSource::Materialized(schema_view)) => {
            let compiled = ancestor_closure_bgp(class_iri)?;
            resolve_closure_set(
                proved,
                &compiled,
                &instance.root_binding_view(),
                schema_view,
                database_root,
            )
        }
        (SchemaClosureProof::Reconciled(proof), SchemaSource::Reconciled(layers)) => {
            let ids = verify_reconciled_transitive_closure(
                layers,
                schema.schema_root(),
                SYS_INHERITS_IRI.as_bytes(),
                class_iri.as_bytes(),
                /* reflexive */ true,
                proof,
            )?;
            let objects = resolve_reconciled_node_ids(layers, &ids)?;
            objects
                .into_iter()
                .map(|object| match object {
                    CanonicalObject::Node(bytes) => Ok(bytes),
                    _ => Err(invalid("reconciled ancestor closure produced a non-node class")),
                })
                .collect()
        }
        // The closure variant (prover data) disagrees with the entry-chosen schema route → reject.
        _ => Err(invalid(
            "ancestor closure route does not match the entry point's schema source",
        )),
    }
}

/// The schema-graph PROVE routing — the prove-side twin of [`SchemaSource`], likewise chosen by the
/// prove ENTRY POINT. It produces a source-agnostic [`SchemaSubjectProof`] / [`SchemaClosureProof`]
/// plus the resolved rows / ancestor set for any bound schema node.
enum SchemaProver<'a> {
    /// Prove schema reads over a single materialized schema layer (the base path).
    Materialized { layer: &'a LayerCommitment, root: ProofHash },
    /// Prove schema reads over a delta schema stack (base..head, head last) — the 6e delta path.
    Reconciled { layers: &'a [&'a LayerCommitment], head_root: ProofHash },
}

impl SchemaProver<'_> {
    fn schema_root(&self) -> ProofHash {
        match self {
            SchemaProver::Materialized { root, .. } => *root,
            SchemaProver::Reconciled { head_root, .. } => *head_root,
        }
    }

    /// Prove ONE bound-subject schema read and resolve its rows. Materialized → a bound-subject BGP;
    /// delta → a reconciled single-atom present filter (`FilterPosition::Subject`) over the schema
    /// chain. The reconciled arm proves AND re-verifies (to recover the resolved rows the prover needs
    /// to discover the frame), exactly as the instance-side twin.
    fn read_node(
        &self,
        ctx: &StoreProverContext<'_>,
        node_iri: &str,
    ) -> Result<(SchemaSubjectProof, SubjectRows), QueryProofError> {
        match self {
            SchemaProver::Materialized { layer, root } => {
                let compiled = bound_subject_bgp(node_iri)?;
                let proved = compiled.prove_with_context(ctx, layer)?;
                let rows = resolve_subject_rows(&proved, &compiled, *layer, *root)?;
                Ok((SchemaSubjectProof::Materialized(proved), rows))
            }
            SchemaProver::Reconciled { layers, head_root } => {
                let constant = subject_filter_constant(node_iri);
                let (reconcile, filter, read) =
                    prove_read_single_atom(layers, FilterPosition::Subject, constant)?;
                let columns = read.columns;
                // I2a: carry the prover's per-column id-hints; the belt-hash inside
                // `verify_and_resolve_single_atom` binds them to the authenticated result hashes.
                let id_hints = read.id_hints;
                let resolved = verify_and_resolve_single_atom(
                    layers,
                    *head_root,
                    FilterPosition::Subject,
                    constant,
                    &reconcile,
                    &filter,
                    &columns,
                    &id_hints,
                )?;
                let rows = reshape_present_subject_rows(resolved)?;
                Ok((SchemaSubjectProof::Reconciled { reconcile, filter, columns, id_hints }, rows))
            }
        }
    }

    /// Prove the reflexive `sys:inherits` ancestor closure of `class_iri` and resolve its class SET.
    /// Materialized → the audited subsumption BGP anchored into the combined `database_root` (needs the
    /// instance anchor from `instance`); delta → the store's `TransitiveClosure`-over-`present` on the
    /// schema chain (FOUNDATION-B) whose `Vec<u64>` output is rendered to IRIs via
    /// `resolve_reconciled_node_ids`.
    fn prove_ancestor_closure(
        &self,
        ctx: &StoreProverContext<'_>,
        class_iri: &str,
        instance: &InstanceProver<'_>,
        db_root: ProofHash,
    ) -> Result<(SchemaClosureProof, Vec<Vec<u8>>), QueryProofError> {
        match self {
            SchemaProver::Materialized { layer, .. } => {
                let compiled = ancestor_closure_bgp(class_iri)?;
                let closure = compiled.prove_with_context(ctx, layer)?;
                let set = resolve_closure_set(
                    &closure,
                    &compiled,
                    instance.root_binding_layer(),
                    *layer,
                    db_root,
                )?;
                Ok((SchemaClosureProof::Materialized(closure), set))
            }
            SchemaProver::Reconciled { layers, .. } => {
                let (proof, ids) = prove_reconciled_transitive_closure(
                    layers,
                    SYS_INHERITS_IRI.as_bytes(),
                    class_iri.as_bytes(),
                    /* reflexive */ true,
                )?;
                let objects = resolve_reconciled_node_ids(layers, &ids)?;
                let set = objects
                    .into_iter()
                    .map(|object| match object {
                        CanonicalObject::Node(bytes) => Ok(bytes),
                        _ => Err(invalid("reconciled ancestor closure produced a non-node class")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((SchemaClosureProof::Reconciled(Box::new(proof)), set))
            }
        }
    }
}

/// Verify + resolve the ancestor-closure sub-proof and return its authenticated class SET as node-IRI
/// byte vectors. The closure carries `graph: ProofGraph::Schema` in its plan, so — UNLIKE the
/// graph-blind bound-subject frame reads — it must be routed through `verify_over_graphs_and_resolve`
/// (which anchors it to the SCHEMA root under the trusted combined `database_root`), not the
/// instance-guarded `verify_and_resolve`. The delivered set is the closure's proven output: complete
/// (empty-delta fixpoint) and de-duplicated (the closure's intrinsic set-collapse `Distinct`).
fn resolve_closure_set(
    proved: &ProvedBgp,
    compiled: &CompiledBgp,
    instance: &impl VerifierLayer,
    schema: &impl VerifierLayer,
    database_root: ProofHash,
) -> Result<Vec<Vec<u8>>, QueryProofError> {
    let columns = proved.verify_over_graphs_and_resolve(compiled, instance, schema, database_root)?;
    let column = match columns.as_slice() {
        [single] => single,
        _ => return Err(invalid("ancestor closure must resolve to exactly one class column")),
    };
    let mut set = Vec::with_capacity(column.len());
    for object in column {
        match object {
            CanonicalObject::Node(bytes) => set.push(bytes.clone()),
            _ => return Err(invalid("ancestor closure column carries a non-node value")),
        }
    }
    Ok(set)
}

/// Bind the class `C` from the document's direct triples (F0): EXACTLY ONE `rdf:type` row, whose
/// object is a node. Zero or multiple `rdf:type` rows, or a non-node type, reject-close.
fn extract_class(rows: &SubjectRows) -> Result<Vec<u8>, QueryProofError> {
    let mut class: Option<Vec<u8>> = None;
    for (predicate, object) in rows {
        if predicate.as_slice() != RDF_TYPE_IRI.as_bytes() {
            continue;
        }
        let node = match object {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => return Err(invalid("document rdf:type object is not a node")),
        };
        if class.is_some() {
            return Err(invalid("document has more than one rdf:type row"));
        }
        class = Some(node);
    }
    class.ok_or_else(|| invalid("document has no rdf:type row"))
}

// ---------------------------------------------------------------------------------------------------
// Frame classification (F1-F3) — one algorithm, two readers.
//
// A `SchemaReader` returns a schema node's direct triples on demand. The PROVER reader proves +
// resolves each node's schema BGP (recording the sub-proof); the VERIFIER reader looks a node up in
// the already-root-verified range map (rejecting on a missing/injected node). The classification logic
// is IDENTICAL for both, which is what keeps prove and verify in lock-step: both request exactly the
// same set of schema nodes.
// ---------------------------------------------------------------------------------------------------

trait SchemaReader {
    fn read(&mut self, node: &[u8]) -> Result<SubjectRows, QueryProofError>;
}

/// Prover-side reader: proves + resolves each requested schema node once through the entry-chosen
/// [`SchemaProver`] (materialized BGP or reconciled present filter), recording a source-agnostic
/// [`ProvedRangeClassification`].
struct ProveReader<'a> {
    ctx: &'a StoreProverContext<'a>,
    schema: &'a SchemaProver<'a>,
    cache: BTreeMap<Vec<u8>, SubjectRows>,
    ranges: Vec<ProvedRangeClassification>,
}

impl SchemaReader for ProveReader<'_> {
    fn read(&mut self, node: &[u8]) -> Result<SubjectRows, QueryProofError> {
        if let Some(rows) = self.cache.get(node) {
            return Ok(rows.clone());
        }
        let node_iri = std::str::from_utf8(node)
            .map_err(|_| invalid("schema node IRI is not valid UTF-8"))?;
        let (schema_read, rows) = self.schema.read_node(self.ctx, node_iri)?;
        self.ranges.push(ProvedRangeClassification {
            node: node.to_vec(),
            schema_read,
        });
        self.cache.insert(node.to_vec(), rows.clone());
        Ok(rows)
    }
}

/// Verifier-side reader: looks a schema node up in the root-verified range map. A node the
/// classification needs but the bundle omitted rejects (a prover cannot hide a range's shape); the
/// caller separately rejects any UNUSED map entry (an injected sub-proof).
struct VerifyReader<'a> {
    map: &'a BTreeMap<Vec<u8>, SubjectRows>,
    used: BTreeSet<Vec<u8>>,
}

impl SchemaReader for VerifyReader<'_> {
    fn read(&mut self, node: &[u8]) -> Result<SubjectRows, QueryProofError> {
        self.used.insert(node.to_vec());
        self.map.get(node).cloned().ok_or_else(|| {
            invalid(format!(
                "bundle is missing the range classification for schema node {}",
                iri_string(node)
            ))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RangeShape {
    Scalar,
    Link,
    Set,
    List,
    /// A 1-D DENSE `sys:Array` property (increment 4). Rendered from an authenticated `OrderedArrayWalk`
    /// (the exact `sys:index`-ordered `sys:value` column), DISTINCT from a `List` (a cons chain) — an
    /// array is `@container:@array`, ordered by an authenticated numeric key rather than chain topology.
    Array,
}

/// One classified frame property: its predicate, its range shape, and whether the frame RANGE node
/// bears `sys:unfold` (making it an `@unfold` property, `schema_property_is_unfold`, schema.pl:716).
/// The `unfold` flag is consulted for a STOP `Link` (a non-subdocument target the reference would
/// unfold) AND for a present `Set`/`List` collection (the reference unfolds EVERY element): in either
/// case v1 renders bare ids, so an `@unfold` property reject-closes rather than omit the unfolded body.
#[derive(Clone, Debug)]
struct ClassifiedProperty {
    predicate: Vec<u8>,
    shape: RangeShape,
    unfold: bool,
}

/// Summary of a schema node's direct triples used by classification.
#[derive(Default)]
struct NodeInfo {
    rdf_types: Vec<Vec<u8>>,
    sys_class: Option<CanonicalObject>,
    /// `sys:dimensions` on THIS node — meaningful only on a `sys:Array` descriptor. `None` = the
    /// default (1-D); a `Some` whose value is not the literal `1` is a multi-dimensional array, rejected
    /// (`classify_range`).
    sys_dimensions: Option<CanonicalObject>,
    has_subdocument: bool,
    /// `sys:unfoldable` (`schema_is_unfoldable`, schema.pl:674) — the class asks to be unfolded inline
    /// even though it is NOT a subdocument. Increment 3 does not inline these; a single-valued LINK to
    /// such a class reject-closes (see [`BgpPlanError::ReadDocumentUnfoldableDeferred`]).
    has_unfoldable: bool,
    /// `sys:unfold` on THIS node — when this node is a property's frame RANGE descriptor, its presence
    /// makes the property an `@unfold` property (`schema_property_is_unfold`, schema.pl:716). Same
    /// reject as `@unfoldable`.
    has_unfold: bool,
    has_inherits: bool,
    has_oneof: bool,
}

impl NodeInfo {
    fn has_type(&self, iri: &str) -> bool {
        self.rdf_types.iter().any(|t| t.as_slice() == iri.as_bytes())
    }

    /// A LINK-eligible target: a `sys:Class` / `sys:TaggedUnion` / `sys:JSON(Document)` node
    /// (`system_class` minus Enum/Unit/Foreign, which are classified/rejected earlier).
    fn is_simple_class(&self) -> bool {
        self.has_type(SYS_CLASS_TYPE)
            || self.has_type(SYS_TAGGED_UNION)
            || self.has_type(SYS_JSON)
            || self.has_type(SYS_JSONDOCUMENT)
    }

    /// Bears ANY of the server's full `system_class/1` set (schema.pl:143) — `sys:Class`,
    /// `TaggedUnion`, `Enum`, `Unit`, `Foreign`, `JSON`, `JSONDocument`. These are the kinds the
    /// reference resolves (as unit/foreign/enum/class) at `schema_type_descriptor_compute` steps
    /// ~1147-1164, i.e. BEFORE it ever consults a collection descriptor. Used only by the
    /// conflicting-type guard; classification proper still uses the narrower `is_simple_class`.
    fn bears_system_class_type(&self) -> bool {
        self.has_type(SYS_CLASS_TYPE)
            || self.has_type(SYS_TAGGED_UNION)
            || self.has_type(SYS_ENUM)
            || self.has_type(SYS_UNIT)
            || self.has_type(SYS_FOREIGN)
            || self.has_type(SYS_JSON)
            || self.has_type(SYS_JSONDOCUMENT)
    }

    /// Bears ANY collection / cardinality descriptor type (schema.pl:1166-1215) — `sys:Set`, `List`,
    /// `Array`, `Table`, `Cardinality`, `Optional`. Disjoint from `bears_system_class_type` on a
    /// well-formed (single-typed) schema node.
    fn bears_collection_descriptor_type(&self) -> bool {
        self.has_type(SYS_SET)
            || self.has_type(SYS_LIST)
            || self.has_type(SYS_ARRAY)
            || self.has_type(SYS_TABLE)
            || self.has_type(SYS_CARDINALITY)
            || self.has_type(SYS_OPTIONAL)
    }
}

fn summarize(rows: &SubjectRows) -> NodeInfo {
    let mut info = NodeInfo::default();
    for (predicate, object) in rows {
        let predicate = predicate.as_slice();
        if predicate == RDF_TYPE_IRI.as_bytes() {
            if let CanonicalObject::Node(bytes) = object {
                info.rdf_types.push(bytes.clone());
            }
        } else if predicate == SYS_CLASS_PRED.as_bytes() {
            info.sys_class = Some(object.clone());
        } else if predicate == SYS_DIMENSIONS.as_bytes() {
            info.sys_dimensions = Some(object.clone());
        } else if predicate == SYS_SUBDOCUMENT.as_bytes() {
            info.has_subdocument = true;
        } else if predicate == SYS_UNFOLDABLE.as_bytes() {
            info.has_unfoldable = true;
        } else if predicate == SYS_UNFOLD.as_bytes() {
            info.has_unfold = true;
        } else if predicate == SYS_INHERITS_IRI.as_bytes() {
            info.has_inherits = true;
        } else if predicate == SYS_ONEOF.as_bytes() {
            info.has_oneof = true;
        }
    }
    info
}

/// Classify a property's range `R` (F2). Mirrors `schema_type_descriptor_compute` (`schema.pl:1147`),
/// rejecting every shape v1 cannot render faithfully.
fn classify_range(
    range: &CanonicalObject,
    reader: &mut dyn SchemaReader,
) -> Result<RangeShape, QueryProofError> {
    let range_iri = match range {
        CanonicalObject::Node(bytes) => bytes.as_slice(),
        _ => return Err(shape_reject("non-node-range")),
    };
    // A base type is a primitive with no schema triples → SCALAR (no schema read needed). This now
    // INCLUDES `rdf:langString` (increment 5): a language-tagged literal round-trips as a single
    // `CanonicalObject::Value { datatype: LangString, lexical }` whose lexical is the store's glued
    // `"lang@text"` (`make_entry_from_lang_term`, store-prolog value.rs; `LangString` is a `stringy_type`
    // storing raw bytes exactly like `xsd:string`, which the flat-scalar path already proves round-trips
    // intact). Both the text AND the language tag are recoverable from the authenticated lexical, so the
    // scalar render is FAITHFUL to json.pl's `{'@value': text, '@lang': lang}` (base_class `Elt = X@L`
    // clause, json.pl:2506) — the tag is carried in the value, not dropped.
    if is_base_type(range_iri) {
        return Ok(RangeShape::Scalar);
    }
    // A non-base range MUST be defined in the schema graph; read its direct triples and classify.
    let rows = reader.read(range_iri)?;
    let info = summarize(&rows);
    if info.rdf_types.is_empty() {
        return Err(shape_reject("undefined-range"));
    }
    // PRECEDENCE HARDENING (fail-closed). The server (`schema_type_descriptor_compute`,
    // schema.pl:1147-1215) settles a node's system-class identity FIRST — `is_schema_simple_class` at
    // ~1162 — and only THEN, if that failed, considers the collection descriptors (`sys:Set`/`List`/…
    // at ~1166-1215). This classifier checks the collection descriptors first (below), so a MALFORMED
    // node claiming BOTH a system-class type (`sys:Class`, `TaggedUnion`, …) AND a collection type
    // would classify differently here (as a set/list) than the server would (as a class/link). A
    // well-formed schema descriptor node is single-typed, so a node that is both a class and a
    // collection cannot arise in a validated (trusted-root) schema. Rather than silently pick a
    // reading and risk diverging from the server, refuse it.
    if info.bears_system_class_type() && info.bears_collection_descriptor_type() {
        return Err(shape_reject("conflicting-class-and-collection"));
    }
    // INCREMENT 3: a SIMPLE-CLASS range is NO LONGER rejected for `sys:subdocument` or `sys:inherits`
    // here. Whether a single-valued class LINK inlines is decided in the per-CHILD recursion from the
    // CHILD's authenticated `rdf:type` (never the schema range — a subclass of the range can introduce
    // subdocument-ness), so this classifier returns the shape-neutral `Link` for ANY simple class and
    // lets [`render`] key the inline/stop decision off the child. The `Array` / cardinality / choice /
    // enum / collection guards below still reject-close (those shapes are unrelated to inlining).
    if info.has_type(SYS_ARRAY) {
        // INCREMENT 4: a 1-D DENSE `sys:Array` of scalar/plain-link elements is now ACCEPTED (rendered
        // from an authenticated `OrderedArrayWalk`). Reject-close the shapes v1 cannot render:
        //   * MULTI-DIMENSIONAL — a present `sys:dimensions` != `1` (the store composite reads only the
        //     level-1 `sys:index` column; a multi-dim array carries `sys:index2…`). ABSENT = default 1-D.
        //   * a non-`check_element_class`-clean element (subdocument / inheriting / `@unfoldable` /
        //     collection / `langString` element — identical element guard to `List`/`Set`).
        if let Some(dimensions) = info.sys_dimensions.as_ref() {
            let is_one_d = match dimensions {
                // The order-preserving lexical of a `1`-valued integer literal is exactly `"1"`.
                CanonicalObject::Value { lexical, .. } => lexical.as_slice() == b"1",
                // A non-literal `sys:dimensions` (a node) is malformed — treat as not 1-D (reject).
                _ => false,
            };
            if !is_one_d {
                return Err(shape_reject("array-multidim"));
            }
        }
        let element = info
            .sys_class
            .as_ref()
            .ok_or_else(|| shape_reject("array-without-class"))?;
        check_element_class(element, reader)?;
        return Ok(RangeShape::Array);
    }
    if info.has_type(SYS_SET) {
        // INCREMENT 5: a `sys:Set` — WITH or WITHOUT cardinality bounds — renders as a SET. The server's
        // `set(Class,N,M)` (bounded) and `set(Class)` (unbounded) descriptors render IDENTICALLY: both go
        // through `set_list_` + `list_type_id_predicate_value_` (json.pl:2453-2460), one rendered value
        // per stored element. The `min`/`max_cardinality` are INSERT-TIME validation and never reach the
        // rendered document, so the existing `Set` render is faithful for a bounded set too. (The old
        // `cardinality-set` reject is removed — under-covering it was safe, but the render IS faithful.)
        let element = info
            .sys_class
            .as_ref()
            .ok_or_else(|| shape_reject("set-without-class"))?;
        check_element_class(element, reader)?;
        return Ok(RangeShape::Set);
    }
    if info.has_type(SYS_LIST) {
        let element = info
            .sys_class
            .as_ref()
            .ok_or_else(|| shape_reject("list-without-class"))?;
        check_element_class(element, reader)?;
        return Ok(RangeShape::List);
    }
    if info.has_type(SYS_TABLE) {
        return Err(shape_reject("table"));
    }
    if info.has_type(SYS_CARDINALITY) {
        // INCREMENT 5: `sys:Cardinality(T, min, max)` renders EXACTLY as a SET. The server's
        // `type_id_predicate_iri_value_(cardinality(C,_,_),…)` (json.pl:2461) reads ALL objects via
        // `set_list_` and renders them with `list_type_id_predicate_value_` (one value per element, with
        // NO singleton collapse — a `max_cardinality == 1` still renders a 1-element array, not a scalar),
        // identical to a plain `Set`. The min/max bounds are insert-time cardinality validation, NOT part
        // of the rendered output. So a Cardinality of a `check_element_class`-clean element is the existing
        // `Set` render — faithful. (We do NOT enforce min-cardinality presence: render-what's-stored, an
        // absent property is omitted, never synthesised.)
        let element = info
            .sys_class
            .as_ref()
            .ok_or_else(|| shape_reject("cardinality-without-class"))?;
        check_element_class(element, reader)?;
        return Ok(RangeShape::Set);
    }
    if info.has_type(SYS_OPTIONAL) {
        // INCREMENT 5: `sys:Optional<T>` renders EXACTLY as its element T's descriptor. The server's
        // `type_id_predicate_iri_value_(optional(C),…)` (json.pl:2489) UNWRAPS the Optional and renders
        // the single stored value AS `schema_type_descriptor(C)` — the Optional adds NO container. PRESENT
        // ⇒ the single value is rendered; ABSENT ⇒ there is no instance triple, so the property never
        // enters the complete bound-subject rows and is omitted (render-what's-stored: an absent optional
        // is dropped, never `null`-filled). v1 accepts a `check_element_class`-clean element:
        //   * a BASE TYPE renders as a SCALAR (the single stored literal);
        //   * a plain (non-subdocument, non-inheriting) SIMPLE CLASS renders as a single-valued LINK — the
        //     existing per-child inline/stop path, so an Optional whose ACTUAL object is a subdocument
        //     subclass still inlines faithfully, and a plain-link target renders its id.
        // A subdocument / unfoldable / inheriting / collection element reject-closes in
        // `check_element_class` (v1 does not resolve those under an Optional wrapper — under-covering,
        // never an unfaithful accept).
        let element = info
            .sys_class
            .as_ref()
            .ok_or_else(|| shape_reject("optional-without-class"))?;
        check_element_class(element, reader)?;
        let element_iri = match element {
            CanonicalObject::Node(bytes) => bytes.as_slice(),
            // `check_element_class` already rejected a non-node element; fail-closed defensively.
            _ => return Err(shape_reject("non-node-element")),
        };
        // Optional<base type> ⇒ SCALAR; Optional<simple class> ⇒ LINK (per-child inline/stop).
        return Ok(if is_base_type(element_iri) {
            RangeShape::Scalar
        } else {
            RangeShape::Link
        });
    }
    if info.has_type(SYS_ENUM) {
        return Err(shape_reject("enum"));
    }
    if info.has_type(SYS_FOREIGN) {
        return Err(shape_reject("foreign"));
    }
    if info.has_type(SYS_UNIT) {
        return Err(shape_reject("unit"));
    }
    if info.is_simple_class() {
        // ANY simple class (inheriting or not, subdocument or not) is a single-valued LINK here; the
        // inline-vs-stop decision is deferred to the per-child recursion (keyed off the child's own
        // authenticated type + closure), so an inheriting or subdocument link target is now ACCEPTED at
        // this classification step.
        return Ok(RangeShape::Link);
    }
    Err(shape_reject("unknown-range"))
}

/// Check a `Set`/`List` ELEMENT class `E` is faithfully renderable as a scalar or link element: a base
/// type, or a non-subdocument, non-inheriting simple class. A subdocument / inheriting / collection
/// element reject-closes (v1 renders element node ids / literals, never inlined children).
fn check_element_class(
    element: &CanonicalObject,
    reader: &mut dyn SchemaReader,
) -> Result<(), QueryProofError> {
    let element_iri = match element {
        CanonicalObject::Node(bytes) => bytes.as_slice(),
        _ => return Err(shape_reject("non-node-element")),
    };
    // `rdf:langString` is a base type (increment 5 — see `classify_range`): a langString collection
    // element round-trips as a faithful langString scalar (datatype + glued `"lang@text"` lexical), so
    // there is no special element reject.
    if is_base_type(element_iri) {
        return Ok(());
    }
    let rows = reader.read(element_iri)?;
    let info = summarize(&rows);
    if info.rdf_types.is_empty() {
        return Err(shape_reject("undefined-element"));
    }
    // A Set/List OF subdocument stays reject-closed in increment 3 (v1 only inlines single-valued
    // `Link` subdocument properties; a subdocument COLLECTION element is a larger effort). The
    // inheriting-element reject is likewise kept — an inheriting collection element could inherit
    // subdocument-ness we do not resolve here.
    if info.has_subdocument {
        return Err(shape_reject("subdocument-element"));
    }
    // A DECLARED `@unfoldable` element class (`schema_is_unfoldable`, schema.pl:674) is unfolded inline
    // by the reference for EVERY element (json.pl set/list clauses 2445-2476 -> `should_unfold_property_`
    // 2343-2352). v1 renders collection elements as bare ids, so an unfoldable declared element would
    // silently OMIT the unfolded content — reject closed. (Reflexive-closure completeness is already
    // satisfied: an INHERITING element reject-closes just below, so a non-inheriting element's reflexive
    // `sys:inherits` closure is exactly itself, making this direct `has_unfoldable` check the whole-
    // closure check. The ACTUAL element object's type — which may be an unfoldable/subdocument SUBCLASS
    // of this declared class — is checked separately, per element, at render time.)
    if info.has_unfoldable {
        return Err(shape_reject("unfoldable-element"));
    }
    if info.has_inherits {
        return Err(shape_reject("inheriting-element"));
    }
    if info.has_type(SYS_SET)
        || info.has_type(SYS_LIST)
        || info.has_type(SYS_ARRAY)
        || info.has_type(SYS_TABLE)
        || info.has_type(SYS_CARDINALITY)
        || info.has_type(SYS_OPTIONAL)
    {
        return Err(shape_reject("collection-element"));
    }
    if info.is_simple_class() {
        return Ok(());
    }
    Err(shape_reject("unknown-element"))
}

/// Nature-check ONE class in the reflexive super-closure and fold its directly-declared properties
/// into the EFFECTIVE-FRAME UNION. This is one class's contribution to the reference
/// `schema_class_predicate_type` (schema.pl:332), which is the UNION over `{C} ∪ ancestors(C)` of each
/// class's OWN declared `(predicate, range)` pairs (built-ins filtered).
///
/// INHERITANCE-CLOSED NATURES (fail-closed). `sys:subdocument` IS inheritance-closed
/// (`schema_is_subdocument`, schema.pl:654 — a class is a subdocument iff IT or ANY super bears it), so
/// a subdocument nature ANYWHERE in the closure makes `C` a subdocument → reject. A tagged-union /
/// `sys:oneOf` / enum nature is NOT strictly inheritance-closed in the reference (a subclass of a
/// tagged union is not itself one), BUT a tagged-union ancestor's oneOf-CHOICE properties DO flow into
/// `C`'s effective frame through a `sys:oneOf`-group node that the direct `Triple(K, ?p, ?r)` BGP does
/// NOT enumerate — so rendering `C` without them would silently OMIT inherited properties. We therefore
/// refuse ANY such nature anywhere in the chain (conservative, never unsound). Every class in a valid
/// super-chain is a plain `sys:Class` (validation forbids inheriting from anything but
/// `sys:Class`/`TaggedUnion`, and we reject the latter), so a non-`sys:Class` class also rejects.
fn collect_class_frame(
    rows: &SubjectRows,
    union: &mut BTreeMap<Vec<u8>, CanonicalObject>,
    allow_subdocument: bool,
) -> Result<(), QueryProofError> {
    let info = summarize(rows);
    // ONEOF / TAGGED-UNION NARROWING (increment 5, fail-closed). A choice class carries its alternative
    // "choice" properties in a place the direct `Triple(K, ?p, ?r)` frame BGP does NOT enumerate, so
    // rendering `K` from its direct properties alone would silently OMIT the choice properties:
    //   * a `sys:TaggedUnion` CLASS (`rdf:type sys:TaggedUnion`) holds the choice properties as its own
    //     direct predicates, but is not a `sys:Class`, so `is_simple_class`/link/inheritance handling
    //     would diverge — refuse it (checked BEFORE the sys:Class requirement so it gives a precise
    //     error, not the generic `class-not-sys-class`);
    //   * a plain `sys:Class` bearing a `sys:oneOf` GROUP node holds the choice properties on the SEPARATE
    //     group node (`t(K, sys:oneOf, R)`, `t(R, choiceProp, Range)` — `schema_class_predicate_oneof`,
    //     schema.pl:322-326) that the direct BGP never reads — refuse it too.
    // Reading the choice group faithfully (multiple groups, per-choice ranges, the "exactly one present"
    // semantics) is a larger, separately-audited effort; v1 reject-closes rather than risk an omission.
    if info.has_type(SYS_TAGGED_UNION) || info.has_type(SYS_ENUM) {
        return Err(shape_reject("tagged-union-or-enum-class"));
    }
    if !info.has_type(SYS_CLASS_TYPE) {
        return Err(shape_reject("class-not-sys-class"));
    }
    if info.has_oneof {
        return Err(shape_reject("tagged-union"));
    }
    // Subdocument nature (inheritance-closed). The ROOT node passes `allow_subdocument = false`: a
    // document's OWN top-level class must not be a subdocument (a subdocument is inlined into a parent,
    // not read directly by id) → the repurposed top-level reject. An INLINED child passes
    // `allow_subdocument = true`: a subdocument child is EXPECTED to bear `sys:subdocument`, so we
    // render its body rather than reject.
    if info.has_subdocument && !allow_subdocument {
        return Err(BgpPlanError::ReadDocumentSubdocumentDeferred.into());
    }
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    for (predicate, range) in rows {
        if is_built_in(predicate) {
            continue;
        }
        // A predicate declared twice WITHIN one class's own frame is malformed (preserves the flat
        // path's duplicate-property reject; the cross-class case is handled by the union below).
        if !seen.insert(predicate.clone()) {
            return Err(shape_reject("duplicate-property"));
        }
        match union.get(predicate) {
            None => {
                union.insert(predicate.clone(), range.clone());
            }
            // Reached again with the SAME range — an identical (re)declaration up the chain, or a
            // diamond reaching one ancestor via two paths. De-duplicate, exactly as the reference's
            // tabling collapses identical `(predicate, type)` tuples.
            Some(existing) if existing == range => {}
            // The SAME predicate declared with a DIFFERENT range across the closure. A validated schema
            // cannot produce this (`schema_class_predicate_type` would return two conflicting answers,
            // which schema validation forbids). Fail-closed rather than pick one arbitrarily.
            Some(_) => return Err(shape_reject("conflicting-inherited-range")),
        }
    }
    Ok(())
}

/// Read + classify the EFFECTIVE (inherited-union) class frame (F1-F3). `class`/`class_rows` are `C`
/// and its OWN direct schema triples (from the already-proved `class_schema` BGP); `ancestor_set` is
/// the AUTHENTICATED reflexive `sys:inherits` closure `{C} ∪ ancestors(C)` (exactly `{C}` for a
/// non-inheriting `C`). For EACH class in the set the frame reads its directly-declared properties —
/// `C` from `class_rows`, every STRICT ancestor through `reader` (a bound-subject schema BGP recorded
/// at prove / root-looked-up at verify, like a range node) — nature-checks it across the whole closure,
/// and UNIONs the `(predicate, range)` pairs; every unioned range is then classified exactly as the
/// flat path. Prove and verify run this identically over the identical authenticated inputs, so they
/// request the identical set of schema nodes (which is what keeps the range-map missing/injected
/// cross-checks exact).
fn classify_frame(
    class: &[u8],
    class_rows: &SubjectRows,
    ancestor_set: &[Vec<u8>],
    reader: &mut dyn SchemaReader,
    allow_subdocument: bool,
) -> Result<Vec<ClassifiedProperty>, QueryProofError> {
    // The reflexive closure MUST contain the seed `C` itself. A valid `build_subsumption_closure` proof
    // always does (`reflexive = true`); assert it defensively (fail-closed) so a degenerate set can
    // never silently drop `C`'s own frame.
    if !ancestor_set.iter().any(|k| k.as_slice() == class) {
        return Err(invalid("ancestor closure omits the document class itself"));
    }
    // Union the directly-declared (predicate -> range) pairs across the reflexive super-closure.
    let mut union: BTreeMap<Vec<u8>, CanonicalObject> = BTreeMap::new();
    for k in ancestor_set {
        if k.as_slice() == class {
            // `C`'s own rows come from the already-proved `class_schema` BGP (never re-read here).
            collect_class_frame(class_rows, &mut union, allow_subdocument)?;
        } else {
            // Every STRICT ancestor is read through the reader (recorded at prove, root-verified at
            // verify). Both sides iterate the IDENTICAL authenticated `ancestor_set`.
            let rows = reader.read(k)?;
            collect_class_frame(&rows, &mut union, allow_subdocument)?;
        }
    }
    // Classify each unioned range (reads any non-primitive range/element node through `reader`).
    let mut properties: Vec<ClassifiedProperty> = Vec::with_capacity(union.len());
    for (predicate, range) in &union {
        let shape = classify_range(range, reader)?;
        // Detect the `@unfold` property annotation: the frame RANGE node bears `sys:unfold`
        // (`schema_property_is_unfold`, schema.pl:716). It is meaningful for a `Link` (a single-valued
        // reference the reference unfolds inline) AND for a `Set`/`List` collection descriptor (whose
        // presence makes the reference unfold EVERY element) — read it for all three. A base-type /
        // scalar range never unfolds. The range read is cached, so this costs no extra sub-proof.
        let unfold = match shape {
            // A `Link` / `Set` / `List` / `Array` range node may bear `sys:unfold` (the reference unfolds
            // the reference / EVERY element inline). v1 renders bare ids, so an `@unfold` collection —
            // arrays included, per the inc-3 collection rule — reject-closes rather than omit the body.
            RangeShape::Link | RangeShape::Set | RangeShape::List | RangeShape::Array => match range {
                CanonicalObject::Node(r) if !is_base_type(r) => {
                    summarize(&reader.read(r)?).has_unfold
                }
                _ => false,
            },
            RangeShape::Scalar => false,
        };
        properties.push(ClassifiedProperty {
            predicate: predicate.clone(),
            shape,
            unfold,
        });
    }
    Ok(properties)
}

/// The subdocument / unfoldable natures of a class, computed across its reflexive `sys:inherits`
/// closure `{class} ∪ ancestors` (LENIENT — unlike [`collect_class_frame`] it does NOT reject a
/// tagged-union / enum / oneOf class, because a STOP link to such a document is faithfully rendered as
/// a bare id). `class_rows` are the class's OWN schema triples; each STRICT ancestor is read through
/// `reader`. `has_subdocument` mirrors `schema_is_subdocument` (schema.pl:654); `has_unfoldable`
/// mirrors `schema_is_unfoldable` (schema.pl:674) — both inheritance-closed.
#[derive(Clone, Copy, Debug, Default)]
struct ClosureNatures {
    has_subdocument: bool,
    has_unfoldable: bool,
}

fn closure_natures(
    class: &[u8],
    class_rows: &SubjectRows,
    ancestor_set: &[Vec<u8>],
    reader: &mut dyn SchemaReader,
) -> Result<ClosureNatures, QueryProofError> {
    let mut natures = ClosureNatures::default();
    let seed = summarize(class_rows);
    natures.has_subdocument |= seed.has_subdocument;
    natures.has_unfoldable |= seed.has_unfoldable;
    for k in ancestor_set {
        if k.as_slice() == class {
            continue;
        }
        let info = summarize(&reader.read(k)?);
        natures.has_subdocument |= info.has_subdocument;
        natures.has_unfoldable |= info.has_unfoldable;
    }
    Ok(natures)
}

// ---------------------------------------------------------------------------------------------------
// Subdocument recursion budget (increment 3). Enforced IDENTICALLY in prove and verify so a prover can
// NEVER make verify accept a TRUNCATED subdocument tree: over the depth OR the total-node budget both
// sides reject-close rather than stop early. Kept small — genuine subdocument nesting is shallow, and
// every node is several BGP proofs plus a closure, so the budget also bounds proving cost.
// ---------------------------------------------------------------------------------------------------

/// Max INLINE depth (the root is depth 0; each inlined subdocument child is one deeper).
const MAX_SUBDOC_DEPTH: usize = 3;
/// Max total document nodes (root + every inlined subdocument).
const MAX_SUBDOC_NODES: usize = 64;

/// Like [`extract_class`] but returns `None` (not an error) when the node has NO `rdf:type` row — a
/// typeless / dangling LINK target, which the reference renders as a plain link id (a typeless target
/// is trivially non-subdocument). Still errors on a non-node type or MULTIPLE `rdf:type` rows.
fn extract_class_opt(rows: &SubjectRows) -> Result<Option<Vec<u8>>, QueryProofError> {
    let mut class: Option<Vec<u8>> = None;
    for (predicate, object) in rows {
        if predicate.as_slice() != RDF_TYPE_IRI.as_bytes() {
            continue;
        }
        let node = match object {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => return Err(invalid("node rdf:type object is not a node")),
        };
        if class.is_some() {
            return Err(invalid("node has more than one rdf:type row"));
        }
        class = Some(node);
    }
    Ok(class)
}

// ---------------------------------------------------------------------------------------------------
// Prove (recursive per-node).
// ---------------------------------------------------------------------------------------------------

/// The proven per-node DECISION CORE: the instance subject BGP (→ class `C`), `C`'s own schema frame
/// read, the reflexive `sys:inherits` closure, and the strict-ancestor nature reads — everything
/// needed to decide whether `C` is a subdocument, WITHOUT classifying `C`'s property ranges. A STOP
/// link keeps ONLY this (never the child's body); the root and an inlined subdocument EXTEND it into a
/// full node, REUSING every proven piece (no re-proving).
struct ProvenCore {
    subject: InstanceSubjectProof,
    d_rows: SubjectRows,
    /// `None` => the node has NO `rdf:type` row (typeless/dangling link → non-subdocument stop).
    class: Option<Vec<u8>>,
    class_schema: Option<SchemaSubjectProof>,
    class_rows: SubjectRows,
    ancestor_closure: Option<SchemaClosureProof>,
    ancestor_set: Vec<Vec<u8>>,
    ancestor_reads: Vec<ProvedRangeClassification>,
    ancestor_cache: BTreeMap<Vec<u8>, SubjectRows>,
    natures: ClosureNatures,
}

/// Prove one node's decision core (see [`ProvenCore`]). Reads the instance subject BGP, binds the
/// class, proves the reflexive `sys:inherits` closure (only when the class inherits), reads each strict
/// ancestor's frame, and computes the inheritance-closed subdocument/unfoldable natures. NO property
/// range classification happens here — so a plain link target's own (possibly unsupported-shape) frame
/// is never touched.
fn prove_core(
    ctx: &StoreProverContext<'_>,
    instance: &InstanceProver<'_>,
    schema: &SchemaProver<'_>,
    db_root: ProofHash,
    id: &str,
) -> Result<ProvenCore, QueryProofError> {
    // (D) COMPLETE direct triples of this node (instance graph) — a materialized bound-subject BGP OR a
    // reconciled single-atom present filter, chosen by the entry point. Source-agnostic downstream.
    let (subject, d_rows) = instance.read_subject(ctx, id)?;

    // (F0) Bind the class. A typeless target (dangling link) is a non-subdocument stop.
    let Some(class) = extract_class_opt(&d_rows)? else {
        return Ok(ProvenCore {
            subject,
            d_rows,
            class: None,
            class_schema: None,
            class_rows: Vec::new(),
            ancestor_closure: None,
            ancestor_set: Vec::new(),
            ancestor_reads: Vec::new(),
            ancestor_cache: BTreeMap::new(),
            natures: ClosureNatures::default(),
        });
    };
    let class_iri =
        std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;

    // (F1) C's OWN schema frame — a source-agnostic bound-subject read (materialized BGP or reconciled
    // present filter over the delta schema head, 6e).
    let (class_schema, class_rows) = schema.read_node(ctx, class_iri)?;

    // (F1b) Reflexive `sys:inherits` closure — proven only when `C` actually inherits (otherwise the
    // reflexive closure is exactly `{C}`, synthesized locally, at no proof cost). Source-agnostic: a
    // materialized subsumption BGP anchored into `database_root`, OR the store's reconciled
    // `TransitiveClosure`-over-`present` on the delta schema chain (6e).
    let (ancestor_closure, ancestor_set) = if summarize(&class_rows).has_inherits {
        let (closure, set) = schema.prove_ancestor_closure(ctx, class_iri, instance, db_root)?;
        (Some(closure), set)
    } else {
        (None, vec![class.clone()])
    };

    // Read each STRICT ancestor's frame (recorded into `reader.ranges`) and compute the
    // inheritance-closed subdocument / unfoldable natures.
    let mut reader = ProveReader {
        ctx,
        schema,
        cache: BTreeMap::new(),
        ranges: Vec::new(),
    };
    let natures = closure_natures(&class, &class_rows, &ancestor_set, &mut reader)?;

    Ok(ProvenCore {
        subject,
        d_rows,
        class: Some(class),
        class_schema: Some(class_schema),
        class_rows,
        ancestor_closure,
        ancestor_set,
        ancestor_reads: reader.ranges,
        ancestor_cache: reader.cache,
        natures,
    })
}

/// Prove the per-element ACTUAL-TYPE witnesses for a PRESENT `Set`/`List` collection property, and
/// reject-close if the reference would UNFOLD the collection (increment-3 audit fix). v1 renders
/// collection elements as bare ids, so this refuses any collection `get_document(unfold=true)` would
/// inline, in two ways:
///   1. an `@unfold` collection property (`unfold` true) — the reference unfolds EVERY element — rejects
///      immediately (no per-element witness needed);
///   2. any NODE-valued element whose ACTUAL type (its reflexive `sys:inherits` closure) bears
///      `sys:subdocument` / `sys:unfoldable` — the reference would unfold that element — rejects. The
///      element's ACTUAL type is read from the root-authenticated instance graph (via [`prove_core`], the
///      SAME decision core the single-`Link` path uses), so a SUBCLASS that introduces subdocument-ness
///      is caught even when the DECLARED element class is a plain non-subdocument class.
/// LITERAL/scalar elements cannot unfold and carry no witness. Returns one witness per DISTINCT node
/// element (empty for an all-literal collection); prove and verify request the identical witness set.
#[allow(clippy::too_many_arguments)]
fn prove_collection_elements(
    ctx: &StoreProverContext<'_>,
    instance: &InstanceProver<'_>,
    schema: &SchemaProver<'_>,
    db_root: ProofHash,
    predicate: &[u8],
    elements: &[CanonicalObject],
    unfold: bool,
) -> Result<ProvedCollectionProperty, QueryProofError> {
    // (1) An `@unfold` collection property: the reference unfolds every element inline. Reject before
    //     touching the elements (fail-closed) — v1 cannot reproduce the unfolded body.
    if unfold {
        return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
    }
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut witnesses = Vec::new();
    for element in elements {
        // (2) Only a NODE element can unfold; a literal/scalar element needs no type check.
        let child_id = match element {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => continue,
        };
        // One witness per DISTINCT element id (an element's type depends only on its IRI).
        if !seen.insert(child_id.clone()) {
            continue;
        }
        let child_iri = std::str::from_utf8(&child_id)
            .map_err(|_| invalid("collection element IRI is not valid UTF-8"))?
            .to_string();
        // Read the element's ACTUAL type + closure (never the schema's DECLARED element class). A
        // typeless/dangling element yields clean natures (the reference renders it as a bare id too).
        // On the delta path this is itself a reconciled bound-subject read over `present`.
        let child_core = prove_core(ctx, instance, schema, db_root, &child_iri)?;
        if child_core.natures.has_subdocument || child_core.natures.has_unfoldable {
            return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
        }
        witnesses.push(ProvedCollectionElement {
            child_id,
            witness: ProvedChildTypeWitness {
                subject_bgp: child_core.subject,
                class_schema: child_core.class_schema,
                ancestor_closure: child_core.ancestor_closure,
                ancestor_reads: child_core.ancestor_reads,
            },
        });
    }
    Ok(ProvedCollectionProperty {
        predicate: predicate.to_vec(),
        elements: witnesses,
    })
}

/// Prove a RENDERED node (the root or an inlined subdocument) by EXTENDING an already-proven
/// [`ProvenCore`]: classify the effective frame, prove one ordered-list walk per present list, and
/// recurse into each present single-valued class LINK (inlining a subdocument child, stopping at a
/// plain link). `allow_subdocument` is `false` for the root (its own class must not be a subdocument)
/// and `true` for an inlined child (it is EXPECTED to be one). The `visited` set + `node_count` +
/// `depth` bound the recursion IDENTICALLY to verify.
#[allow(clippy::too_many_arguments)]
fn prove_body(
    ctx: &StoreProverContext<'_>,
    instance: &InstanceProver<'_>,
    schema: &SchemaProver<'_>,
    db_root: ProofHash,
    id: &str,
    core: ProvenCore,
    allow_subdocument: bool,
    depth: usize,
    visited: &mut BTreeSet<Vec<u8>>,
    node_count: &mut usize,
) -> Result<ProvedReadDocument, QueryProofError> {
    let ProvenCore {
        subject,
        d_rows,
        class,
        class_schema,
        class_rows,
        ancestor_closure,
        ancestor_set,
        ancestor_reads,
        ancestor_cache,
        natures: _,
    } = core;
    let class = class.ok_or_else(|| invalid("rendered document node has no rdf:type"))?;
    let class_schema =
        class_schema.ok_or_else(|| invalid("rendered document node is missing its class schema"))?;

    // (F2-F3) Classify the EFFECTIVE frame. The reader is SEEDED with the core's ancestor rows so
    // classify_frame does NOT re-prove them; only NEW range/element reads accumulate here.
    let mut reader = ProveReader {
        ctx,
        schema,
        cache: ancestor_cache,
        ranges: Vec::new(),
    };
    let properties =
        classify_frame(&class, &class_rows, &ancestor_set, &mut reader, allow_subdocument)?;
    // frame.ranges = ancestor reads (core) ++ range/element reads (body).
    let mut ranges = ancestor_reads;
    ranges.extend(reader.ranges);

    // Group present objects by predicate (rdf:type excluded — it binds the class).
    let mut by_predicate: BTreeMap<Vec<u8>, Vec<CanonicalObject>> = BTreeMap::new();
    for (predicate, object) in &d_rows {
        if predicate.as_slice() == RDF_TYPE_IRI.as_bytes() {
            continue;
        }
        by_predicate
            .entry(predicate.clone())
            .or_default()
            .push(object.clone());
    }

    let mut lists = Vec::new();
    let mut arrays = Vec::new();
    let mut links = Vec::new();
    let mut collections = Vec::new();
    for prop in &properties {
        match prop.shape {
            RangeShape::List => {
                match by_predicate.get(&prop.predicate).map(|o| o.len()).unwrap_or(0) {
                    0 => continue,
                    1 => {}
                    _ => {
                        return Err(BgpPlanError::ReadDocumentMalformedListHead {
                            predicate: iri_string(&prop.predicate),
                        }
                        .into())
                    }
                }
                // The list's ORDERED elements come from the authenticated walk (NOT the raw cons head);
                // re-resolve them to run the per-element ACTUAL-TYPE / `@unfold` reject over the real
                // element sequence, identically to verify. The walk is routed by the ENTRY POINT: a
                // materialized layer proves the `OrderedListWalk` composite; a delta head proves the
                // reconciled ordered-list walk over `present` (6c-iii) — NO new crypto, the reconciled
                // walk reuses the 6c-i foundation + the audited gluing gadgets.
                let (walk, elements) = match instance {
                    InstanceProver::Materialized { layer, root } => {
                        let walk_compiled = ordered_list_walk_bgp(id.as_bytes(), &prop.predicate);
                        let walk = walk_compiled.prove_with_context(ctx, layer)?;
                        let columns = walk.verify_and_resolve(&walk_compiled, *layer, *root)?;
                        let elements = columns.into_iter().next().unwrap_or_default();
                        (ListWalkProof::Materialized(walk), elements)
                    }
                    InstanceProver::Reconciled { layers, .. } => {
                        let (walk, elements) = prove_ordered_list_walk_reconciled(
                            layers,
                            &CanonicalObject::Node(id.as_bytes().to_vec()),
                            &prop.predicate,
                        )?;
                        (ListWalkProof::Reconciled(Box::new(walk)), elements)
                    }
                };
                collections.push(prove_collection_elements(
                    ctx,
                    instance,
                    schema,
                    db_root,
                    &prop.predicate,
                    &elements,
                    prop.unfold,
                )?);
                lists.push(ProvedListProperty {
                    predicate: prop.predicate.clone(),
                    walk,
                });
            }
            RangeShape::Array => {
                // An array property has `n` cell edges `t(id, P, cell)` (NOT a single head like a list),
                // so DO NOT apply the list-specific `!= 1` head reject: absent (0 cells) → skip, any
                // present count → prove the ordered walk. The store composite proves cell-set
                // completeness itself, so we do not re-count here.
                let present = by_predicate
                    .get(&prop.predicate)
                    .map(|objects| !objects.is_empty())
                    .unwrap_or(false);
                if !present {
                    continue;
                }
                // The array's ORDERED elements come from the authenticated walk (the `sys:index`-ordered
                // `sys:value` column, NOT the raw cell ids); re-resolve them to run the per-element
                // ACTUAL-TYPE / `@unfold` reject over the real element sequence, identically to verify.
                // The walk is routed by the ENTRY POINT: a materialized layer proves the
                // `OrderedArrayWalk` composite; a delta head proves the reconciled ordered-array walk
                // over `present` (6d) — NO new crypto, the reconciled walk reuses the 6c-i foundation +
                // the audited array gluing gadgets + the combined-range-dict index authentication. The
                // per-element unfold reject (an array of actually-unfoldable/subdocument elements must
                // reject, EXACTLY as `Set`/`List` do) runs UNCHANGED over the resolved elements.
                let (walk, elements) = match instance {
                    InstanceProver::Materialized { layer, root } => {
                        let walk_compiled = ordered_array_walk_bgp(id.as_bytes(), &prop.predicate);
                        let walk = walk_compiled.prove_with_context(ctx, layer)?;
                        let columns = walk.verify_and_resolve(&walk_compiled, *layer, *root)?;
                        let elements = columns.into_iter().next().unwrap_or_default();
                        (ArrayWalkProof::Materialized(walk), elements)
                    }
                    InstanceProver::Reconciled { layers, .. } => {
                        let (walk, elements) = prove_ordered_array_walk_reconciled(
                            layers,
                            &CanonicalObject::Node(id.as_bytes().to_vec()),
                            &prop.predicate,
                        )?;
                        (ArrayWalkProof::Reconciled(Box::new(walk)), elements)
                    }
                };
                collections.push(prove_collection_elements(
                    ctx,
                    instance,
                    schema,
                    db_root,
                    &prop.predicate,
                    &elements,
                    prop.unfold,
                )?);
                arrays.push(ProvedArrayProperty {
                    predicate: prop.predicate.clone(),
                    walk,
                });
            }
            RangeShape::Link => {
                let objects = match by_predicate.get(&prop.predicate) {
                    Some(objects) => objects,
                    None => continue,
                };
                for object in objects {
                    let child_id = match object {
                        CanonicalObject::Node(bytes) => bytes.clone(),
                        _ => return Err(invalid("class-link object is not a node")),
                    };
                    let child_iri = std::str::from_utf8(&child_id)
                        .map_err(|_| invalid("link child IRI is not valid UTF-8"))?
                        .to_string();
                    // Prove the child's decision core → the inline/stop decision is keyed off the
                    // CHILD's authenticated type + closure, NEVER the schema range. On the delta path
                    // this reads the child's `rdf:type` over `present` (a reconciled bound-subject read),
                    // so the child's subdocument nature is re-derived from ROOT-AUTHENTICATED data.
                    let child_core = prove_core(ctx, instance, schema, db_root, &child_iri)?;
                    if child_core.natures.has_subdocument {
                        // INCREMENT 6b: a subdocument child is INLINED — the inc-3 recursion, now threaded
                        // over the reconciled instance source. `prove_body` recurses with the SAME
                        // `instance` (a materialized layer OR a delta stack), so a nested subdocument
                        // node's own `D` / child type-checks / collection-element reads route over
                        // `present` exactly like the root's — no new proof logic, just the reconciled
                        // reads from 6a lifted into the recursion. If a nested node itself bears a
                        // `List`/`Array` property, the recursive `prove_body` reject-closes with
                        // `ReadDocumentDeltaWalkDeferred` (the ordered-walk composites are not reconciled
                        // until 6c/6d), so 6b supports FLAT subdocuments nested to the budget — the
                        // top-level 6a restriction applied recursively.
                        // INLINE (single-valued only).
                        if objects.len() != 1 {
                            return Err(shape_reject("multi-valued-subdocument"));
                        }
                        if visited.contains(&child_id) {
                            return Err(BgpPlanError::ReadDocumentSubdocumentCycle.into());
                        }
                        if depth + 1 > MAX_SUBDOC_DEPTH {
                            return Err(BgpPlanError::ReadDocumentSubdocumentBudgetExceeded.into());
                        }
                        *node_count += 1;
                        if *node_count > MAX_SUBDOC_NODES {
                            return Err(BgpPlanError::ReadDocumentSubdocumentBudgetExceeded.into());
                        }
                        visited.insert(child_id.clone());
                        let child_node = prove_body(
                            ctx,
                            instance,
                            schema,
                            db_root,
                            &child_iri,
                            child_core,
                            /* allow_subdocument */ true,
                            depth + 1,
                            visited,
                            node_count,
                        )?;
                        links.push(ProvedLink {
                            predicate: prop.predicate.clone(),
                            child_id,
                            target: ProvedLinkTarget::Inline(Box::new(child_node)),
                        });
                    } else {
                        // STOP (plain link). The reference UNFOLDS an `@unfoldable` class or an
                        // `@unfold` property inline; increment 3 inlines only true subdocuments, so
                        // rendering such a target as a bare link would OMIT content — reject closed. A
                        // plain non-subdocument link IS supported on the delta path (render its id).
                        if child_core.natures.has_unfoldable || prop.unfold {
                            return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
                        }
                        links.push(ProvedLink {
                            predicate: prop.predicate.clone(),
                            child_id,
                            target: ProvedLinkTarget::Stop(ProvedChildTypeWitness {
                                subject_bgp: child_core.subject,
                                class_schema: child_core.class_schema,
                                ancestor_closure: child_core.ancestor_closure,
                                ancestor_reads: child_core.ancestor_reads,
                            }),
                        });
                    }
                }
            }
            RangeShape::Set => {
                // A present `Set`: its elements are the direct bound-subject objects. Run the
                // per-element ACTUAL-TYPE / `@unfold` reject; still render the set as bare ids (v1 does
                // not inline collection elements — this only refuses the collections it would omit).
                if let Some(objects) = by_predicate.get(&prop.predicate) {
                    collections.push(prove_collection_elements(
                        ctx,
                        instance,
                        schema,
                        db_root,
                        &prop.predicate,
                        objects,
                        prop.unfold,
                    )?);
                }
            }
            RangeShape::Scalar => {}
        }
    }

    Ok(ProvedReadDocument {
        subject_bgp: subject,
        frame: ProvedFrame {
            class,
            class_schema,
            ancestor_closure,
            ranges,
        },
        lists,
        arrays,
        links,
        collections,
    })
}

impl CompiledReadDocument {
    /// Prove the document over the two committed graphs (instance + schema). Reads the layers to
    /// discover `C` / the properties / the list heads / the inlined subdocument tree, proves each
    /// sub-proof by direct construction, and assembles the bundle. Unsupported shapes reject-close here
    /// (fail-closed prover).
    pub fn prove_on_layers(
        &self,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedReadDocument, QueryProofError> {
        self.prove_on_layers_with_context(&StoreProverContext::cpu(), instance, schema)
    }

    /// [`prove_on_layers`] with a caller-owned prover context.
    pub fn prove_on_layers_with_context(
        &self,
        ctx: &StoreProverContext<'_>,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedReadDocument, QueryProofError> {
        // β re-anchor: the per-graph trust anchor is `resolution_root` (binds `commitment_root` AND
        // both dictionary Merkle roots); `database_root` composes the two, matching the store's
        // `validate_database_root`, and every materialized read re-verifies against `resolution_root`.
        let instance_root = instance.resolution_root();
        let schema_root = schema.resolution_root();
        let db_root = database_root(&instance_root, &schema_root);
        // MATERIALIZED route (BOTH graphs): the entry FIXES the sources (never prover data), so every
        // instance bound-subject read is a `InstanceSubjectProof::Materialized` BGP and every schema read
        // is a `SchemaSubjectProof::Materialized` BGP.
        let instance = InstanceProver::Materialized { layer: instance, root: instance_root };
        let schema = SchemaProver::Materialized { layer: schema, root: schema_root };

        let core = prove_core(ctx, &instance, &schema, db_root, &self.id)?;
        // The ROOT of a `ReadDocument` must be a real typed document.
        if core.class.is_none() {
            return Err(invalid("document has no rdf:type row"));
        }
        // `visited` seeds with the root id; `node_count` counts the root. The ROOT passes
        // `allow_subdocument = false`, so a subdocument top-level class reject-closes in
        // `collect_class_frame`.
        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(self.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        prove_body(
            ctx,
            &instance,
            &schema,
            db_root,
            &self.id,
            core,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )
    }

    /// Prove a RESTRICTED `ReadDocument` over a delta INSTANCE stack (`instance_layers`, base..head,
    /// head last) + a MATERIALIZED schema (increment 6a — the delta-stack path). Every instance
    /// bound-subject read (the node's own `D`, each child type-check read, each collection-element read)
    /// routes to the store's single-atom present FILTER over `present` (`FilterPosition::Subject`); every
    /// SCHEMA read (frame BGPs + the `sys:inherits` closure) stays on the materialized schema. NO new
    /// store crypto — the reconcile filter and the schema machinery are reused verbatim; this entry only
    /// FIXES the instance route (never prover data). A FLAT subdocument child now INLINES over the delta
    /// stack (increment 6b): the inc-3 recursion runs with the reconciled instance source threaded
    /// through, so a nested node's instance reads route over `present` while its schema frame + closure
    /// stay materialized. Reject-closes (delta path): `List`/`Array` — at the TOP level OR inside a nested
    /// subdocument — because the ordered walks are not reconciled yet (6c/6d); and a delta-head schema
    /// (6e) — PLUS every base reject.
    pub fn prove_on_layers_reconciled(
        &self,
        instance_layers: &[&LayerCommitment],
        schema: &LayerCommitment,
    ) -> Result<ProvedReadDocument, QueryProofError> {
        // The SCHEMA graph must be MATERIALIZED (6e reconciles the schema stack). A schema commitment
        // that is itself a delta head reject-closes.
        if schema.state.parent_state_root.is_some() {
            return Err(BgpPlanError::ReadDocumentDeltaSchemaHead.into());
        }
        let head = instance_layers
            .last()
            .ok_or_else(|| invalid("delta instance stack is empty"))?;
        // β re-anchor: `resolution_root` per graph (delta-head instance, materialized schema); the
        // store's reconcile verifier now gates on the head `resolution_root`.
        let instance_head_root = head.resolution_root();
        let schema_root = schema.resolution_root();
        let db_root = database_root(&instance_head_root, &schema_root);
        let ctx = StoreProverContext::cpu();
        // RECONCILED INSTANCE + MATERIALIZED SCHEMA route: the entry FIXES the sources, so every instance
        // read is a `InstanceSubjectProof::Reconciled` single-atom present filter over the delta stack,
        // and every schema read stays a `SchemaSubjectProof::Materialized` BGP (6a–6d unaffected).
        let instance =
            InstanceProver::Reconciled { layers: instance_layers, head_root: instance_head_root };
        let schema = SchemaProver::Materialized { layer: schema, root: schema_root };

        let core = prove_core(&ctx, &instance, &schema, db_root, &self.id)?;
        // The ROOT of a `ReadDocument` must be a real typed document.
        if core.class.is_none() {
            return Err(invalid("document has no rdf:type row"));
        }
        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(self.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        prove_body(
            &ctx,
            &instance,
            &schema,
            db_root,
            &self.id,
            core,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )
    }

    /// Prove a `ReadDocument` over a delta INSTANCE stack (`instance_layers`) AND a delta SCHEMA stack
    /// (`schema_layers`) — BOTH base..head, head last (increment 6e — the FINAL delta-stack increment,
    /// closing the entire delta-path parity). Every instance bound-subject read routes over the reconciled
    /// instance `present`; every SCHEMA read (the class frame BGP, each range/element classification BGP,
    /// each strict-ancestor frame BGP) routes over the reconciled schema `present`; and the reflexive
    /// `sys:inherits` closure routes to the store's `TransitiveClosure`-over-`present` (FOUNDATION-B, 6c-i)
    /// on the SCHEMA chain. NO new store crypto — a reconciled schema read is a bound-subject / closure
    /// read over a schema-chain reconciliation, exactly what the instance side and FOUNDATION-A/B already
    /// do. The delta-head schema reject ([`prove_on_layers_reconciled`]'s guard) is LIFTED here; the route
    /// (materialized vs reconciled) per graph is a PURE FUNCTION of this entry, never prover data. The
    /// per-graph reconcile ≥2-records-per-group floor applies to BOTH chains. Every base reject +
    /// every 6a–6d shape (flat / inheritance / subdocument / list / array) is supported.
    pub fn prove_on_layers_fully_reconciled(
        &self,
        instance_layers: &[&LayerCommitment],
        schema_layers: &[&LayerCommitment],
    ) -> Result<ProvedReadDocument, QueryProofError> {
        let instance_head = instance_layers
            .last()
            .ok_or_else(|| invalid("delta instance stack is empty"))?;
        let schema_head = schema_layers
            .last()
            .ok_or_else(|| invalid("delta schema stack is empty"))?;
        // β re-anchor: both per-graph anchors are the delta-HEAD `resolution_root`s; the combined
        // `database_root` pins both, matching the store's `validate_database_root` / reconcile verifier.
        let instance_head_root = instance_head.resolution_root();
        let schema_head_root = schema_head.resolution_root();
        // BOTH per-graph roots are the delta HEADS now — the combined root pins both trusted head anchors.
        let db_root = database_root(&instance_head_root, &schema_head_root);
        let ctx = StoreProverContext::cpu();
        // FULLY-RECONCILED route: the entry FIXES BOTH sources to their delta stacks.
        let instance =
            InstanceProver::Reconciled { layers: instance_layers, head_root: instance_head_root };
        let schema =
            SchemaProver::Reconciled { layers: schema_layers, head_root: schema_head_root };

        let core = prove_core(&ctx, &instance, &schema, db_root, &self.id)?;
        // The ROOT of a `ReadDocument` must be a real typed document.
        if core.class.is_none() {
            return Err(invalid("document has no rdf:type row"));
        }
        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(self.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        prove_body(
            &ctx,
            &instance,
            &schema,
            db_root,
            &self.id,
            core,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )
    }
}

// ---------------------------------------------------------------------------------------------------
// Verify + resolve.
// ---------------------------------------------------------------------------------------------------

/// Verify a STOP link's [`ProvedChildTypeWitness`] and return the child's authenticated class (or
/// `None` if typeless) plus its inheritance-closed subdocument / unfoldable natures. This reads ONLY
/// the child's type + closure + ancestor natures — never the child's property frame — and root-verifies
/// every carried sub-proof, rejecting an injected one. The CALLER uses the returned natures to force
/// the render mode (a stop witness whose child is actually a subdocument reject-closes as an omission).
fn verify_child_witness(
    witness: &ProvedChildTypeWitness,
    child_id: &[u8],
    instance: &InstanceSource<'_>,
    schema: &SchemaSource<'_>,
    database_root: ProofHash,
) -> Result<(Option<Vec<u8>>, ClosureNatures), QueryProofError> {
    let schema_root = schema.schema_root();
    let child_iri =
        std::str::from_utf8(child_id).map_err(|_| invalid("link child IRI is not valid UTF-8"))?;
    // The child's type — a source-agnostic bound-subject read (materialized BGP or reconciled present
    // filter), reconstructed from `child_id` and re-verified against the trusted instance root.
    let d_rows = witness.subject_bgp.resolve(child_iri, instance, instance.instance_root())?;
    let Some(class) = extract_class_opt(&d_rows)? else {
        // Typeless / dangling link target → non-subdocument stop. It must carry NO schema sub-proofs.
        if witness.class_schema.is_some()
            || witness.ancestor_closure.is_some()
            || !witness.ancestor_reads.is_empty()
        {
            return Err(invalid("typeless link witness carries spurious schema sub-proofs"));
        }
        return Ok((None, ClosureNatures::default()));
    };
    let class_iri =
        std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;
    // C_child's OWN schema frame — a source-agnostic schema read (materialized BGP or reconciled
    // present filter over the delta schema head).
    let class_schema = witness
        .class_schema
        .as_ref()
        .ok_or_else(|| invalid("typed link witness is missing its class schema"))?;
    let class_rows = class_schema.resolve(class_iri, schema, schema_root)?;

    // Re-derive the ancestor set (RE-BUILD + RE-VERIFY the closure when the child's class inherits).
    let ancestor_set = if summarize(&class_rows).has_inherits {
        let closure = witness.ancestor_closure.as_ref().ok_or_else(|| {
            invalid("inheriting link witness is missing its ancestor closure")
        })?;
        resolve_ancestor_closure(closure, class_iri, instance, schema, database_root)?
    } else {
        if witness.ancestor_closure.is_some() {
            return Err(invalid("non-inheriting link witness carries an ancestor closure"));
        }
        vec![class.clone()]
    };

    // Root-verify each strict-ancestor nature read into a trusted map.
    let mut map: BTreeMap<Vec<u8>, SubjectRows> = BTreeMap::new();
    for classification in &witness.ancestor_reads {
        let node_iri = std::str::from_utf8(&classification.node)
            .map_err(|_| invalid("ancestor schema node IRI is not valid UTF-8"))?;
        let rows = classification.schema_read.resolve(node_iri, schema, schema_root)?;
        if map.insert(classification.node.clone(), rows).is_some() {
            return Err(invalid("link witness carries a duplicate ancestor read"));
        }
    }
    // Compute the natures + DEMAND every strict ancestor read; then FORBID any injected one.
    let mut reader = VerifyReader {
        map: &map,
        used: BTreeSet::new(),
    };
    let natures = closure_natures(&class, &class_rows, &ancestor_set, &mut reader)?;
    let used = reader.used;
    for node_key in map.keys() {
        if !used.contains(node_key) {
            return Err(invalid(format!(
                "link witness carries an unused (injected) ancestor read for {}",
                iri_string(node_key)
            )));
        }
    }
    Ok((Some(class), natures))
}

/// Verify a PRESENT `Set`/`List` property's per-element ACTUAL-TYPE witnesses and reject-close if the
/// reference would UNFOLD the collection (increment-3 audit fix; the verify twin of
/// [`prove_collection_elements`], run IDENTICALLY so a prover cannot skip it). `elements` are the
/// authenticated element objects (a `Set`'s direct objects, or a `List`'s ordered WALK elements). The
/// bundle must carry EXACTLY the witnesses for the DISTINCT node-valued elements — no missing, no
/// injected — and each element's ACTUAL type (reflexive `sys:inherits` closure, re-verified against the
/// trusted roots) must NOT bear `sys:subdocument` / `sys:unfoldable`. An `@unfold` collection property
/// rejects outright. Marks `predicate` used in `used_collections` (the caller separately rejects any
/// UNUSED — i.e. injected — collection entry).
#[allow(clippy::too_many_arguments)]
fn verify_collection_elements(
    predicate: &[u8],
    elements: &[CanonicalObject],
    unfold: bool,
    collection_map: &BTreeMap<Vec<u8>, &ProvedCollectionProperty>,
    used_collections: &mut BTreeSet<Vec<u8>>,
    instance: &InstanceSource<'_>,
    schema: &SchemaSource<'_>,
    database_root: ProofHash,
) -> Result<(), QueryProofError> {
    // (1) An `@unfold` collection property: the reference unfolds every element inline → reject.
    if unfold {
        return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
    }
    let collection = *collection_map.get(predicate).ok_or_else(|| {
        invalid(format!(
            "bundle is missing the collection element witnesses for predicate {}",
            iri_string(predicate)
        ))
    })?;
    used_collections.insert(predicate.to_vec());
    // Index the carried witnesses by element id (reject a duplicate).
    let mut witness_map: BTreeMap<Vec<u8>, &ProvedChildTypeWitness> = BTreeMap::new();
    for element in &collection.elements {
        if witness_map
            .insert(element.child_id.clone(), &element.witness)
            .is_some()
        {
            return Err(invalid(format!(
                "collection property {} carries a duplicate element witness for {}",
                iri_string(predicate),
                iri_string(&element.child_id)
            )));
        }
    }
    // (2) Every DISTINCT node-valued element MUST carry a witness whose ACTUAL type is non-inlinable.
    let mut used: BTreeSet<Vec<u8>> = BTreeSet::new();
    for element in elements {
        let child_id = match element {
            CanonicalObject::Node(bytes) => bytes.clone(),
            _ => continue, // literal/scalar element cannot unfold
        };
        if !used.insert(child_id.clone()) {
            continue; // already checked this distinct element
        }
        let witness = *witness_map.get(&child_id).ok_or_else(|| {
            invalid(format!(
                "bundle is missing the collection element witness for predicate {} element {}",
                iri_string(predicate),
                iri_string(&child_id)
            ))
        })?;
        let (_child_class, child_natures) =
            verify_child_witness(witness, &child_id, instance, schema, database_root)?;
        // A subdocument / unfoldable ACTUAL element type would be unfolded by the reference; v1 renders
        // bare ids, so reject rather than omit the unfolded body.
        if child_natures.has_subdocument || child_natures.has_unfoldable {
            return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
        }
    }
    // No injected element witness (for an element not present in the collection, or a literal element).
    for child_id in witness_map.keys() {
        if !used.contains(child_id) {
            return Err(invalid(format!(
                "collection property {} carries an unused (injected) element witness for {}",
                iri_string(predicate),
                iri_string(child_id)
            )));
        }
    }
    Ok(())
}

/// Verify + render ONE document node (the root or an inlined subdocument), recursively. Returns the
/// rendered document PLUS the node's OWN inheritance-closed natures (which the parent uses to force the
/// inline/stop render mode). Every sub-plan is reconstructed from the node `id` + the authenticated
/// frame and verified against the trusted roots; the completeness / no-injection cross-checks (ranges,
/// list walks, AND the recursive per-link demand/reject) run here. `allow_subdocument` is `false` for
/// the root (its own class must not be a subdocument) and `true` for an inlined child.
#[allow(clippy::too_many_arguments)]
fn verify_node(
    node: &ProvedReadDocument,
    id: &str,
    instance: &InstanceSource<'_>,
    schema: &SchemaSource<'_>,
    database_root: ProofHash,
    allow_subdocument: bool,
    depth: usize,
    visited: &mut BTreeSet<Vec<u8>>,
    node_count: &mut usize,
) -> Result<(ResolvedDocument, ClosureNatures), QueryProofError> {
    let instance_root = instance.instance_root();
    let schema_root = schema.schema_root();
    // (2) COMPLETE direct triples — a source-agnostic bound-subject read (materialized BGP or reconciled
    // present filter), reconstructed from `id` and re-verified against the trusted instance root.
    let d_rows = node.subject_bgp.resolve(id, instance, instance_root)?;

    // (F0) Derive C from the authenticated rdf:type row (the instance-C == schema-C tie — now over TWO
    // root-authenticated RECONCILED values when both graphs are delta heads: the instance `C` under the
    // trusted instance-head root and the schema `C` under the trusted schema-head root, both pinned to
    // the combined `database_root`, a single subject → the IRI equality stays sound with no in-circuit
    // cross-root join).
    let class = extract_class(&d_rows)?;
    let class_iri =
        std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;

    // (F1) C's OWN schema frame — a source-agnostic schema read (materialized BGP or reconciled present
    // filter over the delta schema head).
    let class_rows = node.frame.class_schema.resolve(class_iri, schema, schema_root)?;

    // (F1b) Re-derive the ancestor set (RE-BUILD + RE-VERIFY the closure when C inherits). A prover can
    // neither drop a genuine closure nor inject a spurious one, nor shorten the set (empty-delta
    // fixpoint) — source-agnostic (materialized subsumption BGP or reconciled `TransitiveClosure` over
    // the delta schema head's `present`).
    let ancestor_set = if summarize(&class_rows).has_inherits {
        let closure = node.frame.ancestor_closure.as_ref().ok_or_else(|| {
            invalid("class bears a sys:inherits edge but the bundle omits the ancestor closure")
        })?;
        resolve_ancestor_closure(closure, class_iri, instance, schema, database_root)?
    } else {
        if node.frame.ancestor_closure.is_some() {
            return Err(invalid(
                "class bears no sys:inherits edge but the bundle carries an ancestor closure",
            ));
        }
        vec![class.clone()]
    };

    // (5) Root-verify every schema read (strict ancestors + property ranges/elements) into a trusted
    //     node -> rows map — each a source-agnostic schema read.
    let mut range_map: BTreeMap<Vec<u8>, SubjectRows> = BTreeMap::new();
    for classification in &node.frame.ranges {
        let node_iri = std::str::from_utf8(&classification.node)
            .map_err(|_| invalid("range schema node IRI is not valid UTF-8"))?;
        let rows = classification.schema_read.resolve(node_iri, schema, schema_root)?;
        if range_map.insert(classification.node.clone(), rows).is_some() {
            return Err(invalid("bundle carries a duplicate range classification"));
        }
    }

    // This node's OWN inheritance-closed natures (for the CALLER's inline/stop assertion).
    let natures = {
        let mut nat_reader = VerifyReader {
            map: &range_map,
            used: BTreeSet::new(),
        };
        closure_natures(&class, &class_rows, &ancestor_set, &mut nat_reader)?
    };

    // (6/F2-F3) RE-DERIVE the classification; FORBID any injected/unused schema read.
    let mut reader = VerifyReader {
        map: &range_map,
        used: BTreeSet::new(),
    };
    let properties =
        classify_frame(&class, &class_rows, &ancestor_set, &mut reader, allow_subdocument)?;
    let used_nodes = reader.used;
    for node_key in range_map.keys() {
        if !used_nodes.contains(node_key) {
            return Err(invalid(format!(
                "bundle carries an unused (injected) range classification for {}",
                iri_string(node_key)
            )));
        }
    }

    let shape_of: BTreeMap<Vec<u8>, RangeShape> =
        properties.iter().map(|p| (p.predicate.clone(), p.shape)).collect();
    let unfold_of: BTreeMap<Vec<u8>, bool> =
        properties.iter().map(|p| (p.predicate.clone(), p.unfold)).collect();

    // Group present objects by predicate (rdf:type excluded — it binds C).
    let mut by_predicate: BTreeMap<Vec<u8>, Vec<CanonicalObject>> = BTreeMap::new();
    for (predicate, object) in &d_rows {
        if predicate.as_slice() == RDF_TYPE_IRI.as_bytes() {
            continue;
        }
        by_predicate
            .entry(predicate.clone())
            .or_default()
            .push(object.clone());
    }

    // (No-injection) Every instance predicate must be a frame property.
    for predicate in by_predicate.keys() {
        if !shape_of.contains_key(predicate) {
            return Err(BgpPlanError::ReadDocumentUnknownInstanceProperty {
                predicate: iri_string(predicate),
            }
            .into());
        }
    }

    // Index the list-walk sub-proofs by predicate (reject a duplicate).
    let mut walk_map: BTreeMap<Vec<u8>, &ProvedListProperty> = BTreeMap::new();
    for list_property in &node.lists {
        if walk_map
            .insert(list_property.predicate.clone(), list_property)
            .is_some()
        {
            return Err(invalid("bundle carries a duplicate list-walk sub-proof"));
        }
    }
    let mut used_walks: BTreeSet<Vec<u8>> = BTreeSet::new();

    // Index the array-walk sub-proofs by predicate (reject a duplicate). One entry per PRESENT array
    // property (increment 4) — the exact analogue of `walk_map` for lists.
    let mut array_map: BTreeMap<Vec<u8>, &ProvedArrayProperty> = BTreeMap::new();
    for array_property in &node.arrays {
        if array_map
            .insert(array_property.predicate.clone(), array_property)
            .is_some()
        {
            return Err(invalid("bundle carries a duplicate array-walk sub-proof"));
        }
    }
    let mut used_arrays: BTreeSet<Vec<u8>> = BTreeSet::new();

    // Index the link sub-proofs by (predicate, child_id) (reject a duplicate).
    let mut link_map: BTreeMap<(Vec<u8>, Vec<u8>), &ProvedLink> = BTreeMap::new();
    for link in &node.links {
        if link_map
            .insert((link.predicate.clone(), link.child_id.clone()), link)
            .is_some()
        {
            return Err(invalid("bundle carries a duplicate link sub-proof"));
        }
    }
    let mut used_links: BTreeSet<(Vec<u8>, Vec<u8>)> = BTreeSet::new();

    // Index the collection element-witness sub-proofs by predicate (reject a duplicate). One entry per
    // PRESENT `Set`/`List` property (increment-3 audit fix).
    let mut collection_map: BTreeMap<Vec<u8>, &ProvedCollectionProperty> = BTreeMap::new();
    for collection in &node.collections {
        if collection_map
            .insert(collection.predicate.clone(), collection)
            .is_some()
        {
            return Err(invalid("bundle carries a duplicate collection property sub-proof"));
        }
    }
    let mut used_collections: BTreeSet<Vec<u8>> = BTreeSet::new();

    // Render each present property.
    let mut rendered: Vec<(Vec<u8>, ResolvedValue)> = Vec::new();
    for (predicate, objects) in &by_predicate {
        let shape = *shape_of
            .get(predicate)
            .expect("every instance predicate is frame-classified (checked above)");
        match shape {
            RangeShape::List => {
                if objects.len() != 1 {
                    return Err(BgpPlanError::ReadDocumentMalformedListHead {
                        predicate: iri_string(predicate),
                    }
                    .into());
                }
                let list_property = walk_map.get(predicate).copied().ok_or_else(|| {
                    BgpPlanError::ReadDocumentMalformedListHead {
                        predicate: iri_string(predicate),
                    }
                })?;
                used_walks.insert(predicate.clone());
                // Route the ordered element resolution by the ENTRY POINT (never the carried variant):
                // a materialized layer re-verifies the `OrderedListWalk` envelope; a delta head
                // re-verifies the reconciled ordered-list walk over `present` (6c-iii). A walk whose
                // variant disagrees with the entry's `InstanceSource` reject-closes.
                let elements = match (instance, &list_property.walk) {
                    (InstanceSource::Materialized(view), ListWalkProof::Materialized(bgp)) => {
                        let walk_compiled = ordered_list_walk_bgp(id.as_bytes(), predicate);
                        let columns =
                            bgp.verify_and_resolve(&walk_compiled, view, instance_root)?;
                        columns.into_iter().next().unwrap_or_default()
                    }
                    (InstanceSource::Reconciled(layers), ListWalkProof::Reconciled(walk)) => {
                        verify_ordered_list_walk_reconciled(
                            layers,
                            instance_root,
                            &CanonicalObject::Node(id.as_bytes().to_vec()),
                            predicate,
                            walk,
                        )?
                    }
                    _ => {
                        return Err(invalid(
                            "list walk proof route does not match the entry point's instance source",
                        ))
                    }
                };
                // Per-element ACTUAL-TYPE / `@unfold` reject over the authenticated ordered elements
                // (v1 renders bare ids; refuse any element the reference would unfold).
                let unfold = unfold_of.get(predicate).copied().unwrap_or(false);
                verify_collection_elements(
                    predicate,
                    &elements,
                    unfold,
                    &collection_map,
                    &mut used_collections,
                    instance,
                    schema,
                    database_root,
                )?;
                rendered.push((predicate.clone(), ResolvedValue::List(elements)));
            }
            RangeShape::Array => {
                // An array property carries `n` cell edges (NOT a single head), so — unlike the `List`
                // arm — there is NO `objects.len() != 1` reject. A PRESENT array property (it is present
                // here because `by_predicate` has ≥1 row for it) MUST carry exactly one authenticated
                // walk; a missing walk reject-closes (a prover cannot render the raw cell ids).
                let array_property = array_map.get(predicate).copied().ok_or_else(|| {
                    invalid(format!(
                        "bundle is missing the array-walk sub-proof for present array property {}",
                        iri_string(predicate)
                    ))
                })?;
                used_arrays.insert(predicate.clone());
                // Route the ordered element resolution by the ENTRY POINT (never the carried variant):
                // a materialized layer re-verifies the `OrderedArrayWalk` envelope; a delta head
                // re-verifies the reconciled ordered-array walk over `present` (6d). A walk whose variant
                // disagrees with the entry's `InstanceSource` reject-closes.
                let elements = match (instance, &array_property.walk) {
                    (InstanceSource::Materialized(view), ArrayWalkProof::Materialized(bgp)) => {
                        let walk_compiled = ordered_array_walk_bgp(id.as_bytes(), predicate);
                        let columns = bgp.verify_and_resolve(&walk_compiled, view, instance_root)?;
                        columns.into_iter().next().unwrap_or_default()
                    }
                    (InstanceSource::Reconciled(layers), ArrayWalkProof::Reconciled(walk)) => {
                        verify_ordered_array_walk_reconciled(
                            layers,
                            instance_root,
                            &CanonicalObject::Node(id.as_bytes().to_vec()),
                            predicate,
                            walk,
                        )?
                    }
                    _ => {
                        return Err(invalid(
                            "array walk proof route does not match the entry point's instance source",
                        ))
                    }
                };
                // Per-element ACTUAL-TYPE / `@unfold` reject over the authenticated ordered elements
                // (v1 renders bare ids; refuse any element the reference would unfold), reusing the
                // inc-3 collection machinery IDENTICALLY to prove — the array analogue of the list check.
                let unfold = unfold_of.get(predicate).copied().unwrap_or(false);
                verify_collection_elements(
                    predicate,
                    &elements,
                    unfold,
                    &collection_map,
                    &mut used_collections,
                    instance,
                    schema,
                    database_root,
                )?;
                rendered.push((predicate.clone(), ResolvedValue::Array(elements)));
            }
            RangeShape::Link => {
                let unfold = unfold_of.get(predicate).copied().unwrap_or(false);
                let value = if objects.len() == 1 {
                    let child_id = match &objects[0] {
                        CanonicalObject::Node(bytes) => bytes.clone(),
                        _ => return Err(invalid("class-link object is not a node")),
                    };
                    let key = (predicate.clone(), child_id.clone());
                    let link = *link_map.get(&key).ok_or_else(|| {
                        invalid(format!(
                            "bundle is missing the link sub-proof for predicate {} child {}",
                            iri_string(predicate),
                            iri_string(&child_id)
                        ))
                    })?;
                    used_links.insert(key);
                    let child_iri = std::str::from_utf8(&child_id)
                        .map_err(|_| invalid("link child IRI is not valid UTF-8"))?
                        .to_string();
                    match &link.target {
                        ProvedLinkTarget::Inline(child_node) => {
                            // INCREMENT 6b: RECURSE into the inlined subdocument child — the inc-3
                            // recursion, threaded over the reconciled `instance` source (a nested node's
                            // instance reads route over `present`; the schema frame + closure stay
                            // materialized). The child's authenticated subdocument-ness is re-checked
                            // BELOW (a non-subdocument `Inline` is an injected body ⇒ reject), and a nested
                            // `List`/`Array` inside the child reject-closes with
                            // `ReadDocumentDeltaWalkDeferred` — IDENTICALLY on the materialized and delta
                            // paths. The `visited`/depth/node budget bounds the recursion on BOTH paths.
                            if visited.contains(&child_id) {
                                return Err(BgpPlanError::ReadDocumentSubdocumentCycle.into());
                            }
                            if depth + 1 > MAX_SUBDOC_DEPTH {
                                return Err(
                                    BgpPlanError::ReadDocumentSubdocumentBudgetExceeded.into()
                                );
                            }
                            *node_count += 1;
                            if *node_count > MAX_SUBDOC_NODES {
                                return Err(
                                    BgpPlanError::ReadDocumentSubdocumentBudgetExceeded.into()
                                );
                            }
                            visited.insert(child_id.clone());
                            let (child_resolved, child_natures) = verify_node(
                                child_node,
                                &child_iri,
                                instance,
                                schema,
                                database_root,
                                /* allow_subdocument */ true,
                                depth + 1,
                                visited,
                                node_count,
                            )?;
                            // The prover chose INLINE; force it to be a real subdocument (else it is an
                            // INJECTED body — the reference would render a bare link id).
                            if !child_natures.has_subdocument {
                                return Err(invalid(
                                    "bundle inlines a non-subdocument link target (injected body)",
                                ));
                            }
                            ResolvedValue::Subdocument(Box::new(child_resolved))
                        }
                        ProvedLinkTarget::Stop(witness) => {
                            let (_child_class, child_natures) = verify_child_witness(
                                witness,
                                &child_id,
                                instance,
                                schema,
                                database_root,
                            )?;
                            // The prover chose STOP; force the child to NOT be a subdocument (else it is
                            // an OMISSION — the reference would inline the child's contents). SOURCE-
                            // AGNOSTIC (increment 6b): the child's subdocument nature is re-derived from
                            // root-authenticated data (its own bound-subject read — a materialized BGP or a
                            // reconciled `present` filter — plus its schema `sys:inherits` closure), so a
                            // forged `Stop` over a real subdocument child reject-closes on the delta path
                            // EXACTLY as on the materialized path. The honest delta prover now emits an
                            // `Inline` for a subdocument child (6b), so a `Stop` here is a genuine omission.
                            if child_natures.has_subdocument {
                                return Err(invalid(
                                    "bundle stops at a subdocument link target (missing inlined body)",
                                ));
                            }
                            // An `@unfoldable` class or `@unfold` property would be unfolded by the
                            // reference; we inline only true subdocuments, so reject rather than omit.
                            if child_natures.has_unfoldable || unfold {
                                return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
                            }
                            ResolvedValue::Direct(vec![objects[0].clone()])
                        }
                    }
                } else {
                    // A single-valued class LINK with MULTIPLE stored objects: v1 inlines only
                    // single-valued subdocuments, so every object must be a plain (non-subdocument)
                    // link; render the ids.
                    for object in objects {
                        let child_id = match object {
                            CanonicalObject::Node(bytes) => bytes.clone(),
                            _ => return Err(invalid("class-link object is not a node")),
                        };
                        let key = (predicate.clone(), child_id.clone());
                        let link = *link_map.get(&key).ok_or_else(|| {
                            invalid(format!(
                                "bundle is missing the link sub-proof for predicate {} child {}",
                                iri_string(predicate),
                                iri_string(&child_id)
                            ))
                        })?;
                        used_links.insert(key);
                        match &link.target {
                            ProvedLinkTarget::Stop(witness) => {
                                let (_child_class, child_natures) = verify_child_witness(
                                    witness,
                                    &child_id,
                                    instance,
                                    schema,
                                    database_root,
                                )?;
                                if child_natures.has_subdocument {
                                    return Err(shape_reject("multi-valued-subdocument"));
                                }
                                if child_natures.has_unfoldable || unfold {
                                    return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
                                }
                            }
                            ProvedLinkTarget::Inline(_) => {
                                return Err(shape_reject("multi-valued-subdocument"));
                            }
                        }
                    }
                    ResolvedValue::Direct(objects.clone())
                };
                rendered.push((predicate.clone(), value));
            }
            RangeShape::Set => {
                // Per-element ACTUAL-TYPE / `@unfold` reject over the set's direct objects (v1 renders
                // bare ids; refuse any element the reference would unfold), then render the ids.
                let unfold = unfold_of.get(predicate).copied().unwrap_or(false);
                verify_collection_elements(
                    predicate,
                    objects,
                    unfold,
                    &collection_map,
                    &mut used_collections,
                    instance,
                    schema,
                    database_root,
                )?;
                rendered.push((predicate.clone(), ResolvedValue::Direct(objects.clone())));
            }
            RangeShape::Scalar => {
                rendered.push((predicate.clone(), ResolvedValue::Direct(objects.clone())));
            }
        }
    }

    // No injected list walk / link sub-proof (for an absent or wrong-shape predicate/child).
    for predicate in walk_map.keys() {
        if !used_walks.contains(predicate) {
            return Err(BgpPlanError::ReadDocumentMalformedListHead {
                predicate: iri_string(predicate),
            }
            .into());
        }
    }
    // No injected array-walk sub-proof (for an absent or non-array predicate). Every carried array walk
    // must have been demanded by a present array property above.
    for predicate in array_map.keys() {
        if !used_arrays.contains(predicate) {
            return Err(invalid(format!(
                "bundle carries an unused (injected) array-walk sub-proof for {}",
                iri_string(predicate)
            )));
        }
    }
    for key in link_map.keys() {
        if !used_links.contains(key) {
            return Err(invalid(format!(
                "bundle carries an unused (injected) link sub-proof for predicate {} child {}",
                iri_string(&key.0),
                iri_string(&key.1)
            )));
        }
    }
    // No injected collection element-witness sub-proof (for an absent or non-collection predicate).
    for predicate in collection_map.keys() {
        if !used_collections.contains(predicate) {
            return Err(invalid(format!(
                "bundle carries an unused (injected) collection property sub-proof for {}",
                iri_string(predicate)
            )));
        }
    }

    Ok((
        ResolvedDocument {
            id: id.as_bytes().to_vec(),
            class,
            properties: rendered,
        },
        natures,
    ))
}

impl ProvedReadDocument {
    /// Verify every sub-proof against the trusted combined `database_root`, RE-DERIVE the frame + the
    /// recursive subdocument tree, run the completeness / no-injection cross-checks, and render the
    /// document. The verifier reconstructs every sub-plan from the query `id` + the authenticated frame
    /// at EVERY node — it never trusts a prover-supplied compiled plan, classification, or tree shape.
    ///
    /// TRUST ANCHOR: `database_root` MUST come from a trusted OUT-OF-BAND pin (commit log / signature),
    /// never recomputed from the prover-supplied bundle. See [`ProvedBgp::verify_over_graphs`].
    pub fn verify_over_graphs_and_resolve(
        &self,
        compiled: &CompiledReadDocument,
        instance: &impl VerifierLayer,
        schema: &impl VerifierLayer,
        database_root: ProofHash,
    ) -> Result<ResolvedDocument, QueryProofError> {
        // (1) Bind BOTH per-graph commitments to the trusted combined root. From here every per-graph
        //     root the verifier reads is trusted (neither can be forged/swapped).
        validate_database_root(instance, schema, &database_root)?;
        // PARITY GUARD (defense-in-depth): this fully-materialized entry resolves every instance read
        // against a SINGLE materialized layer view, so the instance commitment must be a base layer
        // (`parent_state_root == None`). A delta-head instance belongs on the reconciled entry; it is
        // already fail-closed downstream (each leaf read requires a base layer), but reject it
        // explicitly here to mirror the delta-path schema-head reject and the per-leaf base-layer gate.
        if instance.verifier_view().state.parent_state_root.is_some() {
            return Err(BgpPlanError::ReadDocumentMaterializedInstanceHead.into());
        }
        // MATERIALIZED route (BOTH graphs): the ENTRY fixes both sources, so every instance/schema
        // bound-subject read resolves the `Materialized` leaf against its single layer view
        // (byte-identical to before).
        let source = InstanceSource::Materialized(instance.verifier_view());
        let schema_source = SchemaSource::Materialized(schema.verifier_view());

        // The ROOT node: `visited` seeds with the root id, `node_count` counts the root, and
        // `allow_subdocument = false` so a subdocument top-level class reject-closes. The budget is the
        // IDENTICAL one the prover enforced.
        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(compiled.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        let (resolved, _natures) = verify_node(
            self,
            &compiled.id,
            &source,
            &schema_source,
            database_root,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )?;
        Ok(resolved)
    }

    /// Verify + resolve a `ReadDocument` proven over a delta INSTANCE stack (`instance_layers`,
    /// base..head, head last) + a MATERIALIZED schema (increment 6a). Mirrors
    /// [`Self::verify_over_graphs_and_resolve`]: every instance bound-subject sub-proof is re-verified as
    /// a reconciled single-atom present filter bound to the trusted delta-HEAD anchor, and the schema
    /// frame BGPs + `sys:inherits` closure verify against the materialized schema root; all under the
    /// trusted combined `database_root = database_root(instance_head_root, schema_root)`. A FLAT
    /// subdocument child INLINES recursively (increment 6b) — `verify_node` recurses with the reconciled
    /// source threaded through, and the child's authenticated subdocument-ness forces the inline/stop
    /// render mode exactly as on the materialized path. The delta-path rejects (`List`/`Array` at the top
    /// level OR inside a nested subdocument, delta-head schema) run inside the shared node verify, so a
    /// forged bundle claiming an unsupported shape reject-closes.
    ///
    /// TRUST ANCHOR: `database_root` MUST come from a trusted OUT-OF-BAND pin (commit log / signature),
    /// never recomputed from the prover-supplied bundle.
    pub fn verify_over_graphs_and_resolve_reconciled(
        &self,
        compiled: &CompiledReadDocument,
        instance_layers: &[&LayerCommitment],
        schema: &impl VerifierLayer,
        database_root: ProofHash,
    ) -> Result<ResolvedDocument, QueryProofError> {
        // The SCHEMA graph must be MATERIALIZED (6e reconciles the schema stack). A delta-head schema
        // reject-closes.
        let schema_view = schema.verifier_view();
        if schema_view.state.parent_state_root.is_some() {
            return Err(BgpPlanError::ReadDocumentDeltaSchemaHead.into());
        }
        let head = instance_layers
            .last()
            .ok_or_else(|| invalid("delta instance stack is empty"))?;
        // (1) Bind the delta-HEAD instance root + the materialized schema root to the trusted combined
        //     root. The head layer's commitment root is the trusted delta-head anchor the reconcile
        //     verifier pins, so this ties every instance present-read to the SAME anchor.
        validate_database_root(*head, schema, &database_root)?;
        // RECONCILED INSTANCE + MATERIALIZED SCHEMA route: the ENTRY fixes the instance source to the
        // delta stack and the schema source to the single materialized layer view; a leaf whose variant
        // disagrees reject-closes in `InstanceSubjectProof::resolve` / `SchemaSubjectProof::resolve`.
        let source = InstanceSource::Reconciled(instance_layers);
        let schema_source = SchemaSource::Materialized(schema_view);

        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(compiled.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        let (resolved, _natures) = verify_node(
            self,
            &compiled.id,
            &source,
            &schema_source,
            database_root,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )?;
        Ok(resolved)
    }

    /// Verify + resolve a `ReadDocument` proven over a delta INSTANCE stack (`instance_layers`) AND a
    /// delta SCHEMA stack (`schema_layers`) — BOTH base..head, head last (increment 6e — the FINAL
    /// delta-stack increment, closing the entire delta-path parity). The fully-reconciled twin of
    /// [`Self::verify_over_graphs_and_resolve`] / [`Self::verify_over_graphs_and_resolve_reconciled`]:
    /// every instance bound-subject sub-proof re-verifies as a reconciled single-atom present filter
    /// bound to the trusted instance-HEAD anchor; every SCHEMA read (class frame, ranges, strict
    /// ancestors) re-verifies as a reconciled single-atom present filter bound to the trusted
    /// schema-HEAD anchor; and the `sys:inherits` closure re-verifies as the store's
    /// `TransitiveClosure`-over-`present` on the schema chain. BOTH head roots are pinned to the
    /// combined `database_root = database_root(instance_head_root, schema_head_root)` by
    /// `validate_database_root` (which accepts delta-head views — each recomputes its OWN commitment
    /// root from its committed parts). The instance-`C` == schema-`C` tie stays a verify-time IRI
    /// equality, now over two root-authenticated RECONCILED values (both under the trusted combined
    /// root, single subject → sound). The delta-head schema reject is LIFTED; the route per graph is
    /// fixed by THIS entry, so a leaf whose variant disagrees reject-closes.
    ///
    /// TRUST ANCHOR: `database_root` MUST come from a trusted OUT-OF-BAND pin (commit log / signature),
    /// never recomputed from the prover-supplied bundle.
    pub fn verify_over_graphs_and_resolve_fully_reconciled(
        &self,
        compiled: &CompiledReadDocument,
        instance_layers: &[&LayerCommitment],
        schema_layers: &[&LayerCommitment],
        database_root: ProofHash,
    ) -> Result<ResolvedDocument, QueryProofError> {
        let instance_head = instance_layers
            .last()
            .ok_or_else(|| invalid("delta instance stack is empty"))?;
        let schema_head = schema_layers
            .last()
            .ok_or_else(|| invalid("delta schema stack is empty"))?;
        // (1) Bind BOTH delta-HEAD roots to the trusted combined root. Each head layer's commitment root
        //     is the trusted delta-head anchor its reconciliation pins, so this ties every instance AND
        //     schema present-read (and the schema closure) to the SAME two anchors. `validate_database_root`
        //     accepts a delta-head view — its own `validate()` recomputes the head's commitment root.
        validate_database_root(*instance_head, *schema_head, &database_root)?;
        // FULLY-RECONCILED route: the ENTRY fixes BOTH sources to their delta stacks.
        let source = InstanceSource::Reconciled(instance_layers);
        let schema_source = SchemaSource::Reconciled(schema_layers);

        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(compiled.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        let (resolved, _natures) = verify_node(
            self,
            &compiled.id,
            &source,
            &schema_source,
            database_root,
            /* allow_subdocument */ false,
            0,
            &mut visited,
            &mut node_count,
        )?;
        Ok(resolved)
    }
}

#[cfg(test)]
mod base_types_subset_guard {
    use super::BASE_TYPES;

    /// A snapshot of the server's AUTHORITATIVE `base_type/1` set (`core/triple/base_type.pl`, all 64
    /// facts). Mirrored here ONLY to mechanically enforce the [`BASE_TYPES`] subset invariant — if
    /// `base_type.pl` changes, update this snapshot too. A `BASE_TYPES` entry missing from here is a
    /// SUPERSET, which is a soundness risk (see the `BASE_TYPES` doc comment).
    const REFERENCE_BASE_TYPES: &[&str] = &[
        "http://www.w3.org/2001/XMLSchema#anySimpleType",
        "http://www.w3.org/2001/XMLSchema#string",
        "http://www.w3.org/2001/XMLSchema#boolean",
        "http://www.w3.org/2001/XMLSchema#decimal",
        "http://www.w3.org/2001/XMLSchema#double",
        "http://www.w3.org/2001/XMLSchema#float",
        "http://www.w3.org/2001/XMLSchema#time",
        "http://www.w3.org/2001/XMLSchema#date",
        "http://www.w3.org/2001/XMLSchema#dateTime",
        "http://www.w3.org/2001/XMLSchema#dateTimeStamp",
        "http://www.w3.org/2001/XMLSchema#gYear",
        "http://www.w3.org/2001/XMLSchema#gMonth",
        "http://www.w3.org/2001/XMLSchema#gDay",
        "http://www.w3.org/2001/XMLSchema#gYearMonth",
        "http://www.w3.org/2001/XMLSchema#gMonthDay",
        "http://www.w3.org/2001/XMLSchema#duration",
        "http://www.w3.org/2001/XMLSchema#yearMonthDuration",
        "http://www.w3.org/2001/XMLSchema#dayTimeDuration",
        "http://www.w3.org/2001/XMLSchema#byte",
        "http://www.w3.org/2001/XMLSchema#short",
        "http://www.w3.org/2001/XMLSchema#int",
        "http://www.w3.org/2001/XMLSchema#long",
        "http://www.w3.org/2001/XMLSchema#unsignedByte",
        "http://www.w3.org/2001/XMLSchema#unsignedShort",
        "http://www.w3.org/2001/XMLSchema#unsignedInt",
        "http://www.w3.org/2001/XMLSchema#unsignedLong",
        "http://www.w3.org/2001/XMLSchema#integer",
        "http://www.w3.org/2001/XMLSchema#positiveInteger",
        "http://www.w3.org/2001/XMLSchema#nonNegativeInteger",
        "http://www.w3.org/2001/XMLSchema#negativeInteger",
        "http://www.w3.org/2001/XMLSchema#nonPositiveInteger",
        "http://www.w3.org/2001/XMLSchema#base64Binary",
        "http://www.w3.org/2001/XMLSchema#hexBinary",
        "http://www.w3.org/2001/XMLSchema#anyURI",
        "http://www.w3.org/2001/XMLSchema#language",
        "http://www.w3.org/2001/XMLSchema#normalizedString",
        "http://www.w3.org/2001/XMLSchema#token",
        "http://www.w3.org/2001/XMLSchema#NMTOKEN",
        "http://www.w3.org/2001/XMLSchema#Name",
        "http://www.w3.org/2001/XMLSchema#NCName",
        "http://www.w3.org/2001/XMLSchema#NOTATION",
        "http://www.w3.org/2001/XMLSchema#QName",
        "http://www.w3.org/2001/XMLSchema#ID",
        "http://www.w3.org/2001/XMLSchema#IDREF",
        "http://www.w3.org/2001/XMLSchema#ENTITY",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#XMLLiteral",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#PlainLiteral",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString",
        "http://www.w3.org/2000/01/rdf-schema#Literal",
        "http://terminusdb.com/schema/xdd#coordinate",
        "http://terminusdb.com/schema/xdd#coordinatePolygon",
        "http://terminusdb.com/schema/xdd#coordinatePolyline",
        "http://terminusdb.com/schema/xdd#dateRange",
        "http://terminusdb.com/schema/xdd#dateTimeInterval",
        "http://terminusdb.com/schema/xdd#gYearRange",
        "http://terminusdb.com/schema/xdd#integerRange",
        "http://terminusdb.com/schema/xdd#decimalRange",
        "http://terminusdb.com/schema/xdd#json",
        "http://terminusdb.com/schema/xdd#url",
        "http://terminusdb.com/schema/xdd#email",
        "http://terminusdb.com/schema/xdd#html",
        "http://terminusdb.com/schema/sys#JSONDocument",
        "http://terminusdb.com/schema/sys#JSON",
        "http://terminusdb.com/schema/sys#Dictionary",
    ];

    /// Every entry the classifier treats as a base type MUST be a genuine server base type. This guards
    /// the soundness invariant on `BASE_TYPES`: a superset would let the verifier render a non-primitive
    /// as a scalar.
    #[test]
    fn base_types_is_a_strict_subset_of_reference() {
        for ty in BASE_TYPES {
            assert!(
                REFERENCE_BASE_TYPES.contains(ty),
                "BASE_TYPES entry `{ty}` is NOT in the server's base_type/1 — a SUPERSET is a soundness risk",
            );
        }
        // A subset, and strictly so (the server has base types v1 deliberately does not accept).
        assert!(
            BASE_TYPES.len() < REFERENCE_BASE_TYPES.len(),
            "BASE_TYPES must remain a STRICT subset of the reference base_type/1",
        );
    }
}

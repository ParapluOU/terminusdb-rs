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
//! multi-level `sys:inherits` — and whose effective (inherited-union) properties are scalar, link
//! (non-subdocument), `Set` (no cardinality), or `List` (non-subdocument element). Reject-close:
//! variable id; subdocument / array / optional / cardinality / oneOf / enum / table range;
//! `rdf:langString`; an instance property absent from the (inherited) frame; a list property with != 1
//! head edge; a conflicting inherited range (the same predicate declared with two different ranges up
//! the chain); ANY inheritance-closed subdocument nature or ANY tagged-union/`sys:oneOf`/enum nature
//! anywhere in the ancestor chain (fail-closed: subdocument-ness IS inheritance-closed, and the
//! oneOf-choice properties of a tagged-union ancestor are not enumerable by the direct frame BGP, so
//! we refuse rather than risk omitting them). A LINK/ELEMENT target class that itself inherits still
//! reject-closes (its inherited subdocument-ness is not ruled out — a separate, larger effort).
//! Compiler-only; nothing under `terminusdb-store-paraplu` changes.

use super::*;

use std::collections::{BTreeMap, BTreeSet};

use crate::document::ReadDocument;
use terminus_store::proof::commitment::{database_root, validate_database_root};

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
const SYS_SUBDOCUMENT: &str = "http://terminusdb.com/schema/sys#subdocument";
const SYS_UNFOLDABLE: &str = "http://terminusdb.com/schema/sys#unfoldable";
const SYS_UNFOLD: &str = "http://terminusdb.com/schema/sys#unfold";
const SYS_ONEOF: &str = "http://terminusdb.com/schema/sys#oneOf";
const SYS_MIN_CARDINALITY: &str = "http://terminusdb.com/schema/sys#min_cardinality";
const SYS_MAX_CARDINALITY: &str = "http://terminusdb.com/schema/sys#max_cardinality";

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
    // `rdf:langString` is a base type but v1 reject-closes it (a language-tagged literal needs a
    // fidelity pass) — see `RDF_LANGSTRING` handling in `classify_range`.
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

/// One range/element schema node's classification sub-proof: a bound-subject BGP `Triple(node, ?p, ?o)`
/// on the SCHEMA graph, delivering the node's direct schema triples. The verifier reconstructs the
/// plan from `node` (a pure function of the node IRI) and verifies against the schema root, so the
/// delivered rows are root-authenticated.
#[derive(Debug)]
pub struct ProvedRangeClassification {
    /// The schema node IRI (a range descriptor node OR a class) whose direct triples this proves.
    pub node: Vec<u8>,
    /// `Triple(node, ?p, ?o)` proved over the schema commitment.
    pub schema_bgp: ProvedBgp,
}

/// The authenticated class frame of the document: the class `C`, the schema BGP enumerating `C`'s
/// direct properties, the reflexive `sys:inherits` ancestor closure (for an inheriting `C`), and one
/// classification sub-proof per non-primitive range/element node AND per strict ancestor class.
#[derive(Debug)]
pub struct ProvedFrame {
    /// The prover's `C` (informational). The verifier DERIVES `C` from the root-authenticated instance
    /// `rdf:type` row and uses ITS derived `C` to reconstruct the frame plan — it never trusts this.
    pub class: Vec<u8>,
    /// `Triple(C, ?p, ?r)` proved over the schema commitment — `C`'s OWN direct properties.
    pub class_schema: ProvedBgp,
    /// The reflexive forward `sys:inherits` closure from `C` over the schema graph — the authenticated
    /// ancestor SET `{C} ∪ ancestors(C)`, delivered as ONE node column. Present ONLY when `C`'s
    /// complete, root-authenticated frame bears a `sys:inherits` edge; a non-inheriting `C` has NO
    /// ancestors (so the set is exactly `{C}` and no closure is proved). The verifier RE-BUILDS this
    /// `CompiledBgp` from the derived `C` and re-verifies it, so the set is the closure's proven output
    /// (the empty-delta fixpoint proves it is COMPLETE — no ancestor omitted), never trusted from here.
    pub ancestor_closure: Option<ProvedBgp>,
    /// One entry per distinct schema node the classification reads — property ranges + element classes
    /// (excluding primitives) AND every strict ancestor class of `C`.
    pub ranges: Vec<ProvedRangeClassification>,
}

/// One PRESENT list property's ordered-walk sub-proof.
#[derive(Debug)]
pub struct ProvedListProperty {
    /// The list-valued predicate IRI.
    pub predicate: Vec<u8>,
    /// The store's `OrderedListWalk` proof for `(id, predicate)` — the exact ordered element sequence.
    pub walk: ProvedBgp,
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
    /// The child's instance bound-subject BGP → its `rdf:type` = `C_child` (+ its direct triples,
    /// unused for a stop link). Reconstructed at verify from `child_id`.
    pub subject_bgp: ProvedBgp,
    /// `C_child`'s OWN schema frame read `Triple(C_child, ?p, ?r)`. `None` iff the child has NO
    /// `rdf:type` row (a typeless / dangling link) — the reference renders such a target as a plain
    /// link id, and so do we (a typeless target is trivially non-subdocument).
    pub class_schema: Option<ProvedBgp>,
    /// `C_child`'s reflexive `sys:inherits` closure over the SCHEMA graph (the authenticated
    /// `{C_child} ∪ ancestors`). `None` iff there is no class, or `C_child` bears no `sys:inherits`
    /// edge (then the set is exactly `{C_child}`).
    pub ancestor_closure: Option<ProvedBgp>,
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
    /// The prover's `Triple(id, ?p, ?o)` plan (informational). Verify reconstructs its own copy from
    /// the node `id` and never trusts this one.
    pub subject_compiled: CompiledBgp,
    /// The instance bound-subject BGP proof — the COMPLETE direct triples of this node.
    pub subject_bgp: ProvedBgp,
    /// The authenticated class frame.
    pub frame: ProvedFrame,
    /// One walk per PRESENT list property.
    pub lists: Vec<ProvedListProperty>,
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

/// Prover-side reader: proves + resolves each requested schema node once, recording a
/// [`ProvedRangeClassification`].
struct ProveReader<'a> {
    ctx: &'a StoreProverContext<'a>,
    schema: &'a LayerCommitment,
    schema_root: ProofHash,
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
        let compiled = bound_subject_bgp(node_iri)?;
        let proved = compiled.prove_with_context(self.ctx, self.schema)?;
        let rows = resolve_subject_rows(&proved, &compiled, self.schema, self.schema_root)?;
        self.ranges.push(ProvedRangeClassification {
            node: node.to_vec(),
            schema_bgp: proved,
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
    has_min_cardinality: bool,
    has_max_cardinality: bool,
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
        } else if predicate == SYS_SUBDOCUMENT.as_bytes() {
            info.has_subdocument = true;
        } else if predicate == SYS_UNFOLDABLE.as_bytes() {
            info.has_unfoldable = true;
        } else if predicate == SYS_UNFOLD.as_bytes() {
            info.has_unfold = true;
        } else if predicate == SYS_INHERITS_IRI.as_bytes() {
            info.has_inherits = true;
        } else if predicate == SYS_MIN_CARDINALITY.as_bytes() {
            info.has_min_cardinality = true;
        } else if predicate == SYS_MAX_CARDINALITY.as_bytes() {
            info.has_max_cardinality = true;
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
    // `rdf:langString` is a base type but v1 reject-closes it (language-tagged literals need a
    // fidelity pass).
    if range_iri == RDF_LANGSTRING.as_bytes() {
        return Err(shape_reject("lang-string"));
    }
    // A base type is a primitive with no schema triples → SCALAR (no schema read needed).
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
        return Err(BgpPlanError::ReadDocumentArrayDeferred.into());
    }
    if info.has_type(SYS_SET) {
        if info.has_min_cardinality || info.has_max_cardinality {
            return Err(shape_reject("cardinality-set"));
        }
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
        return Err(shape_reject("cardinality"));
    }
    if info.has_type(SYS_OPTIONAL) {
        return Err(shape_reject("optional"));
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
    if element_iri == RDF_LANGSTRING.as_bytes() {
        return Err(shape_reject("lang-string-element"));
    }
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
    if !info.has_type(SYS_CLASS_TYPE) {
        return Err(shape_reject("class-not-sys-class"));
    }
    if info.has_type(SYS_TAGGED_UNION) || info.has_type(SYS_ENUM) {
        return Err(shape_reject("tagged-union-or-enum-class"));
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
            RangeShape::Link | RangeShape::Set | RangeShape::List => match range {
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
    subject_compiled: CompiledBgp,
    subject_bgp: ProvedBgp,
    d_rows: SubjectRows,
    /// `None` => the node has NO `rdf:type` row (typeless/dangling link → non-subdocument stop).
    class: Option<Vec<u8>>,
    class_schema: Option<ProvedBgp>,
    class_rows: SubjectRows,
    ancestor_closure: Option<ProvedBgp>,
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
    instance: &LayerCommitment,
    schema: &LayerCommitment,
    instance_root: ProofHash,
    schema_root: ProofHash,
    db_root: ProofHash,
    id: &str,
) -> Result<ProvenCore, QueryProofError> {
    // (D) COMPLETE direct triples of this node (instance graph).
    let subject_compiled = bound_subject_bgp(id)?;
    let subject_bgp = subject_compiled.prove_with_context(ctx, instance)?;
    let d_rows = resolve_subject_rows(&subject_bgp, &subject_compiled, instance, instance_root)?;

    // (F0) Bind the class. A typeless target (dangling link) is a non-subdocument stop.
    let Some(class) = extract_class_opt(&d_rows)? else {
        return Ok(ProvenCore {
            subject_compiled,
            subject_bgp,
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

    // (F1) C's OWN schema frame.
    let class_compiled = bound_subject_bgp(class_iri)?;
    let class_schema = class_compiled.prove_with_context(ctx, schema)?;
    let class_rows = resolve_subject_rows(&class_schema, &class_compiled, schema, schema_root)?;

    // (F1b) Reflexive `sys:inherits` closure — proven only when `C` actually inherits (otherwise the
    // reflexive closure is exactly `{C}`, synthesized locally, at no proof cost).
    let (ancestor_closure, ancestor_set) = if summarize(&class_rows).has_inherits {
        let closure_compiled = ancestor_closure_bgp(class_iri)?;
        let closure = closure_compiled.prove_with_context(ctx, schema)?;
        let set = resolve_closure_set(&closure, &closure_compiled, instance, schema, db_root)?;
        (Some(closure), set)
    } else {
        (None, vec![class.clone()])
    };

    // Read each STRICT ancestor's frame (recorded into `reader.ranges`) and compute the
    // inheritance-closed subdocument / unfoldable natures.
    let mut reader = ProveReader {
        ctx,
        schema,
        schema_root,
        cache: BTreeMap::new(),
        ranges: Vec::new(),
    };
    let natures = closure_natures(&class, &class_rows, &ancestor_set, &mut reader)?;

    Ok(ProvenCore {
        subject_compiled,
        subject_bgp,
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
    instance: &LayerCommitment,
    schema: &LayerCommitment,
    instance_root: ProofHash,
    schema_root: ProofHash,
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
        let child_core = prove_core(
            ctx,
            instance,
            schema,
            instance_root,
            schema_root,
            db_root,
            &child_iri,
        )?;
        if child_core.natures.has_subdocument || child_core.natures.has_unfoldable {
            return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
        }
        witnesses.push(ProvedCollectionElement {
            child_id,
            witness: ProvedChildTypeWitness {
                subject_bgp: child_core.subject_bgp,
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
    instance: &LayerCommitment,
    schema: &LayerCommitment,
    instance_root: ProofHash,
    schema_root: ProofHash,
    db_root: ProofHash,
    id: &str,
    core: ProvenCore,
    allow_subdocument: bool,
    depth: usize,
    visited: &mut BTreeSet<Vec<u8>>,
    node_count: &mut usize,
) -> Result<ProvedReadDocument, QueryProofError> {
    let ProvenCore {
        subject_compiled,
        subject_bgp,
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
        schema_root,
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
                let walk_compiled = ordered_list_walk_bgp(id.as_bytes(), &prop.predicate);
                let walk = walk_compiled.prove_with_context(ctx, instance)?;
                // The list's ORDERED elements come from the authenticated walk (NOT the raw cons head);
                // re-resolve them to run the per-element ACTUAL-TYPE / `@unfold` reject over the real
                // element sequence, identically to verify.
                let columns = walk.verify_and_resolve(&walk_compiled, instance, instance_root)?;
                let elements = columns.into_iter().next().unwrap_or_default();
                collections.push(prove_collection_elements(
                    ctx,
                    instance,
                    schema,
                    instance_root,
                    schema_root,
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
                    // CHILD's authenticated type + closure, NEVER the schema range.
                    let child_core = prove_core(
                        ctx,
                        instance,
                        schema,
                        instance_root,
                        schema_root,
                        db_root,
                        &child_iri,
                    )?;
                    if child_core.natures.has_subdocument {
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
                            instance_root,
                            schema_root,
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
                        // rendering such a target as a bare link would OMIT content — reject closed.
                        if child_core.natures.has_unfoldable || prop.unfold {
                            return Err(BgpPlanError::ReadDocumentUnfoldableDeferred.into());
                        }
                        links.push(ProvedLink {
                            predicate: prop.predicate.clone(),
                            child_id,
                            target: ProvedLinkTarget::Stop(ProvedChildTypeWitness {
                                subject_bgp: child_core.subject_bgp,
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
                        instance_root,
                        schema_root,
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
        subject_compiled,
        subject_bgp,
        frame: ProvedFrame {
            class,
            class_schema,
            ancestor_closure,
            ranges,
        },
        lists,
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
        let instance_root = instance.state.commitment_root;
        let schema_root = schema.state.commitment_root;
        let db_root = database_root(&instance_root, &schema_root);

        let core = prove_core(ctx, instance, schema, instance_root, schema_root, db_root, &self.id)?;
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
            instance,
            schema,
            instance_root,
            schema_root,
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
#[allow(clippy::too_many_arguments)]
fn verify_child_witness<I: VerifierLayer, S: VerifierLayer>(
    witness: &ProvedChildTypeWitness,
    child_id: &[u8],
    instance: &I,
    schema: &S,
    instance_root: ProofHash,
    schema_root: ProofHash,
    database_root: ProofHash,
) -> Result<(Option<Vec<u8>>, ClosureNatures), QueryProofError> {
    let child_iri =
        std::str::from_utf8(child_id).map_err(|_| invalid("link child IRI is not valid UTF-8"))?;
    // The child's type — reconstruct the plan from `child_id` (pure function), verify over the
    // instance root.
    let subject_compiled = bound_subject_bgp(child_iri)?;
    let d_rows =
        resolve_subject_rows(&witness.subject_bgp, &subject_compiled, instance, instance_root)?;
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
    let class_compiled = bound_subject_bgp(class_iri)?;
    let class_schema = witness
        .class_schema
        .as_ref()
        .ok_or_else(|| invalid("typed link witness is missing its class schema"))?;
    let class_rows = resolve_subject_rows(class_schema, &class_compiled, schema, schema_root)?;

    // Re-derive the ancestor set (RE-BUILD + RE-VERIFY the closure when the child's class inherits).
    let ancestor_set = if summarize(&class_rows).has_inherits {
        let closure_compiled = ancestor_closure_bgp(class_iri)?;
        let closure = witness.ancestor_closure.as_ref().ok_or_else(|| {
            invalid("inheriting link witness is missing its ancestor closure")
        })?;
        resolve_closure_set(closure, &closure_compiled, instance, schema, database_root)?
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
        let node_compiled = bound_subject_bgp(node_iri)?;
        let rows =
            resolve_subject_rows(&classification.schema_bgp, &node_compiled, schema, schema_root)?;
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
fn verify_collection_elements<I: VerifierLayer, S: VerifierLayer>(
    predicate: &[u8],
    elements: &[CanonicalObject],
    unfold: bool,
    collection_map: &BTreeMap<Vec<u8>, &ProvedCollectionProperty>,
    used_collections: &mut BTreeSet<Vec<u8>>,
    instance: &I,
    schema: &S,
    instance_root: ProofHash,
    schema_root: ProofHash,
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
        let (_child_class, child_natures) = verify_child_witness(
            witness,
            &child_id,
            instance,
            schema,
            instance_root,
            schema_root,
            database_root,
        )?;
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
fn verify_node<I: VerifierLayer, S: VerifierLayer>(
    node: &ProvedReadDocument,
    id: &str,
    instance: &I,
    schema: &S,
    instance_root: ProofHash,
    schema_root: ProofHash,
    database_root: ProofHash,
    allow_subdocument: bool,
    depth: usize,
    visited: &mut BTreeSet<Vec<u8>>,
    node_count: &mut usize,
) -> Result<(ResolvedDocument, ClosureNatures), QueryProofError> {
    // (2) COMPLETE direct triples — reconstruct the plan from `id`, verify over the instance root.
    let subject_compiled = bound_subject_bgp(id)?;
    let d_rows =
        resolve_subject_rows(&node.subject_bgp, &subject_compiled, instance, instance_root)?;

    // (F0) Derive C from the authenticated rdf:type row (the instance-C == schema-C tie).
    let class = extract_class(&d_rows)?;
    let class_iri =
        std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;

    // (F1) C's OWN schema frame.
    let class_compiled = bound_subject_bgp(class_iri)?;
    let class_rows =
        resolve_subject_rows(&node.frame.class_schema, &class_compiled, schema, schema_root)?;

    // (F1b) Re-derive the ancestor set (RE-BUILD + RE-VERIFY the closure when C inherits). A prover can
    // neither drop a genuine closure nor inject a spurious one, nor shorten the set (empty-delta
    // fixpoint).
    let ancestor_set = if summarize(&class_rows).has_inherits {
        let closure_compiled = ancestor_closure_bgp(class_iri)?;
        let closure = node.frame.ancestor_closure.as_ref().ok_or_else(|| {
            invalid("class bears a sys:inherits edge but the bundle omits the ancestor closure")
        })?;
        resolve_closure_set(closure, &closure_compiled, instance, schema, database_root)?
    } else {
        if node.frame.ancestor_closure.is_some() {
            return Err(invalid(
                "class bears no sys:inherits edge but the bundle carries an ancestor closure",
            ));
        }
        vec![class.clone()]
    };

    // (5) Root-verify every schema read (strict ancestors + property ranges/elements) into a trusted
    //     node -> rows map.
    let mut range_map: BTreeMap<Vec<u8>, SubjectRows> = BTreeMap::new();
    for classification in &node.frame.ranges {
        let node_iri = std::str::from_utf8(&classification.node)
            .map_err(|_| invalid("range schema node IRI is not valid UTF-8"))?;
        let node_compiled = bound_subject_bgp(node_iri)?;
        let rows =
            resolve_subject_rows(&classification.schema_bgp, &node_compiled, schema, schema_root)?;
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
                let walk_compiled = ordered_list_walk_bgp(id.as_bytes(), predicate);
                let columns =
                    list_property.walk.verify_and_resolve(&walk_compiled, instance, instance_root)?;
                let elements = columns.into_iter().next().unwrap_or_default();
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
                    instance_root,
                    schema_root,
                    database_root,
                )?;
                rendered.push((predicate.clone(), ResolvedValue::List(elements)));
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
                                instance_root,
                                schema_root,
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
                                instance_root,
                                schema_root,
                                database_root,
                            )?;
                            // The prover chose STOP; force the child to NOT be a subdocument (else it is
                            // an OMISSION — the reference would inline the child's contents).
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
                                    instance_root,
                                    schema_root,
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
                    instance_root,
                    schema_root,
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
        let instance_root = instance.verifier_view().state.commitment_root;
        let schema_root = schema.verifier_view().state.commitment_root;

        // The ROOT node: `visited` seeds with the root id, `node_count` counts the root, and
        // `allow_subdocument = false` so a subdocument top-level class reject-closes. The budget is the
        // IDENTICAL one the prover enforced.
        let mut visited: BTreeSet<Vec<u8>> = BTreeSet::new();
        visited.insert(compiled.id.as_bytes().to_vec());
        let mut node_count = 1usize;
        let (resolved, _natures) = verify_node(
            self,
            &compiled.id,
            instance,
            schema,
            instance_root,
            schema_root,
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

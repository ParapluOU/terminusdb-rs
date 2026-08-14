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
//! ## v1 scope / reject boundary
//!
//! Accept a bound-node `id` whose class has no inheritance and whose direct properties are scalar,
//! link (non-subdocument), `Set` (no cardinality), or `List` (non-subdocument element). Reject-close:
//! variable id, inherited frame, subdocument / array / optional / cardinality / oneOf / enum / table
//! range, `rdf:langString`, an instance property absent from the frame, and a list property with != 1
//! head edge. Compiler-only; nothing under `terminusdb-store-paraplu` changes.

use super::*;

use std::collections::{BTreeMap, BTreeSet};

use crate::document::ReadDocument;
use terminus_store::proof::commitment::validate_database_root;

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
/// direct properties, and one classification sub-proof per non-primitive range/element node.
#[derive(Debug)]
pub struct ProvedFrame {
    /// The prover's `C` (informational). The verifier DERIVES `C` from the root-authenticated instance
    /// `rdf:type` row and uses ITS derived `C` to reconstruct the frame plan — it never trusts this.
    pub class: Vec<u8>,
    /// `Triple(C, ?p, ?r)` proved over the schema commitment.
    pub class_schema: ProvedBgp,
    /// One entry per distinct schema node the classification reads (property ranges + element classes,
    /// excluding primitives).
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

/// A proven `ReadDocument` bundle. The verifier reconstructs every sub-plan from the query `id` +
/// the authenticated frame and verifies each sub-proof against the trusted combined `database_root`,
/// then runs the completeness / no-injection cross-checks.
#[derive(Debug)]
pub struct ProvedReadDocument {
    /// The prover's `Triple(id, ?p, ?o)` plan (informational). Verify reconstructs its own copy from
    /// the query `id` and never trusts this one.
    pub subject_compiled: CompiledBgp,
    /// The instance bound-subject BGP proof — the COMPLETE direct triples of the document.
    pub subject_bgp: ProvedBgp,
    /// The authenticated class frame.
    pub frame: ProvedFrame,
    /// One walk per PRESENT list property.
    pub lists: Vec<ProvedListProperty>,
}

/// A rendered property value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedValue {
    /// A scalar / link / set property: the resolved objects the bound-subject BGP delivered for this
    /// predicate (a scalar is one literal, a link is one node, a set is many — all root-authenticated,
    /// unordered, correct-multiplicity).
    Direct(Vec<CanonicalObject>),
    /// A list property: the ordered element sequence from the authenticated `OrderedListWalk` (order
    /// AND duplicate multiplicity preserved).
    List(Vec<CanonicalObject>),
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

/// Summary of a schema node's direct triples used by classification.
#[derive(Default)]
struct NodeInfo {
    rdf_types: Vec<Vec<u8>>,
    sys_class: Option<CanonicalObject>,
    has_subdocument: bool,
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
    if info.has_subdocument {
        return Err(BgpPlanError::ReadDocumentSubdocumentDeferred.into());
    }
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
        // A link to an inheriting class MIGHT inherit `sys:subdocument`; without the (deferred)
        // inheritance closure we cannot rule that out, so reject-close conservatively.
        if info.has_inherits {
            return Err(shape_reject("inheriting-link-target"));
        }
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
    if info.has_subdocument {
        return Err(BgpPlanError::ReadDocumentSubdocumentDeferred.into());
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

/// Read + classify the class frame from `C`'s direct schema triples (F1-F3). Rejects an inherited
/// frame (`sys:inherits`), a tagged-union (`sys:oneOf`), a subdocument class, and a non-`sys:Class`
/// class; returns the accepted `(predicate, shape)` property list. Each property's range is classified
/// through `reader` (which records/looks-up the range sub-proofs).
fn classify_frame(
    class_rows: &SubjectRows,
    reader: &mut dyn SchemaReader,
) -> Result<Vec<(Vec<u8>, RangeShape)>, QueryProofError> {
    let info = summarize(class_rows);
    // F3 + class sanity: the document class must be a plain `sys:Class` with no inheritance.
    if !info.has_type(SYS_CLASS_TYPE) {
        return Err(shape_reject("class-not-sys-class"));
    }
    if info.has_type(SYS_TAGGED_UNION) || info.has_type(SYS_ENUM) {
        return Err(shape_reject("tagged-union-or-enum-class"));
    }
    if info.has_inherits {
        return Err(BgpPlanError::ReadDocumentInheritedFrame.into());
    }
    if info.has_oneof {
        return Err(shape_reject("tagged-union"));
    }
    if info.has_subdocument {
        return Err(BgpPlanError::ReadDocumentSubdocumentDeferred.into());
    }
    let mut properties: Vec<(Vec<u8>, RangeShape)> = Vec::new();
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    for (predicate, range) in class_rows {
        if is_built_in(predicate) {
            continue;
        }
        if !seen.insert(predicate.clone()) {
            return Err(shape_reject("duplicate-property"));
        }
        let shape = classify_range(range, reader)?;
        properties.push((predicate.clone(), shape));
    }
    Ok(properties)
}

// ---------------------------------------------------------------------------------------------------
// Prove.
// ---------------------------------------------------------------------------------------------------

impl CompiledReadDocument {
    /// Prove the document over the two committed graphs (instance + schema). Reads the layers to
    /// discover `C` / the properties / the list heads, proves each sub-proof by direct construction,
    /// and assembles the bundle. Unsupported shapes reject-close here (fail-closed prover).
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

        // (D) The COMPLETE direct triples of the document (instance graph).
        let subject_compiled = bound_subject_bgp(&self.id)?;
        let subject_bgp = subject_compiled.prove_with_context(ctx, instance)?;
        let d_rows = resolve_subject_rows(&subject_bgp, &subject_compiled, instance, instance_root)?;

        // (F0) Bind C.
        let class = extract_class(&d_rows)?;
        let class_iri =
            std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;

        // (F1) C's direct schema properties.
        let class_compiled = bound_subject_bgp(class_iri)?;
        let class_schema = class_compiled.prove_with_context(ctx, schema)?;
        let class_rows =
            resolve_subject_rows(&class_schema, &class_compiled, schema, schema_root)?;

        // (F2-F3) Classify the frame; this proves each range/element schema BGP as a side effect.
        let mut reader = ProveReader {
            ctx,
            schema,
            schema_root,
            cache: BTreeMap::new(),
            ranges: Vec::new(),
        };
        let properties = classify_frame(&class_rows, &mut reader)?;
        let ranges = reader.ranges;

        // One ordered-list walk per PRESENT list property (a list absent from the instance is simply
        // not in the document — render nothing, prove no walk).
        let mut head_counts: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
        for (predicate, _object) in &d_rows {
            *head_counts.entry(predicate.clone()).or_default() += 1;
        }
        let mut lists = Vec::new();
        for (predicate, shape) in &properties {
            if *shape != RangeShape::List {
                continue;
            }
            match head_counts.get(predicate).copied().unwrap_or(0) {
                0 => continue,
                1 => {}
                _ => {
                    return Err(BgpPlanError::ReadDocumentMalformedListHead {
                        predicate: iri_string(predicate),
                    }
                    .into())
                }
            }
            let walk_compiled = ordered_list_walk_bgp(self.id.as_bytes(), predicate);
            let walk = walk_compiled.prove_with_context(ctx, instance)?;
            lists.push(ProvedListProperty {
                predicate: predicate.clone(),
                walk,
            });
        }

        Ok(ProvedReadDocument {
            subject_compiled,
            subject_bgp,
            frame: ProvedFrame {
                class,
                class_schema,
                ranges,
            },
            lists,
        })
    }
}

// ---------------------------------------------------------------------------------------------------
// Verify + resolve.
// ---------------------------------------------------------------------------------------------------

impl ProvedReadDocument {
    /// Verify every sub-proof against the trusted combined `database_root`, RE-DERIVE the frame, run
    /// the completeness / no-injection cross-checks, and render the document. The verifier reconstructs
    /// every sub-plan from the query `id` + the authenticated frame — it never trusts a
    /// prover-supplied compiled plan or classification.
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

        // (2) The COMPLETE direct triples — reconstruct the plan from the query id (pure function),
        //     verify over the instance root.
        let subject_compiled = bound_subject_bgp(&compiled.id)?;
        let d_rows =
            resolve_subject_rows(&self.subject_bgp, &subject_compiled, instance, instance_root)?;

        // (3/F0) Derive C from the authenticated rdf:type row (the instance-C == schema-C tie).
        let class = extract_class(&d_rows)?;
        let class_iri =
            std::str::from_utf8(&class).map_err(|_| invalid("class IRI is not valid UTF-8"))?;

        // (4/F1) Reconstruct the frame plan from the DERIVED C; verify over the schema root.
        let class_compiled = bound_subject_bgp(class_iri)?;
        let class_rows =
            resolve_subject_rows(&self.frame.class_schema, &class_compiled, schema, schema_root)?;

        // (5) Root-verify every range/element sub-proof (plan reconstructed from its node IRI, a pure
        //     function) into a trusted node -> rows map.
        let mut range_map: BTreeMap<Vec<u8>, SubjectRows> = BTreeMap::new();
        for classification in &self.frame.ranges {
            let node_iri = std::str::from_utf8(&classification.node)
                .map_err(|_| invalid("range schema node IRI is not valid UTF-8"))?;
            let node_compiled = bound_subject_bgp(node_iri)?;
            let rows = resolve_subject_rows(
                &classification.schema_bgp,
                &node_compiled,
                schema,
                schema_root,
            )?;
            if range_map.insert(classification.node.clone(), rows).is_some() {
                return Err(invalid("bundle carries a duplicate range classification"));
            }
        }

        // (6/F2-F3) RE-DERIVE the classification from the authenticated frame + verified range map.
        let mut reader = VerifyReader {
            map: &range_map,
            used: BTreeSet::new(),
        };
        let properties = classify_frame(&class_rows, &mut reader)?;
        let used_nodes = reader.used;
        // No injected/unused range sub-proof may ride along.
        for node in range_map.keys() {
            if !used_nodes.contains(node) {
                return Err(invalid(format!(
                    "bundle carries an unused (injected) range classification for {}",
                    iri_string(node)
                )));
            }
        }

        let shape_of: BTreeMap<Vec<u8>, RangeShape> = properties.into_iter().collect();

        // Group the document's direct objects by predicate (excluding rdf:type — that binds C).
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
        for list_property in &self.lists {
            if walk_map
                .insert(list_property.predicate.clone(), list_property)
                .is_some()
            {
                return Err(invalid("bundle carries a duplicate list-walk sub-proof"));
            }
        }
        let mut used_walks: BTreeSet<Vec<u8>> = BTreeSet::new();

        // Render each present property. LIST → the authenticated ordered walk (exactly one head edge);
        // everything else → the root-authenticated direct objects.
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
                    let walk_compiled = ordered_list_walk_bgp(compiled.id.as_bytes(), predicate);
                    let columns = list_property.walk.verify_and_resolve(
                        &walk_compiled,
                        instance,
                        instance_root,
                    )?;
                    let elements = columns.into_iter().next().unwrap_or_default();
                    rendered.push((predicate.clone(), ResolvedValue::List(elements)));
                }
                _ => {
                    rendered.push((predicate.clone(), ResolvedValue::Direct(objects.clone())));
                }
            }
        }

        // No injected list walk (for an absent or non-list predicate).
        for predicate in walk_map.keys() {
            if !used_walks.contains(predicate) {
                return Err(BgpPlanError::ReadDocumentMalformedListHead {
                    predicate: iri_string(predicate),
                }
                .into());
            }
        }

        Ok(ResolvedDocument {
            id: compiled.id.clone().into_bytes(),
            class,
            properties: rendered,
        })
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

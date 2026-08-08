//! Compile a WOQL conjunctive BGP AST into a safe, variable-blind [`LayerBgpPlan`].
//!
//! See the crate docs for why the PLAN is the sole safety line. This module accepts
//! only the fragment the single-key left-deep join chain can encode *faithfully* and
//! rejects everything else closed:
//!
//! * zero or more nested outer [`Select`]s (projection is read-side only — the proof
//!   still proves the FULL BGP), wrapping
//! * an [`And`] (nested `And`s flattened) of `k >= 1` [`Triple`]s, each a
//!   `<subj> <constant predicate IRI> <obj>` atom where the predicate is a constant IRI
//!   and each of subject/object is EITHER a query VARIABLE (a kept output column) OR an
//!   aligned CONSTANT — a subject/object node IRI ([`AtomTerm::NodeConstant`]) or an
//!   object typed literal of a round-trip-aligned xsd datatype ([`AtomTerm::ValueConstant`]:
//!   all scalar `XSDAnySimpleType` variants except `hexBinary`, routed through the Store's
//!   typed-value construction). Each atom keeps one output column per variable (S,O order);
//!   the atoms join into a left-deep chain where every step shares EXACTLY ONE variable
//!   with the evolving accumulator schema.
//!
//! A composite/cyclic key (a step sharing >= 2 variables), a repeated variable inside
//! one atom, a disconnected step, or a fully-ground atom (no variables) are exactly the
//! shapes that would make the single-key chain prove a wrong-but-verifying superset (or
//! are out of v1 scope) — so they are rejected. Every emitted constant is one the store
//! can authenticate against its committed node/value dictionary, canonicalized IDENTICALLY
//! to how the store stored it (node IRIs by their IRI bytes; `xsd:string` literals by
//! routing through the store's own value construction).

use chrono::Datelike;
use dory_pcs::backends::arkworks::{ArkFr, ArkGT};
use dory_pcs::primitives::arithmetic::Field;
use tdb_succinct::{AnyURI, Date, TdbDataType, TypedDictEntry};
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::query::{
    prove_layer_bgp, verify_layer_bgp, AtomTerm, LayerBgpAtom, LayerBgpPlan, LayerBgpProof,
    ObjectRangeSpec,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject, ProofHash};
use terminus_store::store::sync::SyncStoreLayer;
use terminus_store::ValueTriple;

use crate::query::Query;
use crate::triple::{Data, Link, Triple};
use crate::value::{DataValue, NodeValue, Value};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

/// A compiled, variable-blind BGP plan plus the metadata a caller needs to read the
/// proof's result columns.
#[derive(Clone, Debug)]
pub struct CompiledBgp {
    /// The variable-blind plan handed to `prove_layer_bgp` / `verify_layer_bgp`.
    pub plan: LayerBgpPlan,
    /// Result column index -> BGP variable name, in the proof's output order
    /// (`[r_other.., key, s_other..]` folded over the join steps).
    pub schema: Vec<String>,
    /// If a `Select` wrapped the BGP, the selected variables' column indices into
    /// `schema`. The proof still proves the FULL BGP; projection is which output
    /// columns the caller reads (sound — a projection of the true answer).
    pub projection: Option<Vec<usize>>,
    /// Query-routing metadata retained from outer WOQL wrappers. The cryptographic
    /// statement is anchored to the layer root supplied by the caller; integrations
    /// must use this collection when resolving that layer.
    pub collection: Option<String>,
    /// An outer WOQL `Count` binding. The proven BGP result length is the count, so no
    /// additional cryptographic argument is required.
    pub count_variable: Option<String>,
}

/// An in-memory proof result produced only when a caller explicitly invokes
/// [`CompiledBgp::prove`] or [`CompiledBgp::prove_on_layer`]. It intentionally contains
/// no scheduling or persistence policy; applications own when and where proving runs.
#[derive(Debug)]
pub struct ProvedBgp {
    pub proof: LayerBgpProof,
    pub result_commitments: Vec<ArkGT>,
    pub result_len: usize,
    pub predicate_ids: Vec<u64>,
}

impl ProvedBgp {
    /// Verify against a caller-selected, independently trusted root.
    pub fn verify(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<()> {
        verify_layer_bgp(
            layer,
            expected_root,
            &compiled.plan,
            &self.predicate_ids,
            &self.result_commitments,
            self.result_len,
            &self.proof,
        )
    }

    /// Commitments selected by an outer WOQL `Select`, or every result commitment when
    /// the query has no projection.
    pub fn projected_commitments<'a>(
        &'a self,
        compiled: &'a CompiledBgp,
    ) -> impl Iterator<Item = &'a ArkGT> + 'a {
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.result_commitments.len()).collect());
        projection
            .into_iter()
            .map(|index| &self.result_commitments[index])
    }

    /// Return the variable/value pair bound by an outer WOQL `Count`, if present.
    pub fn count_binding<'a>(&self, compiled: &'a CompiledBgp) -> Option<(&'a str, usize)> {
        compiled
            .count_variable
            .as_deref()
            .map(|variable| (variable, self.result_len))
    }
}

/// Errors at the explicit compile/load/prove boundary.
#[derive(Debug, thiserror::Error)]
pub enum QueryProofError {
    #[error(transparent)]
    Plan(#[from] BgpPlanError),
    #[error(transparent)]
    Store(#[from] std::io::Error),
    #[error("the selected layer has no authenticated proof commitment")]
    LayerNotProofEnabled,
}

impl CompiledBgp {
    /// Generate a proof against an already loaded authenticated commitment.
    pub fn prove(&self, layer: &LayerCommitment) -> std::io::Result<ProvedBgp> {
        let (proof, result_commitments, result_len, predicate_ids) =
            prove_layer_bgp(layer, &self.plan)?;
        Ok(ProvedBgp {
            proof,
            result_commitments,
            result_len,
            predicate_ids,
        })
    }

    /// Explicitly load the commitment from a synchronous Store layer and prove. Calling
    /// this method is the lifecycle decision; no worker or background job is started.
    pub fn prove_on_layer(&self, layer: &SyncStoreLayer) -> Result<ProvedBgp, QueryProofError> {
        let commitment = layer
            .proof_commitment()?
            .ok_or(QueryProofError::LayerNotProofEnabled)?;
        Ok(self.prove(&commitment)?)
    }
}

/// Convenience for app/domain callers that want one explicit compile-and-prove call.
pub fn compile_and_prove(
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    let compiled = plan_bgp(query)?;
    let proof = compiled.prove_on_layer(layer)?;
    Ok((compiled, proof))
}

/// Every reason [`plan_bgp`] refuses a query. One variant per rejection reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BgpPlanError {
    /// A construct that is not a supported proof wrapper, triple pattern, conjunction,
    /// or comparison appeared in the query body.
    #[error(
        "unsupported construct `{construct}`: only a conjunction (And) of triples is supported"
    )]
    UnsupportedConstruct { construct: &'static str },

    /// A conjunct inside the `And` was neither a `Triple` (or nested `And`) nor a numeric
    /// comparison (`Greater`/`Less`/`Gte`/`Lte`, routed to object-value range filters).
    #[error("unsupported conjunct `{construct}`: every conjunct must be a Triple or a numeric comparison")]
    ConjunctNotTriple { construct: &'static str },

    /// A comparison conjunct had a VARIABLE on both sides (`?a op ?b`): a range filter
    /// needs exactly one variable and one numeric constant.
    #[error("range comparison has a variable on both sides: exactly one side must be a numeric constant")]
    RangeBothVariables,

    /// A comparison conjunct had a CONSTANT on both sides (no variable to range over).
    #[error("range comparison has no variable: exactly one side must be the compared variable")]
    RangeBothConstants,

    /// A comparison's constant is not a round-trip-aligned order-preserving numeric literal.
    /// Supported: `xsd:boolean` (`Boolean`), `xsd:unsignedInt` (`UInt32`), `xsd:integer`
    /// (`Int64`), `xsd:float`/`double` (`Float64`). Strings, decimals, other datatypes, and
    /// list operands reject closed — a range over a non-order-preserving encoding would
    /// compare a sentinel and is unsound at the query level.
    #[error(
        "unsupported range constant `{datatype}`: no round-trip-aligned order-preserving encoding"
    )]
    UnsupportedRangeDatatype { datatype: &'static str },

    /// A ranged variable is not the object of any atom, so there is no object-binding atom
    /// to attach the range to.
    #[error("range variable `?{var}` is not the object of any atom")]
    RangeVariableNotObject { var: String },

    /// A ranged variable also occupies a SUBJECT position. v1 attaches a range only to a
    /// variable that is purely an atom's object (never a join key on the subject side).
    #[error(
        "range variable `?{var}` appears as a subject: only a pure object variable can be ranged"
    )]
    RangeVariableIsSubject { var: String },

    /// A ranged variable is the object of MORE THAN ONE atom. v1 requires a unique
    /// object-binding atom to attach the range to.
    #[error("range variable `?{var}` is the object of more than one atom")]
    RangeVariableObjectOfMultiple { var: String },

    /// More than one range filter targets the same variable. v1 supports at most one range
    /// per variable (the store's `ObjectRangeSpec` carries a single operator).
    #[error("more than one range filter on variable `?{var}` (at most one is supported)")]
    DuplicateRangeOnVariable { var: String },

    /// No triple atoms remain after normalization.
    #[error("BGP needs at least one atom, found {count}")]
    TooFewAtoms { count: usize },

    /// The predicate position held a variable. Predicate variables are unsupported.
    #[error("predicate variables unsupported: the predicate must be a constant IRI")]
    PredicateNotConstant,

    /// The object position held a compound value the store's node/value dictionary does
    /// not model as a single constant (a `Dictionary` or `List`). Node IRIs and aligned
    /// typed literals are accepted; these are not.
    #[error(
        "unsupported object value `{kind}`: only node IRIs and aligned typed literals are allowed"
    )]
    UnsupportedObjectValue { kind: &'static str },

    /// The object was a typed literal whose xsd datatype is not one this compiler can
    /// round-trip-align with the store's value canonicalization (only the verified subset
    /// is emitted; every other datatype rejects closed rather than authenticate an
    /// unproven constant). See CP3.
    #[error("unsupported value datatype `{datatype}`: no round-trip-aligned encoding")]
    UnsupportedValueDatatype { datatype: &'static str },

    /// An atom had no variables at all (both subject and object are constants). A
    /// fully-ground atom is an existence check, not a join relation — out of v1 scope
    /// (the store rejects it too).
    #[error("fully-ground atom (no variables): existence checks are unsupported")]
    GroundAtom,

    /// The triple targeted the schema graph.
    #[error("schema-graph triples unsupported: only the instance graph is queried")]
    SchemaGraphUnsupported,

    /// A single atom used the same variable in subject and object (`t(?x, P, ?x)`),
    /// a self-loop the single-key relation cannot express.
    #[error(
        "repeated variable within atom `?{var}`: single-key relations cannot express a self-loop"
    )]
    RepeatedVariableWithinAtom { var: String },

    /// A join step would share two or more variables with the accumulator — a
    /// composite key or a cycle, which the single-key chain cannot encode faithfully.
    #[error("composite-key/cyclic BGP (needs 7.3)")]
    CompositeOrCyclicKey,

    /// A remaining atom shares no variable with the accumulator: the BGP is not
    /// connected, so it is not a single natural-join tree.
    #[error("disconnected BGP")]
    DisconnectedBgp,

    /// A `Select`ed variable does not occur in the BGP body.
    #[error("selected variable `?{var}` does not occur in the BGP")]
    SelectedVariableNotInBgp { var: String },

    /// Count output must be a variable; constant/list outputs need unification semantics
    /// that are not represented by the current result metadata.
    #[error("Count output must be a variable")]
    CountOutputNotVariable,
}

/// The name of a `Query` variant, for descriptive rejection messages. Kept tiny and
/// covering the constructs a caller is most likely to hit.
fn query_variant_name(q: &Query) -> &'static str {
    match q {
        Query::And(_) => "And",
        Query::Or(_) => "Or",
        Query::Not(_) => "Not",
        Query::True(_) => "True",
        Query::Triple(_) => "Triple",
        Query::AddTriple(_) => "AddTriple",
        Query::DeleteTriple(_) => "DeleteTriple",
        Query::Link(_) => "Link",
        Query::Data(_) => "Data",
        Query::Eval(_) => "Eval",
        Query::Path(_) => "Path",
        Query::Equals(_) => "Equals",
        Query::Less(_) => "Less",
        Query::Greater(_) => "Greater",
        Query::Gte(_) => "Gte",
        Query::Lte(_) => "Lte",
        Query::Select(_) => "Select",
        Query::Distinct(_) => "Distinct",
        Query::If(_) => "If",
        Query::WoqlOptional(_) => "Optional",
        Query::Once(_) => "Once",
        Query::OrderBy(_) => "OrderBy",
        Query::GroupBy(_) => "GroupBy",
        Query::Limit(_) => "Limit",
        Query::Start(_) => "Start",
        Query::Count(_) => "Count",
        Query::Sum(_) => "Sum",
        Query::Member(_) => "Member",
        Query::Get(_) => "Get",
        _ => "unsupported",
    }
}

/// A per-atom relation, ready for the variable-arity planner. `layer_atom` is the
/// variable-blind atom emitted into the plan (subject/predicate/object terms). `kept`
/// is the atom's VARIABLE names in S,O order — the relation's output columns, one per
/// variable. An all-variable atom keeps two columns `[subj_var, obj_var]`; a
/// subject/object-constant atom keeps one. The planner joins on shared `kept` names and
/// `rel_key` indexes into `kept`.
struct Atom {
    layer_atom: LayerBgpAtom,
    kept: Vec<String>,
    /// The subject variable name, if the subject is a variable (else the subject is a
    /// constant). Used to route comparison conjuncts to the right object-binding atom.
    subj_var: Option<String>,
    /// The object variable name, if the object is a variable (else a constant). A numeric
    /// range attaches to the unique atom whose `obj_var` is the compared variable.
    obj_var: Option<String>,
}

/// Compile a woql2 conjunctive BGP query into a safe [`CompiledBgp`].
///
/// Accepts nested outer [`Select`]s and supported routing/result wrappers around an
/// `And` of `>= 1` triples of
/// the form `?subj <constant IRI> ?obj` (`subj != obj`), whose atoms connect into a
/// single-key left-deep join tree (each step shares exactly one variable). Everything
/// else is rejected closed with a descriptive [`BgpPlanError`].
pub fn plan_bgp(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    // (1) Unwrap proof-transparent outer controls. `Using` is not discarded: it is
    // retained as routing metadata so an integration can resolve the intended layer.
    // `From(instance)` is safe for the instance-layer proof path; schema or wildcard
    // reads reject closed. `Pin` affects optimization order, not query semantics.
    // Traversal is outer-to-inner. The bool records whether the Select is outside a
    // Count (and therefore selects the Count binding) or inside it (and therefore
    // projects the counted relation before its cardinality is taken).
    let mut selections: Vec<(&[String], bool)> = Vec::new();
    let mut collection: Option<String> = None;
    let mut count_variable: Option<String> = None;
    let mut body = query;
    loop {
        body = match body {
            Query::Select(select) => {
                selections.push((select.variables.as_slice(), count_variable.is_none()));
                select.query.as_ref()
            }
            Query::Using(using) if collection.is_none() => {
                collection = Some(using.collection.clone());
                using.query.as_ref()
            }
            Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => from.query.as_ref(),
            Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
            Query::Pin(pin) => pin.query.as_ref(),
            Query::Count(count) if count_variable.is_none() => {
                count_variable = match &count.count {
                    DataValue::Variable(variable) => Some(variable.clone()),
                    _ => return Err(BgpPlanError::CountOutputNotVariable),
                };
                count.query.as_ref()
            }
            _ => break,
        };
    }

    if !matches!(
        body,
        Query::And(_) | Query::Triple(_) | Query::Link(_) | Query::Data(_)
    ) {
        return Err(BgpPlanError::UnsupportedConstruct {
            construct: query_variant_name(body),
        });
    }

    // (1b) Flatten nested Ands. Triple/Link/Data share one canonical Triple
    // representation here; comparisons become object-value range filters. True and
    // Comment contribute no bindings and are proof-transparent conjunction leaves.
    let mut triples: Vec<Triple> = Vec::new();
    let mut comparisons: Vec<&Query> = Vec::new();
    collect_conjunct(body, &mut triples, &mut comparisons)?;

    // (2) Lift each triple to an Atom, with all per-atom safety checks.
    let mut atoms: Vec<Atom> = triples
        .iter()
        .map(atom_from_triple)
        .collect::<Result<_, _>>()?;

    let k = atoms.len();
    if k == 0 {
        return Err(BgpPlanError::TooFewAtoms { count: k });
    }

    // (2b) Phase 7.4: route each comparison conjunct to a numeric RANGE filter on the
    // object of the (unique) atom that binds the compared variable. Consumes the
    // comparisons; the ranged variable stays a kept output column.
    attach_ranges(&mut atoms, &comparisons)?;

    // (3) Shape guard + left-deep planner: the safety-critical core.
    let (plan, schema) = plan_left_deep(&atoms)?;

    // (4) Validate every nested projection against what its inner query exposes. The
    // proof still authenticates the complete BGP; only the outermost visible Select
    // determines which committed columns the caller reads.
    let projection = if let Some(count) = count_variable.as_deref() {
        let outside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| outside.then_some(*vars))
            .collect();
        let inside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| (!outside).then_some(*vars))
            .collect();
        validate_selection_chain(&inside, &schema)?;
        validate_selection_chain(&outside, &[count.to_owned()])?;
        Some(Vec::new())
    } else if selections.is_empty() {
        None
    } else {
        let selections: Vec<&[String]> = selections.iter().map(|(vars, _)| *vars).collect();
        validate_selection_chain(&selections, &schema)?;
        Some(
            selections[0]
                .iter()
                .map(|var| {
                    schema
                        .iter()
                        .position(|candidate| candidate == var)
                        .expect("validated outer projection must occur in the BGP schema")
                })
                .collect(),
        )
    };

    Ok(CompiledBgp {
        plan,
        schema,
        projection,
        collection,
        count_variable,
    })
}

/// Validate nested Selects listed outer-to-inner. Each Select may only name variables
/// exposed by its directly nested query; the innermost Select reads from `base`.
fn validate_selection_chain(selections: &[&[String]], base: &[String]) -> Result<(), BgpPlanError> {
    let mut available = base;
    for selected in selections.iter().rev() {
        for var in *selected {
            if !available.iter().any(|candidate| candidate == var) {
                return Err(BgpPlanError::SelectedVariableNotInBgp { var: var.clone() });
            }
        }
        available = selected;
    }
    Ok(())
}

/// Flatten a query body recursively, partitioning leaves into canonical triple patterns
/// and comparisons. Any other construct rejects closed.
fn collect_conjunct<'a>(
    query: &'a Query,
    triples: &mut Vec<Triple>,
    comparisons: &mut Vec<&'a Query>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::Triple(triple) => triples.push(triple.clone()),
        Query::Link(link) => triples.push(triple_from_link(link)),
        Query::Data(data) => triples.push(triple_from_data(data)),
        Query::And(inner) => {
            for query in &inner.and {
                collect_conjunct(query, triples, comparisons)?;
            }
        }
        Query::Greater(_) | Query::Less(_) | Query::Gte(_) | Query::Lte(_) => {
            comparisons.push(query)
        }
        Query::True(_) | Query::Comment(_) => {}
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

fn triple_from_link(link: &Link) -> Triple {
    Triple {
        subject: link.subject.clone(),
        predicate: link.predicate.clone(),
        object: match &link.object {
            NodeValue::Node(node) => Value::Node(node.clone()),
            NodeValue::Variable(var) => Value::Variable(var.clone()),
        },
        graph: Some(link.graph.clone()),
    }
}

fn triple_from_data(data: &Data) -> Triple {
    Triple {
        subject: data.subject.clone(),
        predicate: data.predicate.clone(),
        object: value_from_data_value(&data.object),
        graph: Some(data.graph.clone()),
    }
}

fn value_from_data_value(value: &DataValue) -> Value {
    match value {
        DataValue::Data(value) => Value::Data(value.clone()),
        DataValue::Variable(var) => Value::Variable(var.clone()),
        DataValue::List(values) => Value::List(values.iter().map(value_from_data_value).collect()),
    }
}

/// Lift one `Triple` to an [`Atom`]: the predicate must be a constant IRI; the subject
/// and object are each either a query VARIABLE (a kept output column) or an aligned
/// CONSTANT (emitted as an [`AtomTerm`] the store authenticates against its node/value
/// dictionary); and the graph must be the instance graph.
///
/// Accepted constants:
/// * subject/object node IRIs → [`AtomTerm::NodeConstant`] (CP2);
/// * object typed literals of a round-trip-aligned xsd datatype → [`AtomTerm::ValueConstant`]
///   (CP3, via [`value_constant_term`]).
///
/// Rejected closed: predicate variables, schema-graph triples, dictionary/list object
/// values, unaligned typed-literal datatypes, a repeated variable within one atom, and a
/// fully-ground atom (no variables). A value-literal SUBJECT is unrepresentable — woql2's
/// subject is a [`NodeValue`] (node/variable only) — so the store's `validate_atom`
/// value-subject guard is unreachable through this compiler by construction.
fn atom_from_triple(t: &Triple) -> Result<Atom, BgpPlanError> {
    // Graph: None (defaults to instance) or explicit Instance only.
    match &t.graph {
        None | Some(GraphType::Instance) => {}
        Some(GraphType::Schema) => return Err(BgpPlanError::SchemaGraphUnsupported),
    }

    // Predicate must be a constant IRI.
    let predicate_iri = match &t.predicate {
        NodeValue::Node(iri) => iri.clone(),
        NodeValue::Variable(_) => return Err(BgpPlanError::PredicateNotConstant),
    };

    // Subject: a variable (kept column) or a node-IRI constant.
    let (subject_term, subj_var) = match &t.subject {
        NodeValue::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        NodeValue::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
    };

    // Object: a variable (kept column), a node-IRI constant, or an aligned typed literal.
    let (object_term, obj_var) = match &t.object {
        Value::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        Value::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
        Value::Data(xsd) => (value_constant_term(xsd)?, None),
        Value::Dictionary(_) => {
            return Err(BgpPlanError::UnsupportedObjectValue { kind: "dictionary" })
        }
        Value::List(_) => return Err(BgpPlanError::UnsupportedObjectValue { kind: "list" }),
    };

    // A repeated variable within one atom is a self-loop the single-key relation cannot
    // express (only possible when BOTH positions are variables).
    if let (Some(s), Some(o)) = (&subj_var, &obj_var) {
        if s == o {
            return Err(BgpPlanError::RepeatedVariableWithinAtom { var: s.clone() });
        }
    }

    // Kept (variable) columns in S,O order — one per variable position.
    let mut kept = Vec::with_capacity(2);
    if let Some(s) = &subj_var {
        kept.push(s.clone());
    }
    if let Some(o) = &obj_var {
        kept.push(o.clone());
    }
    // A fully-ground atom (no variables) is an existence check, out of scope.
    if kept.is_empty() {
        return Err(BgpPlanError::GroundAtom);
    }

    Ok(Atom {
        layer_atom: LayerBgpAtom {
            subject: subject_term,
            predicate_term: predicate_iri.into_bytes(),
            object: object_term,
            // CP-D Step 1 (migration): every atom starts with no object range. Step 2
            // fills this in for atoms whose object variable a comparison conjunct ranges.
            object_range: None,
        },
        kept,
        subj_var,
        obj_var,
    })
}

/// Route each numeric comparison conjunct to an object-value RANGE filter on the atom
/// that binds the compared variable. For each comparison:
///
/// 1. [`normalize_comparison`] turns it into `(var, op, datatype, c)`, orienting the
///    operator so `c` is always on the right (`k > ?a` ⇒ `?a < k`), rejecting closed a
///    var-vs-var / const-vs-const comparison or an unsupported constant datatype.
/// 2. v1 constraints (all reject closed): `var` must be the object of EXACTLY ONE atom,
///    must NOT appear as any atom's subject, and each `var` may carry at most one range.
/// 3. The [`ObjectRangeSpec`] attaches to that atom's object (its object stays a kept
///    variable — the range is a filter, not a projection change).
fn attach_ranges(atoms: &mut [Atom], comparisons: &[&Query]) -> Result<(), BgpPlanError> {
    let mut ranged: Vec<String> = Vec::new();
    for cmp in comparisons {
        let (var, op, datatype, c) = normalize_comparison(cmp)?;

        // At most one range per variable.
        if ranged.iter().any(|v| v == &var) {
            return Err(BgpPlanError::DuplicateRangeOnVariable { var });
        }
        // The ranged variable may not occupy a subject position anywhere.
        if atoms
            .iter()
            .any(|a| a.subj_var.as_deref() == Some(var.as_str()))
        {
            return Err(BgpPlanError::RangeVariableIsSubject { var });
        }
        // It must be the object variable of exactly one atom.
        let mut object_of: Vec<usize> = Vec::new();
        for (i, a) in atoms.iter().enumerate() {
            if a.obj_var.as_deref() == Some(var.as_str()) {
                object_of.push(i);
            }
        }
        let idx = match object_of.as_slice() {
            [] => return Err(BgpPlanError::RangeVariableNotObject { var }),
            [i] => *i,
            _ => return Err(BgpPlanError::RangeVariableObjectOfMultiple { var }),
        };

        // The matched atom's object is a Variable by construction (obj_var is Some only
        // for a variable object), so a range never lands on a constant object here.
        debug_assert!(matches!(atoms[idx].layer_atom.object, AtomTerm::Variable));
        atoms[idx].layer_atom.object_range = Some(ObjectRangeSpec {
            op,
            datatype,
            c,
            b_bits: OBJECT_RANGE_B_BITS,
        });
        ranged.push(var);
    }
    Ok(())
}

/// Normalize a numeric comparison conjunct into `(var, op, datatype, c)`, where `op` is
/// oriented so the constant is on the RIGHT (`Greater(k, ?a)` ⇒ `?a < k`), and
/// `(datatype, c)` is the store-aligned encoding of the constant (see
/// [`encode_range_constant`]).
///
/// Rejects closed: both sides variables ([`BgpPlanError::RangeBothVariables`]), neither
/// side a variable ([`BgpPlanError::RangeBothConstants`]), or a constant that is not a
/// supported order-preserving numeric literal ([`BgpPlanError::UnsupportedRangeDatatype`]).
fn normalize_comparison(q: &Query) -> Result<(String, RangeOp, ArkFr, ArkFr), BgpPlanError> {
    // `op_var_left` is the operator when the VARIABLE is the left operand; `op_var_right`
    // is its flip (variable on the right) — the same relation with the sides swapped.
    let (left, right, op_var_left, op_var_right) = match q {
        Query::Greater(g) => (&g.left, &g.right, RangeOp::Gt, RangeOp::Lt),
        Query::Less(l) => (&l.left, &l.right, RangeOp::Lt, RangeOp::Gt),
        Query::Gte(g) => (&g.left, &g.right, RangeOp::Ge, RangeOp::Le),
        Query::Lte(l) => (&l.left, &l.right, RangeOp::Le, RangeOp::Ge),
        // Only comparison variants are collected into `comparisons`.
        _ => unreachable!("normalize_comparison only sees Greater/Less/Gte/Lte"),
    };

    let (var, op, konst): (String, RangeOp, &DataValue) = match (left, right) {
        (DataValue::Variable(_), DataValue::Variable(_)) => {
            return Err(BgpPlanError::RangeBothVariables)
        }
        (DataValue::Variable(v), other) => (v.clone(), op_var_left, other),
        (other, DataValue::Variable(v)) => (v.clone(), op_var_right, other),
        // Neither side is a variable.
        _ => return Err(BgpPlanError::RangeBothConstants),
    };

    let xsd = match konst {
        DataValue::Data(k) => k,
        DataValue::List(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "list" })
        }
        // Excluded above (the variable was the other operand).
        DataValue::Variable(_) => unreachable!(),
    };
    let (datatype, c) = encode_range_constant(xsd)?;
    Ok((var, op, datatype, c))
}

/// Build the store's typed dict entry for a woql2 SCALAR literal, routing each supported
/// xsd datatype through its `terminus_store`/`tdb-succinct` typed-value constructor (the
/// [`TdbDataType::make_entry`] family) so the resulting canonicalization is byte-identical
/// to a stored value of the same literal (no hand-rolled byte layouts — the 7.5c alignment
/// rule). Returns the entry, or the xsd datatype name for a literal this compiler does not
/// support (the caller wraps that name into the right rejection error).
///
/// `xsd:string` is handled separately by the caller via
/// [`ValueTriple::new_string_value`]. `hexBinary` remains unsupported until its textual
/// WOQL form is decoded and round-trip tested against the Store type.
fn xsd_scalar_entry(xsd: &XSDAnySimpleType) -> Result<TypedDictEntry, &'static str> {
    Ok(match xsd {
        XSDAnySimpleType::Boolean(b) => bool::make_entry(b),
        // xsd:unsignedInt is the 32-bit unsigned type → store `UInt32`.
        XSDAnySimpleType::UnsignedInt(u) => {
            let v = u32::try_from(*u).map_err(|_| "unsignedInt (exceeds u32)")?;
            u32::make_entry(&v)
        }
        // The woql2 `Integer` variant carries an `i64` → store `Int64` (fixed-width,
        // order-preserving via offset-binary — so it also supports range).
        XSDAnySimpleType::Integer(i) => i64::make_entry(i),
        // The woql2 `Float` variant carries an `f64` → store `Float64` (8-byte IEEE
        // total-order OP encoding).
        XSDAnySimpleType::Float(f) => f64::make_entry(f),
        XSDAnySimpleType::Decimal(decimal) => {
            let value = tdb_succinct::Decimal::new(decimal.to_string()).map_err(|_| "decimal")?;
            tdb_succinct::Decimal::make_entry(&value)
        }
        XSDAnySimpleType::URI(uri) => AnyURI::make_entry(uri),
        // The WOQL AST carries a UTC DateTime; the Store's xsd:dateTime entry is its
        // timezone-free UTC representation.
        XSDAnySimpleType::DateTime(datetime) => {
            chrono::NaiveDateTime::make_entry(&datetime.naive_utc())
        }
        XSDAnySimpleType::Date(date) => Date::make_entry(&Date {
            year: i64::from(date.year()),
            month: date.month() as u8,
            day: date.day() as u8,
            offset: 0,
        }),
        XSDAnySimpleType::Time(time) => chrono::NaiveTime::make_entry(time),
        other => return Err(xsd_datatype_name(other)),
    })
}

/// Encode a numeric range constant to `(datatype_tag, c)` matching the store's committed
/// node/value dictionary encoding EXACTLY, by routing through the store's OWN value
/// canonicalization (7.5c alignment trick): build the typed value via [`xsd_scalar_entry`]
/// + [`ValueTriple::new_value`], read back its [`CanonicalObject::Value`] via
/// [`CanonicalObject::from_store_object`], take its [`CanonicalDatatype`] discriminant as the
/// datatype tag, and compute `c` as the big-endian unsigned integer of its order-preserving
/// `to_lexical` bytes — the SAME conversion `terminus_store`'s (private) `node_value_op_value`
/// applies to the committed dict value column.
///
/// Supported (round-trip-verified, all order-preserving fixed-width): `xsd:boolean`
/// (`Boolean`), `xsd:unsignedInt` (`UInt32`), `xsd:integer` (`Int64`), `xsd:float`/`double`
/// (`Float64`). Every other `XSDAnySimpleType` rejects closed: a range over a
/// non-order-preserving or variable-width datatype (string/decimal/…) would compare a
/// `0` sentinel in the store, which is unsound at the query level. Adding a datatype
/// requires a store-routed construction in [`xsd_scalar_entry`], an
/// [`op_preserving_width`] entry, PLUS a round-trip alignment test.
fn encode_range_constant(xsd: &XSDAnySimpleType) -> Result<(ArkFr, ArkFr), BgpPlanError> {
    let entry = xsd_scalar_entry(xsd)
        .map_err(|datatype| BgpPlanError::UnsupportedRangeDatatype { datatype })?;
    encode_via_store(entry)
}

/// Canonicalize a `TypedDictEntry` through the store's own value path and derive the
/// `(datatype_tag, c)` pair. Fails closed if the resulting datatype is not one of the
/// round-trip-verified order-preserving fixed-width numerics (see [`op_preserving_width`]).
fn encode_via_store(entry: TypedDictEntry) -> Result<(ArkFr, ArkFr), BgpPlanError> {
    // Subject/predicate are irrelevant to the object canonicalization.
    let triple = ValueTriple::new_value("", "", entry);
    let (datatype, lexical) = match CanonicalObject::from_store_object(&triple.object) {
        CanonicalObject::Value { datatype, lexical } => (datatype, lexical),
        // `new_value` always yields a Value object; fail closed rather than emit an
        // unaligned constant.
        CanonicalObject::Node(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "node" })
        }
    };
    // Guard: only order-preserving fixed-width numerics are sound to range over.
    let width = op_preserving_width(datatype).ok_or(BgpPlanError::UnsupportedRangeDatatype {
        datatype: "non-order-preserving datatype",
    })?;
    if lexical.len() != width {
        return Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: "unexpected lexical width",
        });
    }
    // c = big-endian unsigned integer of the fixed-width OP bytes (matches the store's
    // `node_value_op_value`). Widths are ≤ 8, so it fits a u64.
    let mut buf = [0u8; 8];
    buf[8 - width..].copy_from_slice(&lexical);
    let c = ArkFr::from_u64(u64::from_be_bytes(buf));
    let datatype_tag = ArkFr::from_u64(datatype as u64);
    Ok((datatype_tag, c))
}

/// Fixed byte width of a SUPPORTED order-preserving numeric datatype, or `None` for any
/// datatype whose lexical is variable-width or not order-preserving. Mirrors
/// `terminus_store`'s private `order_preserving_numeric_width` EXACTLY: for each of these
/// the store's `to_lexical` writes exactly `width` bytes whose big-endian unsigned read is
/// monotone in value (`Boolean` 0/1; `(U)Int8/16/32/64` raw-BE / offset-binary;
/// `Float32/64` IEEE total-order), so `node_value_op_value` and this compiler compute the
/// SAME `c`. A range over any other datatype rejects closed (see [`encode_range_constant`]).
fn op_preserving_width(datatype: CanonicalDatatype) -> Option<usize> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Boolean => 1,
        UInt8 | Int8 => 1,
        UInt16 | Int16 => 2,
        UInt32 | Int32 | Float32 => 4,
        UInt64 | Int64 | Float64 => 8,
        _ => return None,
    })
}

/// The xsd datatype name of a woql2 typed literal, for descriptive rejection messages.
fn xsd_datatype_name(xsd: &XSDAnySimpleType) -> &'static str {
    match xsd {
        XSDAnySimpleType::String(_) => "string",
        XSDAnySimpleType::Decimal(_) => "decimal",
        XSDAnySimpleType::Float(_) => "float",
        XSDAnySimpleType::Boolean(_) => "boolean",
        XSDAnySimpleType::HexBinary(_) => "hexBinary",
        XSDAnySimpleType::URI(_) => "anyURI",
        XSDAnySimpleType::DateTime(_) => "dateTime",
        XSDAnySimpleType::Date(_) => "date",
        XSDAnySimpleType::Time(_) => "time",
        XSDAnySimpleType::UnsignedInt(_) => "unsignedInt",
        XSDAnySimpleType::Integer(_) => "integer",
    }
}

/// Emit an [`AtomTerm::ValueConstant`] for an object typed literal — but ONLY for xsd
/// datatypes whose `(CanonicalDatatype, lexical bytes)` this compiler can produce so they
/// canonicalize IDENTICALLY to the store's own encoding of the same literal. Any other
/// datatype rejects closed rather than authenticate a constant whose alignment is unproven
/// (a misaligned `(datatype, lexical)` would `hash_object` to the wrong dictionary row, so
/// membership would fail — or, worse, authenticate the wrong constant).
///
/// Equality does NOT require order-preservation (it is id-based via dictionary membership),
/// so ANY datatype the store can canonicalize is sound here. Supported (round-trip-verified,
/// each routed through the store's OWN value construction so the bytes match BY CONSTRUCTION,
/// no re-implementation of the tdb-succinct encoding):
/// * `xsd:string` via [`ValueTriple::new_string_value`] (the exact path the store uses to
///   store a string value);
/// * every other supported WOQL scalar via [`xsd_scalar_entry`] →
///   [`ValueTriple::new_value`] using its matching `tdb-succinct` typed entry.
///
/// See the `object_*_literal_*` round-trip tests. `hexBinary` remains rejected closed.
fn value_constant_term(xsd: &XSDAnySimpleType) -> Result<AtomTerm, BgpPlanError> {
    // Build the store object via the store's OWN construction path for each datatype. String
    // uses the dedicated string-value constructor; every other supported scalar goes through
    // its typed dict entry. Subject/predicate are irrelevant to the object canonicalization,
    // so empty placeholders are fine.
    let object = match xsd {
        XSDAnySimpleType::String(s) => ValueTriple::new_string_value("", "", s).object,
        _ => {
            let entry = xsd_scalar_entry(xsd)
                .map_err(|datatype| BgpPlanError::UnsupportedValueDatatype { datatype })?;
            ValueTriple::new_value("", "", entry).object
        }
    };
    match CanonicalObject::from_store_object(&object) {
        CanonicalObject::Value { datatype, lexical } => {
            Ok(AtomTerm::ValueConstant(datatype, lexical))
        }
        // Unreachable: a value construction always yields a `Value` object. Fail closed
        // rather than emit an unaligned constant.
        CanonicalObject::Node(_) => Err(BgpPlanError::UnsupportedValueDatatype {
            datatype: xsd_datatype_name(xsd),
        }),
    }
}

/// The safety-critical left-deep planner. Tracks the accumulator schema as an ordered
/// list of distinct variable names and emits atoms (in placement order) + `k-1` join
/// steps. Each step joins the accumulator with an unplaced atom on the SINGLE shared
/// variable; the accumulator schema evolves as `[r_other.., key, s_other..]`, matching
/// `terminus_store`'s `bgp` output ordering exactly.
///
/// Each atom relation carries ONE column per VARIABLE, in S,O order (`atom.kept`): two
/// for an all-variable atom, one for a subject/object-constant atom (a constant removes
/// its position from `kept`). `rel_key` is therefore the shared variable's index within
/// the atom's KEPT columns, and `s_other` is the atom's remaining kept columns (possibly
/// empty for a 1-variable atom).
fn plan_left_deep(atoms: &[Atom]) -> Result<(LayerBgpPlan, Vec<String>), BgpPlanError> {
    let k = atoms.len();

    // Deterministic start atom: input order 0. Its kept variables seed the accumulator.
    let mut acc_vars: Vec<String> = atoms[0].kept.clone();
    let mut placed = vec![false; k];
    placed[0] = true;

    let mut ordered_atoms: Vec<LayerBgpAtom> = vec![atoms[0].layer_atom.clone()];
    let mut joins: Vec<JoinStep> = Vec::with_capacity(k - 1);

    let mut placed_count = 1usize;
    while placed_count < k {
        // Find an unplaced atom sharing EXACTLY ONE variable with the accumulator.
        let mut chosen: Option<usize> = None;
        for (i, atom) in atoms.iter().enumerate() {
            if placed[i] {
                continue;
            }
            if shared_count(&acc_vars, &atom.kept) == 1 {
                chosen = Some(i);
                break;
            }
        }

        let i = match chosen {
            Some(i) => i,
            None => {
                // No atom shares exactly one variable. Distinguish cycle/composite key
                // (some unplaced atom shares >= 2) from a disconnected BGP (shares 0).
                let any_multi = atoms
                    .iter()
                    .enumerate()
                    .any(|(i, atom)| !placed[i] && shared_count(&acc_vars, &atom.kept) >= 2);
                if any_multi {
                    return Err(BgpPlanError::CompositeOrCyclicKey);
                } else {
                    return Err(BgpPlanError::DisconnectedBgp);
                }
            }
        };

        // Join accumulator (schema `acc_vars`) with atom i (kept columns `atom.kept`) on
        // the single shared variable `v`. `rel_key` is `v`'s index within `atom.kept`.
        let atom = &atoms[i];
        let rel_key = atom
            .kept
            .iter()
            .position(|kv| acc_vars.iter().any(|a| a == kv))
            .expect("exactly one kept variable is shared with the accumulator");
        let v = atom.kept[rel_key].clone();
        let acc_key = acc_vars
            .iter()
            .position(|a| a == &v)
            .expect("shared variable is present in the accumulator");

        // r_other = acc_vars with acc_key removed (order preserved).
        let mut r_other: Vec<String> = acc_vars.clone();
        r_other.remove(acc_key);
        // s_other = the atom's remaining kept columns (all but rel_key), order preserved.
        // Empty when the atom has a single variable (its other position is a constant).
        let mut s_other: Vec<String> = atom.kept.clone();
        s_other.remove(rel_key);

        // New accumulator schema: [r_other.., key, s_other..] — MUST match bgp.rs's
        // output_commitments ordering.
        let mut new_acc = r_other;
        new_acc.push(v);
        new_acc.extend(s_other);
        acc_vars = new_acc;

        joins.push(JoinStep { acc_key, rel_key });
        ordered_atoms.push(atom.layer_atom.clone());
        placed[i] = true;
        placed_count += 1;
    }

    Ok((
        LayerBgpPlan {
            atoms: ordered_atoms,
            joins,
        },
        acc_vars,
    ))
}

/// How many of an atom's KEPT (variable) columns occur in the accumulator schema. Because
/// an atom's kept names are distinct (a repeated variable within one atom is rejected) and
/// `acc_vars` holds distinct names, this is an exact shared-variable count in `0..=kept.len()`.
fn shared_count(acc_vars: &[String], kept: &[String]) -> usize {
    kept.iter()
        .filter(|kv| acc_vars.iter().any(|a| a == *kv))
        .count()
}

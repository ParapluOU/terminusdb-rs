//! Compile a woql2 conjunctive BGP AST into a safe, variable-blind [`LayerBgpPlan`].
//!
//! See the crate docs for why the PLAN is the sole safety line. This module accepts
//! only the fragment the single-key left-deep join chain can encode *faithfully* and
//! rejects everything else closed:
//!
//! * an optional single outer [`Select`] (projection is read-side only — the proof
//!   still proves the FULL BGP), wrapping
//! * an [`And`] (nested `And`s flattened) of `k >= 2` [`Triple`]s, each a
//!   `<subj> <constant predicate IRI> <obj>` atom where the predicate is a constant IRI
//!   and each of subject/object is EITHER a query VARIABLE (a kept output column) OR an
//!   aligned CONSTANT — a subject/object node IRI ([`AtomTerm::NodeConstant`]) or an
//!   object typed literal of a round-trip-aligned xsd datatype ([`AtomTerm::ValueConstant`],
//!   currently `xsd:string`). Each atom keeps one output column per variable (S,O order);
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

use dory_pcs::backends::arkworks::ArkFr;
use dory_pcs::primitives::arithmetic::Field;
use tdb_succinct::{TdbDataType, TypedDictEntry};
use terminus_store::proof::bgp::JoinStep;
use terminus_store::proof::query::{AtomTerm, LayerBgpAtom, LayerBgpPlan, ObjectRangeSpec};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject};
use terminus_store::ValueTriple;

use terminusdb_schema::{GraphType, XSDAnySimpleType};
use terminusdb_woql2::query::Query;
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

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
}

/// Every reason [`plan_bgp`] refuses a query. One variant per rejection reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BgpPlanError {
    /// A construct that is not a conjunction-of-triples appeared where the BGP body
    /// was expected (the top-level query after unwrapping an optional `Select`).
    #[error("unsupported construct `{construct}`: only a conjunction (And) of triples is supported")]
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
    /// Only `xsd:unsignedInt` (store `UInt32`) is supported; strings, other datatypes, and
    /// list operands reject closed — a range over a non-order-preserving encoding would
    /// compare a sentinel and is unsound at the query level.
    #[error("unsupported range constant `{datatype}`: no round-trip-aligned order-preserving encoding")]
    UnsupportedRangeDatatype { datatype: &'static str },

    /// A ranged variable is not the object of any atom, so there is no object-binding atom
    /// to attach the range to.
    #[error("range variable `?{var}` is not the object of any atom")]
    RangeVariableNotObject { var: String },

    /// A ranged variable also occupies a SUBJECT position. v1 attaches a range only to a
    /// variable that is purely an atom's object (never a join key on the subject side).
    #[error("range variable `?{var}` appears as a subject: only a pure object variable can be ranged")]
    RangeVariableIsSubject { var: String },

    /// A ranged variable is the object of MORE THAN ONE atom. v1 requires a unique
    /// object-binding atom to attach the range to.
    #[error("range variable `?{var}` is the object of more than one atom")]
    RangeVariableObjectOfMultiple { var: String },

    /// More than one range filter targets the same variable. v1 supports at most one range
    /// per variable (the store's `ObjectRangeSpec` carries a single operator).
    #[error("more than one range filter on variable `?{var}` (at most one is supported)")]
    DuplicateRangeOnVariable { var: String },

    /// Fewer than two atoms. A single triple (k=1) is a documented follow-up via the
    /// per-atom filter path; it is rejected here for now.
    #[error("BGP needs at least two atoms, found {count}")]
    TooFewAtoms { count: usize },

    /// The predicate position held a variable. Predicate variables are unsupported.
    #[error("predicate variables unsupported: the predicate must be a constant IRI")]
    PredicateNotConstant,

    /// The object position held a compound value the store's node/value dictionary does
    /// not model as a single constant (a `Dictionary` or `List`). Node IRIs and aligned
    /// typed literals are accepted; these are not.
    #[error("unsupported object value `{kind}`: only node IRIs and aligned typed literals are allowed")]
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
    #[error("repeated variable within atom `?{var}`: single-key relations cannot express a self-loop")]
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
/// Accepts an optional single outer [`Select`] wrapping an `And` of `>= 2` triples of
/// the form `?subj <constant IRI> ?obj` (`subj != obj`), whose atoms connect into a
/// single-key left-deep join tree (each step shares exactly one variable). Everything
/// else is rejected closed with a descriptive [`BgpPlanError`].
pub fn plan_bgp(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    // (1) Unwrap an optional single outer Select; record its projection variables.
    let (selected, body): (Option<&[String]>, &Query) = match query {
        Query::Select(select) => (Some(select.variables.as_slice()), select.query.as_ref()),
        other => (None, other),
    };

    // The body must be an And (a conjunction).
    let and = match body {
        Query::And(and) => and,
        other => {
            return Err(BgpPlanError::UnsupportedConstruct {
                construct: query_variant_name(other),
            })
        }
    };

    // (1b) Flatten nested Ands; every leaf conjunct must be a Triple or a numeric
    // comparison (Greater/Less/Gte/Lte). Comparisons become object-value range filters.
    let mut triples: Vec<&Triple> = Vec::new();
    let mut comparisons: Vec<&Query> = Vec::new();
    collect_conjuncts(&and.and, &mut triples, &mut comparisons)?;

    // (2) Lift each triple to an Atom, with all per-atom safety checks.
    let mut atoms: Vec<Atom> = triples
        .iter()
        .map(|t| atom_from_triple(t))
        .collect::<Result<_, _>>()?;

    let k = atoms.len();
    if k < 2 {
        return Err(BgpPlanError::TooFewAtoms { count: k });
    }

    // (2b) Phase 7.4: route each comparison conjunct to a numeric RANGE filter on the
    // object of the (unique) atom that binds the compared variable. Consumes the
    // comparisons; the ranged variable stays a kept output column.
    attach_ranges(&mut atoms, &comparisons)?;

    // (3) Shape guard + left-deep planner: the safety-critical core.
    let (plan, schema) = plan_left_deep(&atoms)?;

    // (4) Projection: map each selected variable to its result-column index.
    let projection = match selected {
        None => None,
        Some(vars) => {
            let mut cols = Vec::with_capacity(vars.len());
            for v in vars {
                let idx = schema
                    .iter()
                    .position(|s| s == v)
                    .ok_or_else(|| BgpPlanError::SelectedVariableNotInBgp { var: v.clone() })?;
                cols.push(idx);
            }
            Some(cols)
        }
    };

    Ok(CompiledBgp {
        plan,
        schema,
        projection,
    })
}

/// Flatten an `And` body (recursively), partitioning leaf conjuncts into `triples` (the
/// BGP atoms) and `comparisons` (`Greater`/`Less`/`Gte`/`Lte`, later routed to object-value
/// range filters). Any other conjunct rejects closed.
fn collect_conjuncts<'a>(
    conjuncts: &'a [Query],
    triples: &mut Vec<&'a Triple>,
    comparisons: &mut Vec<&'a Query>,
) -> Result<(), BgpPlanError> {
    for q in conjuncts {
        match q {
            Query::Triple(t) => triples.push(t),
            Query::And(inner) => collect_conjuncts(&inner.and, triples, comparisons)?,
            // Numeric comparison conjuncts (Phase 7.4): kept aside, then normalized to a
            // `(var, op, constant)` object-value range in `attach_ranges`.
            Query::Greater(_) | Query::Less(_) | Query::Gte(_) | Query::Lte(_) => {
                comparisons.push(q)
            }
            other => {
                return Err(BgpPlanError::ConjunctNotTriple {
                    construct: query_variant_name(other),
                })
            }
        }
    }
    Ok(())
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

/// Encode a numeric range constant to `(datatype_tag, c)` matching the store's committed
/// node/value dictionary encoding EXACTLY, by routing through the store's OWN value
/// canonicalization (7.5c alignment trick): build the typed value via [`TdbDataType::make_entry`]
/// + [`ValueTriple::new_value`], read back its [`CanonicalObject::Value`] via
/// [`CanonicalObject::from_store_object`], take its [`CanonicalDatatype`] discriminant as the
/// datatype tag, and compute `c` as the big-endian unsigned integer of its order-preserving
/// `to_lexical` bytes — the SAME conversion `terminus_store`'s (private) `node_value_op_value`
/// applies to the committed dict value column.
///
/// Supported (round-trip-verified): `xsd:unsignedInt` → store `UInt32` (4-byte big-endian OP
/// encoding). Every other `XSDAnySimpleType` rejects closed: a range over a
/// non-order-preserving or variable-width datatype (string/decimal/…) would compare a
/// `0` sentinel in the store, which is unsound at the query level. Adding a datatype
/// requires a store-routed construction here PLUS a round-trip alignment test.
fn encode_range_constant(xsd: &XSDAnySimpleType) -> Result<(ArkFr, ArkFr), BgpPlanError> {
    match xsd {
        // xsd:unsignedInt is the 32-bit unsigned type → store `UInt32`.
        XSDAnySimpleType::UnsignedInt(u) => {
            let v = u32::try_from(*u).map_err(|_| BgpPlanError::UnsupportedRangeDatatype {
                datatype: "unsignedInt (exceeds u32)",
            })?;
            encode_via_store(u32::make_entry(&v))
        }
        other => Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: xsd_datatype_name(other),
        }),
    }
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
/// datatype this compiler has not round-trip-verified. Mirrors (a subset of)
/// `terminus_store`'s private `order_preserving_numeric_width`. Extend ONLY alongside a
/// new round-trip alignment test in `prove.rs`.
fn op_preserving_width(datatype: CanonicalDatatype) -> Option<usize> {
    match datatype {
        CanonicalDatatype::UInt32 => Some(4),
        _ => None,
    }
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
/// Supported (round-trip-verified): `xsd:string`, canonicalized by routing the Rust
/// `String` through the store's OWN [`ValueTriple::new_string_value`] →
/// [`CanonicalObject::from_store_object`] path — the exact path the store uses when it
/// stores a string value — so the bytes match BY CONSTRUCTION (no re-implementation of the
/// tdb-succinct encoding). See the `object_string_literal_*` round-trip tests.
///
/// Rejected closed (alignment unproven): every other `XSDAnySimpleType`
/// (boolean/integer/decimal/float/dateTime/…). Adding one requires a store-routed
/// construction plus its own round-trip alignment test.
fn value_constant_term(xsd: &XSDAnySimpleType) -> Result<AtomTerm, BgpPlanError> {
    match xsd {
        XSDAnySimpleType::String(s) => {
            // Route through the store's own string-value construction: the resulting
            // (CanonicalDatatype::String, lexical) is byte-identical to a stored
            // `new_string_value(_, _, s)`. Subject/predicate are irrelevant to the object
            // canonicalization, so empty placeholders are fine.
            let triple = ValueTriple::new_string_value("", "", s);
            match CanonicalObject::from_store_object(&triple.object) {
                CanonicalObject::Value { datatype, lexical } => {
                    Ok(AtomTerm::ValueConstant(datatype, lexical))
                }
                // Unreachable: `new_string_value` always yields a `Value` object. Fail
                // closed rather than emit an unaligned constant.
                CanonicalObject::Node(_) => Err(BgpPlanError::UnsupportedValueDatatype {
                    datatype: "string",
                }),
            }
        }
        other => Err(BgpPlanError::UnsupportedValueDatatype {
            datatype: xsd_datatype_name(other),
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

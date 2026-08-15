//! Property-path lowering — fixed-length paths to BGP + bag union.
//!
//! Lowers `Query::Path` patterns the audited join / union machinery can reproduce EXACTLY:
//! single predicates and their inverses, distinct-predicate sequences, bounded `Times` ranges,
//! and single-hop `Or` alternatives ([`plan_deferred_repeated_predicate_path`]), plus the
//! transitive-closure form ([`plan_transitive_closure_path`]). It rewrites paths into
//! `Triple` / `And` / `Or` / `Select` over fresh intermediate variables (edge-simple, bag
//! multiplicity faithful to the reference path evaluator); `Star` / `Plus` / open closure,
//! wildcard predicates, and repeated-predicate chains reject closed.

use super::*;

// ============================================================================
// Fixed-length property-path lowering: `Query::Path` -> BGP (`Triple`/`And`) +
// bag union (`Or`) + endpoint projection (`Select`).
//
// The lowering reuses the audited BGP/join/union/projection machinery below; it
// adds no cryptography. It is faithful to the leading-Prolog path evaluator
// (`src/core/query/path.pl`), which:
//   * binds the endpoint pair `(subject, object)` (the traversed edge list is
//     bound only when the `path` field is present — we reject that case),
//   * returns BAG multiplicity over endpoint pairs — one solution per distinct
//     path — with NO deduplication (see the `path.pl` `test(p)` oracle), and
//   * is edge-SIMPLE: `Open_Set`/`in_open_set` forbids re-traversing an identical
//     `(source,predicate,target)` edge within a single path.
//
// A conjunctive bag-join over triples reproduces exactly one row per distinct
// intermediate assignment — i.e. one per distinct path — so plain (non-distinct)
// endpoint projection matches Prolog's bag multiplicity EXACTLY *provided no edge
// can repeat*. Edge repetition needs two hops with the SAME predicate (edges with
// different predicates are always distinct). We therefore accept a multi-hop chain
// only when its hop predicates are pairwise distinct (edge-reuse impossible ->
// join == Prolog on all data, including self-loops) and reject repeated-predicate
// chains closed (`PathRepeatedPredicateUnsupported`) — this also excludes every
// `Times{n>=2}`, whose n-fold expansion necessarily repeats a predicate.
//
// Supported (proven exact, data-independently):
//   * `Predicate(p)`            -> `Triple(S, p, T)`
//   * `InversePredicate(p)`     -> `Triple(T, p, S)`
//   * `Sequence([..])`          -> left-deep `And` chain through fresh intermediates,
//                                  projected to the endpoint variables, when hop
//                                  predicates are pairwise distinct
//   * `Or([..])`                -> bag-union `Or` of the (single-hop) alternatives
//   * `Times(pat, n, n)`        -> the n-fold sequence (only n==1 survives the
//                                  distinct-predicate guard)
//   * `Times(pat, a, b)`        -> bag union over exact lengths a..=b (only single-hop
//                                  lengths survive)
// Rejected closed: `Star`/`Plus`/open `Times` (closure), wildcard `.` predicates,
// repeated-predicate / multi-hop union alternatives, zero-length identity, and
// edge-list capture.
// ============================================================================

/// The maximum `Times` repetition count / range width the lowering will expand,
/// guarding against pathological (e.g. `u64::MAX`) bounds before any allocation.
pub(crate) const PATH_TIMES_MAX_BOUND: u64 = 64;

/// A resolved path endpoint or fresh intermediate: a bound node IRI or a variable.
#[derive(Clone)]
pub(crate) enum PathTerm {
    Variable(String),
    Node(String),
}

impl PathTerm {
    /// This term in a `NodeValue` (subject / predicate-less) position.
    fn as_subject(&self) -> NodeValue {
        match self {
            PathTerm::Variable(name) => NodeValue::Variable(name.clone()),
            PathTerm::Node(iri) => NodeValue::Node(iri.clone()),
        }
    }
    /// This term in a `Value` (object) position.
    fn as_object(&self) -> Value {
        match self {
            PathTerm::Variable(name) => Value::Variable(name.clone()),
            PathTerm::Node(iri) => Value::Node(iri.clone()),
        }
    }
    fn variable_name(&self) -> Option<&str> {
        match self {
            PathTerm::Variable(name) => Some(name.as_str()),
            PathTerm::Node(_) => None,
        }
    }
}

pub(crate) fn path_endpoint_term(value: &Value) -> Result<PathTerm, BgpPlanError> {
    match value {
        Value::Variable(name) => Ok(PathTerm::Variable(name.clone())),
        Value::Node(iri) => Ok(PathTerm::Node(iri.clone())),
        _ => Err(BgpPlanError::PathEndpointUnsupported),
    }
}

/// Allocates fresh intermediate variable names that never collide with a query
/// variable (nor with each other), so a lowered sequence's hidden waypoints can
/// never alias a real binding.
pub(crate) struct FreshPathVars {
    reserved: std::collections::BTreeSet<String>,
    counter: usize,
}

impl FreshPathVars {
    pub(crate) fn new(reserved: std::collections::BTreeSet<String>) -> Self {
        Self {
            reserved,
            counter: 0,
        }
    }
    fn next(&mut self) -> String {
        loop {
            let candidate = format!("__path_hop_{}", self.counter);
            self.counter += 1;
            if self.reserved.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

/// True iff a `Query::Path` occurs anywhere in the read-composition fragment the
/// lowering rewrites through. (Paths buried in constructs outside this fragment are
/// reached — and lowered — by `plan_bgp`'s own recursion, or rejected closed there.)
pub(crate) fn query_contains_path(query: &Query) -> bool {
    match query {
        Query::Path(_) => true,
        Query::And(and) => and.and.iter().any(query_contains_path),
        Query::Or(or) => or.or.iter().any(query_contains_path),
        Query::Not(not) => query_contains_path(&not.query),
        Query::Select(select) => query_contains_path(&select.query),
        Query::Distinct(distinct) => query_contains_path(&distinct.query),
        Query::Count(count) => query_contains_path(&count.query),
        Query::Using(using) => query_contains_path(&using.query),
        Query::From(from) => query_contains_path(&from.query),
        Query::Pin(pin) => query_contains_path(&pin.query),
        Query::Immediately(immediately) => query_contains_path(&immediately.query),
        _ => false,
    }
}

/// Reject the three woql2 leaves that are triple-SHAPED but are NOT store triple patterns in
/// TerminusDB's semantics (`TypeOf`/`Dot`/`IsA`), returning the specific, documented
/// [`BgpPlanError`] for the first one encountered. It runs as a pre-pass BEFORE any lowering
/// or shape analysis (see the entry wiring in [`plan_bgp`]), mirroring the path pre-pass's
/// traversal so that:
///  * a NESTED occurrence (e.g. a `TypeOf` inside an `And` inside a `Select`) is caught with
///    its precise soundness rationale rather than a generic fall-through error, and
///  * the guard COMPOSES with the path lowering — a query carrying both a `Path` and one of
///    these leaves rejects on the leaf, because this runs first.
/// Recursion covers exactly the read-composition wrappers the lowering fragment threads
/// through; a leaf buried inside a construct outside that fragment is instead rejected by
/// `plan_bgp`'s own recursion (never silently dropped, which would itself be unsound).
pub(crate) fn reject_non_bgp_typed_leaves(query: &Query) -> Result<(), BgpPlanError> {
    // The whole query starts at BODY position (only outer read-control wrappers above it).
    reject_non_bgp_typed_leaves_at(query, true)
}

/// `body_position` is true while `query` is still reachable as the top-level BODY through outer
/// read-control wrappers ONLY (Select/Distinct/Count/From/…). A standalone (body-position) `IsA` NOW
/// LOWERS via [`plan_isa`] (a cross-root two-root hash join), so it is allowed through here and its
/// precise regime rejection (variable class / bound element) is decided by `plan_isa`. A NESTED `IsA`
/// (inside And/Or/Not — a BGP conjunction or logical combinator) still rejects closed: the cross-root
/// node reads two graphs and is top-level only. `TypeOf`/`Dot` never lower and reject in any position.
pub(crate) fn reject_non_bgp_typed_leaves_at(query: &Query, body_position: bool) -> Result<(), BgpPlanError> {
    match query {
        Query::TypeOf(_) => return Err(BgpPlanError::TypeOfNotStoredType),
        Query::Dot(_) => return Err(BgpPlanError::DotNotStoreEdge),
        Query::IsA(isa) => {
            if !body_position {
                return Err(isa_reject_reason(isa));
            }
        }
        Query::And(and) => and
            .and
            .iter()
            .try_for_each(|q| reject_non_bgp_typed_leaves_at(q, false))?,
        Query::Or(or) => or
            .or
            .iter()
            .try_for_each(|q| reject_non_bgp_typed_leaves_at(q, false))?,
        Query::Not(not) => reject_non_bgp_typed_leaves_at(&not.query, false)?,
        Query::Select(select) => reject_non_bgp_typed_leaves_at(&select.query, body_position)?,
        Query::Distinct(distinct) => reject_non_bgp_typed_leaves_at(&distinct.query, body_position)?,
        Query::Count(count) => reject_non_bgp_typed_leaves_at(&count.query, body_position)?,
        Query::Using(using) => reject_non_bgp_typed_leaves_at(&using.query, body_position)?,
        Query::From(from) => reject_non_bgp_typed_leaves_at(&from.query, body_position)?,
        Query::Pin(pin) => reject_non_bgp_typed_leaves_at(&pin.query, body_position)?,
        Query::Immediately(immediately) => {
            reject_non_bgp_typed_leaves_at(&immediately.query, body_position)?
        }
        _ => {}
    }
    Ok(())
}

/// The PRECISE reject reason for a `Query::IsA` leaf, distinguished by its class term. Both regimes
/// reject closed this pass, but with distinct, documented reasons (matching the reference split in
/// terminusdb `src/core/document/instance.pl`):
///   * a VARIABLE class (`type_of`) → [`BgpPlanError::IsAVariableClass`]: the downward `sys:inherits`
///     subsumption closure needs the class bound as a constant to seed it;
///   * a BOUND class (the node regime `is_instance_class`, instance.pl:80-92) →
///     [`BgpPlanError::IsASubsumptionUnsupported`]: it needs a cross-dictionary IRI-reconciled join
///     between the INSTANCE-graph `rdf:type` relation and the SCHEMA-graph downward subsumption closure
///     (the two graphs commit SEPARATE dictionaries under SEPARATE roots, so no sound raw-id join
///     exists — see that variant's rationale).
///
/// The LITERAL-subject regime of `is_instance` (the XSD datatype lattice, `basetype_subsumption_of`,
/// instance.pl:50-66) is structurally UNREACHABLE from woql2 `IsA`: its `element` field is a
/// [`crate::value::NodeValue`], which is only a node IRI or a variable and cannot carry a typed
/// literal, so a "literal isa" cannot be constructed here and needs no separate arm.
pub(crate) fn isa_reject_reason(isa: &crate::compare::IsA) -> BgpPlanError {
    match &isa.type_of {
        crate::value::NodeValue::Variable(_) => BgpPlanError::IsAVariableClass,
        crate::value::NodeValue::Node(_) => BgpPlanError::IsASubsumptionUnsupported,
    }
}

/// Collect every variable name used in the fragment we lower through, so fresh path
/// intermediates avoid them. Constructs outside this fragment reject closed and never
/// share a join namespace with a lowered path, so their variables need not be gathered.
pub(crate) fn collect_reserved_path_variables(query: &Query, out: &mut std::collections::BTreeSet<String>) {
    fn push_value(value: &Value, out: &mut std::collections::BTreeSet<String>) {
        if let Value::Variable(name) = value {
            out.insert(name.clone());
        }
    }
    fn push_node(value: &NodeValue, out: &mut std::collections::BTreeSet<String>) {
        if let NodeValue::Variable(name) = value {
            out.insert(name.clone());
        }
    }
    match query {
        Query::Path(path) => {
            push_value(&path.subject, out);
            push_value(&path.object, out);
            if let Some(captured) = &path.path {
                push_value(captured, out);
            }
        }
        Query::Triple(triple) => {
            push_node(&triple.subject, out);
            push_node(&triple.predicate, out);
            push_value(&triple.object, out);
        }
        Query::And(and) => and
            .and
            .iter()
            .for_each(|q| collect_reserved_path_variables(q, out)),
        Query::Or(or) => or
            .or
            .iter()
            .for_each(|q| collect_reserved_path_variables(q, out)),
        Query::Not(not) => collect_reserved_path_variables(&not.query, out),
        Query::Select(select) => {
            out.extend(select.variables.iter().cloned());
            collect_reserved_path_variables(&select.query, out);
        }
        Query::Distinct(distinct) => {
            out.extend(distinct.variables.iter().cloned());
            collect_reserved_path_variables(&distinct.query, out);
        }
        Query::Count(count) => {
            if let DataValue::Variable(name) = &count.count {
                out.insert(name.clone());
            }
            collect_reserved_path_variables(&count.query, out);
        }
        Query::Using(using) => collect_reserved_path_variables(&using.query, out),
        Query::From(from) => collect_reserved_path_variables(&from.query, out),
        Query::Pin(pin) => collect_reserved_path_variables(&pin.query, out),
        Query::Immediately(immediately) => {
            collect_reserved_path_variables(&immediately.query, out)
        }
        _ => {}
    }
}

/// Rewrite every `Query::Path` in the read-composition fragment into its lowered
/// `Triple`/`And`/`Or`/`Select` equivalent, leaving all other nodes structurally intact.
pub(crate) fn rewrite_query_paths(query: &Query, alloc: &mut FreshPathVars) -> Result<Query, BgpPlanError> {
    let rewrite_boxed =
        |q: &Query, alloc: &mut FreshPathVars| -> Result<Box<Query>, BgpPlanError> {
            Ok(Box::new(rewrite_query_paths(q, alloc)?))
        };
    Ok(match query {
        Query::Path(path) => lower_path(path, alloc)?,
        Query::And(and) => Query::And(crate::query::And {
            and: and
                .and
                .iter()
                .map(|q| rewrite_query_paths(q, alloc))
                .collect::<Result<_, _>>()?,
        }),
        Query::Or(or) => Query::Or(crate::query::Or {
            or: or
                .or
                .iter()
                .map(|q| rewrite_query_paths(q, alloc))
                .collect::<Result<_, _>>()?,
        }),
        Query::Not(not) => Query::Not(crate::query::Not {
            query: rewrite_boxed(&not.query, alloc)?,
        }),
        Query::Select(select) => Query::Select(crate::control::Select {
            variables: select.variables.clone(),
            query: rewrite_boxed(&select.query, alloc)?,
        }),
        Query::Distinct(distinct) => Query::Distinct(crate::control::Distinct {
            variables: distinct.variables.clone(),
            query: rewrite_boxed(&distinct.query, alloc)?,
        }),
        Query::Count(count) => Query::Count(crate::misc::Count {
            query: rewrite_boxed(&count.query, alloc)?,
            count: count.count.clone(),
        }),
        Query::Using(using) => Query::Using(crate::control::Using {
            collection: using.collection.clone(),
            query: rewrite_boxed(&using.query, alloc)?,
        }),
        Query::From(from) => Query::From(crate::control::From {
            graph: from.graph.clone(),
            query: rewrite_boxed(&from.query, alloc)?,
        }),
        Query::Pin(pin) => Query::Pin(crate::control::Pin {
            query: rewrite_boxed(&pin.query, alloc)?,
        }),
        Query::Immediately(immediately) => Query::Immediately(crate::control::Immediately {
            query: rewrite_boxed(&immediately.query, alloc)?,
        }),
        other => other.clone(),
    })
}

/// Lower a single `Path` node to an equivalent BGP/union `Query`.
///
/// A pairwise-distinct-predicate path (the edge-repetition-free case) lowers here to a
/// plain bag-join `Query`. A single-predicate REPEATED path cannot be captured by a plain
/// bag-join (it edge-simple over-counts), so `ensure_edge_simple_exact` raises
/// `PathRepeatedPredicateUnsupported`; we catch exactly that signal and DEFER — returning
/// the `Path` node UN-lowered so `plan_bgp` routes it to
/// [`plan_deferred_repeated_predicate_path`], which builds the join chain plus the
/// `TupleDisequality` edge-simple filter at the plan level (a `Query` cannot express that
/// filter). Every other rejection (unbounded / wildcard / identity / edge capture)
/// propagates unchanged.
pub(crate) fn lower_path(
    path: &crate::query::Path,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }
    // A `Star`/`Plus` transitive-closure path is compiled at the PLAN level: it needs the endpoint
    // `Distinct` gate and the composite `TransitiveClosure` node, neither of which is expressible as a
    // rewritten `Query`. Leave the `Path` node in place so `plan_bgp` routes it to
    // `plan_transitive_closure_path` (which reject-closes every unsupported closure shape). A closure
    // buried inside a larger `And` is NOT the plan body, so it reaches the conjunctive compiler as a
    // non-triple conjunct and rejects closed there.
    if matches!(
        path.pattern,
        crate::path::PathPattern::Star(_) | crate::path::PathPattern::Plus(_)
    ) {
        return Ok(Query::Path(path.clone()));
    }
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
    match lower_top(&path.pattern, &source, &target, alloc) {
        // A repeated-predicate chain is provable via the deferred TupleDisequality
        // lowering — leave the Path node in place for `plan_bgp` to route there.
        Err(BgpPlanError::PathRepeatedPredicateUnsupported { .. }) => {
            Ok(Query::Path(path.clone()))
        }
        other => other,
    }
}

/// Top-level pattern lowering: may produce a bag union (`Or`) or a conjunctive chain
/// projected to its endpoints (`Select` over `And`).
pub(crate) fn lower_top(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Or(_) => {
            let mut branches = Vec::new();
            lower_union_branches(pattern, source, target, alloc, &mut branches)?;
            match branches.len() {
                0 => Err(BgpPlanError::PathEmptyPattern),
                1 => Ok(branches.pop().unwrap()),
                _ => Ok(Query::Or(crate::query::Or { or: branches })),
            }
        }
        PathPattern::Times(times) if times.from != times.to => {
            lower_times_range_union(times, source, target, alloc)
        }
        PathPattern::Plus(_) | PathPattern::Star(_) => {
            Err(BgpPlanError::UnboundedPathUnsupported)
        }
        _ => {
            let mut triples = Vec::new();
            let mut predicates = Vec::new();
            lower_chain(pattern, source, target, alloc, &mut triples, &mut predicates)?;
            ensure_edge_simple_exact(&predicates)?;
            build_chain_query(triples, source, target)
        }
    }
}

/// Expand a bounded-range `Times(pat, a, b)` into a bag union over each exact length.
/// Every length must lower to a single hop (multi-hop union branches reject closed).
pub(crate) fn lower_times_range_union(
    times: &crate::path::PathTimes,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    if times.to < times.from {
        return Err(BgpPlanError::PathTimesInvalidRange);
    }
    if times.from == 0 {
        return Err(BgpPlanError::PathIdentityUnsupported);
    }
    if times.to > PATH_TIMES_MAX_BOUND {
        return Err(BgpPlanError::UnboundedPathUnsupported);
    }
    let mut branches = Vec::new();
    for length in times.from..=times.to {
        let mut triples = Vec::new();
        let mut predicates = Vec::new();
        lower_n_fold(&times.times, length, source, target, alloc, &mut triples, &mut predicates)?;
        ensure_edge_simple_exact(&predicates)?;
        if triples.len() != 1 {
            return Err(BgpPlanError::PathUnionBranchMultiHop);
        }
        branches.push(triples.into_iter().next().unwrap());
    }
    match branches.len() {
        0 => Err(BgpPlanError::PathEmptyPattern),
        1 => Ok(branches.pop().unwrap()),
        _ => Ok(Query::Or(crate::query::Or { or: branches })),
    }
}

/// Flatten a union pattern into its single-hop alternative triples. Nested `Or`s
/// flatten in source order; a multi-hop alternative rejects closed.
pub(crate) fn lower_union_branches(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    out: &mut Vec<Query>,
) -> Result<(), BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Or(or) => {
            if or.or.is_empty() {
                return Err(BgpPlanError::PathEmptyPattern);
            }
            for branch in &or.or {
                lower_union_branches(branch, source, target, alloc, out)?;
            }
            Ok(())
        }
        PathPattern::Plus(_) | PathPattern::Star(_) => Err(BgpPlanError::UnboundedPathUnsupported),
        _ => {
            let mut triples = Vec::new();
            let mut predicates = Vec::new();
            lower_chain(pattern, source, target, alloc, &mut triples, &mut predicates)?;
            ensure_edge_simple_exact(&predicates)?;
            if triples.len() != 1 {
                return Err(BgpPlanError::PathUnionBranchMultiHop);
            }
            out.push(triples.into_iter().next().unwrap());
            Ok(())
        }
    }
}

/// Lower a pattern that must be a pure conjunctive chain (no unions), threading
/// `source -> ... -> target` through fresh intermediates and appending each hop's
/// triple and predicate string. Rejects any construct that would introduce a union.
pub(crate) fn lower_chain(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    triples: &mut Vec<Query>,
    predicates: &mut Vec<String>,
) -> Result<(), BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Predicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            triples.push(Query::Triple(Triple {
                subject: source.as_subject(),
                predicate: NodeValue::Node(predicate.clone()),
                object: target.as_object(),
                graph: None,
            }));
            predicates.push(predicate);
            Ok(())
        }
        PathPattern::InversePredicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            // Inverse: the edge points target -> source (swap subject/object).
            triples.push(Query::Triple(Triple {
                subject: target.as_subject(),
                predicate: NodeValue::Node(predicate.clone()),
                object: source.as_object(),
                graph: None,
            }));
            predicates.push(predicate);
            Ok(())
        }
        PathPattern::Sequence(sequence) => match sequence.sequence.len() {
            0 => Err(BgpPlanError::PathEmptyPattern),
            1 => lower_chain(&sequence.sequence[0], source, target, alloc, triples, predicates),
            hops => {
                let mut previous = source.clone();
                for (index, sub) in sequence.sequence.iter().enumerate() {
                    let next = if index == hops - 1 {
                        target.clone()
                    } else {
                        PathTerm::Variable(alloc.next())
                    };
                    lower_chain(sub, &previous, &next, alloc, triples, predicates)?;
                    previous = next;
                }
                Ok(())
            }
        },
        PathPattern::Times(times) => {
            if times.from != times.to {
                // A bounded-RANGE repetition is a union; it cannot be a chain hop.
                return Err(BgpPlanError::PathNonElementaryHopUnsupported);
            }
            if times.from == 0 {
                return Err(BgpPlanError::PathIdentityUnsupported);
            }
            if times.from > PATH_TIMES_MAX_BOUND {
                return Err(BgpPlanError::UnboundedPathUnsupported);
            }
            lower_n_fold(&times.times, times.from, source, target, alloc, triples, predicates)
        }
        PathPattern::Or(_) => Err(BgpPlanError::PathNonElementaryHopUnsupported),
        PathPattern::Plus(_) | PathPattern::Star(_) => Err(BgpPlanError::UnboundedPathUnsupported),
    }
}

/// Lower the `n`-fold sequence of `pattern` into a chain `source -> ... -> target`.
pub(crate) fn lower_n_fold(
    pattern: &crate::path::PathPattern,
    n: u64,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    triples: &mut Vec<Query>,
    predicates: &mut Vec<String>,
) -> Result<(), BgpPlanError> {
    if n == 0 {
        return Err(BgpPlanError::PathIdentityUnsupported);
    }
    if n == 1 {
        return lower_chain(pattern, source, target, alloc, triples, predicates);
    }
    let mut previous = source.clone();
    for index in 0..n {
        let next = if index == n - 1 {
            target.clone()
        } else {
            PathTerm::Variable(alloc.next())
        };
        lower_chain(pattern, &previous, &next, alloc, triples, predicates)?;
        previous = next;
    }
    Ok(())
}

/// Reject a chain whose hop predicates are not pairwise distinct — the guard that
/// keeps a bag-join faithful to edge-simple Prolog path semantics (see the module
/// header). Distinct predicates make edge repetition impossible, so the join equals
/// the Prolog result on ALL data; a repeat could over-count on self-loops.
pub(crate) fn ensure_edge_simple_exact(predicates: &[String]) -> Result<(), BgpPlanError> {
    let mut seen = std::collections::BTreeSet::new();
    for predicate in predicates {
        if !seen.insert(predicate.as_str()) {
            return Err(BgpPlanError::PathRepeatedPredicateUnsupported {
                predicate: predicate.clone(),
            });
        }
    }
    Ok(())
}

/// Assemble a conjunctive chain into a `Query`, projecting out intermediates by
/// wrapping a multi-hop `And` in a `Select` over the endpoint variables. A single
/// hop already has exactly the endpoint schema and needs no projection.
pub(crate) fn build_chain_query(
    triples: Vec<Query>,
    source: &PathTerm,
    target: &PathTerm,
) -> Result<Query, BgpPlanError> {
    let hops = triples.len();
    if hops == 0 {
        return Err(BgpPlanError::PathEmptyPattern);
    }
    if hops == 1 {
        return Ok(triples.into_iter().next().unwrap());
    }
    let body = Query::And(crate::query::And { and: triples });
    // Endpoint variables, in [subject, object] order, deduplicated (subject == object
    // is a legal cycle query binding a single variable).
    let mut endpoints: Vec<String> = Vec::new();
    if let Some(name) = source.variable_name() {
        endpoints.push(name.to_string());
    }
    if let Some(name) = target.variable_name() {
        if !endpoints.iter().any(|existing| existing == name) {
            endpoints.push(name.to_string());
        }
    }
    Ok(Query::Select(crate::control::Select {
        variables: endpoints,
        query: Box::new(body),
    }))
}

/// Build the `C(n,2)` hop-pair disequality specs for a lowered single-predicate chain.
/// Each triple is one stored edge `(subject, object)` (the fixed predicate cancels); a
/// pair drops rows where two edges coincide. A variable position becomes a committed-column
/// reference (variable name -> the join schema index); a bound node position becomes a
/// constant node reference. The verifier reconstructs these EXACT pairs from the trusted
/// compiled plan (folded into the query digest), so the selection is not prover-chosen.
pub(crate) fn build_disequality_pairs(
    triples: &[Query],
    schema: &[String],
) -> Result<Vec<TupleDisequalityPairPlan>, BgpPlanError> {
    fn column_of(name: &str, schema: &[String]) -> Result<TupleDisequalityNode, BgpPlanError> {
        schema
            .iter()
            .position(|candidate| candidate == name)
            .map(TupleDisequalityNode::Column)
            .ok_or(BgpPlanError::PathEndpointUnsupported)
    }
    let subject_ref = |subject: &NodeValue| -> Result<TupleDisequalityNode, BgpPlanError> {
        match subject {
            NodeValue::Variable(name) => column_of(name, schema),
            NodeValue::Node(iri) => Ok(TupleDisequalityNode::Constant(CanonicalObject::Node(
                iri.clone().into_bytes(),
            ))),
        }
    };
    let object_ref = |object: &Value| -> Result<TupleDisequalityNode, BgpPlanError> {
        match object {
            Value::Variable(name) => column_of(name, schema),
            Value::Node(iri) => Ok(TupleDisequalityNode::Constant(CanonicalObject::Node(
                iri.clone().into_bytes(),
            ))),
            // Path node positions are only variables or bound node IRIs (endpoints are
            // guarded by `path_endpoint_term`; intermediates are fresh variables).
            _ => Err(BgpPlanError::PathEndpointUnsupported),
        }
    };
    let mut edges = Vec::with_capacity(triples.len());
    for triple in triples {
        let Query::Triple(triple) = triple else {
            return Err(BgpPlanError::PathNonElementaryHopUnsupported);
        };
        edges.push((subject_ref(&triple.subject)?, object_ref(&triple.object)?));
    }
    let mut pairs = Vec::new();
    for e in 0..edges.len() {
        for f in (e + 1)..edges.len() {
            pairs.push(TupleDisequalityPairPlan {
                a0: edges[e].0.clone(),
                a1: edges[e].1.clone(),
                b0: edges[f].0.clone(),
                b1: edges[f].1.clone(),
            });
        }
    }
    Ok(pairs)
}

/// Project a filtered path chain to its endpoint variables (bag multiplicity preserved), so
/// a single-length chain and every range branch share the `[source?, target?]` endpoint
/// schema. Bound (constant) endpoints contribute no column; a fully-ground path projects to
/// the canonical zero-column (unit) relation.
pub(crate) fn project_path_endpoints(
    relation: CompiledRelation,
    source: &PathTerm,
    target: &PathTerm,
) -> Result<CompiledRelation, BgpPlanError> {
    let mut endpoints: Vec<&str> = Vec::new();
    if let Some(name) = source.variable_name() {
        endpoints.push(name);
    }
    if let Some(name) = target.variable_name() {
        if !endpoints.contains(&name) {
            endpoints.push(name);
        }
    }
    let mut indices = Vec::with_capacity(endpoints.len());
    for name in &endpoints {
        let index = relation
            .schema
            .iter()
            .position(|candidate| candidate == name)
            .ok_or(BgpPlanError::PathEndpointUnsupported)?;
        indices.push(index);
    }
    let schema: Vec<String> = indices.iter().map(|i| relation.schema[*i].clone()).collect();
    let domains: Vec<_> = indices.iter().map(|i| relation.domains[*i]).collect();
    Ok(CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::BagProjection {
            input: Box::new(relation),
            indices,
        },
    })
}

/// Compile a DEFERRED finite single-predicate repeated-predicate path (see [`lower_path`])
/// to `BagProjection(TupleDisequality(join-chain))` — the edge-simple-exact plan a plain
/// bag-join cannot express. A range `Times{a,b}` fans out to a bag union over each exact
/// length. Reuses the audited BGP compiler for the join chains and the `plan_or_relation`
/// union path; adds only the `TupleDisequality` selection and the endpoint projection.
pub(crate) fn plan_deferred_repeated_predicate_path(
    path: &crate::query::Path,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    use crate::path::PathPattern;
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }

    // RANGE `Times{a,b}` (a != b): a bag union over each exact length, each an independently
    // edge-simple-filtered chain projected to the endpoints. Delegating each length to a
    // fresh `Query::Path` re-enters `plan_bgp` -> this handler (single-length arm) per branch.
    if let PathPattern::Times(times) = &path.pattern {
        if times.from != times.to {
            if times.to < times.from {
                return Err(BgpPlanError::PathTimesInvalidRange);
            }
            if times.from == 0 {
                return Err(BgpPlanError::PathIdentityUnsupported);
            }
            if times.to > PATH_TIMES_MAX_BOUND {
                return Err(BgpPlanError::UnboundedPathUnsupported);
            }
            let mut branches = Vec::with_capacity((times.to - times.from + 1) as usize);
            for length in times.from..=times.to {
                branches.push(Query::Path(crate::query::Path {
                    subject: path.subject.clone(),
                    pattern: PathPattern::Times(crate::path::PathTimes {
                        times: times.times.clone(),
                        from: length,
                        to: length,
                    }),
                    object: path.object.clone(),
                    path: None,
                }));
            }
            let disjunction = crate::query::Or { or: branches };
            return plan_or_relation(
                &disjunction,
                selections,
                collection,
                count_variable,
                distincts,
            );
        }
    }

    // SINGLE-LENGTH: lower to the triple chain, then wrap it in the edge-simple filter.
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
    let mut reserved = std::collections::BTreeSet::new();
    collect_reserved_path_variables(&Query::Path(path.clone()), &mut reserved);
    let mut alloc = FreshPathVars::new(reserved);
    let mut triples = Vec::new();
    let mut predicates = Vec::new();
    lower_chain(
        &path.pattern,
        &source,
        &target,
        &mut alloc,
        &mut triples,
        &mut predicates,
    )?;

    // Single predicate is the whole scope of this pass: a mixed-predicate repeat needs the
    // predicate id folded into the fingerprint (a later sub-effort) and rejects closed.
    let first = predicates.first().ok_or(BgpPlanError::PathEmptyPattern)?;
    if predicates.iter().any(|predicate| predicate != first) {
        return Err(BgpPlanError::PathMixedPredicateRepeatUnsupported);
    }

    // Compile the join chain by reusing the audited BGP machinery on the raw `And`.
    let chain_query = if triples.len() == 1 {
        triples[0].clone()
    } else {
        Query::And(crate::query::And { and: triples.clone() })
    };
    let chain = plan_bgp(&chain_query)?;
    if chain.projection.is_some() || chain.count_variable.is_some() || chain.full_row_distinct {
        // A bare triple chain never sets these; a defensive guard against a surprising shape.
        return Err(BgpPlanError::PathNonElementaryHopUnsupported);
    }
    let mut relation = chain.relation;
    let mut path_collection = collection;
    merge_collection(
        &mut path_collection,
        chain.collection.as_ref(),
        BgpPlanError::OrRoutingConflict,
    )?;

    // Wrap the join in the edge-simple TupleDisequality filter (all C(n,2) hop-pairs). A
    // single hop yields no pair (nothing to filter) — but a deferred path always has >= 2.
    let pairs = build_disequality_pairs(&triples, &relation.schema)?;
    if !pairs.is_empty() {
        let schema = relation.schema.clone();
        let domains = relation.domains.clone();
        relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::TupleDisequality {
                input: Box::new(relation),
                pairs,
            },
        };
    }

    // Project to the endpoint variables so the schema matches Prolog's endpoint bindings
    // (and every range branch shares one endpoint schema).
    relation = project_path_endpoints(relation, &source, &target)?;

    // Apply the outer Distinct/Select/Count wrappers exactly as the other body dispatches.
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, count_variable.as_deref())?;
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: path_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Is this conjunct a single-input calendar-family op (whether ship-supported or reject-closed)?
/// The planner claims ALL of them so a supported op compiles and an unsupported one reject-closes —
/// never silently falling through to a mis-plan.
/// Compile a `Star`/`Plus` transitive-closure property path to the store's `TransitiveClosure`
/// composite node, ONLY under the endpoint-`Distinct` gate that makes the reference's edge-simple BAG
/// of endpoints equal the reachable SET. Every unsupported shape rejects CLOSED with a specific error.
///
/// SCOPE (this pass): a SINGLE concrete predicate (forward or inverse), exactly one BOUND endpoint
/// (bound source, or bound target reached by traversing the inverse edge), under a `Distinct` naming
/// the free endpoint — OPTIONALLY wrapped in a `Count` of that reachable set (the count is delivered
/// over the closure's OWN deduped accumulator `A_K`; see the count-composition note below). Deferred
/// (reject closed): compound sub-patterns, two free / two bound endpoints, wildcards, edge-list capture.
pub(crate) fn plan_transitive_closure_path(
    path: &crate::query::Path,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    use crate::path::PathPattern;
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }

    // `star` = zero-or-more (identity branch always succeeds, path.pl:183); `plus` = one-or-more
    // (path.pl:186). This function is only reached for these two top-level operators.
    let (reflexive, inner) = match &path.pattern {
        PathPattern::Star(star) => (true, star.star.as_ref()),
        PathPattern::Plus(plus) => (false, plus.plus.as_ref()),
        _ => return Err(BgpPlanError::UnboundedPathUnsupported),
    };

    // A SINGLE concrete predicate takes the fast path below. A COMPOUND body — a flat alternation
    // `(p|q|…)` or a 2-hop sequence `(p/q)` of single (possibly-inverse) predicates — routes to the
    // compound-edge closure planner (P3), which derives the 2-column edge relation `E'` and closes over
    // it by commitment (single-seed only; other endpoint shapes / unfaithful bodies reject closed
    // there). A wildcard `.` rejects closed.
    let (predicate, inner_inverse) = match inner {
        PathPattern::Predicate(pred) => (
            pred.predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?,
            false,
        ),
        PathPattern::InversePredicate(pred) => (
            pred.predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?,
            true,
        ),
        _ => {
            return plan_compound_closure(
                path,
                reflexive,
                inner,
                selections,
                collection,
                count_variable,
                distincts,
            )
        }
    };

    // Exactly one endpoint bound (the seed) and one free (the delivered reachable-set column). A bound
    // TARGET mirrors the traversal to the inverse edge (path.pl:129-134 `run_pattern`, `invert_path`).
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;

    // ALL-PAIRS (P2): both endpoints FREE (`path(?x, star|plus(p), ?y)`). This is a different plan
    // shape (a 2-column pairs closure, no bound seed), so it is handled separately from the single-seed
    // path below. `plus` is provable; `star` reject-closes (its reflexive diagonal is not faithfully
    // reconstructible — see [`plan_all_pairs_closure`]).
    if let (PathTerm::Variable(x), PathTerm::Variable(y)) = (&source, &target) {
        return plan_all_pairs_closure(
            x.clone(),
            y.clone(),
            reflexive,
            predicate,
            inner_inverse,
            selections,
            collection,
            count_variable,
            distincts,
        );
    }

    // BOTH-BOUND (P4): both endpoints are node CONSTANTS (`path(a, star|plus(p), b)`). This is a GROUND
    // boolean reachability check — "is `b` reachable from `a`?" — not a set query. `path.pl`'s
    // `run_pattern` (both `X`, `Y` ground) walks FORWARD from the subject `a` to the object `b` (the
    // `ground(Y), var(X)` inversion clause at path.pl:129-134 does NOT fire), succeeding AT MOST ONCE.
    // We lower it to the single-seed closure from `a` (seed = `a`, forward direction) plus an
    // AUTHENTICATED membership gate that keeps only the reachable rows equal to the constant `b`; the
    // ground existence (0 rows = unreachable, ≥1 row = reachable) is the boolean. See
    // [`plan_both_bound_closure`].
    if let (PathTerm::Node(a), PathTerm::Node(b)) = (&source, &target) {
        return plan_both_bound_closure(
            a.clone(),
            b.clone(),
            reflexive,
            predicate,
            inner_inverse,
            selections,
            collection,
            count_variable,
            distincts,
        );
    }

    // Only the two ONE-bound cases reach here: both-free (all-pairs) and both-bound (P4) are dispatched
    // by the early returns above. The two error arms are defensive fallbacks — never fired in this order,
    // but they fail CLOSED (never mis-plan) if the dispatch above is ever changed.
    let (seed_iri, endpoint_var, bound_target) = match (&source, &target) {
        (PathTerm::Node(a), PathTerm::Variable(y)) => (a.clone(), y.clone(), false),
        (PathTerm::Variable(x), PathTerm::Node(b)) => (b.clone(), x.clone(), true),
        (PathTerm::Node(_), PathTerm::Node(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsBound)
        }
        (PathTerm::Variable(_), PathTerm::Variable(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsFree)
        }
    };

    // Effective traversal direction. A bound target and an inverse predicate are each ONE mirroring;
    // two mirrorings cancel (`invert_path(plus(n(P))) = plus(p(P))`, path.pl:114-134).
    let inverted = bound_target ^ inner_inverse;

    // The edge relation E = Triple(?s, p, ?o): a single-atom 2-column [subject, object] node BGP.
    let edge = LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(predicate.into_bytes())],
        joins: Vec::new(),
    };
    let relation = CompiledRelation {
        schema: vec![endpoint_var],
        domains: vec![None],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge,
            seed: CanonicalObject::Node(seed_iri.into_bytes()),
            reflexive,
            inverted,
            // A WOQL `Path` closure walks the INSTANCE graph — byte-identical to before the
            // schema-graph foundation (the `Instance` selector folds nothing into the digest).
            graph: ProofGraph::Instance,
        },
    };

    // ENFORCE the gate: the closure delivers a SET, so the surrounding `Distinct` must name its full
    // (single-column) schema. `compile_distinct_relation` returns `full_row_distinct == true` exactly
    // when the `Distinct` covers the schema over a duplicate-free child (the closure is duplicate-free
    // by construction — see `is_provably_duplicate_free`). Anything else (no `Distinct`, or a
    // `Distinct` on a different variable) fails the gate and rejects closed.
    //
    // COUNT COMPOSITION (P1): a `Count(n, Distinct([y], path(a, star|plus(p), ?y)))` counts the
    // reachable SET. We pass `None` (never `count_variable`) here so the endpoint `Distinct` is
    // always validated as the SAME transparent full-row no-op over the closure's already
    // strict-sorted, dedup-free accumulator `A_K` — NOT re-routed through the store `Distinct`
    // dedup gadget (which would re-key a fresh column and break commitment continuity). The count
    // is then attached purely as read-side metadata (zero-column projection + `count_variable`
    // below), exactly as Count-over-BGP does: the delivered count is `A_K`'s proven row length, so
    // it binds the closure's own final accumulator commitment (soundness: commitment continuity;
    // the deduped set — with Star's reflexive seed counted once — is what is measured).
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, None)?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Compile an ALL-PAIRS `Star`/`Plus` closure (`path(?x, star|plus(p), ?y)`, BOTH endpoints free) to
/// the store's `TransitiveClosurePairs` composite node — the 2-column reachability relation `[x, y]` —
/// ONLY under a `Distinct([x,y])` gate (so the reference's edge-simple BAG of endpoint pairs equals the
/// pairs SET). Reused verbatim from the single-seed path: the store InnerJoin/BagUnion/Distinct/LeftAnti
/// cores, the round-by-commitment threading, the fixpoint completeness certificate, the 64-round budget
/// — only the accumulator arity (2) and the seed (E itself, no bound node) differ. NO new crypto.
///
/// OPTIONALLY wrapped in a `Count` of the pairs relation (P2c): `Count(n, Distinct([x,y], ...))` proves
/// `n = |pairs relation|` (the number of distinct connected pairs). The count is delivered over the
/// pairs closure's OWN authenticated accumulator `result_len`, exactly as the single-seed count gate
/// (P1) delivers `|A_K|` — see the count-composition note at the `compile_distinct_relation` call below.
///
/// PLUS is provable. STAR reject-closes (`ClosureStarBothFreeDomainDeferred`) via the FIDELITY GATE:
/// `path.pl`'s both-free identity branch (`X = Y`, path.pl:183) unifies two UNBOUND variables — it
/// yields a single unbound row, NOT the domain diagonal `{(x,x) : x ∈ nodes(E)}` (nor all instance
/// subjects, nor all dictionary nodes). Emitting an E-derived diagonal would INJECT `(n,n)` pairs the
/// reference never returns, so we fail closed pending a domain-authentication design. All-pairs `plus`
/// has no reflexive rows: it is exactly the ≥1-hop transitive-reachability pairs (a shortest walk
/// repeats no edge, so edge-simple ≥1-hop reachability = transitive reachability), so it is faithful.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_all_pairs_closure(
    source_var: String,
    target_var: String,
    reflexive: bool,
    predicate: String,
    inner_inverse: bool,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    // FIDELITY GATE — all-pairs `star`'s reflexive diagonal is not faithfully reconstructible (see the
    // fn-doc); fail closed. All-pairs `plus` proceeds (no reflexive rows).
    if reflexive {
        return Err(BgpPlanError::ClosureStarBothFreeDomainDeferred);
    }

    // `path(?x, plus(p), ?x)` (the SAME variable at both endpoints) is not a two-column pairs relation —
    // it is a self-reachability filter this pass does not model. Reject closed (mirrors the prior
    // both-free reject).
    if source_var == target_var {
        return Err(BgpPlanError::ClosureBothEndpointsFree);
    }

    // The edge relation E = Triple(?s, p, ?o): a single-atom 2-column [subject, object] node BGP.
    let edge = LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(predicate.into_bytes())],
        joins: Vec::new(),
    };
    // The delivered pairs relation is [source_var, target_var] = [x, y]. The seed A_0 = π(E) and every
    // round preserve this schema; the store node commits both columns.
    let relation = CompiledRelation {
        schema: vec![source_var, target_var],
        domains: vec![None, None],
        statement: CompiledRelationStatement::TransitiveClosurePairs {
            edge,
            // Both endpoints free, so the ONLY direction source is an inverse inner predicate.
            inverted: inner_inverse,
            // A WOQL path closure walks the INSTANCE graph (folds nothing into the digest).
            graph: ProofGraph::Instance,
        },
    };

    // ENFORCE the gate: the pairs closure delivers a 2-column SET, so the surrounding `Distinct` must
    // name its FULL schema `[x, y]`. `compile_distinct_relation` returns `full_row_distinct == true`
    // exactly when a bare `Distinct` covers the whole schema over a duplicate-free child (the pairs
    // closure is duplicate-free by construction — see `is_provably_duplicate_free`). Anything else
    // (no `Distinct`, a partial `Distinct([x])`, or a projected/count form) rejects closed.
    //
    // COUNT COMPOSITION (P2c): `Count(n, Distinct([x,y], path(?x, plus(p), ?y)))` counts the pairs SET
    // — `n = |pairs relation|`. Exactly as the single-seed count gate (P1), we pass `None` (never
    // `count_variable`) here so the `Distinct([x,y])` is always validated as the SAME transparent
    // full-row no-op over the pairs closure's already strict-sorted, dedup-free accumulator `A_K` —
    // NOT re-routed through the store `Distinct` dedup gadget (which would re-key fresh columns and
    // break commitment continuity). The count is then attached purely as read-side metadata (the
    // zero-column projection + `count_variable` below): the delivered count is the pairs relation's
    // proven ROW length `result_len` (the number of distinct pairs — the count reads rows, not
    // columns), so it binds the `TransitiveClosurePairs` node's OWN authenticated accumulator
    // commitment — identical soundness to P1's |A_K|, only over a 2-column relation.
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, None)?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Compile a BOTH-endpoints-BOUND `Star`/`Plus` closure (`path(a, star|plus(p), b)`, both endpoints
/// node CONSTANTS) to a GROUND boolean reachability existence: "is `b` reachable from `a` over the
/// closure?". The delivered result is a zero-free-variable relation — `≥1` row iff reachable, `0` rows
/// iff not — exactly how a fully-ground query (e.g. a constant `Triple(a, p, b)`, or the both-bound
/// `Subsumption` existence in [`build_subsumption_membership_gate`]) is represented in this proof
/// system.
///
/// FIDELITY (`src/core/query/path.pl`). With both `X = a` and `Y = b` ground, `run_pattern` walks
/// FORWARD from the subject `a` to the object `b` (the `ground(Y), var(X)` inversion clause,
/// path.pl:129-134, does NOT fire) and succeeds AT MOST ONCE — a boolean, producing NO bindings.
///   * `plus` (path.pl:186) succeeds iff `b` is reachable from `a` via `≥1` `p`-edge — INCLUDING
///     `a == b` only when `a` lies on a cycle back to itself (`b ∈` the `≥1`-hop reachable set).
///   * `star` (path.pl:182-185) is `(X = Y ; plus)`: with both ends ground `X = Y` is a CONCRETE
///     equality `a == b`, so `path(a, star(p), a)` succeeds REFLEXIVELY (zero edges needed) and
///     `path(a, star(p), b)` (`a ≠ b`) succeeds iff `b` is plus-reachable. This is exactly
///     `b ∈ ({a} ∪ reachable≥1(a))` — the reflexive single-seed closure's accumulator `A_K`.
/// Edge-simple `≥1`-hop reachability equals transitive reachability (a shortest walk repeats no edge),
/// so the single-seed closure's least-fixpoint `A_K` is the faithful reachable set — the SAME
/// edge-simple faithfulness the single-seed and all-pairs closures already rely on.
///
/// DESIGN. Build the from-`a` single-seed [`CompiledRelationStatement::TransitiveClosure`] (seed = `a`,
/// reflexive per star/plus, forward direction modulo the inner predicate's own inversion), then an
/// AUTHENTICATED membership gate — the audited [`RelationEqualityPlan::ColumnConstant`] over the `Node`
/// domain — keeping only the reachable rows equal to the constant `b`. The gate's `input` IS the
/// committed closure relation, so the membership is decided against the SAME authenticated `A_K` the
/// closure proved (commitment continuity), and the query digest folds the closure (seed `a` / reflexive
/// / inverted / graph) AND the equality constant `b`, so a both-bound proof cannot be replayed as the
/// single-seed set closure nor under different constants/operator. An absent-from-dictionary `b`
/// resolves to a fresh sentinel id (`resolve_relation_equality_constraint`, plan.rs) that no committed
/// `A_K` row can equal → `0` rows → faithfully "unreachable", with prover and verifier computing the
/// sentinel identically (sound). Reuses the exact composite the both-bound `Subsumption` gate ships —
/// only the edge (arbitrary predicate `p` vs `sys:inherits`) and the graph (`Instance` vs `Schema`)
/// differ — so NO store change is needed (the store recursion is graph-blind).
///
/// A `Count` gate rejects closed (`ClosureCountGateDeferred`): counting a boolean existence is a
/// separate follow-up, exactly as the both-bound `Subsumption` gate defers it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_both_bound_closure(
    seed_iri: String,
    target_iri: String,
    reflexive: bool,
    predicate: String,
    inner_inverse: bool,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if count_variable.is_some() {
        return Err(BgpPlanError::ClosureCountGateDeferred);
    }

    // A fresh internal endpoint variable: both user endpoints are constants, so this column is never
    // user-visible; it exists only to carry the reachable set into the membership gate.
    let endpoint_var = "__closure_reachable".to_string();

    // The edge relation E = Triple(?s, p, ?o): a single-atom 2-column [subject, object] node BGP.
    let edge = LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(predicate.into_bytes())],
        joins: Vec::new(),
    };
    let closure = CompiledRelation {
        schema: vec![endpoint_var.clone()],
        // Pin the delivered column to the Node equality domain so the `ColumnConstant` gate below can
        // compare it against the `b` node constant (an unpinned column has no equality domain).
        domains: vec![Some(EqualityValueDomain::Node)],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge,
            seed: CanonicalObject::Node(seed_iri.into_bytes()),
            reflexive,
            // Both endpoints are bound, so the seed is ALWAYS the subject `a` and the walk is forward;
            // the only direction source is the inner predicate's own inversion.
            inverted: inner_inverse,
            // A WOQL `Path` closure walks the INSTANCE graph (folds nothing into the digest).
            graph: ProofGraph::Instance,
        },
    };

    // Membership gate: keep only the reachable rows whose node == the constant `b`. Reuses the audited
    // `RelationEqualityPlan::ColumnConstant` gadget over the Node domain; its `input` is the committed
    // closure relation, so the membership is bound to the closure's OWN authenticated `A_K`.
    let gated = CompiledRelation {
        schema: closure.schema.clone(),
        domains: closure.domains.clone(),
        statement: CompiledRelationStatement::Equality {
            input: Box::new(closure),
            constraint: RelationEqualityPlan::ColumnConstant {
                column: 0,
                domain: EqualityValueDomain::Node,
                value: CanonicalObject::Node(target_iri.into_bytes()),
                constant_on_left: false,
            },
        },
    };

    // Collapse to the set (intrinsic `distinct`, in addition to any user-supplied one), then project.
    // With both endpoints constant the query has NO free variables, so the projection is empty and the
    // result is a ground existence (0 rows = `b` NOT reachable from `a`; ≥1 row = reachable).
    let intrinsic = [endpoint_var.clone()];
    let mut gated_distincts: Vec<&[String]> = distincts.to_vec();
    gated_distincts.push(&intrinsic);
    let (relation, full_row_distinct) =
        compile_distinct_relation(gated, &gated_distincts, selections, count_variable.as_deref())?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Compile a single-seed `Star`/`Plus` closure over a COMPOUND body (`(p|q)+`, `(p/q)+`; P3). The
/// closure driver is source-agnostic over its edge relation `E`, so we DERIVE the compound edge `E'`
/// (a deduped 2-column [start, end] relation — a bag-union of the branch edges, or the two-atom join
/// projected to its endpoints) via the audited BGP machinery and close over it by commitment through
/// the [`CompiledRelationStatement::TransitiveClosureCompound`] node. NO new crypto: only what `E` *is*
/// changes versus the single-predicate closure.
///
/// SCOPE (fail-closed): exactly ONE bound endpoint (single-seed; both-free / both-bound compound
/// closures reject closed), under the endpoint `Distinct` gate. Count-gated compound closures reject
/// closed (do the `Distinct` form first). The compound body must be a flat alternation `(p|q|…)` or a
/// 2-hop DISTINCT-predicate sequence `(p/q)` of single (possibly-inverse) predicates — every other
/// shape (nested unbounded, `Times`, wildcard, non-elementary branch, same-predicate or longer
/// sequence) rejects closed via [`build_compound_edge`]'s guards.
///
/// FIDELITY (`src/core/query/path.pl`, verified): `path.pl`'s `plus`/`star` are edge-simple over
/// PRIMITIVE `(s,p,o)` edges (`Open_Set`). The compound-hop transitive closure of `E'` equals
/// `path.pl`'s edge-simple compound-path reachability AT THE SET LEVEL (delivered by the required
/// `Distinct`): any reachable pair has a NODE-simple `E'`-hop walk, which uses DISTINCT primitive edges
/// for `(p|q)` always, and for `(p/q)` iff `p ≠ q` — so `path.pl` finds every reachable pair, and every
/// `path.pl` solution is a real `E'`-walk. A bound TARGET walks `E'` BACKWARD (`inverted`), which at the
/// SET level equals `path.pl`'s `invert_path` of the compound pattern; `star`'s reflexive `{a}` seed is
/// concrete (a bound endpoint), so the all-pairs-`star` diagonal-injection concern does not arise here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_compound_closure(
    path: &crate::query::Path,
    reflexive: bool,
    inner: &crate::path::PathPattern,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    // Count-gated compound closure is a follow-up (do the `Distinct` form first), exactly as the
    // both-bound single-predicate closure defers its count gate.
    if count_variable.is_some() {
        return Err(BgpPlanError::ClosureCountGateDeferred);
    }

    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;

    // Endpoint shape: EXACTLY ONE bound endpoint (single-seed). A bound SOURCE walks `E'` forward from
    // `a`; a bound TARGET walks `E'` backward from `b`. Both-free (a 2-column pairs closure over a
    // compound edge) and both-bound (a membership gate over a compound edge) are separate increments,
    // reject-closed here.
    let (seed_iri, endpoint_var, bound_target) = match (&source, &target) {
        (PathTerm::Node(a), PathTerm::Variable(y)) => (a.clone(), y.clone(), false),
        (PathTerm::Variable(x), PathTerm::Node(b)) => (b.clone(), x.clone(), true),
        (PathTerm::Variable(_), PathTerm::Variable(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsFree)
        }
        (PathTerm::Node(_), PathTerm::Node(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsBound)
        }
    };

    // Derive the compound edge relation `E'` (deduped 2-column [start, end]). Its fidelity guards live
    // in `build_compound_edge`.
    let edge = build_compound_edge(inner, path)?;

    let relation = CompiledRelation {
        schema: vec![endpoint_var],
        domains: vec![None],
        statement: CompiledRelationStatement::TransitiveClosureCompound {
            edge: Box::new(edge),
            seed: CanonicalObject::Node(seed_iri.into_bytes()),
            reflexive,
            // Inner inverse orientation is baked into `E'`'s structure (an inverse hop swaps the triple's
            // subject/object), so ONLY a bound TARGET flips the closure walk direction.
            inverted: bound_target,
            // A WOQL `Path` closure walks the INSTANCE graph (folds nothing into the digest).
            graph: ProofGraph::Instance,
        },
    };

    // ENFORCE the endpoint `Distinct` gate (SET semantics) — identical to the single-predicate closure:
    // pass `None` so the `Distinct` is validated as the transparent full-row no-op over the closure's
    // own strict-sorted dedup-free accumulator `A_K`. Anything else rejects closed.
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, None)?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Lower ONE elementary hop `Predicate(p)` / `InversePredicate(p)` to its stored-edge `Triple` between
/// `start` and `end`, returning the triple and the (bare) predicate IRI (used for the sequence
/// distinct-predicate fidelity guard). A forward hop is `Triple(start, p, end)`; an inverse hop is
/// `Triple(end, p, start)` (the edge points the other way). A wildcard predicate or a non-elementary
/// (compound) sub-pattern rejects closed.
fn single_hop_triple(
    pattern: &crate::path::PathPattern,
    start: &PathTerm,
    end: &PathTerm,
) -> Result<(Query, String), BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Predicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            Ok((
                Query::Triple(Triple {
                    subject: start.as_subject(),
                    predicate: NodeValue::Node(predicate.clone()),
                    object: end.as_object(),
                    graph: None,
                }),
                predicate,
            ))
        }
        PathPattern::InversePredicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            // Inverse hop start -> end travels the stored edge `end -> start` (swap subject/object).
            Ok((
                Query::Triple(Triple {
                    subject: end.as_subject(),
                    predicate: NodeValue::Node(predicate.clone()),
                    object: start.as_object(),
                    graph: None,
                }),
                predicate,
            ))
        }
        // A branch / hop that is itself a sequence, union, or (bounded / unbounded) repetition is NOT
        // an elementary edge — this pass models only single-predicate hops inside a compound body.
        _ => Err(BgpPlanError::ClosureCompoundSubpattern),
    }
}

/// Derive the COMPOUND closure edge `E'` — a deduped 2-column [start, end] node/value relation — from a
/// compound body, reusing the audited BGP / union / join / projection machinery (NO new crypto):
///   * flat alternation `(p₁|…|pₙ)` → `Distinct(π_{start,end}(BagUnion(edge(pᵢ))))` — each hop is ONE
///     primitive edge, so it is edge-simple-faithful for ANY predicates;
///   * 2-hop sequence `(p/q)` → `Distinct(π_{start,end}(InnerJoin(edge(p)[start,mid], edge(q)[mid,end])))`
///     — faithful IFF `p ≠ q` (else a primitive edge could repeat across hops; see the fidelity note on
///     [`plan_compound_closure`] and [`BgpPlanError::ClosureCompoundSequenceUnfaithful`]).
/// Every other shape (a sequence of length ≠ 2, a same-predicate 2-sequence, a nested unbounded / `Times`
/// body, a wildcard, or a non-elementary alternation branch) rejects closed.
fn build_compound_edge(
    inner: &crate::path::PathPattern,
    path: &crate::query::Path,
) -> Result<CompiledRelation, BgpPlanError> {
    use crate::path::PathPattern;
    // Fresh, collision-free start / end / mid variables (the edge relation is self-contained: its
    // columns are projected into the closure's single delivered column, so these names never alias a
    // user binding).
    let mut reserved = std::collections::BTreeSet::new();
    collect_reserved_path_variables(&Query::Path(path.clone()), &mut reserved);
    let mut alloc = FreshPathVars::new(reserved);
    let start = PathTerm::Variable(alloc.next());
    let end = PathTerm::Variable(alloc.next());

    let edge_query = match inner {
        PathPattern::Or(or) => {
            if or.or.is_empty() {
                return Err(BgpPlanError::PathEmptyPattern);
            }
            let mut branches = Vec::with_capacity(or.or.len());
            for branch in &or.or {
                branches.push(single_hop_triple(branch, &start, &end)?.0);
            }
            if branches.len() == 1 {
                // A single-branch alternation is just that edge (still a valid — if degenerate —
                // compound edge; the outer single-predicate fast path handles a bare predicate).
                branches.pop().unwrap()
            } else {
                Query::Or(crate::query::Or { or: branches })
            }
        }
        PathPattern::Sequence(seq) => {
            // FIDELITY GATE: only a 2-hop DISTINCT-predicate sequence is edge-simple-faithful.
            if seq.sequence.len() != 2 {
                return Err(BgpPlanError::ClosureCompoundSequenceUnfaithful);
            }
            let mid = PathTerm::Variable(alloc.next());
            let (t0, p0) = single_hop_triple(&seq.sequence[0], &start, &mid)?;
            let (t1, p1) = single_hop_triple(&seq.sequence[1], &mid, &end)?;
            if p0 == p1 {
                return Err(BgpPlanError::ClosureCompoundSequenceUnfaithful);
            }
            Query::And(crate::query::And { and: vec![t0, t1] })
        }
        // Nested unbounded (`Star`/`Plus`) or finite `Times` body — a compound sub-pattern this pass
        // does not faithfully model.
        _ => return Err(BgpPlanError::ClosureCompoundSubpattern),
    };

    // Compile the edge query with the audited BGP machinery. A bare union / join carries no outer
    // projection / count / distinct; a surprising shape rejects closed defensively.
    let compiled = plan_bgp(&edge_query)?;
    if compiled.projection.is_some()
        || compiled.count_variable.is_some()
        || compiled.full_row_distinct
        || compiled.collection.is_some()
    {
        return Err(BgpPlanError::ClosureCompoundSubpattern);
    }

    // Project to exactly [start, end] (dropping the sequence's middle waypoint), then dedup on the full
    // 2-tuple — `E'` is the SET of compound edges (path.pl's edge-simple SET semantics).
    let projected = project_path_endpoints(compiled.relation, &start, &end)?;
    if projected.schema.len() != 2 {
        return Err(BgpPlanError::ClosureCompoundSubpattern);
    }
    let schema = projected.schema.clone();
    let domains = projected.domains.clone();
    Ok(CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::Distinct {
            input: Box::new(projected),
            key_columns: vec![0, 1],
        },
    })
}


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

    // The closure body must be a SINGLE concrete predicate; a compound body (sequence/union/nested
    // repetition) needs the general BGP-in-the-loop lowering (deferred). A wildcard `.` rejects closed.
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
        _ => return Err(BgpPlanError::ClosureCompoundSubpattern),
    };

    // Exactly one endpoint bound (the seed) and one free (the delivered reachable-set column). A bound
    // TARGET mirrors the traversal to the inverse edge (path.pl:129-134 `run_pattern`, `invert_path`).
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
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


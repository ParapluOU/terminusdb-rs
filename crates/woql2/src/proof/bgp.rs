//! Basic-graph-pattern planning — the core left-deep join compiler.
//!
//! This is the heart of the compiler. [`plan_bgp`] turns a conjunctive triple query (an `And`
//! of triples under optional `Select` / routing wrappers) into an authenticated left-deep join
//! plan. It lifts each triple to an [`Atom`] via [`atom_from_triple`] (with the per-atom safety
//! checks), attaches numeric range filters, and chains the atoms with [`plan_left_deep`] —
//! natural joins on shared variables, verifier-known unit-key Cartesian products across
//! disconnected components. Also here: the sequential read-operator / `Or` / `Distinct` /
//! projection composition, the equality- and range-constant encoding, and the structural
//! validation that keeps the emitted plan variable-blind and faithful. Anything the plan cannot
//! encode is rejected closed (see the `errors` submodule). [`Atom`] and [`TypedTriple`] — the
//! planner's per-triple primitives — are defined here alongside their users.

use super::*;

/// A per-atom relation, ready for the variable-arity planner. `layer_atom` is the
/// variable-blind atom emitted into the plan (subject/predicate/object terms). `kept`
/// is the atom's VARIABLE names in S,P,O order — the relation's output columns, one per
/// variable. An all-variable atom keeps two columns `[subj_var, obj_var]`; a
/// subject/object-constant atom keeps one. The planner joins on shared `kept` names and
/// `rel_key` indexes into `kept`.
pub(crate) struct Atom {
    layer_atom: LayerBgpAtom,
    kept: Vec<String>,
    /// The subject variable name, if the subject is a variable (else the subject is a
    /// constant). Used to route comparison conjuncts to the right object-binding atom.
    subj_var: Option<String>,
    /// Predicate variables use the independent predicate dictionary ID namespace.
    pred_var: Option<String>,
    /// The object variable name, if the object is a variable (else a constant). A numeric
    /// range attaches to the unique atom whose `obj_var` is the compared variable.
    obj_var: Option<String>,
}

#[derive(Clone)]
pub(crate) struct TypedTriple {
    pub(crate) triple: Triple,
    pub(crate) object_kind: ObjectKindFilter,
    /// Which committed column set this atom reads. A plain `Triple`/`Link`/`Data` reads the
    /// materialized head view; a WOQL `AddedTriple`/`DeletedTriple` reads this layer's own
    /// addition/removal delta file. Threaded per-atom onto [`LayerBgpAtom::source`] (via
    /// [`atom_from_triple`]), so a BGP leaf MAY mix sources — a cross-source join. The plan
    /// DEFAULT is [`plan_default_column_source`] (the uniform source, else materialized).
    pub(crate) source: ColumnSource,
}


/// Compile a woql2 conjunctive BGP query into a safe [`CompiledBgp`].
///
/// Accepts nested outer [`Select`]s and supported routing/result wrappers around an
/// `And` of `>= 1` triples of
/// the form `?subj <constant IRI> ?obj`, whose atoms connect into a
/// left-deep join tree (each step shares one or two variables). Everything
/// else is rejected closed with a descriptive [`BgpPlanError`].
pub fn plan_bgp(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    // (0a) Reject the triple-SHAPED-but-not-BGP leaves (`TypeOf`/`Dot`/`IsA`) up front —
    // before any lowering or shape analysis — so a NESTED occurrence gets its specific
    // soundness rationale and the guard COMPOSES with the path lowering below (a query
    // carrying both a `Path` and one of these leaves rejects on the leaf). See
    // `reject_non_bgp_typed_leaves` for why each is unsound to lower to a store triple.
    reject_non_bgp_typed_leaves(query)?;

    // (0b) Lower fixed-length property paths (`Query::Path`) to BGP + bag union before
    // any shape analysis, so all soundness reuses the audited BGP machinery below.
    let lowered_query;
    let query = if query_contains_path(query) {
        let mut reserved = std::collections::BTreeSet::new();
        collect_reserved_path_variables(query, &mut reserved);
        let mut alloc = FreshPathVars::new(reserved);
        lowered_query = rewrite_query_paths(query, &mut alloc)?;
        &lowered_query
    } else {
        query
    };

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
    let mut distincts: Vec<&[String]> = Vec::new();
    let mut body = query;
    loop {
        body = match body {
            Query::Select(select) => {
                selections.push((select.variables.as_slice(), count_variable.is_none()));
                select.query.as_ref()
            }
            Query::Using(using) => {
                // Nested Using scopes replace the active collection for their inner query;
                // the innermost scope is therefore the layer-routing metadata we retain.
                collection = Some(using.collection.clone());
                using.query.as_ref()
            }
            Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => from.query.as_ref(),
            Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
            Query::Pin(pin) => pin.query.as_ref(),
            Query::Immediately(immediately) => immediately.query.as_ref(),
            Query::Distinct(distinct) => {
                distincts.push(distinct.variables.as_slice());
                distinct.query.as_ref()
            }
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

    // A finite single-predicate REPEATED-predicate path that `lower_path` DEFERRED (a plain
    // bag-join over-counts edge-simple solutions). Compile it at the plan level: the join
    // chain + a `TupleDisequality` edge-simple filter + endpoint projection. A deferred path
    // that is NOT the whole body (nested inside a larger `And`) reaches the conjunctive
    // compiler as a non-triple conjunct and rejects closed there.
    if let Query::Path(path) = body {
        // A `Star`/`Plus` closure routes to the transitive-closure planner; every other deferred path
        // (single-predicate repeat / `Times`) routes to the edge-simple TupleDisequality planner.
        if matches!(
            path.pattern,
            crate::path::PathPattern::Star(_) | crate::path::PathPattern::Plus(_)
        ) {
            return plan_transitive_closure_path(
                path,
                &selections,
                collection,
                count_variable,
                &distincts,
            );
        }
        return plan_deferred_repeated_predicate_path(
            path,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }

    // SCHEMA-GRAPH FOUNDATION — `Subsumption child << parent` compiles to the reflexive-transitive
    // `sys:inherits` closure over the SCHEMA graph, Distinct-gated (the reference's intrinsic
    // `distinct([child,parent])`). Handled here — after outer-control unwrapping, alongside the path
    // closure — so it reuses the same audited transitive-closure machinery, only pointed at the schema
    // graph. Every unsupported subsumption shape (both endpoints free, count-gated, …) rejects closed.
    if let Query::Subsumption(sub) = body {
        return plan_subsumption(sub, &selections, collection, count_variable, &distincts);
    }

    // SCHEMA-GRAPH FOUNDATION — `IsA(?x, C)` (node regime, bound class) lowers to a CROSS-DICTIONARY
    // two-root hash join: the downward `sys:inherits` closure of `C` (schema graph) hash-joined against
    // the instance `rdf:type` relation (instance graph). Standalone only (a nested `IsA` inside a BGP
    // conjunction rejects via `reject_non_bgp_typed_leaves`). See [`plan_isa`].
    if let Query::IsA(isa) = body {
        return plan_isa(isa, &selections, collection, count_variable, &distincts);
    }

    if matches!(body, Query::Start(_) | Query::Limit(_) | Query::Once(_)) {
        if (distincts.is_empty() && (!selections.is_empty() || count_variable.is_some()))
            || (!distincts.is_empty() && count_variable.is_none())
        {
            return Err(BgpPlanError::OrderedSliceUnsupportedShape);
        }
        let mut compiled = plan_exact_ordered_slice(body)?;
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Query::OrderBy(order_by) = body {
        if (distincts.is_empty() && (!selections.is_empty() || count_variable.is_some()))
            || (!distincts.is_empty() && count_variable.is_none())
        {
            return Err(BgpPlanError::OrderByUnsupportedShape);
        }
        let mut compiled = plan_exact_scalar_order(order_by)?;
        // Prolog's nested routing scope is innermost-wins. Preserve an inner collection
        // already found while compiling the ordered child; otherwise retain this outer one.
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Some(mut compiled) = plan_ordered_triple_value_construct(body)? {
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Some(mut compiled) = plan_triple_count_comparison_body(body)? {
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Query::Or(disjunction) = body {
        return plan_or_relation(
            disjunction,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }

    if let Some(mut aggregate) = plan_collection_extrema_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_collection_sum_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_collection_count_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_count_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_sum_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_extrema_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut evaluated) = plan_eval_form(body)? {
        if collection.is_some() {
            if evaluated.collection.is_some() && evaluated.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            evaluated.collection = collection;
        }
        evaluated.count_variable = count_variable;
        evaluated.projection = compile_projection(
            &selections,
            evaluated.count_variable.as_deref(),
            &evaluated.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&evaluated.relation)?;
        return Ok(evaluated);
    }

    if let Some(mut cast) = plan_typecast_form(body)? {
        if collection.is_some() {
            if cast.collection.is_some() && cast.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            cast.collection = collection;
        }
        cast.count_variable = count_variable;
        cast.projection = compile_projection(
            &selections,
            cast.count_variable.as_deref(),
            &cast.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&cast.relation)?;
        return Ok(cast);
    }

    if let Some(mut concatenated) = plan_concat_form(body)? {
        if collection.is_some() {
            if concatenated.collection.is_some() && concatenated.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            concatenated.collection = collection;
        }
        // The concat result is terminal/revealed (not a committed schema column), so it cannot be
        // projected or counted; a Select/Count naming it fails via `compile_projection`'s
        // SelectedVariableNotInBgp / a count over the child columns. Distinct is rejected too.
        concatenated.count_variable = count_variable;
        concatenated.projection = compile_projection(
            &selections,
            concatenated.count_variable.as_deref(),
            &concatenated.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&concatenated.relation)?;
        return Ok(concatenated);
    }

    if let Some(mut joined) = plan_join_form(body)? {
        if collection.is_some() {
            if joined.collection.is_some() && joined.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            joined.collection = collection;
        }
        // The join result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        joined.count_variable = count_variable;
        joined.projection = compile_projection(
            &selections,
            joined.count_variable.as_deref(),
            &joined.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&joined.relation)?;
        return Ok(joined);
    }

    if let Some(mut substr) = plan_substring_form(body)? {
        if collection.is_some() {
            if substr.collection.is_some() && substr.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            substr.collection = collection;
        }
        // The substring result is terminal/revealed (not a committed schema column): a
        // Select/Count naming it (or `?after`) fails via `compile_projection`; Distinct too.
        substr.count_variable = count_variable;
        substr.projection = compile_projection(
            &selections,
            substr.count_variable.as_deref(),
            &substr.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&substr.relation)?;
        return Ok(substr);
    }

    if let Some(mut padded) = plan_pad_form(body)? {
        if collection.is_some() {
            if padded.collection.is_some() && padded.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            padded.collection = collection;
        }
        // The pad result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        padded.count_variable = count_variable;
        padded.projection = compile_projection(
            &selections,
            padded.count_variable.as_deref(),
            &padded.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&padded.relation)?;
        return Ok(padded);
    }

    if let Some(mut hashed) = plan_hashkey_form(body)? {
        if collection.is_some() {
            if hashed.collection.is_some() && hashed.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            hashed.collection = collection;
        }
        // The hash result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        hashed.count_variable = count_variable;
        hashed.projection = compile_projection(
            &selections,
            hashed.count_variable.as_deref(),
            &hashed.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&hashed.relation)?;
        return Ok(hashed);
    }

    if let Some(mut calendar) = plan_calendar_form(body)? {
        if collection.is_some() {
            if calendar.collection.is_some() && calendar.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            calendar.collection = collection;
        }
        // Integer-result calendar ops APPEND their result column(s) to the schema (Select/Count over
        // them works); date-result calendar ops keep the result TERMINAL/revealed (a Select/Count
        // naming it fails via `compile_projection`, exactly like Concat). Distinct is rejected too.
        calendar.count_variable = count_variable;
        calendar.projection = compile_projection(
            &selections,
            calendar.count_variable.as_deref(),
            &calendar.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&calendar.relation)?;
        return Ok(calendar);
    }

    // Deferred string-case ops (`Upper`/`Lower`) reject closed with a documented locale reason
    // (see `reject_unsupported_string_case_ops`) before the generic UnsupportedConstruct path.
    reject_unsupported_string_case_ops(body)?;

    if has_top_level_read_operator(body) {
        return plan_sequential_read_operators(
            body,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }
    if !matches!(
        body,
        Query::And(_)
            | Query::Triple(_)
            | Query::Link(_)
            | Query::Data(_)
            | Query::AddedTriple(_)
            | Query::DeletedTriple(_)
    ) {
        return Err(BgpPlanError::UnsupportedConstruct {
            construct: query_variant_name(body),
        });
    }

    // (1b) Flatten nested Ands. Triple leaves remain kind-agnostic; Link/Data leaves
    // explicitly authenticate the node/literal half of the Store object dictionary.
    // Comparisons become object-value range filters. True and Comment contribute no
    // bindings and are proof-transparent conjunction leaves.
    let mut triples: Vec<TypedTriple> = Vec::new();
    let mut comparisons: Vec<NormalizedRange> = Vec::new();
    collect_conjunct(body, &mut triples, &mut comparisons)?;

    // (1c) CP-D: each atom carries its OWN column source (materialized head, or this layer's
    // additions/removals delta), so a CROSS-SOURCE conjunction — e.g. `AddedTriple(?x,p,?y) &
    // Triple(?y,q,?z)` (additions ⋈ materialized) or additions ⋈ removals — is now supported:
    // all sources share the layer's predicate/node-value dictionaries, so their IDs are
    // directly comparable and the audited store join gadget joins them unchanged. We only
    // compute the plan DEFAULT source here (the uniform source when every atom agrees, else
    // materialized); the per-atom sources are threaded onto each `LayerBgpAtom` below.
    let source = plan_default_column_source(&triples);

    // (2) Lift each triple to an Atom, with all per-atom safety checks.
    let mut atoms: Vec<Atom> = triples
        .iter()
        .map(atom_from_triple)
        .collect::<Result<_, _>>()?;

    let k = atoms.len();
    if k == 0 {
        return Err(BgpPlanError::TooFewAtoms { count: k });
    }
    validate_variable_namespaces(&atoms)?;

    // (2b) Phase 7.4: route each comparison conjunct to a numeric RANGE filter on the
    // object of the (unique) atom that binds the compared variable. Consumes the
    // comparisons; the ranged variable stays a kept output column.
    attach_ranges(&mut atoms, &comparisons)?;

    // The Store's mixed-arity plan is a relational prefix followed by zero-arity
    // existence bits. Stable ordering preserves query order within both groups.
    atoms.sort_by_key(|atom| atom.kept.is_empty());

    // (3) Shape guard + left-deep planner: the safety-critical core. Each ordered plan atom
    // already carries its own `source` (threaded via `atom_from_triple` and cloned through by
    // `plan_left_deep`), so cross-source joins are represented directly.
    let (mut plan, schema) = plan_left_deep(&atoms)?;
    // Stamp the plan DEFAULT source (uniform source, else materialized) — the transcript
    // baseline the store measures per-atom overrides against, and the domain of the all-ground
    // unit. The verifier reconstructs this same plan (default + every atom's source) from its
    // own trusted query, so all sources are fixed by trusted compilation; `plan_digest`/the
    // Fiat–Shamir transcript additionally bind them.
    plan.source = source;
    let domains = infer_schema_domains(&schema, &atoms, &comparisons);

    let relation = CompiledRelation {
        schema: schema.clone(),
        domains,
        statement: CompiledRelationStatement::Bgp(plan),
    };
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        &distincts,
        &selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(
        &selections,
        count_variable.as_deref(),
        &relation.schema,
    )?;

    Ok(CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    })
}

pub(crate) fn infer_schema_domains(
    schema: &[String],
    atoms: &[Atom],
    comparisons: &[NormalizedRange],
) -> Vec<Option<EqualityValueDomain>> {
    schema
        .iter()
        .map(|variable| {
            if atoms
                .iter()
                .any(|atom| atom.pred_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Predicate)
            } else if atoms
                .iter()
                .any(|atom| atom.subj_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Node)
            } else {
                comparisons
                    .iter()
                    .find(|comparison| &comparison.var == variable)
                    .map(|comparison| EqualityValueDomain::Datatype(comparison.canonical_datatype))
            }
        })
        .collect()
}

pub(crate) fn plan_or_relation(
    disjunction: &crate::query::Or,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if disjunction.or.len() < 2 {
        return Err(BgpPlanError::OrTooFewBranches {
            count: disjunction.or.len(),
        });
    }
    let mut compiled = Vec::with_capacity(disjunction.or.len());
    for query in &disjunction.or {
        let branch = plan_bgp(query)?;
        let internal_ground_projection = branch.relation.schema.is_empty()
            && branch.projection.as_deref() == Some(&[]);
        if (branch.projection.is_some() && !internal_ground_projection)
            || branch.count_variable.is_some()
            || branch.full_row_distinct
        {
            return Err(BgpPlanError::OrUnsupportedBranchWrapper);
        }
        compiled.push(branch);
    }
    let schema = compiled[0].relation.schema.clone();
    let mut effective_collection = collection;
    for branch in &compiled {
        merge_collection(
            &mut effective_collection,
            branch.collection.as_ref(),
            BgpPlanError::OrRoutingConflict,
        )?;
    }

    let mut branches = Vec::new();
    let mut mappings = Vec::new();
    let mut expected_descriptors = None;
    let mut merged_domains: Option<Vec<Option<EqualityValueDomain>>> = None;
    for branch in compiled {
        let mapping = schema_mapping(&schema, &branch.relation.schema)
            .ok_or(BgpPlanError::OrIncompatibleSchema)?;
        let descriptors = relation_result_descriptors(&branch.relation)
            .map_err(|_| BgpPlanError::OrIncompatibleNamespaces)?;
        let mapped: Vec<_> = mapping.iter().map(|index| descriptors[*index]).collect();
        if expected_descriptors
            .as_ref()
            .is_some_and(|expected| expected != &mapped)
        {
            return Err(BgpPlanError::OrIncompatibleNamespaces);
        }
        expected_descriptors = Some(mapped);
        let mapped_domains = mapping
            .iter()
            .map(|index| branch.relation.domains[*index])
            .collect::<Vec<_>>();
        if let Some(prior) = &mut merged_domains {
            for (prior, current) in prior.iter_mut().zip(&mapped_domains) {
                if *prior != *current {
                    *prior = None;
                }
            }
        } else {
            merged_domains = Some(mapped_domains);
        }
        match branch.relation.statement {
            CompiledRelationStatement::BagUnion {
                branches: nested,
                mappings: nested_mappings,
            } => {
                for (nested, nested_mapping) in nested.into_iter().zip(nested_mappings) {
                    mappings.push(mapping.iter().map(|index| nested_mapping[*index]).collect());
                    branches.push(nested);
                }
            }
            statement => {
                mappings.push(if schema.is_empty() {
                    Vec::new()
                } else {
                    mapping
                });
                branches.push(CompiledRelation {
                    schema: branch.relation.schema,
                    domains: branch.relation.domains,
                    statement,
                });
            }
        }
    }
    let relation = CompiledRelation {
        schema,
        domains: merged_domains.unwrap_or_default(),
        statement: CompiledRelationStatement::BagUnion { branches, mappings },
    };
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        distincts,
        selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(
        selections,
        count_variable.as_deref(),
        &relation.schema,
    )?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

pub(crate) enum ReadOperator {
    Optional(Query),
    Not(Query),
    Equals(crate::compare::Equals),
    When(crate::control::When),
}

pub(crate) fn has_top_level_read_operator(query: &Query) -> bool {
    match query {
        Query::And(and) => and.and.iter().any(has_top_level_read_operator),
        Query::WoqlOptional(_) | Query::Not(_) | Query::Equals(_) | Query::If(_) => true,
        _ => false,
    }
}

pub(crate) fn collect_read_operator_sequence(
    query: &Query,
    positive: &mut Vec<Query>,
    operators: &mut Vec<ReadOperator>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::And(and) => {
            for child in &and.and {
                collect_read_operator_sequence(child, positive, operators)?;
            }
        }
        Query::WoqlOptional(optional) => {
            operators.push(ReadOperator::Optional((*optional.query).clone()))
        }
        Query::Not(not) => operators.push(ReadOperator::Not((*not.query).clone())),
        Query::Equals(equals) => operators.push(ReadOperator::Equals(equals.clone())),
        Query::If(when) => operators.push(ReadOperator::When(when.clone())),
        other if operators.is_empty() => positive.push(other.clone()),
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) enum EqualityOperand {
    Variable(String),
    Constant(CanonicalObject),
}

pub(crate) fn equality_operand(value: &crate::value::WoqlValue) -> Result<EqualityOperand, BgpPlanError> {
    match value {
        crate::value::WoqlValue::Variable(variable) => {
            Ok(EqualityOperand::Variable(variable.clone()))
        }
        crate::value::WoqlValue::Node(iri) => Ok(EqualityOperand::Constant(CanonicalObject::Node(
            iri.clone().into_bytes(),
        ))),
        crate::value::WoqlValue::Data(value) => {
            let term = value_constant_term(value)?;
            match term {
                AtomTerm::ValueConstant(datatype, lexical) => {
                    Ok(EqualityOperand::Constant(CanonicalObject::Value {
                        datatype,
                        lexical,
                    }))
                }
                _ => unreachable!("value_constant_term returned a non-value"),
            }
        }
        crate::value::WoqlValue::Dictionary(_) | crate::value::WoqlValue::List(_) => {
            Err(BgpPlanError::EqualsUnsupportedOperand)
        }
    }
}

pub(crate) fn numeric_equality_datatype(datatype: CanonicalDatatype) -> bool {
    use CanonicalDatatype::*;
    matches!(
        datatype,
        UInt8
            | Int8
            | UInt16
            | Int16
            | UInt32
            | Int32
            | Float32
            | UInt64
            | Int64
            | Float64
            | Decimal
            | BigInt
            | PositiveInteger
            | NonNegativeInteger
            | NonPositiveInteger
            | NegativeInteger
    )
}

pub(crate) fn bound_equality_column(
    relation: &CompiledRelation,
    variable: &str,
) -> Result<(usize, EqualityValueDomain), BgpPlanError> {
    let column = relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::EqualsUnboundVariable {
            var: variable.to_string(),
        })?;
    let descriptors =
        relation_result_descriptors(relation).map_err(|_| BgpPlanError::EqualsDomainMismatch)?;
    if descriptors[column].nullable {
        return Err(BgpPlanError::EqualsNullableVariable {
            var: variable.to_string(),
        });
    }
    let domain = relation.domains[column].ok_or_else(|| BgpPlanError::EqualsUnknownDomain {
        var: variable.to_string(),
    })?;
    Ok((column, domain))
}

pub(crate) fn constant_matches_domain(value: &CanonicalObject, domain: EqualityValueDomain) -> bool {
    match (value, domain) {
        (CanonicalObject::Node(_), EqualityValueDomain::Node) => true,
        (CanonicalObject::Value { datatype, .. }, EqualityValueDomain::Datatype(expected)) => {
            *datatype == expected
        }
        _ => false,
    }
}

pub(crate) fn compile_equality_constraint(
    relation: &CompiledRelation,
    equals: &crate::compare::Equals,
) -> Result<RelationEqualityPlan, BgpPlanError> {
    let left = equality_operand(&equals.left)?;
    let right = equality_operand(&equals.right)?;
    match (left, right) {
        (EqualityOperand::Variable(left), EqualityOperand::Variable(right)) => {
            let (left_column, left_domain) = bound_equality_column(relation, &left)?;
            let (right_column, right_domain) = bound_equality_column(relation, &right)?;
            if left_domain != right_domain {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::Columns {
                left: left_column,
                right: right_column,
                domain: left_domain,
            })
        }
        (EqualityOperand::Variable(variable), EqualityOperand::Constant(value))
        | (EqualityOperand::Constant(value), EqualityOperand::Variable(variable)) => {
            let constant_on_left = matches!(
                equality_operand(&equals.left)?,
                EqualityOperand::Constant(_)
            );
            let (column, domain) = bound_equality_column(relation, &variable)?;
            if !constant_matches_domain(&value, domain) {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::ColumnConstant {
                column,
                domain,
                value,
                constant_on_left,
            })
        }
        (EqualityOperand::Constant(left), EqualityOperand::Constant(right)) => {
            if let (
                CanonicalObject::Value { datatype: left, .. },
                CanonicalObject::Value {
                    datatype: right, ..
                },
            ) = (&left, &right)
            {
                if left != right
                    && (numeric_equality_datatype(*left) || numeric_equality_datatype(*right))
                {
                    return Err(BgpPlanError::EqualsCoerciveNumeric);
                }
            }
            Ok(RelationEqualityPlan::Constants { left, right })
        }
    }
}

pub(crate) fn inner_join_relation(
    left: CompiledRelation,
    right: CompiledRelation,
    key_pairs: Vec<(usize, usize)>,
) -> CompiledRelation {
    let shared_right = key_pairs.iter().map(|pair| pair.1).collect::<Vec<_>>();
    let mut schema = left.schema.clone();
    let mut domains = left.domains.clone();
    schema.extend(
        right
            .schema
            .iter()
            .enumerate()
            .filter(|(index, _)| !shared_right.contains(index))
            .map(|(_, name)| name.clone()),
    );
    domains.extend(
        right
            .domains
            .iter()
            .enumerate()
            .filter(|(index, _)| !shared_right.contains(index))
            .map(|(_, domain)| *domain),
    );
    CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::InnerJoin {
            left: Box::new(left),
            right: Box::new(right),
            key_pairs,
        },
    }
}

pub(crate) fn first_local_in_schema(schema: &[String], locals: &[String]) -> Option<String> {
    schema.iter().find(|name| locals.contains(name)).cloned()
}

pub(crate) fn later_operator_local(
    operator: &ReadOperator,
    locals: &[String],
) -> Result<Option<String>, BgpPlanError> {
    match operator {
        ReadOperator::Optional(query) | ReadOperator::Not(query) => {
            let compiled = plan_bgp(query)?;
            Ok(first_local_in_schema(&compiled.relation.schema, locals))
        }
        ReadOperator::Equals(equals) => {
            for value in [&equals.left, &equals.right] {
                if let Value::Variable(name) = value {
                    if locals.contains(name) {
                        return Ok(Some(name.clone()));
                    }
                }
            }
            Ok(None)
        }
        ReadOperator::When(when) => {
            let condition = plan_bgp(&when.query)?;
            if let Some(local) = first_local_in_schema(&condition.relation.schema, locals) {
                return Ok(Some(local));
            }
            let consequent = plan_bgp(&when.consequent)?;
            Ok(first_local_in_schema(&consequent.relation.schema, locals))
        }
    }
}

pub(crate) fn plan_sequential_read_operators(
    query: &Query,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    let mut positive = Vec::new();
    let mut operators = Vec::new();
    collect_read_operator_sequence(query, &mut positive, &mut operators)?;
    if positive.is_empty() {
        return Err(match operators.first() {
            Some(ReadOperator::Optional(_)) => BgpPlanError::OptionalMissingOuter,
            Some(ReadOperator::Equals(_)) => BgpPlanError::EqualsMissingInput,
            Some(ReadOperator::When(_)) => BgpPlanError::WhenMissingOuter,
            _ => BgpPlanError::NotMissingOuter,
        });
    }
    let positive_query = if positive.len() == 1 {
        positive.pop().unwrap()
    } else {
        Query::And(crate::query::And { and: positive })
    };
    let left = plan_bgp(&positive_query)?;
    let mut relation = left.relation;
    let mut effective_collection = collection;
    let first_optional = matches!(operators.first(), Some(ReadOperator::Optional(_)));
    merge_collection(
        &mut effective_collection,
        left.collection.as_ref(),
        if first_optional {
            BgpPlanError::OptionalRoutingConflict
        } else {
            BgpPlanError::NotRoutingConflict
        },
    )?;

    for (operator_index, operator) in operators.iter().enumerate() {
        if let ReadOperator::When(when) = operator {
            let true_branch = relation.clone();
            let condition = plan_bgp(&when.query)?;
            reject_when_subplan(&condition)?;
            merge_collection(&mut effective_collection, condition.collection.as_ref(), BgpPlanError::WhenRoutingConflict)?;
            let condition_keys = when_key_pairs(&relation, &condition.relation)?;
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            let matched_condition = inner_join_relation(relation, condition.relation, condition_keys);

            let consequent = plan_bgp(&when.consequent)?;
            reject_when_subplan(&consequent)?;
            merge_collection(&mut effective_collection, consequent.collection.as_ref(), BgpPlanError::WhenRoutingConflict)?;
            let consequent_keys = when_key_pairs(&matched_condition, &consequent.relation)?;
            let successful = inner_join_relation(
                matched_condition,
                consequent.relation,
                consequent_keys,
            );
            let locals = successful
                .schema
                .iter()
                .filter(|name| !schema.contains(name))
                .cloned()
                .collect::<Vec<_>>();
            if !locals.is_empty() {
                let hidden_by_result_wrapper = count_variable.is_some()
                    || (!selections.is_empty()
                        && selections.iter().all(|(variables, _)| {
                            variables.iter().all(|name| !locals.contains(name))
                        }));
                if !hidden_by_result_wrapper {
                    return Err(BgpPlanError::WhenBranchLocalsEscape { vars: locals });
                }
                for later in &operators[operator_index + 1..] {
                    if let Some(var) = later_operator_local(later, &locals)? {
                        return Err(BgpPlanError::WhenBranchLocalUsedLater { var });
                    }
                }
            }
            let successful = if locals.is_empty() {
                successful
            } else {
                let indices = schema
                    .iter()
                    .map(|name| {
                        successful
                            .schema
                            .iter()
                            .position(|candidate| candidate == name)
                            .expect("outer When column remains in the joined relation")
                    })
                    .collect();
                CompiledRelation {
                    schema: schema.clone(),
                    domains: domains.clone(),
                    statement: CompiledRelationStatement::BagProjection {
                        input: Box::new(successful),
                        indices,
                    },
                }
            };
            let identity = (0..schema.len()).collect::<Vec<_>>();
            let expanded = CompiledRelation {
                schema: schema.clone(), domains: domains.clone(),
                statement: CompiledRelationStatement::BagUnion {
                    branches: vec![successful, true_branch],
                    mappings: vec![identity.clone(), identity],
                },
            };
            relation = CompiledRelation {
                schema, domains,
                statement: CompiledRelationStatement::When { expanded: Box::new(expanded) },
            };
            continue;
        }
        if let ReadOperator::Equals(equals) = operator {
            let constraint = compile_equality_constraint(&relation, &equals)?;
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::Equality {
                    input: Box::new(relation),
                    constraint,
                },
            };
            continue;
        }
        let (right_query, optional) = match operator {
            ReadOperator::Optional(query) => (query, true),
            ReadOperator::Not(query) => (query, false),
            ReadOperator::Equals(_) => unreachable!(),
            ReadOperator::When(_) => unreachable!(),
        };
        let right = plan_bgp(right_query)?;
        reject_subplan_wrappers(&right, optional)?;
        merge_collection(
            &mut effective_collection,
            right.collection.as_ref(),
            if optional {
                BgpPlanError::OptionalRoutingConflict
            } else {
                BgpPlanError::NotRoutingConflict
            },
        )?;
        let key_pairs = shared_key_pairs(&relation, &right.relation, optional)?;
        if optional {
            let mut schema = relation.schema.clone();
            let mut domains = relation.domains.clone();
            domains.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .zip(&right.relation.domains)
                    .filter(|(name, _)| !relation.schema.contains(name))
                    .map(|(_, domain)| *domain),
            );
            schema.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .filter(|name| !relation.schema.contains(name))
                    .cloned(),
            );
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftOuter {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        } else {
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftAnti {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        }
    }
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        distincts,
        selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

pub(crate) fn validate_compiled_structure(relation: &CompiledRelation) -> Result<(), BgpPlanError> {
    fn visit(
        relation: &CompiledRelation,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), BgpPlanError> {
        if depth > MAX_RELATION_PROOF_DEPTH || *nodes >= MAX_RELATION_PROOF_NODES {
            return Err(BgpPlanError::RecursiveStructureLimit);
        }
        *nodes += 1;
        match &relation.statement {
            CompiledRelationStatement::Bgp(_)
            | CompiledRelationStatement::LayerTripleCount
            // The transitive closure is a leaf composite: it carries a raw edge BGP plan, not a
            // CompiledRelation child, so there is nothing further to recurse into here.
            | CompiledRelationStatement::TransitiveClosure { .. }
            // The ordered-list walk is likewise a leaf: it carries a bound (subject, predicate), not
            // a CompiledRelation child, so there is nothing to recurse into.
            | CompiledRelationStatement::OrderedListWalk { .. } => Ok(()),
            CompiledRelationStatement::GeneratedU64Filter { input, .. }
            | CompiledRelationStatement::GeneratedI64Filter { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::BagUnion { branches, .. } => {
                for branch in branches {
                    visit(branch, depth + 1, nodes)?;
                }
                Ok(())
            }
            CompiledRelationStatement::BagProjection { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::Distinct { input, key_columns } => {
                if !(1..=2).contains(&key_columns.len()) {
                    return Err(BgpPlanError::DistinctUnsupportedKeyCount {
                        count: key_columns.len(),
                    });
                }
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::When { expanded } => visit(expanded, depth + 1, nodes),
            CompiledRelationStatement::LeftAnti { left, right, .. }
            | CompiledRelationStatement::InnerJoin { left, right, .. }
            | CompiledRelationStatement::CrossRootHashJoin { left, right, .. }
            | CompiledRelationStatement::LeftOuter { left, right, .. } => {
                visit(left, depth + 1, nodes)?;
                visit(right, depth + 1, nodes)
            }
            CompiledRelationStatement::Equality { input, .. }
            | CompiledRelationStatement::TupleDisequality { input, .. }
            | CompiledRelationStatement::ExactScalarOrder { input, .. }
            | CompiledRelationStatement::ExactMultiOrder { input, .. }
            | CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::GroupExtrema { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::GroupSum { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::GroupCount { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionCount { input } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionSum { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionExtrema { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::Eval { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::FloorEval { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::RationalArith { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Typecast { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Concat { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Join { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Substring { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Pad { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::HashKey { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CalendarInt { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CalendarDate { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::TypecastNumberToString { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::TypecastStringToNumber { input, .. } => {
                visit(input, depth + 1, nodes)
            }
        }
    }
    let mut nodes = 0;
    visit(relation, 1, &mut nodes)
}

/// SCHEMA-GRAPH FOUNDATION — the SINGLE committed graph a compiled query reads (see [`ProofGraph`]).
///
/// Every triple-reading leaf reads one graph: a `Bgp`/`LayerTripleCount` reads the INSTANCE graph;
/// a `TransitiveClosure` reads its own `graph` selector (Instance for a WOQL path, Schema for
/// Subsumption). Wrappers (Distinct/Select/joins/aggregations/…) inherit their children's. This pass
/// only ever emits SINGLE-GRAPH queries — instance-only, or the schema-only Subsumption closure — so
/// the whole proof anchors to ONE `LayerCommitment` + `commitment_root`, and the audited recursion is
/// handed exactly that layer (it stays graph-blind).
///
/// A query that MIXES an instance leaf with a schema leaf (the deferred cross-layer `IsA` regime — a
/// schema subsumption set joined against instance `rdf:type` triples) is REJECTED CLOSED here: this
/// pass does not thread two live views through a single BGP. An empty/ground relation (no reading
/// leaf) defaults to `Instance`, byte-identical to today.
pub(crate) fn relation_graph(relation: &CompiledRelation) -> std::io::Result<ProofGraph> {
    fn combine(acc: &mut Option<ProofGraph>, g: ProofGraph) -> std::io::Result<()> {
        match *acc {
            None => {
                *acc = Some(g);
                Ok(())
            }
            Some(prev) if prev == g => Ok(()),
            Some(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cross-layer query mixes instance and schema graph leaves (deferred); reject closed",
            )),
        }
    }
    fn walk(relation: &CompiledRelation, acc: &mut Option<ProofGraph>) -> std::io::Result<()> {
        match &relation.statement {
            // Instance-graph reading leaves.
            CompiledRelationStatement::Bgp(_) | CompiledRelationStatement::LayerTripleCount => {
                combine(acc, ProofGraph::Instance)
            }
            // The closure is the ONE graph-parameterized leaf.
            CompiledRelationStatement::TransitiveClosure { graph, .. } => combine(acc, *graph),
            // The ordered-list walk carries its own graph selector too (always `Instance` in this
            // increment — a stored `rdf:List` lives in the instance graph).
            CompiledRelationStatement::OrderedListWalk { graph, .. } => combine(acc, *graph),
            // Wrappers: inherit their children's graph (mirrors `validate_compiled_structure::visit`).
            CompiledRelationStatement::GeneratedU64Filter { input, .. }
            | CompiledRelationStatement::GeneratedI64Filter { input, .. }
            | CompiledRelationStatement::BagProjection { input, .. }
            | CompiledRelationStatement::Distinct { input, .. }
            | CompiledRelationStatement::When { expanded: input }
            | CompiledRelationStatement::Equality { input, .. }
            | CompiledRelationStatement::TupleDisequality { input, .. }
            | CompiledRelationStatement::ExactScalarOrder { input, .. }
            | CompiledRelationStatement::ExactMultiOrder { input, .. }
            | CompiledRelationStatement::ExactOrderedSlice { input, .. }
            | CompiledRelationStatement::GroupExtrema { input, .. }
            | CompiledRelationStatement::GroupSum { input, .. }
            | CompiledRelationStatement::GroupCount { input, .. }
            | CompiledRelationStatement::CollectionCount { input }
            | CompiledRelationStatement::CollectionSum { input, .. }
            | CompiledRelationStatement::CollectionExtrema { input, .. }
            | CompiledRelationStatement::Eval { input, .. }
            | CompiledRelationStatement::FloorEval { input, .. }
            | CompiledRelationStatement::RationalArith { input, .. }
            | CompiledRelationStatement::Typecast { input, .. }
            | CompiledRelationStatement::Concat { input, .. }
            | CompiledRelationStatement::Join { input, .. }
            | CompiledRelationStatement::Substring { input, .. }
            | CompiledRelationStatement::Pad { input, .. }
            | CompiledRelationStatement::HashKey { input, .. }
            | CompiledRelationStatement::CalendarInt { input, .. }
            | CompiledRelationStatement::CalendarDate { input, .. }
            | CompiledRelationStatement::TypecastNumberToString { input, .. }
            | CompiledRelationStatement::TypecastStringToNumber { input, .. } => walk(input, acc),
            CompiledRelationStatement::BagUnion { branches, .. } => {
                for branch in branches {
                    walk(branch, acc)?;
                }
                Ok(())
            }
            CompiledRelationStatement::LeftAnti { left, right, .. }
            | CompiledRelationStatement::InnerJoin { left, right, .. }
            | CompiledRelationStatement::LeftOuter { left, right, .. } => {
                walk(left, acc)?;
                walk(right, acc)
            }
            // The cross-root hash join reads TWO graphs; it is TOP-LEVEL only (handled by the two-layer
            // prove_cross_root/verify_cross_root BEFORE this single-graph routing). Nested inside a
            // single-graph relation it cannot be served — reject closed.
            CompiledRelationStatement::CrossRootHashJoin { .. } => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cross-root hash join must be the top-level relation (two-graph); reject closed",
            )),
        }
    }
    let mut acc = None;
    walk(relation, &mut acc)?;
    Ok(acc.unwrap_or(ProofGraph::Instance))
}

pub(crate) fn reject_subplan_wrappers(compiled: &CompiledBgp, optional: bool) -> Result<(), BgpPlanError> {
    if compiled.projection.is_some()
        || compiled.count_variable.is_some()
        || compiled.full_row_distinct
    {
        Err(if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        })
    } else {
        Ok(())
    }
}

pub(crate) fn reject_when_subplan(compiled: &CompiledBgp) -> Result<(), BgpPlanError> {
    if compiled.projection.is_some() || compiled.count_variable.is_some() || compiled.full_row_distinct {
        Err(BgpPlanError::WhenUnsupportedSubplan)
    } else {
        Ok(())
    }
}

pub(crate) fn when_key_pairs(
    outer: &CompiledRelation,
    subplan: &CompiledRelation,
) -> Result<Vec<(usize, usize)>, BgpPlanError> {
    let pairs = subplan
        .schema
        .iter()
        .enumerate()
        .filter_map(|(right, name)| {
            outer
                .schema
                .iter()
                .position(|candidate| candidate == name)
                .map(|left| (left, right))
        })
        .collect::<Vec<_>>();
    if !(1..=2).contains(&pairs.len()) {
        return Err(BgpPlanError::WhenUnsupportedVariables);
    }
    let left_descriptors = relation_result_descriptors(outer)
        .map_err(|_| BgpPlanError::WhenUnsupportedSubplan)?;
    let right_descriptors = relation_result_descriptors(subplan)
        .map_err(|_| BgpPlanError::WhenUnsupportedSubplan)?;
    for (left, right) in &pairs {
        if left_descriptors[*left].namespace != right_descriptors[*right].namespace
            || left_descriptors[*left].nullable || right_descriptors[*right].nullable
        {
            return Err(BgpPlanError::WhenUnsupportedSubplan);
        }
    }
    Ok(pairs)
}

/// A Store dictionary ID namespace whose field values are guaranteed `< 2^64`. Composite
/// two-key joins pack the two keys as `col0 + 2^64·col1`, which is injective only for such
/// bounded keys; a generated-decimal namespace can reach ~2^254 and must never be packed.
pub(crate) fn is_store_id_namespace(namespace: ResultIdNamespace) -> bool {
    matches!(
        namespace,
        ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate
    )
}

pub(crate) fn shared_key_pairs(
    left: &CompiledRelation,
    right: &CompiledRelation,
    optional: bool,
) -> Result<Vec<(usize, usize)>, BgpPlanError> {
    let shared: Vec<_> = left
        .schema
        .iter()
        .enumerate()
        .filter_map(|(left_index, name)| {
            right
                .schema
                .iter()
                .position(|candidate| candidate == name)
                .map(|right_index| (name, left_index, right_index))
        })
        .collect();
    if shared.len() > 2 {
        return Err(if optional {
            BgpPlanError::OptionalSharedVariableCount {
                count: shared.len(),
            }
        } else {
            BgpPlanError::NotSharedVariableCount {
                count: shared.len(),
            }
        });
    }
    let left_descriptors = relation_result_descriptors(left).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    let right_descriptors = relation_result_descriptors(right).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    for (name, left_index, right_index) in &shared {
        if left_descriptors[*left_index].namespace != right_descriptors[*right_index].namespace {
            return Err(BgpPlanError::VariableNamespaceConflict {
                var: (*name).clone(),
            });
        }
        // Independent authenticated child relations model equality correlation. A
        // nullable outer binding is Prolog-unbound and would instead become bindable,
        // so this composition is deliberately rejected closed.
        if left_descriptors[*left_index].nullable || right_descriptors[*right_index].nullable {
            return Err(if optional {
                BgpPlanError::OptionalUnsupportedSubplan
            } else {
                BgpPlanError::NotUnsupportedSubplan
            });
        }
    }
    // A composite (two-key) correlation packs the two keys as `col0 + 2^64·col1`, injective
    // only when both keys are Store dictionary IDs (`< 2^64`). Namespace equality alone does
    // not bound the field width, so require every packed key to be a Store-ID namespace;
    // scalar (one-key) joins use injective equality and are deliberately left unrestricted.
    if shared.len() == 2 {
        for (name, left_index, right_index) in &shared {
            if !is_store_id_namespace(left_descriptors[*left_index].namespace)
                || !is_store_id_namespace(right_descriptors[*right_index].namespace)
            {
                return Err(BgpPlanError::CompositeJoinKeyNotStoreId {
                    var: (*name).clone(),
                });
            }
        }
    }
    Ok(shared
        .into_iter()
        .map(|(_, left, right)| (left, right))
        .collect())
}

pub(crate) fn schema_mapping(expected: &[String], actual: &[String]) -> Option<Vec<usize>> {
    if expected.len() != actual.len() {
        return None;
    }
    expected
        .iter()
        .map(|name| actual.iter().position(|candidate| candidate == name))
        .collect()
}

pub(crate) fn merge_collection(
    current: &mut Option<String>,
    candidate: Option<&String>,
    error: BgpPlanError,
) -> Result<(), BgpPlanError> {
    match (&current, candidate) {
        (Some(expected), Some(actual)) if expected != actual => Err(error),
        (None, Some(actual)) => {
            *current = Some(actual.clone());
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Whether a compiled relation's rows are provably free of duplicate full rows by
/// construction, so a full-schema `Distinct` over it is a sound transparent no-op. Only
/// pure natural-join relations qualify: a leaf `Bgp` assigns set-valued (unique) triples,
/// and the natural join of two duplicate-free relations stays unique because a repeated
/// joined row would require a repeated row in one side. Row-eliminating filters, orders and
/// slices preserve that uniqueness, and `Distinct`/aggregation statements themselves emit at
/// most one row per key/group. Every bag-valued composition (`Or`/`BagUnion`, `Not`/
/// `LeftAnti`, `Optional`/`LeftOuter`, `BagProjection`, `When`) can carry duplicate full
/// rows and is rejected closed. The match is exhaustive on purpose: a new statement variant
/// must consciously classify itself here rather than defaulting to a silent no-op.
pub(crate) fn is_provably_duplicate_free(relation: &CompiledRelation) -> bool {
    match &relation.statement {
        // Set-valued triple assignments are unique.
        CompiledRelationStatement::Bgp(_) => true,
        // A single generated count row is trivially unique.
        CompiledRelationStatement::LayerTripleCount => true,
        // Natural join of two duplicate-free relations is duplicate-free.
        CompiledRelationStatement::InnerJoin { left, right, .. } => {
            is_provably_duplicate_free(left) && is_provably_duplicate_free(right)
        }
        // The cross-root SEMI-JOIN keeps a SUBSET of the instance `rdf:type` rows. Whether the kept
        // `?x` column is duplicate-free depends on whether an instance can carry two DISTINCT subsumed
        // types (two kept rows sharing `?x`); conservatively NOT provably unique (a `Distinct` is not
        // proven redundant over it).
        CompiledRelationStatement::CrossRootHashJoin { .. } => false,
        // Row-eliminating filters/orders/slices never introduce duplicates. Tuple
        // disequality only DROPS rows (edge-reuse over-counts), so it inherits uniqueness.
        CompiledRelationStatement::GeneratedU64Filter { input, .. }
        | CompiledRelationStatement::GeneratedI64Filter { input, .. }
        | CompiledRelationStatement::Equality { input, .. }
        | CompiledRelationStatement::TupleDisequality { input, .. }
        | CompiledRelationStatement::ExactScalarOrder { input, .. }
        | CompiledRelationStatement::ExactMultiOrder { input, .. }
        | CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
            is_provably_duplicate_free(input)
        }
        // The transitive closure delivers the deduped reachable SET, so it is unique by construction.
        CompiledRelationStatement::TransitiveClosure { .. } => true,
        // Distinct and the aggregations emit at most one row per key/group, hence unique.
        CompiledRelationStatement::Distinct { .. }
        | CompiledRelationStatement::GroupExtrema { .. }
        | CompiledRelationStatement::GroupSum { .. }
        | CompiledRelationStatement::GroupCount { .. }
        | CompiledRelationStatement::CollectionCount { .. }
        | CompiledRelationStatement::CollectionSum { .. }
        | CompiledRelationStatement::CollectionExtrema { .. } => true,
        // Appending a per-row derived column is an injective row map: it can only make rows
        // MORE distinguishable, never merge two distinct child rows, so uniqueness is
        // inherited from the child. (Typecast appends a deterministic function of an existing
        // child column, so the same argument holds.)
        CompiledRelationStatement::Eval { input, .. }
        | CompiledRelationStatement::FloorEval { input, .. }
        | CompiledRelationStatement::Typecast { input, .. } => is_provably_duplicate_free(input),
        // Concat/Join/Substring carry the child's committed columns through VERBATIM (their
        // synthesized result is a terminal, non-committed value), so the committed rows'
        // uniqueness is exactly the child's. Inherit it.
        CompiledRelationStatement::Concat { input, .. }
        | CompiledRelationStatement::Join { input, .. }
        | CompiledRelationStatement::Substring { input, .. }
        // Pad/HashKey likewise carry the child's committed columns through VERBATIM (their
        // synthesized result is a terminal, non-committed value), so committed-row uniqueness is
        // exactly the child's. Inherit it.
        | CompiledRelationStatement::Pad { input, .. }
        | CompiledRelationStatement::HashKey { input, .. }
        // RationalArith likewise carries the child's committed columns through VERBATIM (the
        // xsd:decimal result is terminal/non-committed), so uniqueness is exactly the child's.
        | CompiledRelationStatement::RationalArith { input, .. }
        // CalendarDate carries the child's committed columns through VERBATIM (its xsd:date result is
        // terminal/non-committed), so committed-row uniqueness is exactly the child's. Inherit it.
        | CompiledRelationStatement::CalendarDate { input, .. }
        // Number↔string Typecasts likewise carry the child's committed columns through VERBATIM (the
        // cast result is a terminal, non-committed value), so uniqueness is exactly the child's.
        | CompiledRelationStatement::TypecastNumberToString { input, .. }
        | CompiledRelationStatement::TypecastStringToNumber { input, .. } => is_provably_duplicate_free(input),
        // CalendarInt appends per-row derived integer column(s) — an injective row map (like Eval),
        // so uniqueness is inherited from the child.
        CompiledRelationStatement::CalendarInt { input, .. } => is_provably_duplicate_free(input),
        // The ordered-list walk delivers a stored `rdf:List` IN ORDER, and a list is an ordered BAG:
        // `[a, b, a]` yields the element `a` in two rows. So its single element column can carry
        // duplicate rows — conservatively NOT provably unique (a surrounding `Distinct` is never
        // proven redundant over it, so multiplicity is never silently collapsed). This is the ONE
        // place OrderedListWalk deliberately classifies UNLIKE `TransitiveClosure` (whose reachable
        // SET is deduped, hence `true`): the semantics differ (bag vs set), so the sound answer does.
        CompiledRelationStatement::OrderedListWalk { .. } => false,
        // Bag-valued compositions can carry duplicate full rows.
        CompiledRelationStatement::BagUnion { .. }
        | CompiledRelationStatement::BagProjection { .. }
        | CompiledRelationStatement::When { .. }
        | CompiledRelationStatement::LeftAnti { .. }
        | CompiledRelationStatement::LeftOuter { .. } => false,
    }
}

pub(crate) fn compile_distinct_relation(
    relation: CompiledRelation,
    distincts: &[&[String]],
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
) -> Result<(CompiledRelation, bool), BgpPlanError> {
    if distincts.is_empty() {
        return Ok((relation, false));
    }
    if distincts.len() != 1 {
        return Err(BgpPlanError::NestedDistinctUnsupported);
    }
    let variables = distincts[0];
    if count_variable.is_none() {
        if selections.is_empty() {
            // A bare `Distinct` over the COMPLETE schema is a transparent full-row no-op.
            let covers_schema = variables.len() == relation.schema.len()
                && relation.schema.iter().all(|name| {
                    variables
                        .iter()
                        .filter(|candidate| *candidate == name)
                        .count()
                        == 1
                });
            if !covers_schema {
                return Err(BgpPlanError::DistinctNotFullSchema);
            }
            // The transparent full-row no-op relies on the child's rows already being unique.
            // Over a duplicate-producing child (Or/Not/Optional/projection/When) it would
            // silently return a NON-deduplicated result marked as `Distinct`, so reject closed
            // and defer real bag dedup to the gadget-backed path below.
            if !is_provably_duplicate_free(&relation) {
                return Err(BgpPlanError::DistinctOverDuplicateProducingChild);
            }
            return Ok((relation, true));
        }
        // A VISIBLE, top-level projected `Distinct` (e.g. `SELECT DISTINCT ?x …`): route the
        // projected key columns through the audited store distinct gadget (real bag dedup),
        // exactly as the beneath-`Count` path below does — the gadget proves the output tuples
        // are EXACTLY the distinct key tuples of the authenticated input BAG (so a
        // duplicate-producing child is fine; that is what dedup exists for). Two inner shapes are
        // sound here, for the SAME reason:
        //   * a compilable conjunctive `Bgp` — a single authenticated bag of rows; and
        //   * an audited `BagUnion` (a WOQL `Or`) — the multiset UNION of its branch bags is still
        //     just an authenticated bag, and the store's Distinct envelope node is generic over its
        //     input (it runs `prove_bounded` on whatever child it wraps, including a `BagUnion`
        //     envelope). The dedup gadget proves output == the distinct key tuples of that bag, so
        //     the overlap/duplicates the `Or` branches produce are exactly what it collapses. This
        //     is precisely the reasoning the beneath-`Count` path already relies on (it has no
        //     statement-type guard and composes over `Or` today — see the passing test
        //     `count_projected_distinct_composes_over_duplicate_bag_union`).
        // EVERYTHING else still rejects closed on the visible path: property-path closures
        // (`TransitiveClosure`/`TupleDisequality`), subsumption/cross-root joins, sequential
        // read-operators (`LeftAnti`/`LeftOuter`/`When`/`Equality`), and ordered/range/slice
        // constructs — their committed-output order or bag shape is not a plain authenticated bag
        // that the dedup gadget can key on. The soundness of the accepted cases does NOT rest on
        // the statement shape alone: the downstream key-column checks below (domain KNOWN,
        // non-nullable, ≤2 keys) still run and still gate every case. They resolve the key columns
        // against `relation.schema`, which for a `BagUnion` is the branches' UNIFIED schema — the
        // identical resolution the count path performs. Every selection here is `outside` (there is
        // no enclosing Count), so the Select-inside-Count guard below is vacuous for this path.
        if !matches!(
            relation.statement,
            CompiledRelationStatement::Bgp(_) | CompiledRelationStatement::BagUnion { .. }
        ) {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
    } else {
        // A Select nested inside Count/Distinct changes the generated witness stream and is
        // not merely a selection of the count binding. Keep it fail-closed. An outer Select
        // of the Count binding has `outside == true` and is validated by compile_projection.
        if selections.iter().any(|(_, outside)| !outside) {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
    }
    if !(1..=2).contains(&variables.len()) {
        return Err(BgpPlanError::DistinctUnsupportedKeyCount {
            count: variables.len(),
        });
    }
    let mut key_columns = Vec::with_capacity(variables.len());
    for variable in variables {
        let index = relation
            .schema
            .iter()
            .position(|candidate| candidate == variable)
            .ok_or_else(|| BgpPlanError::SelectedVariableNotInBgp {
                var: variable.clone(),
            })?;
        if key_columns.contains(&index)
            || relation.domains.get(index).copied().flatten().is_none()
        {
            return Err(BgpPlanError::DistinctUnsupportedDomain {
                var: variable.clone(),
            });
        }
        key_columns.push(index);
    }
    let descriptors = relation_result_descriptors(&relation)
        .map_err(|_| BgpPlanError::DistinctUnsupportedDomain {
            var: variables[0].clone(),
        })?;
    for (&index, variable) in key_columns.iter().zip(variables) {
        if descriptors[index].nullable {
            return Err(BgpPlanError::DistinctUnsupportedDomain {
                var: variable.clone(),
            });
        }
    }
    let schema = variables.to_vec();
    let domains = key_columns
        .iter()
        .map(|index| relation.domains[*index])
        .collect();
    Ok((
        CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Distinct {
                input: Box::new(relation),
                key_columns,
            },
        },
        false,
    ))
}

pub(crate) fn compile_projection(
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
    schema: &[String],
) -> Result<Option<Vec<usize>>, BgpPlanError> {
    if let Some(count) = count_variable {
        let outside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| outside.then_some(*vars))
            .collect();
        let inside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| (!outside).then_some(*vars))
            .collect();
        validate_selection_chain(&inside, schema)?;
        validate_selection_chain(&outside, &[count.to_owned()])?;
        Ok(Some(Vec::new()))
    } else if selections.is_empty() {
        // Ground relations use one authenticated internal UNIT column that remains
        // hidden from the public WOQL schema, including a bag union of ground branches.
        Ok(schema.is_empty().then(Vec::new))
    } else {
        let selections: Vec<&[String]> = selections.iter().map(|(vars, _)| *vars).collect();
        validate_selection_chain(&selections, schema)?;
        Ok(Some(
            selections[0]
                .iter()
                .map(|var| {
                    schema
                        .iter()
                        .position(|candidate| candidate == var)
                        .expect("validated outer projection must occur in the BGP schema")
                })
                .collect(),
        ))
    }
}

/// Validate nested Selects listed outer-to-inner. Each Select may only name variables
/// exposed by its directly nested query; the innermost Select reads from `base`.
pub(crate) fn validate_selection_chain(selections: &[&[String]], base: &[String]) -> Result<(), BgpPlanError> {
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
pub(crate) fn collect_conjunct<'a>(
    query: &'a Query,
    triples: &mut Vec<TypedTriple>,
    comparisons: &mut Vec<NormalizedRange>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::Triple(triple) => triples.push(TypedTriple {
            triple: triple.clone(),
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Materialized,
        }),
        // CP-D delta reads: `AddedTriple`/`DeletedTriple` have the SAME shape as `Triple`
        // (subject: NodeValue, predicate: NodeValue, object: Value, graph). The reference
        // compiles them to `triple_addition`/`triple_removal` over EXACTLY this commit's
        // layer delta file ("no deeper"), so we reuse the identical atom lowering pointed at
        // the layer's additions/removals column source. The graph field is validated (instance
        // only) by `atom_from_triple`, exactly as for `Triple`.
        Query::AddedTriple(added) => triples.push(TypedTriple {
            triple: Triple {
                subject: added.subject.clone(),
                predicate: added.predicate.clone(),
                object: added.object.clone(),
                graph: added.graph.clone(),
            },
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Additions,
        }),
        Query::DeletedTriple(deleted) => triples.push(TypedTriple {
            triple: Triple {
                subject: deleted.subject.clone(),
                predicate: deleted.predicate.clone(),
                object: deleted.object.clone(),
                graph: deleted.graph.clone(),
            },
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Removals,
        }),
        Query::Link(link) => triples.push(TypedTriple {
            triple: Triple {
                subject: link.subject.clone(),
                predicate: link.predicate.clone(),
                object: match &link.object {
                    NodeValue::Node(iri) => Value::Node(iri.clone()),
                    NodeValue::Variable(name) => Value::Variable(name.clone()),
                },
                graph: Some(link.graph.clone()),
            },
            object_kind: ObjectKindFilter::Node,
            source: ColumnSource::Materialized,
        }),
        Query::Data(data) => {
            let object = match &data.object {
                DataValue::Data(value) => Value::Data(value.clone()),
                DataValue::Variable(name) => Value::Variable(name.clone()),
                DataValue::List(_) => {
                    return Err(BgpPlanError::UnsupportedObjectValue { kind: "data list" })
                }
            };
            triples.push(TypedTriple {
                triple: Triple {
                    subject: data.subject.clone(),
                    predicate: data.predicate.clone(),
                    object,
                    graph: Some(data.graph.clone()),
                },
                object_kind: ObjectKindFilter::Literal,
                source: ColumnSource::Materialized,
            });
        }
        Query::And(inner) => {
            for query in &inner.and {
                collect_conjunct(query, triples, comparisons)?;
            }
        }
        Query::Greater(_) | Query::Less(_) | Query::Gte(_) | Query::Lte(_) => {
            comparisons.push(normalize_comparison(query)?)
        }
        Query::InRange(range) => comparisons.push(normalize_in_range(range)?),
        Query::Pin(pin) => collect_conjunct(pin.query.as_ref(), triples, comparisons)?,
        Query::Immediately(immediately) => {
            collect_conjunct(immediately.query.as_ref(), triples, comparisons)?
        }
        Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => {
            collect_conjunct(from.query.as_ref(), triples, comparisons)?
        }
        Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
        Query::True(_) | Query::Comment(_) => {}
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
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
/// Rejected closed: cross-namespace reuse of one variable in predicate and S/O positions,
/// schema-graph triples, dictionary/list object values, and unaligned typed-literal datatypes. A
/// value-literal SUBJECT is unrepresentable — woql2's
/// subject is a [`NodeValue`] (node/variable only) — so the store's `validate_atom`
/// value-subject guard is unreachable through this compiler by construction.
pub(crate) fn atom_from_triple(typed: &TypedTriple) -> Result<Atom, BgpPlanError> {
    let t = &typed.triple;
    // Graph: None (defaults to instance) or explicit Instance only.
    match &t.graph {
        None | Some(GraphType::Instance) => {}
        Some(GraphType::Schema) => return Err(BgpPlanError::SchemaGraphUnsupported),
    }

    let (predicate_term, pred_var) = match &t.predicate {
        NodeValue::Node(iri) => (iri.clone().into_bytes(), None),
        NodeValue::Variable(name) => (Vec::new(), Some(name.clone())),
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

    let subject_object_equal = matches!((&subj_var, &obj_var), (Some(s), Some(o)) if s == o);

    // Kept columns are materialized in S,P,O order. Predicate IDs occupy an independent
    // Store namespace, so one WOQL variable may not alias P with S or O.
    let mut kept = Vec::with_capacity(3);
    if let Some(s) = &subj_var {
        kept.push(s.clone());
    }
    if let Some(p) = &pred_var {
        if kept.contains(p) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: p.clone() });
        }
        kept.push(p.clone());
    }
    if let Some(o) = &obj_var {
        if pred_var.as_ref() == Some(o) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: o.clone() });
        }
        if !kept.contains(o) {
            kept.push(o.clone());
        }
    }
    let object_kind = if matches!(object_term, AtomTerm::Variable) {
        typed.object_kind
    } else {
        // A node/data constant already pins its kind through the authenticated
        // dictionary entry; the explicit filter is needed only for variables.
        ObjectKindFilter::Any
    };
    Ok(Atom {
        layer_atom: LayerBgpAtom {
            subject: subject_term,
            predicate_term,
            predicate_variable: pred_var.is_some(),
            object: object_term,
            // CP-D Step 1 (migration): every atom starts with no object range. Step 2
            // fills this in for atoms whose object variable a comparison conjunct ranges.
            object_range: None,
            object_kind,
            subject_object_equal,
            // Thread THIS atom's committed column source (materialized head for
            // `Triple`/`Link`/`Data`, additions for `AddedTriple`, removals for
            // `DeletedTriple`). Making it per-atom is what admits a CROSS-SOURCE join; the
            // store prove/verify read each atom's columns/count from this field, and it is
            // bound by `plan_digest` + the Fiat–Shamir transcript.
            source: typed.source,
        },
        kept,
        subj_var,
        pred_var,
        obj_var,
    })
}

pub(crate) fn validate_variable_namespaces(atoms: &[Atom]) -> Result<(), BgpPlanError> {
    for atom in atoms {
        if let Some(predicate) = &atom.pred_var {
            if atoms.iter().any(|candidate| {
                candidate.subj_var.as_ref() == Some(predicate)
                    || candidate.obj_var.as_ref() == Some(predicate)
            }) {
                return Err(BgpPlanError::VariableNamespaceConflict {
                    var: predicate.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Route each numeric comparison conjunct to an object-value RANGE filter on the atom
/// that binds the compared variable. For each comparison:
///
/// 1. [`normalize_comparison`] turns it into `(var, op, datatype, c)`, orienting the
///    operator so `c` is always on the right (`k > ?a` ⇒ `?a < k`), rejecting closed a
///    var-vs-var / const-vs-const comparison or an unsupported constant datatype.
/// 2. v1 constraints (all reject closed): `var` must be the object of EXACTLY ONE atom,
///    must NOT appear as any atom's subject, and each `var` may carry at most two bounds.
/// 3. The [`ObjectRangeSpec`] attaches to that atom's object (its object stays a kept
///    variable — the range is a filter, not a projection change).
pub(crate) fn attach_ranges(atoms: &mut [Atom], comparisons: &[NormalizedRange]) -> Result<(), BgpPlanError> {
    for comparison in comparisons {
        let var = comparison.var.clone();
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
        match atoms[idx].layer_atom.object_kind {
            ObjectKindFilter::Node => {
                return Err(BgpPlanError::RangeOnNodeObject { var });
            }
            ObjectKindFilter::Literal => {
                // The range proof authenticates equality to one exact literal datatype,
                // so it subsumes Data's broader literal-kind constraint.
                atoms[idx].layer_atom.object_kind = ObjectKindFilter::Any;
            }
            ObjectKindFilter::Any => {}
        }
        for bound in &comparison.bounds {
            match &mut atoms[idx].layer_atom.object_range {
                None => {
                    atoms[idx].layer_atom.object_range = Some(ObjectRangeSpec {
                        op: bound.op,
                        datatype: comparison.datatype,
                        c: bound.c,
                        b_bits: OBJECT_RANGE_B_BITS,
                        second: None,
                    });
                }
                Some(existing) => {
                    if existing.datatype != comparison.datatype {
                        return Err(BgpPlanError::RangeDatatypeMismatch { var });
                    }
                    if existing.second.is_some() {
                        return Err(BgpPlanError::DuplicateRangeOnVariable { var });
                    }
                    existing.second = Some(ObjectRangeBound {
                        op: bound.op,
                        c: bound.c,
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct NormalizedBound {
    pub(crate) op: RangeOp,
    pub(crate) c: ArkFr,
}

#[derive(Clone)]
pub(crate) struct NormalizedRange {
    pub(crate) var: String,
    pub(crate) datatype: ArkFr,
    pub(crate) canonical_datatype: CanonicalDatatype,
    pub(crate) bounds: Vec<NormalizedBound>,
}

/// Normalize a numeric comparison conjunct into `(var, op, datatype, c)`, where `op` is
/// oriented so the constant is on the RIGHT (`Greater(k, ?a)` ⇒ `?a < k`), and
/// `(datatype, c)` is the store-aligned encoding of the constant (see
/// [`encode_range_constant`]).
///
/// Rejects closed: both sides variables ([`BgpPlanError::RangeBothVariables`]), neither
/// side a variable ([`BgpPlanError::RangeBothConstants`]), or a constant that is not a
/// supported order-preserving numeric literal ([`BgpPlanError::UnsupportedRangeDatatype`]).
pub(crate) fn normalize_comparison(q: &Query) -> Result<NormalizedRange, BgpPlanError> {
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
    let (datatype, c, canonical_datatype) = encode_range_constant(xsd)?;
    Ok(NormalizedRange {
        var,
        datatype,
        canonical_datatype,
        bounds: vec![NormalizedBound { op, c }],
    })
}

pub(crate) fn normalize_in_range(range: &InRange) -> Result<NormalizedRange, BgpPlanError> {
    let var = match &range.value {
        DataValue::Variable(var) => var.clone(),
        _ => return Err(BgpPlanError::InRangeValueNotVariable),
    };
    let start = match &range.start {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let end = match &range.end {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let (start_datatype, start_c, start_canonical_datatype) = encode_range_constant(start)?;
    let (end_datatype, end_c, end_canonical_datatype) = encode_range_constant(end)?;
    if start_datatype != end_datatype || start_canonical_datatype != end_canonical_datatype {
        return Err(BgpPlanError::RangeDatatypeMismatch { var });
    }
    Ok(NormalizedRange {
        var,
        datatype: start_datatype,
        canonical_datatype: start_canonical_datatype,
        bounds: vec![
            NormalizedBound {
                op: RangeOp::Ge,
                c: start_c,
            },
            NormalizedBound {
                op: RangeOp::Lt,
                c: end_c,
            },
        ],
    })
}

/// Build the store's typed dict entry for a woql2 SCALAR literal, routing each supported
/// xsd datatype through its `terminus_store`/`tdb-succinct` typed-value constructor (the
/// [`TdbDataType::make_entry`] family) so the resulting canonicalization is byte-identical
/// to a stored value of the same literal (no hand-rolled byte layouts — the 7.5c alignment
/// rule). Returns the entry, or the xsd datatype name for a literal this compiler does not
/// support (the caller wraps that name into the right rejection error).
///
/// `xsd:string` is handled separately by the caller via
/// [`ValueTriple::new_string_value`]. Invalid scalar lexical forms reject closed.
pub(crate) fn xsd_scalar_entry(xsd: &XSDAnySimpleType) -> Result<TypedDictEntry, &'static str> {
    Ok(match xsd {
        XSDAnySimpleType::Boolean(b) => bool::make_entry(b),
        XSDAnySimpleType::UnsignedByte(v) => u8::make_entry(v),
        XSDAnySimpleType::Byte(v) => i8::make_entry(v),
        XSDAnySimpleType::UnsignedShort(v) => u16::make_entry(v),
        XSDAnySimpleType::Short(v) => i16::make_entry(v),
        // xsd:unsignedInt is the 32-bit unsigned type → store `UInt32`.
        XSDAnySimpleType::UnsignedInt(u) => {
            let v = u32::try_from(*u).map_err(|_| "unsignedInt (exceeds u32)")?;
            u32::make_entry(&v)
        }
        XSDAnySimpleType::Int(v) => i32::make_entry(v),
        XSDAnySimpleType::UnsignedLong(v) => u64::make_entry(v),
        XSDAnySimpleType::Long(v) => i64::make_entry(v),
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
        XSDAnySimpleType::HexBinary(value) => {
            let bytes = hex::decode(value).map_err(|_| "hexBinary (invalid lexical value)")?;
            HexBinary::make_entry(&HexBinary(bytes))
        }
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
/// (`Boolean`), all explicit fixed-width XSD integer types (`UInt8`/`Int8` through
/// `UInt64`/`Int64`), `xsd:integer` (`Int64`), and `xsd:float`/`double` (`Float64`).
/// Every other `XSDAnySimpleType` rejects closed: a range over a
/// non-order-preserving or variable-width datatype (string/decimal/…) would compare a
/// `0` sentinel in the store, which is unsound at the query level. Adding a datatype
/// requires a store-routed construction in [`xsd_scalar_entry`], an
/// [`op_preserving_width`] entry, PLUS a round-trip alignment test.
pub(crate) fn encode_range_constant(
    xsd: &XSDAnySimpleType,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    match xsd {
        // Order-preserving fixed-width numerics: the store's `to_lexical` bytes ARE the OP
        // encoding, so `encode_via_store` -> `node_value_op_value` big-endian reads them.
        XSDAnySimpleType::Boolean(_)
        | XSDAnySimpleType::UnsignedByte(_)
        | XSDAnySimpleType::Byte(_)
        | XSDAnySimpleType::UnsignedShort(_)
        | XSDAnySimpleType::Short(_)
        | XSDAnySimpleType::UnsignedInt(_)
        | XSDAnySimpleType::Int(_)
        | XSDAnySimpleType::UnsignedLong(_)
        | XSDAnySimpleType::Long(_)
        | XSDAnySimpleType::Integer(_)
        | XSDAnySimpleType::Float(_) => {}
        // date/dateTime: order-preserving via the store's DECODED i64-nanoseconds key
        // (`node_value_op_value` decodes the native lexical). REJECT-CLOSE a constant at or
        // outside the representable i64-nanoseconds window here, BEFORE building it, so the
        // saturation of far DATA dates stays strictly order-correct against it (see
        // `date_constant_nanos`). The encoding itself still comes from the store routing
        // below (single source of truth) — this only gates the value.
        XSDAnySimpleType::DateTime(_) | XSDAnySimpleType::Date(_) => {
            let _ = date_constant_nanos(xsd)?;
        }
        // Everything else (string/decimal/hexBinary/anyURI/time/…) rejects closed.
        _ => {
            return Err(BgpPlanError::UnsupportedRangeDatatype {
                datatype: xsd_datatype_name(xsd),
            });
        }
    }
    let entry = xsd_scalar_entry(xsd)
        .map_err(|datatype| BgpPlanError::UnsupportedRangeDatatype { datatype })?;
    encode_via_store(entry)
}

/// The i64 nanoseconds-since-epoch key of a `date`/`dateTime` range CONSTANT, or
/// [`BgpPlanError::DateRangeConstantOutOfExactRange`] if it is not STRICTLY inside the
/// representable i64-nanoseconds window (endpoints `i64::MIN`/`i64::MAX` excluded — those
/// are exactly the saturated values a far DATA date takes, so excluding them keeps `c`
/// strictly between every far-past and far-future data date). Used only as a pre-flight
/// reject; the emitted `c` is still the store-routed value in [`encode_via_store`].
///
/// The constant carries no timezone offset (a WOQL `Date` is a bare `NaiveDate`; a
/// `DateTime` a UTC instant), matching the `offset: 0` form [`xsd_scalar_entry`] builds —
/// so the constant's key is a pure midnight (date) / instant (dateTime), and the `date`
/// offset tiebreaker in `node_value_op_value` is only ever exercised by DATA rows.
pub(crate) fn date_constant_nanos(xsd: &XSDAnySimpleType) -> Result<i64, BgpPlanError> {
    let nanos = match xsd {
        XSDAnySimpleType::DateTime(datetime) => datetime.timestamp_nanos_opt(),
        XSDAnySimpleType::Date(date) => date
            .and_hms_opt(0, 0, 0)
            .and_then(|ndt| ndt.and_utc().timestamp_nanos_opt()),
        // Only date/dateTime reach here.
        _ => return Err(BgpPlanError::DateRangeConstantOutOfExactRange),
    };
    match nanos {
        Some(n) if n != i64::MIN && n != i64::MAX => Ok(n),
        _ => Err(BgpPlanError::DateRangeConstantOutOfExactRange),
    }
}

/// Canonicalize a `TypedDictEntry` through the store's own value path and derive the
/// `(datatype_tag, c)` pair. Fails closed if the resulting datatype is not one of the
/// round-trip-verified order-preserving datatypes (fixed-width numerics, plus
/// `date`/`dateTime`; see [`op_preserving_width`]).
pub(crate) fn encode_via_store(
    entry: TypedDictEntry,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    // Subject/predicate are irrelevant to the object canonicalization.
    let triple = ValueTriple::new_value("", "", entry);
    let object = CanonicalObject::from_store_object(&triple.object);
    let datatype = match &object {
        CanonicalObject::Value { datatype, .. } => *datatype,
        // `new_value` always yields a Value object; fail closed rather than emit an
        // unaligned constant.
        CanonicalObject::Node(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "node" })
        }
    };
    // Guard: only order-preserving datatypes are sound to range over (the fixed-width
    // numerics, plus date/dateTime).
    if op_preserving_width(datatype).is_none() {
        return Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: "non-order-preserving datatype",
        });
    }
    // c = the store's OWN order-preserving value for this object, computed by the EXACT
    // function applied to the committed dict VALUE column (`node_value_op_value`). This is
    // the single source of truth: for fixed-width numerics it big-endian reads the lexical;
    // for date/dateTime it decodes the native lexical to the i64-nanoseconds key and
    // re-encodes as offset-binary. So `c` is byte-identical to a stored value's committed OP
    // entry BY CONSTRUCTION — no re-implemented byte layout here (the previous fixed-width
    // BE read now lives inside `node_value_op_value`). A corrupt/short numeric lexical, or a
    // date whose committed lexical is malformed, fails closed inside that function.
    let c = node_value_op_value(&object).map_err(|_| BgpPlanError::UnsupportedRangeDatatype {
        datatype: "value is not order-preserving",
    })?;
    let datatype_tag = ArkFr::from_u64(datatype as u64);
    Ok((datatype_tag, c, datatype))
}

/// Fixed byte width of a SUPPORTED order-preserving numeric datatype, or `None` for any
/// datatype whose lexical is variable-width or not order-preserving. Mirrors
/// `terminus_store`'s private `order_preserving_numeric_width` EXACTLY: for each of these
/// the store's `to_lexical` writes exactly `width` bytes whose big-endian unsigned read is
/// monotone in value (`Boolean` 0/1; `(U)Int8/16/32/64` raw-BE / offset-binary;
/// `Float32/64` IEEE total-order), so `node_value_op_value` and this compiler compute the
/// SAME `c`. A range over any other datatype rejects closed (see [`encode_range_constant`]).
pub(crate) fn op_preserving_width(datatype: CanonicalDatatype) -> Option<usize> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Boolean => 1,
        UInt8 | Int8 => 1,
        UInt16 | Int16 => 2,
        UInt32 | Int32 | Float32 => 4,
        UInt64 | Int64 | Float64 => 8,
        // `Date`/`DateTime` are order-preserving at the `Int64` width (8), but — unlike
        // every entry above — the OP value is NOT a big-endian read of the lexical: it is
        // the store's DECODED i64-nanoseconds key (`node_value_op_value`), routed through
        // that exact function in `encode_via_store`. This mirror table only gates that a
        // range over the datatype is allowed and reports the gadget width; it does NOT
        // imply an 8-byte lexical.
        Date | DateTime => 8,
        _ => return None,
    })
}

/// The xsd datatype name of a woql2 typed literal, for descriptive rejection messages.
pub(crate) fn xsd_datatype_name(xsd: &XSDAnySimpleType) -> &'static str {
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
        XSDAnySimpleType::UnsignedByte(_) => "unsignedByte",
        XSDAnySimpleType::Byte(_) => "byte",
        XSDAnySimpleType::UnsignedShort(_) => "unsignedShort",
        XSDAnySimpleType::Short(_) => "short",
        XSDAnySimpleType::UnsignedInt(_) => "unsignedInt",
        XSDAnySimpleType::Int(_) => "int",
        XSDAnySimpleType::UnsignedLong(_) => "unsignedLong",
        XSDAnySimpleType::Long(_) => "long",
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
/// See the `object_*_literal_*` round-trip tests. Invalid scalar lexical forms reject closed.
pub(crate) fn value_constant_term(xsd: &XSDAnySimpleType) -> Result<AtomTerm, BgpPlanError> {
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
/// `terminus_store`'s `bgp` output ordering exactly. If no remaining atom shares a
/// variable, the next input-order component starts with a verifier-known UNIT-key
/// Cartesian step whose output ordering is `[acc.., relation..]`.
///
/// Each atom relation carries ONE column per VARIABLE, in S,P,O order (`atom.kept`); a
/// constant removes its position from `kept`. `rel_key` is the shared variable's index within
/// the atom's KEPT columns, and `s_other` is the atom's remaining kept columns (possibly
/// empty for a 1-variable atom).
pub(crate) fn plan_left_deep(atoms: &[Atom]) -> Result<(LayerBgpPlan, Vec<String>), BgpPlanError> {
    let k = atoms.len();

    let ground_count = atoms.iter().filter(|atom| atom.kept.is_empty()).count();
    if ground_count == k {
        return Ok((
            LayerBgpPlan {
                atoms: atoms.iter().map(|atom| atom.layer_atom.clone()).collect(),
                joins: Vec::new(),
                source: ColumnSource::Materialized,
            },
            Vec::new(),
        ));
    }
    let relational_count = k - ground_count;

    // Deterministic start atom: input order 0. Its kept variables seed the accumulator.
    let mut acc_vars: Vec<String> = atoms[0].kept.clone();
    let mut placed = vec![false; k];
    placed[0] = true;

    let mut ordered_atoms: Vec<LayerBgpAtom> = vec![atoms[0].layer_atom.clone()];
    let mut joins: Vec<JoinStep> = Vec::with_capacity(relational_count - 1);

    let mut placed_count = 1usize;
    while placed_count < relational_count {
        // Find an unplaced atom sharing at least one variable with the accumulator.
        let mut chosen: Option<usize> = None;
        for (i, atom) in atoms.iter().take(relational_count).enumerate() {
            if placed[i] {
                continue;
            }
            if shared_count(&acc_vars, &atom.kept) >= 1 {
                chosen = Some(i);
                break;
            }
        }

        let i = chosen.unwrap_or_else(|| {
            placed
                .iter()
                .take(relational_count)
                .position(|is_placed| !is_placed)
                .expect("an unplaced relational atom must remain")
        });

        // Join on every shared variable. The first pair remains the scalar-key fields;
        // further pairs activate the Store chain's authenticated composite-key mode.
        let atom = &atoms[i];
        let shared: Vec<(usize, usize)> = atom
            .kept
            .iter()
            .enumerate()
            .filter_map(|(rel, variable)| {
                acc_vars
                    .iter()
                    .position(|acc| acc == variable)
                    .map(|acc| (acc, rel))
            })
            .collect();
        if shared.is_empty() {
            joins.push(JoinStep::cross_product());
            acc_vars.extend(atom.kept.iter().cloned());
            ordered_atoms.push(atom.layer_atom.clone());
            placed[i] = true;
            placed_count += 1;
            continue;
        }
        let (acc_key, rel_key) = shared[0];
        let v = atom.kept[rel_key].clone();
        let additional_keys = shared[1..].to_vec();

        if additional_keys.is_empty() {
            // Scalar path preserves the audited binary-join output ordering.
            let mut r_other = acc_vars.clone();
            r_other.remove(acc_key);
            let mut s_other = atom.kept.clone();
            s_other.remove(rel_key);
            r_other.push(v);
            r_other.extend(s_other);
            acc_vars = r_other;
        } else {
            // Composite path retains the accumulator in place and appends only variables
            // not already represented by one of the shared key columns.
            acc_vars.extend(
                atom.kept
                    .iter()
                    .filter(|variable| !acc_vars.contains(variable))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        }

        joins.push(JoinStep {
            acc_key,
            rel_key,
            additional_keys,
        });
        ordered_atoms.push(atom.layer_atom.clone());
        placed[i] = true;
        placed_count += 1;
    }

    ordered_atoms.extend(
        atoms[relational_count..]
            .iter()
            .map(|atom| atom.layer_atom.clone()),
    );

    Ok((
        LayerBgpPlan {
            atoms: ordered_atoms,
            joins,
            source: ColumnSource::Materialized,
        },
        acc_vars,
    ))
}

/// The plan DEFAULT column source for a BGP leaf. Each atom now carries its OWN source
/// ([`LayerBgpAtom::source`], threaded in [`atom_from_triple`]), so a CROSS-SOURCE conjunction
/// — a materialized `Triple` joined with a delta atom, or additions joined with removals — is
/// now SUPPORTED (the store joins over the shared dictionaries). This returns the source to
/// stamp on `plan.source`: when every atom agrees it is that uniform source (which keeps a
/// single-source plan's transcript byte-identical to the pre-per-atom encoding — the store
/// binds a per-atom source override ONLY when it diverges from `plan.source`); when they
/// diverge it is `Materialized`, the neutral default that emits no plan-level source tag. An
/// empty triple list defaults to materialized (the surrounding `k == 0` check then rejects it
/// as too few atoms).
pub(crate) fn plan_default_column_source(triples: &[TypedTriple]) -> ColumnSource {
    let mut sources = triples.iter().map(|triple| triple.source);
    let first = sources.next().unwrap_or(ColumnSource::Materialized);
    if sources.all(|source| source == first) {
        first
    } else {
        ColumnSource::Materialized
    }
}

/// How many of an atom's KEPT (variable) columns occur in the accumulator schema. Because
/// repeated variables within an atom are represented once after their same-row equality
/// constraint is recorded, and `acc_vars` also holds distinct names, so this is an exact
/// shared-variable count in `0..=kept.len()`.
pub(crate) fn shared_count(acc_vars: &[String], kept: &[String]) -> usize {
    kept.iter()
        .filter(|kv| acc_vars.iter().any(|a| a == *kv))
        .count()
}


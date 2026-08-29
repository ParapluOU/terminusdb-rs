//! Ordering lowering — `OrderBy`, exact ordered slices, and triple-count comparisons.
//!
//! Lowers ordering / slicing constructs into exact-order proof statements:
//! `plan_exact_scalar_order` and `plan_exact_ordered_slice` (scalar / multi-key `OrderBy`,
//! `Start` / `Limit` windows), the ordered-triple value scans (`compile_ordered_triple_slice`,
//! `plan_ordered_triple_value_construct`), and the layer triple-count comparisons
//! (`plan_layer_triple_count`, the `TripleCount` relation). The proof establishes the exact
//! sorted order, not merely a permutation.
//!
//! See [`terminus_store::proof::primer`] for the cryptographic background (§6 the sort / order
//! gadgets these plans drive, and §11 soundness-vs-fidelity — an order proof must attest the
//! exact TerminusDB ordering, not just some permutation).

use super::*;

/// Compile a WOQL `OrderBy` into an exact-order plan: `ExactScalarOrder` for a single sort column,
/// `ExactMultiOrder` for two or three. The proof attests the exact TerminusDB order, not merely a
/// permutation.
///
/// ACCEPTS: a bare, non-projected / non-aggregated / non-distinct child BGP of arity 1..=3 whose
/// columns are all non-nullable node/value columns pinned to a fixed-width INTEGRAL datatype, under
/// a FULL-key template (every column named, keys distinct).
///
/// REJECTS CLOSED: a projected / counted / distinct / wrong-arity child (`OrderByUnsupportedShape`);
/// a repeated or unknown sort key (`OrderByUnsupportedShape`); a SUBSET-key order — fewer keys than
/// columns — which would need an un-attestable invented tie-break (`OrderBySubsetKeyUnsupported`, the
/// "maximum order attestation" rule); and a non-node/value, nullable, or FLOAT sort column
/// (`OrderByUnsupportedValue`) — floats are excluded because the store's IEEE total order splits
/// `-0.0`/`+0.0` and misplaces `NaN` versus WOQL numeric order.
pub(crate) fn plan_exact_scalar_order(
    order_by: &terminusdb_woql2::order::OrderBy,
) -> Result<CompiledBgp, BgpPlanError> {
    if !(1..=3).contains(&order_by.ordering.len()) {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    let child = plan_bgp(&order_by.query)?;
    // The audited Store order gadgets (`ExactScalarOrder`/`ExactMultiOrder`) prove a full-row
    // permutation over one-to-three columns, so the inner BGP must expose a bare, non-projected,
    // non-aggregated relation of arity 1..=3. The template must name EVERY column (a FULL-key order);
    // a subset template is reject-closed below (its invented tie order would not be attestable).
    if child.projection.is_some()
        || child.count_variable.is_some()
        || child.full_row_distinct
        || !(1..=3).contains(&child.relation.schema.len())
        || order_by.ordering.len() > child.relation.schema.len()
    {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    // Map each template key to its canonical column index; every key must name an existing column,
    // and the named keys must be DISTINCT columns (SWI `order_by/2` with a repeated key is
    // meaningless — the second sort on the same witness is a no-op — so reject rather than guess).
    let named_indices = order_by.ordering.iter()
        .map(|ordering| child.relation.schema.iter().position(|schema| schema == &ordering.variable))
        .collect::<Option<Vec<_>>>()
        .ok_or(BgpPlanError::OrderByUnsupportedShape)?;
    let mut distinct = named_indices.clone();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() != named_indices.len() {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    // MAXIMUM ORDER ATTESTATION: a SUBSET-key OrderBy (fewer named keys than columns) would force
    // an INVENTED ascending tie-break over the unnamed columns. That invented tie order cannot be
    // attested against TerminusDB (SWI order_by/2 leaves rows tying on the named keys in findall
    // enumeration order, an implementation artefact our committed triple store does not expose).
    // Rather than attest a tie order we cannot prove, we reject-close EVERY subset-key order. Only
    // a FULL-key order (every column named) — a deterministic total order that matches TerminusDB
    // exactly — is attested. Long-term lift: a stable-sort proof carrying an authenticated input ordinal.
    if named_indices.len() < child.relation.schema.len() {
        return Err(BgpPlanError::OrderBySubsetKeyUnsupported);
    }
    // FULL-key order. The subset guard above guarantees the template names every column, so
    // `named_indices` is already a permutation of `0..schema.len()` and the append loop below is a
    // defensive no-op (there are no unnamed columns to fold in). It is kept so `key_indices` is
    // provably a full permutation over the complete row — exactly the contract the Store
    // `ExactMultiOrder` gadget requires — and stays correct if the guard is ever relaxed. The
    // resulting deterministic lexicographic total order matches TerminusDB's `order_by/2` exactly.
    let mut key_indices = named_indices.clone();
    let mut directions = order_by.ordering.iter().map(|ordering| match ordering.order {
        terminusdb_woql2::order::Order::Asc => OrderDirection::Ascending,
        terminusdb_woql2::order::Order::Desc => OrderDirection::Descending,
    }).collect::<Vec<_>>();
    for column in 0..child.relation.schema.len() {
        if !named_indices.contains(&column) {
            key_indices.push(column);
            directions.push(OrderDirection::Ascending);
        }
    }
    let descriptor = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::OrderByUnsupportedShape)?;
    if descriptor.iter().any(|descriptor| descriptor.namespace != ResultIdNamespace::NodeValue || descriptor.nullable) {
        return Err(BgpPlanError::OrderByUnsupportedValue);
    }
    // Float32/Float64 are DELIBERATELY excluded. Ordering by the store's IEEE total-order
    // float key splits `-0.0` (sorts below `+0.0`) and orders `NaN` above `+∞`, diverging
    // from WOQL's numeric order (which ties `-0.0`/`+0.0` and leaves `NaN` unordered); `-0.0`
    // is reachable in committed data, so a float sort key reject-closes.
    let datatypes = child.relation.domains.iter().map(|domain| match domain {
        Some(EqualityValueDomain::Datatype(datatype)) if matches!(datatype,
                CanonicalDatatype::Boolean
                    | CanonicalDatatype::UInt8
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int64
            ) => Ok(*datatype),
        _ => return Err(BgpPlanError::OrderByUnsupportedValue),
    }).collect::<Result<Vec<_>, _>>()?;
    let schema = child.relation.schema.clone();
    let domains = child.relation.domains.clone();
    let collection = child.collection;
    let statement = if datatypes.len() == 1 {
        CompiledRelationStatement::ExactScalarOrder { input: Box::new(child.relation), datatype: datatypes[0], direction: directions[0] }
    } else {
        CompiledRelationStatement::ExactMultiOrder { input: Box::new(child.relation), key_indices, datatypes, directions }
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema,
            domains,
            statement,
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Compile a `Start` (offset) / `Limit` / `Once` window into an `ExactOrderedSlice` over an already
/// exactly-ordered child. The child must itself be an `ExactScalarOrder` / `ExactMultiOrder` /
/// `ExactOrderedSlice` relation (a window only has a well-defined answer over a proven total order);
/// a non-ordered, projected, counted, or distinct child rejects closed (`OrderedSliceUnsupportedShape`).
/// `Once` becomes `limit = 1`.
pub(crate) fn plan_exact_ordered_slice(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    let (child_query, offset, limit, once) = match query {
        Query::Start(start) => (start.query.as_ref(), start.start, None, false),
        Query::Limit(limit) => (limit.query.as_ref(), 0, Some(limit.limit), false),
        Query::Once(once) => (once.query.as_ref(), 0, Some(1), true),
        _ => return Err(BgpPlanError::OrderedSliceUnsupportedShape),
    };
    let child = plan_bgp(child_query)?;
    if child.projection.is_some()
        || child.count_variable.is_some()
        || child.full_row_distinct
        || !matches!(
            child.relation.statement,
            CompiledRelationStatement::ExactScalarOrder { .. }
                | CompiledRelationStatement::ExactMultiOrder { .. }
                | CompiledRelationStatement::ExactOrderedSlice { .. }
        )
    {
        return Err(BgpPlanError::OrderedSliceUnsupportedShape);
    }
    let collection = child.collection;
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: child.relation.schema.clone(),
            domains: child.relation.domains.clone(),
            statement: CompiledRelationStatement::ExactOrderedSlice {
                input: Box::new(child.relation),
                offset,
                limit,
                once,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Encode an object `Value::Data` literal to its order-preserving `(datatype-field, constant-field,
/// canonical datatype)` triple via `encode_range_constant`, restricted to the fixed-width INTEGRAL
/// datatypes. A non-`Data` value, or any datatype `encode_range_constant` rejects (float and every
/// other non-order-preserving type), rejects closed (`TripleValueOrderUnsupportedValue`) — the same
/// float exclusion as `plan_exact_scalar_order`.
pub(crate) fn encoded_ordered_triple_value(
    value: &Value,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    let Value::Data(value) = value else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    };
    // `encode_range_constant` already reject-closes float (and every non-order-preserving
    // datatype), so `Float32`/`Float64` are intentionally absent from the allow-list below:
    // ordering by the store's IEEE total-order float key diverges from WOQL numeric order
    // on `-0.0`/`+0.0` and `NaN`.
    let encoded = encode_range_constant(value)
        .map_err(|_| BgpPlanError::TripleValueOrderUnsupportedValue)?;
    if !matches!(
        encoded.2,
        CanonicalDatatype::UInt8
            | CanonicalDatatype::Int8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::Int16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::Int32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int64
    ) {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    }
    Ok(encoded)
}

/// Build the exact-ordered value scan behind the TripleSlice/TripleNext cursors: a constant-S/P
/// `Triple(s, p, ?variable)` under the numeric `bounds`, sorted by object value into an
/// `ExactScalarOrder` (wrapped in a `limit = 1` `ExactOrderedSlice` when `first_only`). Constant S/P
/// leave the object as the single kept column, so the range + exact-order arguments prove both
/// membership and the executor-visible order. Rejects closed a variable or schema-graph S/P, or any
/// shape that does not reduce to the single `[variable]` object column (`TripleValueOrderUnsupportedShape`).
pub(crate) fn compile_ordered_triple_value_scan(
    subject: &NodeValue,
    predicate: &NodeValue,
    graph: &Option<GraphType>,
    variable: &str,
    datatype: ArkFr,
    canonical_datatype: CanonicalDatatype,
    bounds: Vec<NormalizedBound>,
    direction: OrderDirection,
    first_only: bool,
) -> Result<CompiledBgp, BgpPlanError> {
    if !matches!(subject, NodeValue::Node(_))
        || !matches!(predicate, NodeValue::Node(_))
        || matches!(graph, Some(GraphType::Schema))
    {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    }

    let typed = TypedTriple {
        triple: Triple {
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: Value::Variable(variable.to_owned()),
            graph: graph.clone(),
        },
        object_kind: ObjectKindFilter::Any,
        source: ColumnSource::Materialized,
    };
    let mut atoms = vec![atom_from_triple(&typed)?];
    let comparisons = vec![NormalizedRange {
        var: variable.to_owned(),
        datatype,
        canonical_datatype,
        bounds,
    }];
    attach_ranges(&mut atoms, &comparisons)?;
    let (plan, schema) = plan_left_deep(&atoms)?;
    if schema != [variable] {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    }
    let ordered = CompiledRelation {
        schema: schema.clone(),
        domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
        statement: CompiledRelationStatement::ExactScalarOrder {
            input: Box::new(CompiledRelation {
                schema: schema.clone(),
                domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
                statement: CompiledRelationStatement::Bgp(plan),
            }),
            datatype: canonical_datatype,
            direction,
        },
    };
    let relation = if first_only {
        CompiledRelation {
            schema,
            domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
            statement: CompiledRelationStatement::ExactOrderedSlice {
                input: Box::new(ordered),
                offset: 0,
                limit: Some(1),
                once: false,
            },
        }
    } else {
        ordered
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection: None,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Compile a half-open `[low, high)` ordered-object slice for a constant-S/P triple: encode both
/// bounds (which must share one datatype), then delegate to `compile_ordered_triple_value_scan` with
/// `Ge low` / `Lt high` bounds. A non-variable object, or mismatched bound datatypes, rejects closed.
pub(crate) fn compile_ordered_triple_slice(
    subject: &NodeValue,
    predicate: &NodeValue,
    object: &Value,
    low: &Value,
    high: &Value,
    graph: &Option<GraphType>,
    direction: OrderDirection,
) -> Result<CompiledBgp, BgpPlanError> {
    let Value::Variable(variable) = object else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    };
    let (low_datatype, low_c, low_canonical) = encoded_ordered_triple_value(low)?;
    let (high_datatype, high_c, high_canonical) = encoded_ordered_triple_value(high)?;
    if low_datatype != high_datatype || low_canonical != high_canonical {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    }
    compile_ordered_triple_value_scan(
        subject,
        predicate,
        graph,
        variable,
        low_datatype,
        low_canonical,
        vec![
            NormalizedBound {
                op: RangeOp::Ge,
                c: low_c,
            },
            NormalizedBound {
                op: RangeOp::Lt,
                c: high_c,
            },
        ],
        direction,
        false,
    )
}

/// Turn an ordered value scan bound to a hidden object `variable` into a GROUND existence over an
/// `expected` constant object: gate the scan with an authenticated `Equality` (variable == expected),
/// then project to the zero-column (unit) relation. Used when the cursor's object position is itself
/// a constant. A non-`Data` expected value rejects closed (`TripleValueOrderUnsupportedValue`).
pub(crate) fn constrain_ordered_triple_value_to_ground(
    compiled: CompiledBgp,
    variable: &str,
    expected: &Value,
) -> Result<CompiledBgp, BgpPlanError> {
    let Value::Data(expected) = expected else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    };
    let equals = terminusdb_woql2::compare::Equals {
        left: Value::Variable(variable.to_owned()),
        right: Value::Data(expected.clone()),
    };
    let constraint = compile_equality_constraint(&compiled.relation, &equals)?;
    let equality = CompiledRelation {
        schema: compiled.relation.schema.clone(),
        domains: compiled.relation.domains.clone(),
        statement: CompiledRelationStatement::Equality {
            input: Box::new(compiled.relation),
            constraint,
        },
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: Vec::new(),
            domains: Vec::new(),
            statement: CompiledRelationStatement::BagProjection {
                input: Box::new(equality),
                indices: Vec::new(),
            },
        },
        projection: Some(Vec::new()),
        collection: compiled.collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Compile the largest currently exact ordered-value subset shared by the four
/// TerminusDB 12 cursor constructs.  Constant S/P leaves one complete object column,
/// so the existing range, exact-order, and exact-slice arguments prove both membership
/// and the executor-visible order without relying on physical dictionary IDs.
pub(crate) fn plan_ordered_triple_value_construct(
    query: &Query,
) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let compiled = match query {
        Query::TripleSlice(slice) => match &slice.object {
            Value::Variable(_) => compile_ordered_triple_slice(
                &slice.subject,
                &slice.predicate,
                &slice.object,
                &slice.low,
                &slice.high,
                &slice.graph,
                OrderDirection::Ascending,
            )?,
            Value::Data(_) => {
                const HIDDEN: &str = "__woql_proof_ground_triple_value";
                let hidden = Value::Variable(HIDDEN.to_owned());
                let compiled = compile_ordered_triple_slice(
                    &slice.subject,
                    &slice.predicate,
                    &hidden,
                    &slice.low,
                    &slice.high,
                    &slice.graph,
                    OrderDirection::Ascending,
                )?;
                constrain_ordered_triple_value_to_ground(compiled, HIDDEN, &slice.object)?
            }
            _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
        },
        Query::TripleSliceRev(slice) => match &slice.object {
            Value::Variable(_) => compile_ordered_triple_slice(
                &slice.subject,
                &slice.predicate,
                &slice.object,
                &slice.low,
                &slice.high,
                &slice.graph,
                OrderDirection::Descending,
            )?,
            Value::Data(_) => {
                const HIDDEN: &str = "__woql_proof_ground_triple_value";
                let hidden = Value::Variable(HIDDEN.to_owned());
                let compiled = compile_ordered_triple_slice(
                    &slice.subject,
                    &slice.predicate,
                    &hidden,
                    &slice.low,
                    &slice.high,
                    &slice.graph,
                    OrderDirection::Descending,
                )?;
                constrain_ordered_triple_value_to_ground(compiled, HIDDEN, &slice.object)?
            }
            _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
        },
        Query::TripleNext(next) => {
            let (variable, reference, expected, op, direction) = match (&next.object, &next.next) {
                (Value::Data(reference), Value::Variable(variable)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Gt, OrderDirection::Ascending)
                }
                (Value::Variable(variable), Value::Data(reference)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Lt, OrderDirection::Descending)
                }
                (Value::Data(reference), Value::Data(expected)) => {
                    ("__woql_proof_ground_triple_value", Value::Data(reference.clone()), Some(Value::Data(expected.clone())), RangeOp::Gt, OrderDirection::Ascending)
                }
                _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
            };
            let (datatype, c, canonical) = encoded_ordered_triple_value(&reference)?;
            let compiled = compile_ordered_triple_value_scan(
                &next.subject,
                &next.predicate,
                &next.graph,
                variable,
                datatype,
                canonical,
                vec![NormalizedBound { op, c }],
                direction,
                true,
            )?;
            match expected {
                Some(expected) => constrain_ordered_triple_value_to_ground(compiled, variable, &expected)?,
                None => compiled,
            }
        }
        Query::TriplePrevious(previous) => {
            let (variable, reference, expected, op, direction) = match (&previous.object, &previous.previous) {
                (Value::Data(reference), Value::Variable(variable)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Lt, OrderDirection::Descending)
                }
                (Value::Variable(variable), Value::Data(reference)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Gt, OrderDirection::Ascending)
                }
                (Value::Data(reference), Value::Data(expected)) => {
                    ("__woql_proof_ground_triple_value", Value::Data(reference.clone()), Some(Value::Data(expected.clone())), RangeOp::Lt, OrderDirection::Descending)
                }
                _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
            };
            let (datatype, c, canonical) = encoded_ordered_triple_value(&reference)?;
            let compiled = compile_ordered_triple_value_scan(
                &previous.subject,
                &previous.predicate,
                &previous.graph,
                variable,
                datatype,
                canonical,
                vec![NormalizedBound { op, c }],
                direction,
                true,
            )?;
            match expected {
                Some(expected) => constrain_ordered_triple_value_to_ground(compiled, variable, &expected)?,
                None => compiled,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(compiled))
}

/// Compile a WOQL `TripleCount(resource, ?n)` into the `LayerTripleCount` relation — one column
/// delivering the layer's authenticated triple count, routed to the resource's collection. The
/// resource must be a non-empty `.../instance` graph (`TripleCountUnsupportedResource`) and the count
/// output a variable (`TripleCountOutputNotVariable`).
pub(crate) fn plan_layer_triple_count(
    count: &terminusdb_woql2::misc::TripleCount,
) -> Result<CompiledBgp, BgpPlanError> {
    let collection = count
        .resource
        .strip_suffix("/instance")
        .filter(|collection| !collection.is_empty())
        .ok_or(BgpPlanError::TripleCountUnsupportedResource)?;
    let variable = match &count.count {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TripleCountOutputNotVariable),
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: vec![variable],
            domains: vec![None],
            statement: CompiledRelationStatement::LayerTripleCount,
        },
        projection: None,
        collection: Some(collection.to_owned()),
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Decode an `xsd:decimal` literal to an EXACT nonnegative `u64` triple-count comparison constant.
/// Rejects closed (`TripleCountComparisonUnsupportedConstant`) a non-decimal, negative, or
/// negative-scale value, and any value not exactly representable as the round-tripped `u64` — the
/// bounded contract is deliberately NOT broadened to scientific notation / fractional decimals.
pub(crate) fn exact_integral_decimal_u64(value: &XSDAnySimpleType) -> Result<u64, BgpPlanError> {
    let XSDAnySimpleType::Decimal(decimal) = value else {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    };
    // A negative scale is the retained representation of a positive decimal exponent.
    // Do not silently broaden this bounded contract to scientific notation.
    if decimal.is_sign_negative() || decimal.scale() < 0 {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    }
    let value = u64::try_from(decimal)
        .map_err(|_| BgpPlanError::TripleCountComparisonUnsupportedConstant)?;
    if decimal != &decimal_rs::Decimal::from(value) {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    }
    Ok(value)
}

/// Flatten a `TripleCount`-comparison body into its comparison LEAVES: recurse through
/// `And` / `Pin` / `Immediately`, drop `True` / `Comment` no-ops, and push every other node as a
/// leaf. The first leaf is expected to be the `TripleCount`; the rest are the comparisons over it.
pub(crate) fn flatten_triple_count_comparison_body<'a>(query: &'a Query, leaves: &mut Vec<&'a Query>) {
    match query {
        Query::And(and) => {
            for child in &and.and {
                flatten_triple_count_comparison_body(child, leaves);
            }
        }
        Query::Pin(pin) => flatten_triple_count_comparison_body(pin.query.as_ref(), leaves),
        Query::Immediately(immediately) => {
            flatten_triple_count_comparison_body(immediately.query.as_ref(), leaves)
        }
        Query::True(_) | Query::Comment(_) => {}
        other => leaves.push(other),
    }
}

/// Parse one comparison leaf over the count `variable` into a `GeneratedU64Filter` spec
/// `(op, constant, constant_on_left)`: `Greater`/`Gte`/`Less`/`Lte`/`Equals` between the variable and
/// an exact-`u64` decimal constant (either operand order). `constant_on_left` records which side the
/// constant sat on. Any other shape — a comparison not involving the variable, or a non-exact-integral
/// constant — rejects closed (`TripleCountComparisonUnsupportedShape`).
pub(crate) fn generated_triple_count_constraint(
    query: &Query,
    variable: &str,
) -> Result<(GeneratedU64ComparisonOp, u64, bool), BgpPlanError> {
    let range = |left: &DataValue,
                 right: &DataValue,
                 op: GeneratedU64ComparisonOp|
     -> Result<_, BgpPlanError> {
        match (left, right) {
            (DataValue::Variable(candidate), DataValue::Data(value)) if candidate == variable => {
                Ok((op, exact_integral_decimal_u64(value)?, false))
            }
            (DataValue::Data(value), DataValue::Variable(candidate)) if candidate == variable => {
                Ok((op, exact_integral_decimal_u64(value)?, true))
            }
            _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
        }
    };
    match query {
        Query::Greater(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Gt),
        Query::Gte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Ge),
        Query::Less(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Lt),
        Query::Lte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Le),
        Query::Equals(value) => {
            use terminusdb_woql2::value::WoqlValue;
            match (&value.left, &value.right) {
                (WoqlValue::Variable(candidate), WoqlValue::Data(value))
                    if candidate == variable =>
                {
                    Ok((
                        GeneratedU64ComparisonOp::Eq,
                        exact_integral_decimal_u64(value)?,
                        false,
                    ))
                }
                (WoqlValue::Data(value), WoqlValue::Variable(candidate))
                    if candidate == variable =>
                {
                    Ok((
                        GeneratedU64ComparisonOp::Eq,
                        exact_integral_decimal_u64(value)?,
                        true,
                    ))
                }
                _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
            }
        }
        _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
    }
}

/// Compile a `TripleCount` followed by comparison leaves into a `LayerTripleCount` wrapped in one
/// `GeneratedU64Filter` per comparison (each recomputed by the verifier over the authenticated count).
/// Returns `Ok(None)` when the body carries no `TripleCount` (so other planners can try it); a
/// `TripleCount` in a non-leading or otherwise unsupported position rejects closed
/// (`TripleCountComparisonUnsupportedShape`).
pub(crate) fn plan_triple_count_comparison_body(
    body: &Query,
) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let mut leaves = Vec::new();
    flatten_triple_count_comparison_body(body, &mut leaves);
    let Some(Query::TripleCount(triple_count)) = leaves.first().copied() else {
        if leaves.iter().any(|leaf| matches!(leaf, Query::TripleCount(_))) {
            return Err(BgpPlanError::TripleCountComparisonUnsupportedShape);
        }
        return Ok(None);
    };
    let mut compiled = plan_layer_triple_count(triple_count)?;
    let variable = compiled.relation.schema[0].clone();
    for comparison in leaves.into_iter().skip(1) {
        let (op, constant, constant_on_left) =
            generated_triple_count_constraint(comparison, &variable)?;
        let input = compiled.relation;
        compiled.relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: 0,
                op,
                constant,
                constant_on_left,
            },
        };
    }
    Ok(Some(compiled))
}


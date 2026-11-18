//! Aggregate lowering — group / collection sum, count, and extrema.
//!
//! Lowers the aggregation forms over an authenticated child relation: group-by and
//! whole-collection `sum` (`plan_group_sum_form`, `plan_collection_sum_form`), `count`
//! (`plan_group_count_form`, `plan_collection_count_form`), and `min` / `max` extrema
//! (`plan_group_extrema_form`, `plan_collection_extrema_form`). The verifier recomputes the
//! aggregate over the root-authenticated group columns; only the exact provable numeric
//! fragment is accepted, everything else is rejected closed.
//!
//! See [`terminus_store::proof::primer`] for the cryptographic background (§5 the committed
//! columns a verifier-recompute re-derives over, §7 the LogUp/grouping machinery behind the store
//! group gadgets, and §11 soundness-vs-fidelity).

use super::*;

/// Compile `GroupBy(keys.., grouped_value, template) & RangeMin/RangeMax(list, result)` into a
/// `GroupExtrema` relation — the per-group numeric MIN (`RangeMin`) or MAX (`RangeMax`) of an
/// authenticated value column, recomputed by the verifier over the root-authenticated child rows.
///
/// ACCEPTS: an `And` of exactly a `GroupBy` and a matching `RangeMin`/`RangeMax` whose `list` is the
/// group's hidden `grouped_value`; 1..=2 DISTINCT group keys pinned to the `Node` domain; a variable
/// `template` naming a non-nullable node/value column pinned to a fixed-width INTEGRAL datatype; and
/// fresh, mutually distinct template/hidden/result/key variables over a bare (non-projected /
/// non-aggregated / non-distinct) child BGP.
///
/// Returns `Ok(None)` when the query is not a group-extrema at all; rejects closed a malformed shape
/// (`GroupExtremaUnsupportedShape`) or a non-integral / float / nullable value column
/// (`GroupExtremaUnsupportedValue`).
pub(crate) fn plan_group_extrema_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() == 2 => &and.and,
        _ => return Ok(None),
    };
    let (group, range, kind) = match (&conjuncts[0], &conjuncts[1]) {
        (Query::GroupBy(group), Query::RangeMin(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::GroupBy(group), Query::RangeMax(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        _ if conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_) | Query::RangeMin(_) | Query::RangeMax(_))) => {
            return Err(BgpPlanError::GroupExtremaUnsupportedShape)
        }
        _ => return Ok(None),
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let list = match range.0 {
        DataValue::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let result = match range.1 {
        DataValue::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    if hidden != list
        || hidden == &result
        || template == *hidden
        || template == result
        || group.group_by.iter().any(|key| key == hidden || key == &result || key == &template)
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    if child.relation.schema.iter().any(|variable| variable == hidden || variable == &result) {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let key_columns = group.group_by.iter().map(|key| {
        child.relation.schema.iter().position(|variable| variable == key)
            .ok_or(BgpPlanError::GroupExtremaUnsupportedShape)
    }).collect::<Result<Vec<_>, _>>()?;
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|v| v == &template)
        .ok_or(BgpPlanError::GroupExtremaUnsupportedValue)?;
    if key_columns.iter().any(|key_column| *key_column == value_column) {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupExtremaUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        child.relation.domains.get(*key_column).copied().flatten()
            != Some(EqualityValueDomain::Node)
            || descriptors[*key_column].nullable
            || descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
    }) || descriptors[value_column].nullable
        || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let collection = child.collection;
    let mut schema = group.group_by.clone();
    schema.push(result);
    let mut domains = key_columns.iter().map(|key_column| child.relation.domains[*key_column])
        .collect::<Vec<_>>();
    domains.push(Some(EqualityValueDomain::Datatype(datatype)));
    Ok(Some(CompiledBgp {
        relation: CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::GroupExtrema {
                input: Box::new(child.relation),
                key_columns,
                value_column,
                datatype,
                kind,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile `Collect(template into hidden) & RangeMin/RangeMax(list, result)` — a WHOLE-collection
/// (un-grouped) numeric min/max — into a `CollectionExtrema` relation over the single authenticated
/// integral value column. Yields (`Ok(None)`) to `plan_group_extrema_form` when a `GroupBy` is present;
/// a `RangeMin`/`RangeMax` without a leading `Collect` rejects closed
/// (`CollectionExtremaUnsupportedShape`), as does a non-integral value (`CollectionExtremaUnsupportedValue`).
pub(crate) fn plan_collection_extrema_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() == 2 => &and.and,
        _ => return Ok(None),
    };
    let (collect, range, kind) = match (&conjuncts[0], &conjuncts[1]) {
        (Query::Collect(collect), Query::RangeMin(range)) => {
            (collect, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::Collect(collect), Query::RangeMax(range)) => {
            (collect, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        // GroupBy extrema has its own planner and remains reachable.
        _ if conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_)))
            && conjuncts.iter().any(|query| matches!(query, Query::RangeMin(_) | Query::RangeMax(_))) =>
        {
            return Ok(None)
        }
        _ if conjuncts
            .iter()
            .any(|query| matches!(query, Query::RangeMin(_) | Query::RangeMax(_))) =>
        {
            return Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        }
        _ => return Ok(None),
    };
    let template = match &collect.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedValue),
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    let list = match range.0 {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    let result = match range.1 {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    if hidden != list || hidden == result || template == hidden || template == result {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::CollectionExtremaUnsupportedValue)?;
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::CollectionExtremaUnsupportedShape)?;
    if descriptors[value_column].nullable
        || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
    {
        return Err(BgpPlanError::CollectionExtremaUnsupportedValue);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedValue),
    };
    let collection = child.collection;
    Ok(Some(CompiledBgp {
        relation: CompiledRelation {
            schema: vec![result.clone()],
            domains: vec![Some(EqualityValueDomain::Datatype(datatype))],
            statement: CompiledRelationStatement::CollectionExtrema {
                input: Box::new(child.relation),
                value_column,
                datatype,
                kind,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Decode an `xsd:decimal` literal to an EXACT `i64` group-sum comparison constant (the signed
/// sibling of order.rs's `exact_integral_decimal_u64`). Rejects closed a non-decimal, negative-scale,
/// or out-of-range / non-round-tripping value (`GroupSumComparisonUnsupportedConstant`).
pub(crate) fn exact_integral_decimal_i64(value: &XSDAnySimpleType) -> Result<i64, BgpPlanError> {
    let XSDAnySimpleType::Decimal(decimal) = value else {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    };
    if decimal.scale() < 0 {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    }
    let value = i64::try_from(decimal)
        .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?;
    if decimal != &decimal_rs::Decimal::from(value) {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    }
    Ok(value)
}

/// Parse one comparison over the aggregate `variable` into a filter spec
/// `(op, namespace, i128 constant, constant_on_left)`: `Greater`/`Gte`/`Less`/`Lte`/`Equals` between
/// the variable and an exact integral decimal constant (either operand order). `signed` selects the
/// generated `GeneratedDecimalI64` vs `GeneratedDecimalU64` namespace (and the exact-`i64` vs exact-`u64`
/// decoding). Any other comparison shape or non-exact constant rejects closed
/// (`GroupSumComparisonUnsupportedShape` / `GroupSumComparisonUnsupportedConstant`).
pub(crate) fn generated_group_sum_constraint(
    query: &Query,
    variable: &str,
    signed: bool,
) -> Result<(GeneratedU64ComparisonOp, ResultIdNamespace, i128, bool), BgpPlanError> {
    let constant = |value: &XSDAnySimpleType| -> Result<_, BgpPlanError> {
        if signed {
            Ok((
                ResultIdNamespace::GeneratedDecimalI64,
                i128::from(exact_integral_decimal_i64(value)?),
            ))
        } else {
            let value = exact_integral_decimal_u64(value)
                .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?;
            Ok((ResultIdNamespace::GeneratedDecimalU64, i128::from(value)))
        }
    };
    let range = |left: &DataValue,
                 right: &DataValue,
                 op: GeneratedU64ComparisonOp|
     -> Result<_, BgpPlanError> {
        match (left, right) {
            (DataValue::Variable(candidate), DataValue::Data(value)) if candidate == variable => {
                let (domain, constant) = constant(value)?;
                Ok((op, domain, constant, false))
            }
            (DataValue::Data(value), DataValue::Variable(candidate)) if candidate == variable => {
                let (domain, constant) = constant(value)?;
                Ok((op, domain, constant, true))
            }
            _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
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
                    let (domain, constant) = constant(value)?;
                    Ok((GeneratedU64ComparisonOp::Eq, domain, constant, false))
                }
                (WoqlValue::Data(value), WoqlValue::Variable(candidate))
                    if candidate == variable =>
                {
                    let (domain, constant) = constant(value)?;
                    Ok((GeneratedU64ComparisonOp::Eq, domain, constant, true))
                }
                _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
            }
        }
        _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
    }
}

/// Map an `xsd` integral literal to a constant-summand `(CollectionSumTemplate, CanonicalDatatype)`
/// for a `sum` over a CONSTANT template — one arm per fixed-width integral xsd type (`unsignedByte` ..
/// `long`/`integer`). A non-integral literal, or an `unsignedInt` outside `u32`, rejects closed
/// (`CollectionSumUnsupportedValue`).
pub(crate) fn exact_collection_sum_constant(
    value: &XSDAnySimpleType,
) -> Result<(CollectionSumTemplate, CanonicalDatatype), BgpPlanError> {
    Ok(match value {
        XSDAnySimpleType::UnsignedByte(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(*value)),
            CanonicalDatatype::UInt8,
        ),
        XSDAnySimpleType::Byte(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int8,
        ),
        XSDAnySimpleType::UnsignedShort(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(*value)),
            CanonicalDatatype::UInt16,
        ),
        XSDAnySimpleType::Short(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int16,
        ),
        XSDAnySimpleType::UnsignedInt(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(
                u32::try_from(*value).map_err(|_| BgpPlanError::CollectionSumUnsupportedValue)?,
            )),
            CanonicalDatatype::UInt32,
        ),
        XSDAnySimpleType::Int(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int32,
        ),
        XSDAnySimpleType::UnsignedLong(value) => (
            CollectionSumTemplate::UnsignedConstant(*value),
            CanonicalDatatype::UInt64,
        ),
        XSDAnySimpleType::Long(value) | XSDAnySimpleType::Integer(value) => (
            CollectionSumTemplate::SignedConstant(*value),
            CanonicalDatatype::Int64,
        ),
        _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
    })
}

/// Compile `Collect(template into hidden) & Sum(list, result)` (optionally followed by comparison
/// leaves) into a `CollectionSum` relation — the numeric sum over the whole collection's authenticated
/// value column, or over a constant template — with each trailing comparison lowered to a
/// `GeneratedU64Filter` / `GeneratedI64Filter` per the value's signedness. The template must be a
/// variable naming a non-nullable node/value column pinned to a fixed-width INTEGRAL datatype, or a
/// constant integral literal; `hidden` must equal the `Sum` list and be distinct from `result`, over a
/// bare child BGP. Yields (`Ok(None)`) to the GroupBy planners and to the sequential planner when a
/// trailing `If`/`Optional`/`Not` appears; a `Collect`-without-`GroupBy` in an unsupported shape rejects
/// closed (`CollectionSumUnsupportedShape` / `CollectionSumUnsupportedValue`).
pub(crate) fn plan_collection_sum_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::Collect(collect), Query::Sum(sum)) = (&conjuncts[0], &conjuncts[1]) else {
        if matches!(conjuncts.first(), Some(Query::GroupBy(_)))
            || conjuncts.iter().any(|query| matches!(query, Query::Length(_)))
        {
            return Ok(None);
        }
        if conjuncts.iter().any(|query| matches!(query, Query::Collect(_)))
            && !conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_)))
        {
            return Err(BgpPlanError::CollectionSumUnsupportedShape);
        }
        return Ok(None);
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    let list = match &sum.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    let result = match &sum.result {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    if hidden != list || hidden == result {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    if matches!(&collect.template, Value::Variable(variable) if variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::CollectionSumUnsupportedShape)?;
    let (template, datatype) = match &collect.template {
        Value::Variable(variable) => {
            let column = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or(BgpPlanError::CollectionSumUnsupportedValue)?;
            if descriptors[column].nullable
                || descriptors[column].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::CollectionSumUnsupportedValue);
            }
            let datatype = match child.relation.domains.get(column).copied().flatten() {
                Some(EqualityValueDomain::Datatype(
                    datatype @ (CanonicalDatatype::UInt8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::Int64),
                )) => datatype,
                _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
            };
            (CollectionSumTemplate::Column(column), datatype)
        }
        Value::Data(value) => exact_collection_sum_constant(value)?,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
    };
    let collection = child.collection;
    let signed = matches!(
        datatype,
        CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64
    );
    let mut relation = CompiledRelation {
        schema: vec![result.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::CollectionSum {
            input: Box::new(child.relation),
            template,
            datatype,
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, signed)?;
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: match domain {
                ResultIdNamespace::GeneratedDecimalU64 => {
                    CompiledRelationStatement::GeneratedU64Filter {
                        input: Box::new(input),
                        column: 0,
                        op,
                        constant: u64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                ResultIdNamespace::GeneratedDecimalI64 => {
                    CompiledRelationStatement::GeneratedI64Filter {
                        input: Box::new(input),
                        column: 0,
                        op,
                        constant: i64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                _ => unreachable!(),
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}


/// Compile `Collect(template into hidden) & Length(list, result)` (optionally followed by unsigned
/// comparison leaves) into a `CollectionCount` relation — the cardinality of the whole collection,
/// filtered by each trailing `GeneratedU64Filter`. Same shape discipline as `plan_collection_sum_form`
/// (variable `hidden` == the `Length` list, fresh distinct `result`, bare child); yields (`Ok(None)`) to
/// the GroupBy planners, and a stray `Collect`/`Length` in an unsupported shape rejects closed
/// (`CollectionCountUnsupportedShape`).
pub(crate) fn plan_collection_count_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::Collect(collect), Query::Length(length)) = (&conjuncts[0], &conjuncts[1]) else {
        if matches!(conjuncts.first(), Some(Query::GroupBy(_))) {
            return Ok(None);
        }
        if conjuncts
            .iter()
            .any(|query| matches!(query, Query::Collect(_) | Query::Length(_)))
        {
            return Err(BgpPlanError::CollectionCountUnsupportedShape);
        }
        return Ok(None);
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    let list = match &length.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    let result = match &length.length {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    if hidden != list || hidden == result {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    if matches!(&collect.template, Value::Variable(variable) if variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    let collection = child.collection;
    let mut relation = CompiledRelation {
        schema: vec![result.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::CollectionCount {
            input: Box::new(child.relation),
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, false)?;
        if domain != ResultIdNamespace::GeneratedDecimalU64 {
            return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
        }
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: 0,
                op,
                constant: u64::try_from(constant)
                    .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?,
                constant_on_left,
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile `GroupBy(keys.., grouped_value, template) & Length(list, result)` (optionally followed by
/// unsigned comparison leaves) into a `GroupCount` relation — the per-group solution count, recomputed
/// by the verifier over the root-authenticated child rows and filtered by each trailing
/// `GeneratedU64Filter`. Requires 1..=2 distinct `Node`-domain group keys, a template naming a
/// non-nullable node/value OR predicate column, `hidden == list`, and fresh distinct variables over a
/// bare child BGP. Returns `Ok(None)` when it is not a group-count; rejects closed a malformed shape
/// (`GroupCountUnsupportedShape`) or an unsupported key/template value (`GroupCountUnsupportedValue`); a
/// non-`u64` comparison constant rejects (`GroupSumComparisonUnsupportedConstant`).
pub(crate) fn plan_group_count_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::GroupBy(group), Query::Length(length)) = (&conjuncts[0], &conjuncts[1]) else {
        if conjuncts.iter().any(|query| matches!(query, Query::Length(_))) {
            return Err(BgpPlanError::GroupCountUnsupportedShape);
        }
        return Ok(None);
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    let list = match &length.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    let result = match &length.length {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    if hidden != list
        || group
            .group_by
            .iter()
            .any(|key| key == template || key == hidden || key == result)
        || template == hidden
        || template == result
        || hidden == result
    {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let key_columns = group
        .group_by
        .iter()
        .map(|key| {
            child
                .relation
                .schema
                .iter()
                .position(|variable| variable == key)
                .ok_or(BgpPlanError::GroupCountUnsupportedShape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let template_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::GroupCountUnsupportedValue)?;
    if key_columns.iter().any(|key_column| {
        *key_column == template_column
            || child.relation.domains.get(*key_column).copied().flatten()
                != Some(EqualityValueDomain::Node)
    }) {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupCountUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
            || descriptors[*key_column].nullable
    }) || descriptors[template_column].nullable
        || !matches!(
            descriptors[template_column].namespace,
            ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate
        )
    {
        return Err(BgpPlanError::GroupCountUnsupportedValue);
    }
    let collection = child.collection;
    let mut schema = Vec::with_capacity(key_columns.len() + 1);
    let mut domains = Vec::with_capacity(schema.capacity());
    for (key, key_column) in group.group_by.iter().zip(&key_columns) {
        schema.push(key.clone());
        domains.push(child.relation.domains[*key_column]);
    }
    schema.push(result.clone());
    domains.push(None);
    let mut relation = CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::GroupCount {
            input: Box::new(child.relation),
            key_columns: key_columns.clone(),
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, false)?;
        if domain != ResultIdNamespace::GeneratedDecimalU64 {
            return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
        }
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: key_columns.len(),
                op,
                constant: u64::try_from(constant)
                    .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?,
                constant_on_left,
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile `GroupBy(keys.., grouped_value, template) & Sum(list, result)` (optionally followed by
/// comparison leaves) into a `GroupSum` relation — the per-group numeric sum over an authenticated
/// integral value column, recomputed by the verifier and filtered by each trailing generated-decimal
/// comparison (`GeneratedU64Filter` / `GeneratedI64Filter`). Same key / child discipline as
/// `plan_group_count_form`, with the template pinned to a fixed-width INTEGRAL datatype. Yields
/// (`Ok(None)`) to the sequential planner for a trailing `If`/`Optional`/`Not`, and when it is not a
/// group-sum; a malformed shape rejects closed (`GroupSumUnsupportedShape`).
pub(crate) fn plan_group_sum_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    // Let the general sequential-relation planner compose supported When/Optional/Not
    // operators over the aggregate prefix. Equals and range comparisons remain here so
    // generated-decimal domains are handled without Store-ID coercion.
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::GroupBy(group), Query::Sum(sum)) = (&conjuncts[0], &conjuncts[1]) else {
        if conjuncts.iter().any(|query| matches!(query, Query::Sum(_))) {
            return Err(BgpPlanError::GroupSumUnsupportedShape);
        }
        return Ok(None);
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    let list = match &sum.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    let result = match &sum.result {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    if hidden != list
        || group
            .group_by
            .iter()
            .any(|key| key == template || key == hidden || key == result)
        || template == hidden
        || template == result
        || hidden == result
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let key_columns = group
        .group_by
        .iter()
        .map(|key| {
            child
                .relation
                .schema
                .iter()
                .position(|variable| variable == key)
                .ok_or(BgpPlanError::GroupSumUnsupportedShape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::GroupSumUnsupportedValue)?;
    if key_columns.iter().any(|key_column| {
        *key_column == value_column
            || child.relation.domains.get(*key_column).copied().flatten()
                != Some(EqualityValueDomain::Node)
    }) {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupSumUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
            || descriptors[*key_column].nullable
    }) || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
        || descriptors[value_column].nullable
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::GroupSumUnsupportedValue),
    };
    let collection = child.collection;
    let mut schema = Vec::with_capacity(key_columns.len() + 1);
    let mut domains = Vec::with_capacity(schema.capacity());
    for (key, key_column) in group.group_by.iter().zip(&key_columns) {
        schema.push(key.clone());
        domains.push(child.relation.domains[*key_column]);
    }
    schema.push(result.clone());
    domains.push(None);
    let signed = matches!(
        datatype,
        CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64
    );
    let mut relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::GroupSum {
                input: Box::new(child.relation),
                key_columns: key_columns.clone(),
                value_column,
                datatype,
            },
        };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, signed)?;
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: match domain {
                ResultIdNamespace::GeneratedDecimalU64 => {
                    CompiledRelationStatement::GeneratedU64Filter {
                        input: Box::new(input),
                        column: key_columns.len(),
                        op,
                        constant: u64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                ResultIdNamespace::GeneratedDecimalI64 => {
                    CompiledRelationStatement::GeneratedI64Filter {
                        input: Box::new(input),
                        column: key_columns.len(),
                        op,
                        constant: i64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                _ => unreachable!(),
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}


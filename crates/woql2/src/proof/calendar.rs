//! Calendar / date-function lowering.
//!
//! Lowers the calendar predicates over store-authenticated date operands into verifier-recompute
//! relations: the integer-valued functions (weekday, day-of-year, week number, ...) via
//! [`build_calendar_int`] and the date-valued functions (next / previous day, ...) via
//! [`build_calendar_date`], dispatched by [`plan_calendar_form`]. The verifier re-derives each
//! result from the decoded date column.

use super::*;

pub(crate) fn is_calendar_conjunct(query: &Query) -> bool {
    matches!(
        query,
        Query::Weekday(_)
            | Query::WeekdaySundayStart(_)
            | Query::IsoWeek(_)
            | Query::DayAfter(_)
            | Query::DayBefore(_)
            | Query::MonthStartDate(_)
            | Query::MonthEndDate(_)
            | Query::DateDuration(_)
            | Query::Interval(_)
            | Query::IntervalStartDuration(_)
            | Query::IntervalDurationEnd(_)
            | Query::IntervalRelation(_)
            | Query::IntervalRelationTyped(_)
            | Query::MonthStartDates(_)
            | Query::MonthEndDates(_)
    )
}

/// Resolve a calendar op's operand `DataValue` to an authenticated child column index, requiring it
/// to be a VARIABLE bound as a nonnullable node/value store column whose pinned datatype (if any) is
/// `required` (an UNPINNED column is allowed — the actual datatype is enforced per-row at recompute).
pub(crate) fn calendar_operand_column(
    operand: &DataValue,
    child: &CompiledBgp,
    required: CanonicalDatatype,
    not_bound: impl Fn(String) -> BgpPlanError,
    not_typed: impl Fn(String) -> BgpPlanError,
) -> Result<usize, BgpPlanError> {
    let variable = match operand {
        DataValue::Variable(variable) => variable,
        // A literal/list operand has no per-row store authentication.
        _ => return Err(BgpPlanError::CalendarOperandNotVariable),
    };
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| not_bound(variable.clone()))?;
    let descriptors =
        relation_result_descriptors(&child.relation).map_err(|_| not_typed(variable.clone()))?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(not_typed(variable.clone()));
    }
    // An UNPINNED column (`None`) is allowed (recompute enforces the datatype per row); a datatype
    // pinned to EXACTLY `required` is allowed; any other pinned datatype rejects closed.
    match child.relation.domains.get(index).copied().flatten() {
        None => {}
        Some(EqualityValueDomain::Datatype(datatype)) if datatype == required => {}
        Some(_) => return Err(not_typed(variable.clone())),
    }
    Ok(index)
}

/// A calendar op's result field must be a FRESH output VARIABLE (never a literal / validate mode).
pub(crate) fn calendar_result_var(result: &DataValue) -> Result<String, BgpPlanError> {
    match result {
        DataValue::Variable(variable) => Ok(variable.clone()),
        _ => Err(BgpPlanError::CalendarResultNotVariable),
    }
}

/// Build an INTEGER-result calendar [`CompiledBgp`]: append each `(result_var, function)` as a fresh
/// committed output column (schema + `None` domain), mirroring `plan_eval_form`'s integer path but
/// for one-or-more outputs. Each result variable must be fresh (not in the child schema) and DISTINCT
/// from the others (the running `schema` already holds earlier results, so a single membership check
/// enforces both).
pub(crate) fn build_calendar_int(
    child: CompiledBgp,
    date_column: usize,
    outputs: Vec<(String, CalendarIntFn)>,
) -> Result<CompiledBgp, BgpPlanError> {
    let mut schema = child.relation.schema.clone();
    let mut domains = child.relation.domains.clone();
    let collection = child.collection.clone();
    let mut functions = Vec::with_capacity(outputs.len());
    for (variable, function) in outputs {
        if schema.iter().any(|candidate| candidate == &variable) {
            return Err(BgpPlanError::CalendarResultAlreadyBound { var: variable });
        }
        schema.push(variable);
        // The derived column is a generated integer, not an executor value domain.
        domains.push(None);
        functions.push(function);
    }
    let relation = CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::CalendarInt {
            input: Box::new(child.relation),
            date_column,
            outputs: functions,
        },
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
        order_invented_tiebreak: false,
    })
}

/// Build a DATE-result calendar [`CompiledBgp`]: the result is TERMINAL/revealed (NOT a committed
/// schema column), so the child schema/domains pass through unchanged and the result variable lives
/// only in the statement — mirroring `plan_concat_form`. The result must be a fresh variable.
pub(crate) fn build_calendar_date(
    child: CompiledBgp,
    input_column: usize,
    op: CalendarDateFn,
    result: String,
) -> Result<CompiledBgp, BgpPlanError> {
    if child
        .relation
        .schema
        .iter()
        .any(|candidate| candidate == &result)
    {
        return Err(BgpPlanError::CalendarResultAlreadyBound { var: result });
    }
    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::CalendarDate {
            input: Box::new(child.relation),
            op,
            input_column,
            result,
        },
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
        order_invented_tiebreak: false,
    })
}

/// Compile a single-input WOQL CALENDAR op conjunct into a verifier-recompute
/// [`CompiledRelationStatement::CalendarInt`] / [`CompiledRelationStatement::CalendarDate`], modelled
/// on [`plan_eval_form`]/[`plan_concat_form`].
///
/// SHIP (reject-closed everywhere else): `Weekday`/`WeekdaySundayStart`/`IsoWeek` (INTEGER outputs
/// over a bound `xsd:date`) and `DayAfter`/`DayBefore` (`xsd:date`) / `MonthStartDate`/`MonthEndDate`
/// (`xsd:gYearMonth`) (DATE output). The operand must be a variable bound as a nonnullable store
/// node/value column; every result must be a fresh, distinct output variable (validate mode and
/// already-bound results reject closed). The two-input / interval / duration / 3-arg generator ops
/// are claimed and reject-closed ([`BgpPlanError::CalendarTwoInputUnsupported`]).
pub(crate) fn plan_calendar_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| is_calendar_conjunct(conjunct))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        // No calendar conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple calendar ops would need a dependency-ordered chain; v1 takes exactly one.
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }
    let calendar = &conjuncts[positions[0]];

    // Reject-closed the two-input / interval / duration / range-generator ops UP FRONT (before even
    // building a child) — heavier follow-ups, never approximated.
    let reject_op = match calendar {
        Query::DateDuration(_) => Some("DateDuration"),
        Query::Interval(_) => Some("Interval"),
        Query::IntervalStartDuration(_) => Some("IntervalStartDuration"),
        Query::IntervalDurationEnd(_) => Some("IntervalDurationEnd"),
        Query::IntervalRelation(_) => Some("IntervalRelation"),
        Query::IntervalRelationTyped(_) => Some("IntervalRelationTyped"),
        Query::MonthStartDates(_) => Some("MonthStartDates"),
        Query::MonthEndDates(_) => Some("MonthEndDates"),
        _ => None,
    };
    if let Some(op) = reject_op {
        return Err(BgpPlanError::CalendarTwoInputUnsupported { op });
    }

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare calendar op with no store binding cannot authenticate its operand.
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }

    let date_operand = |operand: &DataValue, child: &CompiledBgp| {
        calendar_operand_column(
            operand,
            child,
            CanonicalDatatype::Date,
            |var| BgpPlanError::CalendarOperandNotBound { var },
            |var| BgpPlanError::CalendarOperandNotDate { var },
        )
    };
    let yearmonth_operand = |operand: &DataValue, child: &CompiledBgp| {
        calendar_operand_column(
            operand,
            child,
            CanonicalDatatype::GYearMonth,
            |var| BgpPlanError::CalendarOperandNotBound { var },
            |var| BgpPlanError::CalendarOperandNotYearMonth { var },
        )
    };

    let compiled = match calendar {
        Query::Weekday(weekday) => {
            let date_column = date_operand(&weekday.date, &child)?;
            let result = calendar_result_var(&weekday.weekday)?;
            build_calendar_int(child, date_column, vec![(result, CalendarIntFn::Weekday)])?
        }
        Query::WeekdaySundayStart(weekday) => {
            let date_column = date_operand(&weekday.date, &child)?;
            let result = calendar_result_var(&weekday.weekday)?;
            build_calendar_int(child, date_column, vec![(result, CalendarIntFn::WeekdaySundayStart)])?
        }
        Query::IsoWeek(iso) => {
            let date_column = date_operand(&iso.date, &child)?;
            // The reference binds BOTH the week-numbering year and the week number; both must be
            // fresh, distinct output variables (order [year, week] matches `definition.pl` fields).
            let year = calendar_result_var(&iso.year)?;
            let week = calendar_result_var(&iso.week)?;
            build_calendar_int(
                child,
                date_column,
                vec![
                    (year, CalendarIntFn::IsoWeekYear),
                    (week, CalendarIntFn::IsoWeekWeek),
                ],
            )?
        }
        Query::DayAfter(day) => {
            let input_column = date_operand(&day.date, &child)?;
            let result = calendar_result_var(&day.next)?;
            build_calendar_date(child, input_column, CalendarDateFn::DayAfter, result)?
        }
        Query::DayBefore(day) => {
            let input_column = date_operand(&day.date, &child)?;
            let result = calendar_result_var(&day.previous)?;
            build_calendar_date(child, input_column, CalendarDateFn::DayBefore, result)?
        }
        Query::MonthStartDate(month) => {
            let input_column = yearmonth_operand(&month.year_month, &child)?;
            let result = calendar_result_var(&month.date)?;
            build_calendar_date(child, input_column, CalendarDateFn::MonthStartDate, result)?
        }
        Query::MonthEndDate(month) => {
            let input_column = yearmonth_operand(&month.year_month, &child)?;
            let result = calendar_result_var(&month.date)?;
            build_calendar_date(child, input_column, CalendarDateFn::MonthEndDate, result)?
        }
        _ => unreachable!("is_calendar_conjunct + the reject filter cover every calendar variant"),
    };
    Ok(Some(compiled))
}


//! String-op lowering — concat / join / substring / pad / hashkey.
//!
//! Lowers the read-only string value-circuits: `plan_concat_form` (`Concatenate`),
//! `plan_join_form` (`Join`), `plan_substring_form` (`Substring`), `plan_pad_form`
//! (`Pad`), and `plan_hashkey_form` (`HashKey`). Each takes store-authenticated string
//! columns bound by a child BGP and synthesizes a terminal, revealed result the verifier
//! recomputes; unsupported shapes (runtime lists, non-string operands, locale case-ops,
//! already-bound results) reject closed.
//!
//! See [`terminus_store::proof::primer`] for the cryptographic background (§5 the committed
//! columns a verifier-recompute re-derives over, and §11 soundness-vs-fidelity).

use super::*;

/// Compile a WOQL `Concatenate` conjunct (`concat(List, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Concat`] — the FOUNDATION of the read-only string value-circuit
/// family, modelled on `plan_eval_form`.
///
/// V1 SCOPE (reject-closed everywhere else): the list must be a CONCRETE inline list whose
/// elements are xsd:string LITERALS and/or variables bound (in the SAME query) as nonnullable
/// store-authenticated node/value columns; the result must be a FRESH variable (not a literal,
/// not already bound). Non-string list elements (numbers/nodes — `interpolate_string` would
/// stringify them, a separate circuit), a runtime list variable, a pinned non-string operand
/// datatype, and a result literal/already-bound variable all reject closed.
///
/// The result variable is deliberately NOT added to the committed schema: the concatenation is a
/// TERMINAL, revealed value (a synthesized string does not encode into a committed field element
/// and feeds nothing downstream). It is exposed post-verify via [`ProvedBgp::concat_result_objects`].
pub(crate) fn plan_concat_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Concatenate(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        // No Concatenate conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple concats would need a dependency-ordered chain of derived strings; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::ConcatUnsupportedShape);
    }
    let Query::Concatenate(concat) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Concatenate conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare concat with no store binding cannot authenticate any operand variable. (A concat
        // of only literals is a constant string with no rows — out of the per-row read fragment.)
        return Err(BgpPlanError::ConcatUnsupportedShape);
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
        return Err(BgpPlanError::ConcatUnsupportedShape);
    }

    // The result must be a FRESH variable (never a literal, never a rebinding of an operand).
    let result = match &concat.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::ConcatResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::ConcatResultAlreadyBound { var: result });
    }

    // The list must be a CONCRETE inline list (a runtime list variable's elements are unknown at
    // compile time, so their string-ness/order cannot be pinned into the query — reject closed).
    let items = match &concat.list {
        ListOrVariable::List(items) => items,
        ListOrVariable::Variable(_) => return Err(BgpPlanError::ConcatListNotConcrete),
    };
    if items.is_empty() {
        // `interpolate_string([], "")` yields the empty string with no per-row derivation; there
        // is no operand to authenticate, so it is out of the per-row provable fragment.
        return Err(BgpPlanError::ConcatEmptyList);
    }
    let elements = items
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;

    // Keep the child schema/domains: the result variable is NOT a committed column (terminal,
    // revealed). Track it only inside the statement (for the query digest and result exposure).
    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Concat {
            input: Box::new(child.relation),
            elements,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Lower one `concat` list element (a `DataValue`) to a [`ConcatElement`].
///
/// A variable resolves to a nonnullable store-authenticated node/value column (its per-row
/// xsd:string-ness is re-checked against the AUTHENTICATED value at recompute time — a runtime
/// non-string row fails closed there); a pinned NON-string operand datatype rejects at compile
/// time. A string literal contributes its UTF-8 bytes. Non-string literals and nested lists
/// reject closed (they would require the stringification circuit).
pub(crate) fn compile_concat_element(
    value: &DataValue,
    child: &CompiledBgp,
) -> Result<ConcatElement, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::ConcatOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value column (a store value), not a
            // node IRI, predicate, or nullable Optional column.
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::ConcatOperandNotString {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::ConcatOperandNotString {
                    var: variable.clone(),
                });
            }
            // If a datatype IS pinned for this column (e.g. by a range comparison), it must be
            // xsd:string. An UNPINNED column (`None`) is allowed: its string-ness is enforced
            // per-row at recompute (a non-string row fails closed there). A pinned NON-string
            // datatype (a number/bool via a range filter) rejects closed here.
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::ConcatOperandNotString {
                        var: variable.clone(),
                    })
                }
            }
            Ok(ConcatElement::Column { index })
        }
        // An xsd:string literal contributes its UTF-8 content bytes verbatim.
        DataValue::Data(XSDAnySimpleType::String(literal)) => {
            Ok(ConcatElement::Literal(literal.clone().into_bytes()))
        }
        // A non-string scalar literal (number/bool/…) would be STRINGIFIED by SWI
        // `interpolate_string` (`term_to_atom`/`atom_string`); that rendering is a separate
        // circuit, so reject closed rather than guess it.
        DataValue::Data(_) => Err(BgpPlanError::ConcatLiteralNotString),
        // A nested list element is not a scalar string operand.
        DataValue::List(_) => Err(BgpPlanError::ConcatNestedList),
    }
}

/// Compile a WOQL `Join` conjunct (`join(List, Sep, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Join`] — a SEPARATOR-interposed extension of `plan_concat_form`.
///
/// V1 SCOPE (reject-closed everywhere else, mirroring concat): the list must be a CONCRETE inline
/// list of xsd:string LITERALS and/or variables bound (in the SAME query) as nonnullable
/// store-authenticated string columns; the SEPARATOR must be a string literal or such a bound
/// variable (a non-string separator would be `term_to_atom`-stringified by `interpolate_string`,
/// a separate circuit — reject); the result must be a FRESH variable. `join`'s reference mode is
/// `[ground, ground, any]` (`woql_compile.pl:2902`), matched by the concrete-list + string-sep +
/// fresh-result shape. Runtime list variable, non-string element/separator, unbound/pinned-
/// non-string operand, result literal/already-bound all reject closed.
///
/// The result variable is NOT added to the committed schema (terminal, revealed — exactly as
/// concat). Exposed post-verify via [`ProvedBgp::join_result_objects`].
pub(crate) fn plan_join_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Join(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple joins would need a dependency-ordered chain of derived strings; v1 takes one.
        return Err(BgpPlanError::JoinUnsupportedShape);
    }
    let Query::Join(join) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Join conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare join with no store binding cannot authenticate any operand variable.
        return Err(BgpPlanError::JoinUnsupportedShape);
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
        return Err(BgpPlanError::JoinUnsupportedShape);
    }

    // The result must be a FRESH variable (never a literal, never a rebinding of an operand).
    let result = match &join.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::JoinResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::JoinResultAlreadyBound { var: result });
    }

    // The list must be a CONCRETE inline list (a runtime list variable's elements/order/string-ness
    // cannot be pinned into the query — reject closed).
    let items = match &join.list {
        ListOrVariable::List(items) => items,
        ListOrVariable::Variable(_) => return Err(BgpPlanError::JoinListNotConcrete),
    };
    if items.is_empty() {
        // `join([], Sep, "")` yields the empty string with no per-row derivation.
        return Err(BgpPlanError::JoinEmptyList);
    }
    // Elements and separator reuse concat-element lowering (string literal or authenticated string
    // column; non-string/unbound/pinned-non-string/nested-list all reject closed there). The
    // concat-element errors surfaced here are still accurate for join operands.
    let elements = items
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;
    let separator = compile_concat_element(&join.separator, &child)?;

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Join {
            input: Box::new(child.relation),
            elements,
            separator,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Extract a non-negative `usize` character index from a `Substring` position `DataValue`.
///
/// Accepts only an integer LITERAL (reusing `eval_integer_literal`'s exact-integer decode) that
/// is `>= 0`; a variable index (a store-authenticated per-row index — a heavier circuit) is a
/// documented follow-up and rejects closed, as does a non-integer literal and a NEGATIVE literal
/// (SWI `sub_string/5` requires non-negative offsets — a negative one fails/throws, so FAIL
/// CLOSED, matching the reference).
pub(crate) fn substring_index_literal(value: &DataValue) -> Result<usize, BgpPlanError> {
    let raw = match value {
        DataValue::Data(data) => {
            eval_integer_literal(data).map_err(|_| BgpPlanError::SubstringIndexNotInteger)?
        }
        DataValue::Variable(_) | DataValue::List(_) => {
            return Err(BgpPlanError::SubstringIndexNotLiteral)
        }
    };
    if raw < 0 {
        return Err(BgpPlanError::SubstringNegativeIndex);
    }
    usize::try_from(raw).map_err(|_| BgpPlanError::SubstringIndexTooLarge)
}

/// Resolve a `Substring` string operand to an authenticated child string column index. The operand
/// must be a VARIABLE bound (in the same query) as a nonnullable node/value column whose per-row
/// xsd:string-ness is enforced at recompute (an unpinned column) or pinned to xsd:string; a pinned
/// NON-string datatype, a nullable/node/predicate column, an unbound variable, or a literal all
/// reject closed (the task requires an AUTHENTICATED xsd:string input).
pub(crate) fn substring_string_column(value: &DataValue, child: &CompiledBgp) -> Result<usize, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::SubstringStringNotBound {
                    var: variable.clone(),
                })?;
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::SubstringStringNotAuthenticated {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::SubstringStringNotAuthenticated {
                    var: variable.clone(),
                });
            }
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::SubstringStringNotAuthenticated {
                        var: variable.clone(),
                    })
                }
            }
            Ok(index)
        }
        // A literal string is a constant substring (no per-row read tying it to the trusted root);
        // the task requires an AUTHENTICATED input. Reject closed.
        _ => Err(BgpPlanError::SubstringStringNotVariable),
    }
}

/// Compile a WOQL `Substring` conjunct (`sub_string(String, Before, Length, After, SubString)`)
/// into a verifier-recompute [`CompiledRelationStatement::Substring`].
///
/// V1 SCOPE (reject-closed everywhere else): `String` an authenticated child xsd:string column
/// (variable); `Before`/`Length` NON-NEGATIVE integer LITERALS (the deterministic constant-index
/// mode we prove faithful — variable indices are a follow-up); `After` a FRESH variable (an
/// output — a bound `After` adds a length-equality/occurrence filter we do not model); `SubString`
/// a FRESH variable (the terminal revealed result), distinct from `After`. Out-of-range is handled
/// at recompute (FAIL CLOSED, matching SWI `sub_string`'s failure), so only the clean per-row-map
/// case ever produces a proof (see the `Substring` variant docs for the full fidelity argument).
///
/// Neither `After` nor `SubString` is added to the committed schema (terminal, revealed — exactly
/// as concat/join). The substring is exposed post-verify via
/// [`ProvedBgp::substring_result_objects`]; `After` is derived but NOT revealed.
pub(crate) fn plan_substring_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Substring(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::SubstringUnsupportedShape);
    }
    let Query::Substring(substring) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Substring conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::SubstringUnsupportedShape);
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
        return Err(BgpPlanError::SubstringUnsupportedShape);
    }

    let string_column = substring_string_column(&substring.string, &child)?;
    let before = substring_index_literal(&substring.before)?;
    let length = substring_index_literal(&substring.length)?;

    // `After` must be a FRESH output variable (not a constant filter, not a rebinding of an
    // existing column).
    let after = match &substring.after {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::SubstringAfterNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &after) {
        return Err(BgpPlanError::SubstringResultAlreadyBound { var: after });
    }

    // `SubString` (the result) must be a FRESH variable, distinct from `After`.
    let result = match &substring.substring {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::SubstringResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::SubstringResultAlreadyBound { var: result });
    }
    if result == after {
        return Err(BgpPlanError::SubstringResultAliasesAfter);
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Substring {
            input: Box::new(child.relation),
            string_column,
            before,
            length,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Resolve a `Pad` string operand to an authenticated child string column index. Same discipline
/// as `substring_string_column`: the operand must be a VARIABLE bound (in the same query) as a
/// nonnullable node/value column whose per-row xsd:string-ness is enforced at recompute (unpinned)
/// or pinned to xsd:string; a pinned NON-string datatype, a nullable/node column, an unbound
/// variable, or a literal all reject closed (the task requires an AUTHENTICATED xsd:string input).
pub(crate) fn pad_string_column(value: &DataValue, child: &CompiledBgp) -> Result<usize, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::PadStringNotBound {
                    var: variable.clone(),
                })?;
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::PadStringNotAuthenticated {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::PadStringNotAuthenticated {
                    var: variable.clone(),
                });
            }
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::PadStringNotAuthenticated {
                        var: variable.clone(),
                    })
                }
            }
            Ok(index)
        }
        // A literal string is a constant with no per-row read tying it to the trusted root; the
        // task requires an AUTHENTICATED input. Reject closed.
        _ => Err(BgpPlanError::PadStringNotVariable),
    }
}

/// Resolve a `Pad` pad-char operand to its UTF-8 bytes. Reference `utils:pad` requires the char to
/// interpolate to a string of `string_length == 1` (exactly ONE code point) else it throws
/// `pad_not_length_one`. A variable char is a deferred follow-up; a non-string is unsupported.
pub(crate) fn pad_char_literal(value: &DataValue) -> Result<Vec<u8>, BgpPlanError> {
    match value {
        DataValue::Data(XSDAnySimpleType::String(literal)) => {
            // CODE-POINT count, not byte length: a multi-BYTE single char (e.g. 'é') is length 1.
            if literal.chars().count() != 1 {
                return Err(BgpPlanError::PadCharNotSingleChar);
            }
            Ok(literal.clone().into_bytes())
        }
        DataValue::Data(_) => Err(BgpPlanError::PadCharNotString),
        DataValue::List(_) => Err(BgpPlanError::PadCharNotString),
        DataValue::Variable(_) => Err(BgpPlanError::PadCharNotLiteral),
    }
}

/// Resolve a `Pad` target-length operand to a non-negative `usize`. A negative `L` always throws
/// `bad_pad_length` in the reference (`L2 = L - L1 < 0` for any `L1 >= 0`), so it rejects closed. A
/// variable target length is a deferred follow-up.
pub(crate) fn pad_target_length(value: &DataValue) -> Result<usize, BgpPlanError> {
    let raw = match value {
        DataValue::Data(data) => {
            eval_integer_literal(data).map_err(|_| BgpPlanError::PadLengthNotInteger)?
        }
        DataValue::Variable(_) | DataValue::List(_) => {
            return Err(BgpPlanError::PadLengthNotLiteral)
        }
    };
    if raw < 0 {
        return Err(BgpPlanError::PadNegativeLength);
    }
    usize::try_from(raw).map_err(|_| BgpPlanError::PadLengthTooLarge)
}

/// Compile a WOQL `Pad` conjunct (`pad(String, Char, Times, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Pad`].
///
/// V1 SCOPE (reject-closed everywhere else): `String` an authenticated child xsd:string column
/// (variable); `Char` a single-code-point xsd:string LITERAL; `Times` (semantically the TARGET
/// total CHARACTER length `L`) a NON-NEGATIVE integer LITERAL (a variable char/length is a
/// follow-up); `Result` a FRESH variable (the terminal revealed result). Operand-longer-than-target
/// is handled at recompute (FAIL CLOSED, matching `utils:pad`'s `bad_pad_length` throw), so only
/// the clean per-row-map case ever produces a proof. The result is NOT added to the committed
/// schema (terminal, revealed — exactly as concat/substring); it is exposed post-verify via
/// [`ProvedBgp::pad_result_objects`].
pub(crate) fn plan_pad_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Pad(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::PadUnsupportedShape);
    }
    let Query::Pad(pad) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Pad conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::PadUnsupportedShape);
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
        return Err(BgpPlanError::PadUnsupportedShape);
    }

    let string_column = pad_string_column(&pad.string, &child)?;
    let pad_char = pad_char_literal(&pad.char)?;
    let target_len = pad_target_length(&pad.times)?;

    let result = match &pad.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::PadResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::PadResultAlreadyBound { var: result });
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Pad {
            input: Box::new(child.relation),
            string_column,
            pad_char,
            target_len,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile a WOQL `HashKey`/`hash` conjunct (`hash(Base, KeyList, Uri)`) into a verifier-recompute
/// [`CompiledRelationStatement::HashKey`].
///
/// V1 SCOPE (reject-closed everywhere else): `Base` an authenticated child xsd:string column or an
/// xsd:string LITERAL; each `KeyList` element likewise (authenticated column or string literal —
/// reusing the `compile_concat_element` auth, so non-string/unbound/nested-list elements reject
/// closed with the shared `Concat*` operand errors); `Uri` a FRESH variable (the terminal revealed
/// result). `KeyList` is ALWAYS a concrete `Vec<DataValue>` in the AST (there is no runtime list
/// variable to reject). The result is NOT added to the committed schema (terminal, revealed); it is
/// exposed post-verify via [`ProvedBgp::hashkey_result_objects`] as a `CanonicalObject::Node`.
pub(crate) fn plan_hashkey_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::HashKey(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::HashKeyUnsupportedShape);
    }
    let Query::HashKey(hash) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes HashKey conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::HashKeyUnsupportedShape);
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
        return Err(BgpPlanError::HashKeyUnsupportedShape);
    }

    // Base + args authenticated via the reusable ConcatElement lowering (Concat* operand errors on
    // an unbound/non-string/nested-list operand). Base is later RAW-prepended; args are encoded.
    let base = compile_concat_element(&hash.base, &child)?;
    let args = hash
        .key_list
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;

    // Result URI must be a FRESH variable (a constant node result would add an equality constraint).
    let result = match &hash.uri {
        NodeValue::Variable(variable) => variable.clone(),
        NodeValue::Node(_) => return Err(BgpPlanError::HashKeyResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::HashKeyResultAlreadyBound { var: result });
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::HashKey {
            input: Box::new(child.relation),
            base,
            args,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Reject-closed for the DEFERRED string-case operators `Upper`/`Lower`.
///
/// SEMANTIC-FIDELITY DEFERRAL (soundness-first): WOQL `upper`/`lower` compile to SWI-Prolog
/// `string_upper`/`string_lower`, which fold case per code point via C `towupper`/`towlower` —
/// LOCALE-DEPENDENT (`LC_CTYPE`). TerminusDB's canonical Docker deployment pins NO locale
/// (no `LANG`/`LC_ALL`/`LC_CTYPE` in the Dockerfiles or `start.pl`; SWI inherits the environment
/// locale), so under a Turkic `LC_CTYPE` `string_upper("i")` yields "İ" (U+0130) and
/// `string_lower("I")` yields "ı" (U+0131) — a NON-ASCII result diverging from a naive ASCII
/// a–z↔A–Z map EVEN ON ASCII INPUT 'i'/'I'. A "reject non-ASCII INPUT" guard therefore does NOT
/// establish fidelity. We cannot prove agreement with Rust ASCII casing across deployments, so
/// we REJECT CLOSED here and defer upper/lower to a follow-up that pins the runtime locale (e.g.
/// `LC_ALL=C`) or proves the case fold directly. (`concat`/`interpolate_string` is unaffected:
/// it is pure `string_concat`, locale-independent — and IS shipped.)
pub(crate) fn reject_unsupported_string_case_ops(query: &Query) -> Result<(), BgpPlanError> {
    fn is_case_op(query: &Query) -> bool {
        matches!(query, Query::Upper(_) | Query::Lower(_))
    }
    let present = match query {
        Query::And(and) => and.and.iter().any(is_case_op),
        other => is_case_op(other),
    };
    if present {
        return Err(BgpPlanError::StringCaseOpLocaleUnsupported);
    }
    Ok(())
}


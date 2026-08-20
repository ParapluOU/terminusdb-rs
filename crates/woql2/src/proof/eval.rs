//! Eval / value-circuit lowering — arithmetic and typecasts.
//!
//! Lowers a WOQL `Eval` over integer arithmetic ([`plan_eval_form`]) and the exact decimal /
//! rational value layer (`Divide`, `Floor`, decimal operands) into a per-row derived-column
//! relation, and lowers `Typecast` ([`plan_typecast_form`]) between numeric and string types.
//! Each is proven by verifier-recompute: the operand columns are authenticated by a child BGP
//! and the verifier re-derives the result column. Anything outside the provable fragment
//! (operands not store-bound, unsupported casts, overflow) is rejected closed.

use super::*;

/// Compile a WOQL `Eval` over integer arithmetic into a per-row derived-column relation.
///
/// Shape: an `And` whose conjuncts are an operand-binding child BGP plus EXACTLY ONE `Eval`.
/// The child is compiled with [`plan_bgp`] (so its triples + range comparisons authenticate
/// and datatype-pin the operand columns exactly as the Sum forms rely on), and the `Eval`
/// expression is lowered to an [`EvalExpr`] over child column indices and integer literals.
///
/// Everything outside the provable integer fragment rejects closed with a specific error:
/// `Divide`/`Floor` (rational layer), a static negative `Exp` exponent, non-integer
/// literals/operands, operands not bound by a store read, and result overflow (enforced later,
/// in the recompute's checked i64 narrowing). `Div`/`Exp` (non-negative exponent) ARE provable.
pub(crate) fn plan_eval_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let eval_positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Eval(_)))
        .map(|(index, _)| index)
        .collect();
    if eval_positions.is_empty() {
        // No Eval conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if eval_positions.len() > 1 {
        // Multiple Evals would need a dependency-ordered chain of derived columns; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::EvalUnsupportedShape);
    }
    let Query::Eval(eval) = &conjuncts[eval_positions[0]] else {
        unreachable!("eval_positions only indexes Eval conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != eval_positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare Eval with no store binding cannot authenticate any operand variable.
        return Err(BgpPlanError::EvalUnsupportedShape);
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
        return Err(BgpPlanError::EvalUnsupportedShape);
    }

    let result = match &eval.result_value {
        ArithmeticValue::Variable(variable) => variable.clone(),
        ArithmeticValue::Data(_) => return Err(BgpPlanError::EvalResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::EvalResultAlreadyBound { var: result });
    }

    // ROUTING. Pure-INTEGER arithmetic (pinned fixed-width integer operands, only
    // Plus/Minus/Times/Div/Exp) keeps the existing integer `Eval` delivery (a committed
    // `GeneratedDecimalI64` column). Anything with a RATIONAL SIGNAL — `Divide` (exact rational),
    // `Floor`, or an `xsd:decimal` literal — uses the exact-rational value layer: a TOP-LEVEL
    // `Floor` still delivers an integer (`GeneratedDecimalI64`), everything else delivers a
    // TERMINAL, REVEALED `xsd:decimal` value (like `Concat`). Decimal OPERAND columns without such
    // a signal are indistinguishable from unpinned integers at compile time, so they remain
    // reject-closed (via the integer path) rather than guessed — a decimal operand is provable only
    // when the tree ALSO carries a `Divide`/`Floor`/decimal-literal that routes it here.
    let collection = child.collection.clone();
    if !arith_signals_rational(&eval.expression) {
        // ---- Integer `Eval` (unchanged) ----
        let expr = compile_eval_expr(&eval.expression, &child)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        // The derived column is a generated integer, not an executor value domain.
        domains.push(None);
        let relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Eval {
                input: Box::new(child.relation),
                expr,
            },
        };
        return Ok(Some(CompiledBgp {
            relation,
            projection: None,
            collection,
            count_variable: None,
            full_row_distinct: false,
            order_invented_tiebreak: false,
        }));
    }

    // ---- Exact-rational value layer ----
    let relation = if let ArithmeticExpression::Floor(floor) = &eval.expression {
        // TOP-LEVEL `floor(x)` -> INTEGER delivery (append a `GeneratedDecimalI64` column, exactly
        // like `Eval`). The floor is applied by `expected_floor_eval_columns`; store its ARGUMENT.
        let expr = compile_decimal_expr(&floor.argument, &child)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        domains.push(None);
        CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::FloorEval {
                input: Box::new(child.relation),
                expr,
            },
        }
    } else {
        // Fractional-capable result -> TERMINAL, REVEALED `xsd:decimal` (like `Concat`): the child
        // schema/domains pass through UNCHANGED and the result variable lives only in the statement.
        let expr = compile_decimal_expr(&eval.expression, &child)?;
        CompiledRelation {
            schema: child.relation.schema.clone(),
            domains: child.relation.domains.clone(),
            statement: CompiledRelationStatement::RationalArith {
                input: Box::new(child.relation),
                expr,
                result,
            },
        }
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
        order_invented_tiebreak: false,
    }))
}

/// Does this arithmetic tree carry a RATIONAL SIGNAL that routes it to the exact-rational value
/// layer? True iff it contains a `Divide` (`rdiv` — always fractional-capable), a `Floor`, or an
/// `xsd:decimal` literal. `Div` (integer floor division) is NOT itself a signal (it stays on the
/// integer path unless a nested `Divide`/`Floor`/decimal forces routing, in which case
/// [`compile_decimal_expr`] rejects the integer-only `Div`). A bare integer/variable leaf is not a
/// signal — so an UNPINNED store operand keeps its existing reject-closed behavior on the integer
/// path rather than being silently reinterpreted as a decimal.
pub(crate) fn arith_signals_rational(expression: &ArithmeticExpression) -> bool {
    match expression {
        ArithmeticExpression::Value(ArithmeticValue::Data(XSDAnySimpleType::Decimal(_))) => true,
        ArithmeticExpression::Value(_) => false,
        ArithmeticExpression::Divide(_) => true,
        ArithmeticExpression::Floor(_) => true,
        ArithmeticExpression::Plus(plus) => {
            arith_signals_rational(&plus.left) || arith_signals_rational(&plus.right)
        }
        ArithmeticExpression::Minus(minus) => {
            arith_signals_rational(&minus.left) || arith_signals_rational(&minus.right)
        }
        ArithmeticExpression::Times(times) => {
            arith_signals_rational(&times.left) || arith_signals_rational(&times.right)
        }
        ArithmeticExpression::Div(div) => {
            arith_signals_rational(&div.left) || arith_signals_rational(&div.right)
        }
        ArithmeticExpression::Exp(exp) => {
            arith_signals_rational(&exp.left) || arith_signals_rational(&exp.right)
        }
    }
}

/// Lower a WOQL `ArithmeticExpression` to a [`DecimalExpr`] (exact-rational layer), rejecting every
/// out-of-scope node. Integer-only `Div` is rejected here (it belongs to the integer `Eval` path);
/// `Exp` requires a static non-negative integer exponent; a static zero `Divide` divisor rejects
/// closed at compile time (a runtime zero divisor fails closed in the recompute).
pub(crate) fn compile_decimal_expr(
    expression: &ArithmeticExpression,
    child: &CompiledBgp,
) -> Result<DecimalExpr, BgpPlanError> {
    match expression {
        ArithmeticExpression::Value(value) => compile_decimal_value(value, child),
        ArithmeticExpression::Plus(plus) => Ok(DecimalExpr::Plus(
            Box::new(compile_decimal_expr(&plus.left, child)?),
            Box::new(compile_decimal_expr(&plus.right, child)?),
        )),
        ArithmeticExpression::Minus(minus) => Ok(DecimalExpr::Minus(
            Box::new(compile_decimal_expr(&minus.left, child)?),
            Box::new(compile_decimal_expr(&minus.right, child)?),
        )),
        ArithmeticExpression::Times(times) => Ok(DecimalExpr::Times(
            Box::new(compile_decimal_expr(&times.left, child)?),
            Box::new(compile_decimal_expr(&times.right, child)?),
        )),
        ArithmeticExpression::Divide(divide) => {
            // A STATIC zero divisor is `evaluation_error(zero_divisor)` in Prolog: reject at compile
            // time (a runtime/column zero divisor fails closed in the recompute instead).
            if arith_is_static_zero(&divide.right) {
                return Err(BgpPlanError::EvalDivideByZero);
            }
            Ok(DecimalExpr::Divide(
                Box::new(compile_decimal_expr(&divide.left, child)?),
                Box::new(compile_decimal_expr(&divide.right, child)?),
            ))
        }
        ArithmeticExpression::Exp(exp) => {
            // Only a STATIC non-negative integer exponent gives an exact rational power here.
            let exponent = match exp.right.as_ref() {
                ArithmeticExpression::Value(ArithmeticValue::Data(literal)) => {
                    eval_integer_literal(literal)?
                }
                _ => {
                    return Err(BgpPlanError::EvalUnsupportedOperator {
                        operator: "Exp (non-static exponent in the decimal layer)",
                    })
                }
            };
            if exponent < 0 {
                return Err(BgpPlanError::EvalUnsupportedOperator {
                    operator: "Exp (negative exponent)",
                });
            }
            Ok(DecimalExpr::Exp(
                Box::new(compile_decimal_expr(&exp.left, child)?),
                Box::new(DecimalExpr::IntegerLiteral(exponent)),
            ))
        }
        ArithmeticExpression::Floor(floor) => Ok(DecimalExpr::Floor(Box::new(compile_decimal_expr(
            &floor.argument,
            child,
        )?))),
        // Integer FLOOR division (`div`) is not part of the exact-rational layer (its operands must
        // be integers, and its result is an integer already handled by the integer `Eval` path). If
        // it appears alongside a rational signal, the mixed form is not provable: reject closed.
        ArithmeticExpression::Div(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Div (integer floor division mixed with rational arithmetic)",
        }),
    }
}

/// Lower an arithmetic LEAF (numeric operand variable or integer/decimal literal) to a
/// [`DecimalExpr`] leaf for the exact-rational layer. An operand VARIABLE must be a nonnullable
/// node/value store column (its exact numeric datatype is resolved at recompute — see
/// [`DecimalExpr::Column`]); a non-numeric literal (float/string/boolean/…) rejects closed.
pub(crate) fn compile_decimal_value(
    value: &ArithmeticValue,
    child: &CompiledBgp,
) -> Result<DecimalExpr, BgpPlanError> {
    match value {
        ArithmeticValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::EvalOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value STORE column (never a node IRI,
            // predicate, or nullable Optional column). Its exact datatype (integer family or
            // xsd:decimal) is NOT pinned at compile time — it is read from the ROOT-authenticated
            // dictionary value at recompute, where a non-numeric row rejects closed.
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::EvalOperandNotNumeric {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::EvalOperandNotNumeric {
                    var: variable.clone(),
                });
            }
            Ok(DecimalExpr::Column { index })
        }
        ArithmeticValue::Data(data) => match data {
            // Exact integer literals (all fit i128) — interpreted as `value/1`.
            XSDAnySimpleType::UnsignedByte(_)
            | XSDAnySimpleType::Byte(_)
            | XSDAnySimpleType::UnsignedShort(_)
            | XSDAnySimpleType::Short(_)
            | XSDAnySimpleType::UnsignedInt(_)
            | XSDAnySimpleType::Int(_)
            | XSDAnySimpleType::UnsignedLong(_)
            | XSDAnySimpleType::Long(_)
            | XSDAnySimpleType::Integer(_) => {
                Ok(DecimalExpr::IntegerLiteral(eval_integer_literal(data)?))
            }
            // Exact xsd:decimal literal — canonicalize to its decimal string and validate it parses
            // to an exact rational (rejecting scientific/garbage forms) so the recompute re-parses
            // it identically.
            XSDAnySimpleType::Decimal(decimal) => {
                let lexical = decimal.to_string();
                parse_decimal_to_rational(&lexical).map_err(|_| {
                    BgpPlanError::EvalUnsupportedLiteral {
                        datatype: "xsd:decimal (non-canonical)",
                    }
                })?;
                Ok(DecimalExpr::DecimalLiteral(lexical))
            }
            other => Err(BgpPlanError::EvalUnsupportedLiteral {
                datatype: xsd_simple_type_name(other),
            }),
        },
    }
}

/// Is this arithmetic node a STATIC numeric literal equal to zero (an integer `0` or a decimal
/// whose value is zero)? Used to reject a compile-time `Divide` by a zero constant.
pub(crate) fn arith_is_static_zero(expression: &ArithmeticExpression) -> bool {
    let ArithmeticExpression::Value(ArithmeticValue::Data(data)) = expression else {
        return false;
    };
    match data {
        XSDAnySimpleType::UnsignedByte(v) => *v == 0,
        XSDAnySimpleType::Byte(v) => *v == 0,
        XSDAnySimpleType::UnsignedShort(v) => *v == 0,
        XSDAnySimpleType::Short(v) => *v == 0,
        XSDAnySimpleType::UnsignedInt(v) => *v == 0,
        XSDAnySimpleType::Int(v) => *v == 0,
        XSDAnySimpleType::UnsignedLong(v) => *v == 0,
        XSDAnySimpleType::Long(v) => *v == 0,
        XSDAnySimpleType::Integer(v) => *v == 0,
        XSDAnySimpleType::Decimal(d) => parse_decimal_to_rational(&d.to_string())
            .map(|r| r.numer().is_zero())
            .unwrap_or(false),
        _ => false,
    }
}

/// Lower a WOQL `ArithmeticExpression` to an [`EvalExpr`], rejecting every out-of-scope node.
pub(crate) fn compile_eval_expr(
    expression: &ArithmeticExpression,
    child: &CompiledBgp,
) -> Result<EvalExpr, BgpPlanError> {
    match expression {
        ArithmeticExpression::Value(value) => compile_eval_value(value, child),
        ArithmeticExpression::Plus(plus) => Ok(EvalExpr::Plus(
            Box::new(compile_eval_expr(&plus.left, child)?),
            Box::new(compile_eval_expr(&plus.right, child)?),
        )),
        ArithmeticExpression::Minus(minus) => Ok(EvalExpr::Minus(
            Box::new(compile_eval_expr(&minus.left, child)?),
            Box::new(compile_eval_expr(&minus.right, child)?),
        )),
        ArithmeticExpression::Times(times) => Ok(EvalExpr::Times(
            Box::new(compile_eval_expr(&times.left, child)?),
            Box::new(compile_eval_expr(&times.right, child)?),
        )),
        // INTEGER FLOOR division: recompute-provable (verifier re-derives the floored quotient,
        // fails closed on a zero divisor — see [`EvalExpr::Div`]).
        ArithmeticExpression::Div(div) => Ok(EvalExpr::Div(
            Box::new(compile_eval_expr(&div.left, child)?),
            Box::new(compile_eval_expr(&div.right, child)?),
        )),
        // INTEGER power over a non-negative exponent: recompute-provable with checked pow.
        // If the exponent is a STATIC negative literal, reject closed at compile time (its
        // rational result is out of this integer layer); a runtime column exponent that turns
        // out negative fails closed in the recompute instead.
        ArithmeticExpression::Exp(exp) => {
            if let ArithmeticExpression::Value(ArithmeticValue::Data(literal)) = exp.right.as_ref() {
                if eval_integer_literal(literal)? < 0 {
                    return Err(BgpPlanError::EvalUnsupportedOperator {
                        operator: "Exp (negative exponent)",
                    });
                }
            }
            Ok(EvalExpr::Exp(
                Box::new(compile_eval_expr(&exp.left, child)?),
                Box::new(compile_eval_expr(&exp.right, child)?),
            ))
        }
        // `Divide` (`/`/`rdiv`) is EXACT RATIONAL, and `Floor` over rationals needs the
        // rational value layer; both are out of the integer fragment. Reject closed.
        ArithmeticExpression::Divide(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Divide",
        }),
        ArithmeticExpression::Floor(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Floor",
        }),
    }
}

/// Lower an arithmetic LEAF (variable operand or literal) to an [`EvalExpr`] leaf.
pub(crate) fn compile_eval_value(
    value: &ArithmeticValue,
    child: &CompiledBgp,
) -> Result<EvalExpr, BgpPlanError> {
    match value {
        ArithmeticValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::EvalOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value column (a store value), not a
            // node IRI, predicate, or nullable Optional column.
            let descriptors = relation_result_descriptors(&child.relation)
                .map_err(|_| BgpPlanError::EvalOperandNotInteger {
                    var: variable.clone(),
                })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::EvalOperandNotInteger {
                    var: variable.clone(),
                });
            }
            // Its exact fixed-width integer datatype must be pinned (by a range comparison, as
            // in the Sum forms). Anything else — floats, decimals, unpinned values — rejects.
            let datatype = match child.relation.domains.get(index).copied().flatten() {
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
                _ => {
                    return Err(BgpPlanError::EvalOperandNotInteger {
                        var: variable.clone(),
                    })
                }
            };
            Ok(EvalExpr::Column { index, datatype })
        }
        ArithmeticValue::Data(data) => Ok(EvalExpr::Literal(eval_integer_literal(data)?)),
    }
}

/// Accept only exact integer XSD literals (all fit `i128`); reject float/decimal/other closed.
pub(crate) fn eval_integer_literal(value: &XSDAnySimpleType) -> Result<i128, BgpPlanError> {
    Ok(match value {
        XSDAnySimpleType::UnsignedByte(v) => i128::from(*v),
        XSDAnySimpleType::Byte(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedShort(v) => i128::from(*v),
        XSDAnySimpleType::Short(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedInt(v) => i128::try_from(*v).map_err(|_| {
            BgpPlanError::EvalUnsupportedLiteral {
                datatype: "xsd:unsignedInt",
            }
        })?,
        XSDAnySimpleType::Int(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedLong(v) => i128::from(*v),
        XSDAnySimpleType::Long(v) => i128::from(*v),
        XSDAnySimpleType::Integer(v) => i128::from(*v),
        other => {
            return Err(BgpPlanError::EvalUnsupportedLiteral {
                datatype: xsd_simple_type_name(other),
            })
        }
    })
}

/// A stable XSD datatype name for a non-integer literal, used only in reject-closed errors.
pub(crate) fn xsd_simple_type_name(value: &XSDAnySimpleType) -> &'static str {
    match value {
        XSDAnySimpleType::String(_) => "xsd:string",
        XSDAnySimpleType::Decimal(_) => "xsd:decimal",
        XSDAnySimpleType::Float(_) => "xsd:double",
        XSDAnySimpleType::Boolean(_) => "xsd:boolean",
        XSDAnySimpleType::HexBinary(_) => "xsd:hexBinary",
        XSDAnySimpleType::URI(_) => "xsd:anyURI",
        XSDAnySimpleType::DateTime(_) => "xsd:dateTime",
        XSDAnySimpleType::Date(_) => "xsd:date",
        XSDAnySimpleType::Time(_) => "xsd:time",
        XSDAnySimpleType::UnsignedByte(_) => "xsd:unsignedByte",
        XSDAnySimpleType::Byte(_) => "xsd:byte",
        XSDAnySimpleType::UnsignedShort(_) => "xsd:unsignedShort",
        XSDAnySimpleType::Short(_) => "xsd:short",
        XSDAnySimpleType::UnsignedInt(_) => "xsd:unsignedInt",
        XSDAnySimpleType::Int(_) => "xsd:int",
        XSDAnySimpleType::UnsignedLong(_) => "xsd:unsignedLong",
        XSDAnySimpleType::Long(_) => "xsd:long",
        XSDAnySimpleType::Integer(_) => "xsd:integer",
    }
}

/// Compile a WOQL `Typecast` of ONE authenticated source column into a per-row cast relation.
///
/// Shape: an `And` whose conjuncts are a source-binding child BGP plus EXACTLY ONE `Typecast`.
/// The child is compiled with [`plan_bgp`] (so its triples + range comparisons authenticate and
/// datatype-pin the source column exactly as the Eval/Sum forms rely on). Provable cast families,
/// dispatched on the TARGET type:
///   * fixed-width integer → fixed-width integer (value-preserving; appends a COMMITTED derived
///     integer column) — the source must be a pinned fixed-width integer;
///   * number → `xsd:string` (integer via `~w`, xsd:decimal via `rational_to_decimal_string_20`),
///     and `xsd:string` → `xsd:integer` / `xsd:decimal` (exact reference-grammar parse) — delivered
///     as TERMINAL, revealed values (like `Concat`), the source a nonnullable store value column.
/// Every other case (float/double↔string, scientific/`+`/malformed string parses, a non-numeric /
/// non-string source, `xsd:string`→fixed-width, a variable target, an unbound source, a literal or
/// already-bound result) rejects closed with a specific error, or fails closed at recompute.
pub(crate) fn plan_typecast_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let typecast_positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Typecast(_)))
        .map(|(index, _)| index)
        .collect();
    if typecast_positions.is_empty() {
        // No Typecast conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if typecast_positions.len() > 1 {
        // Multiple casts would need a dependency-ordered chain of derived columns; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::TypecastUnsupportedShape);
    }
    let Query::Typecast(typecast) = &conjuncts[typecast_positions[0]] else {
        unreachable!("typecast_positions only indexes Typecast conjuncts")
    };

    // The source-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != typecast_positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare Typecast with no store binding cannot authenticate the source variable.
        return Err(BgpPlanError::TypecastUnsupportedShape);
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
        return Err(BgpPlanError::TypecastUnsupportedShape);
    }

    // Source: a variable (its authentication contract differs per cast kind, resolved below).
    let source_variable = match &typecast.value {
        Value::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TypecastSourceNotVariable),
    };

    // Target: a concrete node IRI (never a variable).
    let target_iri = match &typecast.type_uri {
        NodeValue::Node(iri) => iri.clone(),
        NodeValue::Variable(_) => return Err(BgpPlanError::TypecastTargetNotNode),
    };

    // Result: a fresh output variable, never a literal and never a redefinition of a binding.
    let result = match &typecast.result_value {
        Value::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TypecastResultInvalid),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::TypecastResultInvalid);
    }

    let collection = child.collection.clone();

    // A fixed-width INTEGER target is the value-PRESERVING integer→integer cast: it appends a
    // COMMITTED derived integer column, and the source must be a pinned fixed-width integer.
    if let Some(target_datatype) = canonical_integer_datatype_from_iri(&target_iri) {
        let (source_column, source_datatype) = resolve_typecast_source(&child, &source_variable)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        // The derived cast column is a generated integer, not an executor value domain.
        domains.push(None);
        let relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Typecast {
                input: Box::new(child.relation),
                source_column,
                source_datatype,
                target_datatype,
            },
        };
        return Ok(Some(CompiledBgp {
            relation,
            projection: None,
            collection,
            count_variable: None,
            full_row_distinct: false,
            order_invented_tiebreak: false,
        }));
    }

    // Otherwise: a NUMBER↔STRING cast, dispatched on the target's XSD local name. The source must be
    // a NONNULLABLE node/value store column; its exact datatype (numeric for number→string,
    // xsd:string for string→number) is re-checked against the ROOT-authenticated value at recompute,
    // so a wrong-typed row FAILS CLOSED there. Unlike the integer→integer path, the result is a
    // TERMINAL, revealed value (child schema/domains pass through verbatim; the result is NOT a
    // committed column) — exactly like `plan_concat_form`.
    let cast = cast_string_target_from_iri(&target_iri)
        .ok_or(BgpPlanError::TypecastUnsupportedTarget { iri: target_iri })?;
    let source_kind = match cast {
        CastStringTarget::ToString => ValueSourceKind::Numeric,
        CastStringTarget::FromStringInteger | CastStringTarget::FromStringDecimal => {
            ValueSourceKind::Stringy
        }
    };
    let source_column = resolve_authenticated_value_source(&child, &source_variable, source_kind)?;
    let schema = child.relation.schema.clone();
    let domains = child.relation.domains.clone();
    let statement = match cast {
        // number → xsd:string
        CastStringTarget::ToString => CompiledRelationStatement::TypecastNumberToString {
            input: Box::new(child.relation),
            source_column,
            result,
        },
        // xsd:string → xsd:integer
        CastStringTarget::FromStringInteger => CompiledRelationStatement::TypecastStringToNumber {
            input: Box::new(child.relation),
            source_column,
            target: StringNumericTarget::Integer,
            result,
        },
        // xsd:string → xsd:decimal
        CastStringTarget::FromStringDecimal => CompiledRelationStatement::TypecastStringToNumber {
            input: Box::new(child.relation),
            source_column,
            target: StringNumericTarget::Decimal,
            result,
        },
    };
    let relation = CompiledRelation {
        schema,
        domains,
        statement,
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
        order_invented_tiebreak: false,
    }))
}

/// Which NUMBER↔STRING cast a `Typecast` target IRI selects, recognized ONLY in the XSD namespace
/// (the full `http://www.w3.org/2001/XMLSchema#<local>` IRI or the standard `xsd:<local>` prefix
/// form — see [`xsd_local_name`]), exactly like [`canonical_integer_datatype_from_iri`]. Disjoint
/// from the fixed-width integer set that function recognizes: `integer`/`decimal`/`string` are never
/// fixed-width integers.
pub(crate) enum CastStringTarget {
    /// target `xsd:string` — a NUMBER→STRING cast.
    ToString,
    /// target `xsd:integer` (arbitrary precision) — a STRING→INTEGER cast.
    FromStringInteger,
    /// target `xsd:decimal` — a STRING→DECIMAL cast.
    FromStringDecimal,
}

/// The XSD namespace and its standard `xsd:` prefix — the ONLY forms a `Typecast` target datatype
/// is accepted in.
const XSD_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema#";
const XSD_PREFIX: &str = "xsd:";

/// Return the XSD *local name* IFF `iri` names a datatype in the XSD namespace — the full
/// `http://www.w3.org/2001/XMLSchema#<local>` IRI or the standard `xsd:<local>` prefix form — else
/// `None`.
///
/// This mirrors the reference `casting.pl` `typecast/4`, which prefix-expands the target through the
/// default prefix env — where ONLY `xsd` maps to this namespace (`global_prefixes(xsd,
/// 'http://www.w3.org/2001/XMLSchema#')`; `xdd` and everything else map elsewhere) — and then
/// requires an EXACT `base_type/1` match, raising `unknown_type_casting_error` otherwise. A bare
/// local name (no namespace) and ANY foreign namespace never expand into XSD, so they reject.
///
/// It replaces the previous namespace-blind `rsplit(['#', ':'])` local-name extraction, which
/// wrongly accepted an arbitrary-namespace IRI such as `https://attacker.example/types#long` as
/// `xsd:long` (F3).
fn xsd_local_name(iri: &str) -> Option<&str> {
    iri.strip_prefix(XSD_NAMESPACE)
        .or_else(|| iri.strip_prefix(XSD_PREFIX))
}

pub(crate) fn cast_string_target_from_iri(iri: &str) -> Option<CastStringTarget> {
    let local = xsd_local_name(iri)?;
    Some(match local {
        "string" => CastStringTarget::ToString,
        "integer" => CastStringTarget::FromStringInteger,
        "decimal" => CastStringTarget::FromStringDecimal,
        _ => return None,
    })
}

/// The kind of source a NUMBER↔STRING `Typecast` expects, used to reject a PINNED-incompatible
/// source datatype at compile (an UNPINNED source is always allowed — its type is re-checked at
/// recompute, exactly like `compile_concat_element`).
#[derive(Clone, Copy)]
pub(crate) enum ValueSourceKind {
    /// number→string: the source must be numeric (integer family or `xsd:decimal`).
    Numeric,
    /// string→number: the source must be `xsd:string`.
    Stringy,
}

/// Resolve a NUMBER↔STRING `Typecast` source variable to its child result-column index, requiring a
/// NONNULLABLE node/value store column (a store value). Its exact per-row datatype is re-checked
/// against the ROOT-authenticated value at recompute (numeric for number→string, xsd:string for
/// string→number), so a wrong-typed row FAILS CLOSED there. A node IRI / predicate / nullable /
/// generated column rejects here (namespace/nullable check); an unbound variable rejects here; and —
/// mirroring `compile_concat_element` — a PINNED datatype INCOMPATIBLE with `kind` (e.g. a range
/// comparison pinned the column to a fixed-width integer but the cast is string→number) rejects
/// closed here, while an UNPINNED (`None`) column is allowed.
pub(crate) fn resolve_authenticated_value_source(
    child: &CompiledBgp,
    variable: &str,
    kind: ValueSourceKind,
) -> Result<usize, BgpPlanError> {
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::TypecastSourceNotBound {
            var: variable.to_string(),
        })?;
    let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
        BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        }
    })?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        });
    }
    // Pinned-datatype compatibility (like Concat): `None` (unpinned) is always allowed; a pinned
    // datatype must match the cast direction, else reject closed.
    let compatible = match (kind, child.relation.domains.get(index).copied().flatten()) {
        (_, None) => true,
        (ValueSourceKind::Stringy, Some(EqualityValueDomain::Datatype(CanonicalDatatype::String))) => {
            true
        }
        (
            ValueSourceKind::Numeric,
            Some(EqualityValueDomain::Datatype(
                CanonicalDatatype::UInt8
                | CanonicalDatatype::UInt16
                | CanonicalDatatype::UInt32
                | CanonicalDatatype::UInt64
                | CanonicalDatatype::Int8
                | CanonicalDatatype::Int16
                | CanonicalDatatype::Int32
                | CanonicalDatatype::Int64
                | CanonicalDatatype::BigInt
                | CanonicalDatatype::Decimal,
            )),
        ) => true,
        (_, Some(_)) => false,
    };
    if !compatible {
        return Err(BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        });
    }
    Ok(index)
}

/// Resolve a `Typecast` source variable to its (nonnullable, fixed-width-integer) child result
/// column and pinned datatype — the same authentication contract Eval operands require.
pub(crate) fn resolve_typecast_source(
    child: &CompiledBgp,
    variable: &str,
) -> Result<(usize, CanonicalDatatype), BgpPlanError> {
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::TypecastSourceNotBound {
            var: variable.to_string(),
        })?;
    // The source column must be a nonnullable node/value column (a store value), not a node IRI,
    // predicate, or nullable Optional column.
    let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
        BgpPlanError::TypecastSourceNotInteger {
            var: variable.to_string(),
        }
    })?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(BgpPlanError::TypecastSourceNotInteger {
            var: variable.to_string(),
        });
    }
    // Its exact fixed-width integer datatype must be pinned (by a range comparison, as in the Sum
    // forms). Anything else — floats, decimals, unpinned values — rejects.
    let datatype = match child.relation.domains.get(index).copied().flatten() {
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
        _ => {
            return Err(BgpPlanError::TypecastSourceNotInteger {
                var: variable.to_string(),
            })
        }
    };
    Ok((index, datatype))
}

/// Map a target type IRI IN THE XSD NAMESPACE (the full `http://www.w3.org/2001/XMLSchema#long` IRI
/// or the standard prefixed `xsd:long` form — see [`xsd_local_name`]) to its fixed-width integer
/// [`CanonicalDatatype`], or `None` for any non-fixed-width-integer type (string/float/decimal,
/// `xsd:integer` arbitrary precision, the `positiveInteger`/`nonNegativeInteger`/… derived unbounded
/// types), a bare local name, a foreign namespace, or an unknown IRI. Reject closed on `None`: only
/// the eight order-preserving fixed-width integer widths in the XSD namespace are provable.
pub(crate) fn canonical_integer_datatype_from_iri(iri: &str) -> Option<CanonicalDatatype> {
    // Only a datatype in the XSD namespace (full IRI or `xsd:` prefix) is a candidate; a bare local
    // name or foreign namespace rejects, matching casting.pl's prefix-expand + `base_type/1` match.
    let local = xsd_local_name(iri)?;
    Some(match local {
        "byte" => CanonicalDatatype::Int8,
        "short" => CanonicalDatatype::Int16,
        "int" => CanonicalDatatype::Int32,
        "long" => CanonicalDatatype::Int64,
        "unsignedByte" => CanonicalDatatype::UInt8,
        "unsignedShort" => CanonicalDatatype::UInt16,
        "unsignedInt" => CanonicalDatatype::UInt32,
        "unsignedLong" => CanonicalDatatype::UInt64,
        _ => return None,
    })
}


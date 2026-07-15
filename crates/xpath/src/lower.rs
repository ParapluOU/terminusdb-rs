//! Lower the `xee-xpath-ast` AST onto our small [`ir`] subset.
//!
//! Anything outside the supported subset becomes [`XPathError::Unsupported`],
//! so the boundary of what we handle is always explicit (and testable).

use xee_xpath_ast::ast::{
    Axis as XAxis, BinaryOperator, Expr, ExprSingle, KindTest, Literal as XLit, NameTest, NodeTest,
    PathExpr, PrimaryExpr, StepExpr, StepExprS,
};
use xot::xmlname::NameStrInfo;

use crate::error::{Result, XPathError};
use crate::ir;

/// Lower a parsed XPath expression into an [`ir::XPathQuery`].
pub fn lower(xpath: &xee_xpath_ast::ast::XPath) -> Result<ir::XPathQuery> {
    let expr = &xpath.0.value;
    let single = single_expr(expr)?;
    let path = match deparen(single) {
        ExprSingle::Path(p) => p,
        other => {
            return Err(XPathError::unsupported(format!(
                "top-level expression must be a location path, got {}",
                expr_kind(other)
            )))
        }
    };

    // Consume any leading TerminusDB context functions: db(...), document(...).
    let mut db: Option<String> = None;
    let mut doc: Option<String> = None;
    let mut idx = 0;
    while idx < path.steps.len() {
        match as_context_function(&path.steps[idx].value)? {
            Some((name, arg)) => {
                match name.as_str() {
                    "db" => db = Some(arg),
                    "document" | "doc" => doc = Some(arg),
                    _ => unreachable!(),
                }
                idx += 1;
            }
            None => break,
        }
    }

    let head = match doc {
        Some(id) => ir::ContextHead::Document { db, id },
        None => ir::ContextHead::Root { db },
    };

    let steps = axis_steps_to_ir(&path.steps[idx..])?;
    Ok(ir::XPathQuery { head, steps })
}

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

/// Convert a run of xee step nodes into IR steps, folding the synthetic
/// `descendant-or-self::node()` step that `//` expands to into the next step.
fn axis_steps_to_ir(steps: &[StepExprS]) -> Result<Vec<ir::Step>> {
    let mut out = Vec::new();
    let mut pending_descendant = false;

    for step in steps {
        let axis_step = match &step.value {
            StepExpr::AxisStep(a) => a,
            StepExpr::PrimaryExpr(_) | StepExpr::PostfixExpr { .. } => {
                return Err(XPathError::unsupported(
                    "only axis steps are supported after the context head (no inline \
                     primary/function steps mid-path)",
                ))
            }
        };

        // `//` -> `descendant-or-self::node()` synthetic step: remember it and
        // fold it into the following real step.
        if matches!(axis_step.axis, XAxis::DescendantOrSelf)
            && matches!(axis_step.node_test, NodeTest::KindTest(KindTest::Any))
            && axis_step.predicates.is_empty()
        {
            pending_descendant = true;
            continue;
        }

        let mut axis = match &axis_step.axis {
            XAxis::Child => ir::Axis::Child,
            XAxis::Attribute => ir::Axis::Attribute,
            XAxis::Descendant | XAxis::DescendantOrSelf => ir::Axis::Descendant,
            other => {
                return Err(XPathError::unsupported(format!("{other:?} axis")));
            }
        };
        if pending_descendant {
            axis = ir::Axis::Descendant;
            pending_descendant = false;
        }

        let test = node_test_to_ir(&axis_step.node_test)?;
        let predicates = axis_step
            .predicates
            .iter()
            .map(|p| predicate_to_ir(&p.value))
            .collect::<Result<Vec<_>>>()?;

        out.push(ir::Step {
            axis,
            test,
            predicates,
        });
    }

    if pending_descendant {
        return Err(XPathError::unsupported("trailing `//` with no following step"));
    }
    Ok(out)
}

fn node_test_to_ir(test: &NodeTest) -> Result<ir::NodeTest> {
    match test {
        NodeTest::NameTest(NameTest::Name(n)) => Ok(ir::NodeTest::Name(n.value.local_name().to_string())),
        NodeTest::NameTest(NameTest::Star) => Ok(ir::NodeTest::Wildcard),
        NodeTest::NameTest(other) => Err(XPathError::unsupported(format!("name test {other:?}"))),
        NodeTest::KindTest(kind) => Err(XPathError::unsupported(format!("kind test {kind:?}"))),
    }
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

fn predicate_to_ir(expr: &Expr) -> Result<ir::Predicate> {
    let single = single_expr(expr)?;
    match deparen(single) {
        // [rel op literal]
        ExprSingle::Binary(b) => {
            let op = cmp_op(b.operator)?;
            let rel = axis_steps_to_ir(&b.left.steps)?;
            let value = pathexpr_as_literal(&b.right).ok_or_else(|| {
                XPathError::unsupported(
                    "predicate right-hand side must be a literal (e.g. `@x = \"y\"`)",
                )
            })?;
            Ok(ir::Predicate::Compare { rel, op, value })
        }
        // [rel] existence
        ExprSingle::Path(p) => {
            let rel = axis_steps_to_ir(&p.steps)?;
            if rel.is_empty() {
                return Err(XPathError::unsupported("empty predicate"));
            }
            Ok(ir::Predicate::Exists { rel })
        }
        other => Err(XPathError::unsupported(format!(
            "predicate expression {}",
            expr_kind(other)
        ))),
    }
}

fn cmp_op(op: BinaryOperator) -> Result<ir::CmpOp> {
    use BinaryOperator::*;
    Ok(match op {
        GenEq | ValueEq => ir::CmpOp::Eq,
        GenNe | ValueNe => ir::CmpOp::Ne,
        GenLt | ValueLt => ir::CmpOp::Lt,
        GenLe | ValueLe => ir::CmpOp::Le,
        GenGt | ValueGt => ir::CmpOp::Gt,
        GenGe | ValueGe => ir::CmpOp::Ge,
        other => return Err(XPathError::unsupported(format!("{other:?} operator in predicate"))),
    })
}

// ---------------------------------------------------------------------------
// Context functions & literals
// ---------------------------------------------------------------------------

/// If `step` is a call to a known TerminusDB context function (`db`, `document`,
/// `doc`) with a single string-literal argument, return `(name, arg)`.
fn as_context_function(step: &StepExpr) -> Result<Option<(String, String)>> {
    let primary = match step {
        StepExpr::PrimaryExpr(pe) => &pe.value,
        StepExpr::PostfixExpr { primary, postfixes } if postfixes.is_empty() => &primary.value,
        _ => return Ok(None),
    };

    let PrimaryExpr::FunctionCall(fc) = primary else {
        return Ok(None);
    };
    let name = fc.name.value.local_name().to_string();
    if !matches!(name.as_str(), "db" | "document" | "doc") {
        return Ok(None);
    }
    if fc.arguments.len() != 1 {
        return Err(XPathError::unsupported(format!(
            "{name}() expects exactly one string argument"
        )));
    }
    let arg = expr_single_as_string(&fc.arguments[0].value).ok_or_else(|| {
        XPathError::unsupported(format!("{name}() argument must be a string literal"))
    })?;
    Ok(Some((name, arg)))
}

/// Extract a plain string from an argument expression (`"MyModel/1234"`).
fn expr_single_as_string(es: &ExprSingle) -> Option<String> {
    if let ExprSingle::Path(p) = deparen(es) {
        if let Some(ir::Literal::Str(s)) = pathexpr_as_literal(p) {
            return Some(s);
        }
    }
    None
}

/// If a path expression is a single literal step, return that literal.
fn pathexpr_as_literal(p: &PathExpr) -> Option<ir::Literal> {
    if p.steps.len() != 1 {
        return None;
    }
    let StepExpr::PrimaryExpr(pe) = &p.steps[0].value else {
        return None;
    };
    match &pe.value {
        PrimaryExpr::Literal(lit) => Some(literal_to_ir(lit)),
        _ => None,
    }
}

fn literal_to_ir(lit: &XLit) -> ir::Literal {
    match lit {
        XLit::String(s) => ir::Literal::Str(s.clone()),
        XLit::Integer(i) => i
            .to_string()
            .parse::<i64>()
            .map(ir::Literal::Int)
            // Fall back to float for integers that don't fit i64.
            .unwrap_or_else(|_| ir::Literal::Float(i.to_string().parse::<f64>().unwrap_or(0.0))),
        XLit::Decimal(d) => ir::Literal::Float(d.to_string().parse::<f64>().unwrap_or(0.0)),
        XLit::Double(d) => ir::Literal::Float(d.0),
    }
}

// ---------------------------------------------------------------------------
// Small AST helpers
// ---------------------------------------------------------------------------

/// A top-level `Expr` is a comma-separated sequence; we only support a single
/// expression (no sequences yet).
fn single_expr(expr: &Expr) -> Result<&ExprSingle> {
    match expr.0.as_slice() {
        [only] => Ok(&only.value),
        [] => Err(XPathError::Empty),
        _ => Err(XPathError::unsupported("sequence expressions (`,`)")),
    }
}

/// Unwrap parenthesization: xee wraps a predicate/parenthesized body as
/// `Path([PrimaryExpr::Expr(Some(inner))])`. Peel those layers so callers can
/// match on the meaningful inner expression.
fn deparen(es: &ExprSingle) -> &ExprSingle {
    if let ExprSingle::Path(p) = es {
        if p.steps.len() == 1 {
            if let StepExpr::PrimaryExpr(pe) = &p.steps[0].value {
                if let PrimaryExpr::Expr(inner) = &pe.value {
                    if let Some(inner_expr) = &inner.value {
                        if let [only] = inner_expr.0.as_slice() {
                            return deparen(&only.value);
                        }
                    }
                }
            }
        }
    }
    es
}

fn expr_kind(es: &ExprSingle) -> &'static str {
    match es {
        ExprSingle::Path(_) => "path",
        ExprSingle::Apply(_) => "apply/postfix",
        ExprSingle::Let(_) => "let",
        ExprSingle::If(_) => "if",
        ExprSingle::Binary(_) => "binary",
        ExprSingle::For(_) => "for",
        ExprSingle::Quantified(_) => "quantified",
    }
}

//! Lower the `spargebra` algebra onto our narrow [`crate::ir`].
//!
//! `spargebra` already normalizes a `SELECT` into an algebra tree whose outer
//! layers are the solution modifiers (`Slice`, `Distinct`/`Reduced`, `Project`,
//! `OrderBy`) wrapping the `WHERE` graph pattern. We peel those layers to fill in
//! [`ir::SparqlQuery`]'s modifier fields, then project the remaining
//! `GraphPattern` onto [`ir::GraphPattern`]. Anything outside the supported
//! subset becomes [`SparqlError::Unsupported`], keeping the boundary explicit.

use spargebra::algebra::{Expression, GraphPattern as SgP, OrderExpression};
use spargebra::term::{NamedNodePattern, TermPattern, TriplePattern as SgTriple};
use spargebra::Query;

use crate::error::{Result, SparqlError};
use crate::ir;

/// Lower a parsed SPARQL query into the IR. Only `SELECT` is compiled.
pub fn lower(query: &Query) -> Result<ir::SparqlQuery> {
    let pattern = match query {
        Query::Select { pattern, .. } => pattern,
        Query::Ask { .. } => return Err(SparqlError::UnsupportedForm("ASK".into())),
        Query::Construct { .. } => return Err(SparqlError::UnsupportedForm("CONSTRUCT".into())),
        Query::Describe { .. } => return Err(SparqlError::UnsupportedForm("DESCRIBE".into())),
    };
    lower_select(pattern)
}

/// Peel the solution-modifier layers off a `SELECT`'s algebra and lower the core
/// `WHERE` pattern.
fn lower_select(root: &SgP) -> Result<ir::SparqlQuery> {
    let mut distinct = false;
    let mut order: Vec<ir::OrderKey> = Vec::new();
    let mut limit: Option<usize> = None;
    let mut offset: usize = 0;
    let mut projection: Option<Vec<String>> = None;

    // The modifiers nest outermost-first: Slice(Distinct(Project(OrderBy(core)))).
    // We peel them in whatever order they appear.
    let mut node = root;
    loop {
        match node {
            SgP::Slice {
                inner,
                start,
                length,
            } => {
                offset = *start;
                limit = *length;
                node = inner;
            }
            SgP::Distinct { inner } => {
                distinct = true;
                node = inner;
            }
            // REDUCED is a permission to de-duplicate, not a requirement; the
            // faithful (superset-safe) image is to keep duplicates, so we ignore
            // it rather than forcing DISTINCT.
            SgP::Reduced { inner } => {
                node = inner;
            }
            SgP::Project { inner, variables } => {
                projection = Some(variables.iter().map(|v| v.as_str().to_string()).collect());
                node = inner;
            }
            SgP::OrderBy { inner, expression } => {
                order = expression
                    .iter()
                    .map(lower_order)
                    .collect::<Result<Vec<_>>>()?;
                node = inner;
            }
            other => {
                let pattern = lower_pattern(other)?;
                return Ok(ir::SparqlQuery {
                    pattern,
                    projection: projection.unwrap_or_default(),
                    distinct,
                    order,
                    limit,
                    offset,
                });
            }
        }
    }
}

fn lower_order(o: &OrderExpression) -> Result<ir::OrderKey> {
    let (expr, desc) = match o {
        OrderExpression::Asc(e) => (e, false),
        OrderExpression::Desc(e) => (e, true),
    };
    match expr {
        Expression::Variable(v) => Ok(ir::OrderKey {
            var: v.as_str().to_string(),
            desc,
        }),
        _ => Err(SparqlError::unsupported(
            "ORDER BY over an expression (only ORDER BY ?var is supported)",
        )),
    }
}

/// Project a spargebra `GraphPattern` onto the IR pattern.
fn lower_pattern(gp: &SgP) -> Result<ir::GraphPattern> {
    match gp {
        SgP::Bgp { patterns } => Ok(ir::GraphPattern::Bgp(
            patterns
                .iter()
                .map(lower_triple)
                .collect::<Result<Vec<_>>>()?,
        )),
        SgP::Join { left, right } => Ok(ir::GraphPattern::Join(
            Box::new(lower_pattern(left)?),
            Box::new(lower_pattern(right)?),
        )),
        SgP::LeftJoin {
            left,
            right,
            expression,
        } => {
            let cond = expression.as_ref().map(lower_expr).transpose()?;
            Ok(ir::GraphPattern::Optional(
                Box::new(lower_pattern(left)?),
                Box::new(lower_pattern(right)?),
                cond,
            ))
        }
        SgP::Union { left, right } => Ok(ir::GraphPattern::Union(
            Box::new(lower_pattern(left)?),
            Box::new(lower_pattern(right)?),
        )),
        SgP::Filter { expr, inner } => Ok(ir::GraphPattern::Filter(
            lower_expr(expr)?,
            Box::new(lower_pattern(inner)?),
        )),

        SgP::Path { .. } => Err(SparqlError::unsupported("property paths")),
        SgP::Minus { .. } => Err(SparqlError::unsupported("MINUS")),
        SgP::Graph { .. } => Err(SparqlError::unsupported("GRAPH (named graphs)")),
        SgP::Extend { .. } => Err(SparqlError::unsupported("BIND / SELECT expressions")),
        SgP::Values { .. } => Err(SparqlError::unsupported("VALUES")),
        SgP::Group { .. } => Err(SparqlError::unsupported("GROUP BY / aggregates")),
        SgP::Service { .. } => Err(SparqlError::unsupported("SERVICE (federation)")),
        // A solution modifier nested here means a sub-SELECT.
        SgP::Project { .. }
        | SgP::Distinct { .. }
        | SgP::Reduced { .. }
        | SgP::Slice { .. }
        | SgP::OrderBy { .. } => Err(SparqlError::unsupported("sub-SELECT")),
    }
}

fn lower_triple(t: &SgTriple) -> Result<ir::TriplePattern> {
    Ok(ir::TriplePattern {
        subject: lower_term(&t.subject)?,
        predicate: lower_predicate(&t.predicate)?,
        object: lower_term(&t.object)?,
    })
}

fn lower_term(t: &TermPattern) -> Result<ir::Term> {
    match t {
        TermPattern::Variable(v) => Ok(ir::Term::Var(v.as_str().to_string())),
        TermPattern::NamedNode(n) => Ok(ir::Term::Iri(n.as_str().to_string())),
        TermPattern::Literal(l) => Ok(ir::Term::Literal(lower_literal(l))),
        TermPattern::BlankNode(_) => Err(SparqlError::unsupported("blank nodes")),
    }
}

fn lower_predicate(p: &NamedNodePattern) -> Result<ir::Term> {
    match p {
        NamedNodePattern::NamedNode(n) => Ok(ir::Term::Iri(n.as_str().to_string())),
        NamedNodePattern::Variable(v) => Ok(ir::Term::Var(v.as_str().to_string())),
    }
}

fn lower_literal(l: &oxrdf::Literal) -> ir::Literal {
    use terminusdb_format::prefix::{xsd_local_name, RDF_LANGSTRING_IRI};
    use terminusdb_format::XsdCategory;

    let value = l.value().to_string();
    let dt = l.datatype().as_str().to_string();
    // The numeric/boolean categories are the ones whose grouping is identical to
    // the SQL decoder's, so they come from the shared classifier. The string
    // decision is intentionally narrower here than the shared `Text` category: we
    // only collapse `xsd:string` / `rdf:langString` to a bare string, and keep
    // every other datatype (`xsd:token`, `xsd:anyURI`, temporal, …) as a `Typed`
    // literal so the emitted WOQL preserves its datatype.
    match terminusdb_format::classify_datatype(&dt) {
        XsdCategory::Integer => value
            .parse::<i64>()
            .map(ir::Literal::Int)
            .unwrap_or(ir::Literal::Typed(value, dt)),
        XsdCategory::Double => value
            .parse::<f64>()
            .map(ir::Literal::Double)
            .unwrap_or(ir::Literal::Typed(value, dt)),
        XsdCategory::Decimal => ir::Literal::Decimal(value),
        XsdCategory::Boolean => match value.as_str() {
            "true" | "1" => ir::Literal::Bool(true),
            "false" | "0" => ir::Literal::Bool(false),
            _ => ir::Literal::Typed(value, dt),
        },
        _ => {
            if xsd_local_name(&dt) == Some("string") || dt == RDF_LANGSTRING_IRI {
                ir::Literal::Str(value)
            } else {
                ir::Literal::Typed(value, dt)
            }
        }
    }
}

fn lower_expr(e: &Expression) -> Result<ir::Expr> {
    match e {
        Expression::Variable(v) => Ok(ir::Expr::Var(v.as_str().to_string())),
        Expression::Literal(l) => Ok(ir::Expr::Lit(lower_literal(l))),
        Expression::NamedNode(n) => Ok(ir::Expr::Iri(n.as_str().to_string())),
        Expression::Equal(a, b) => bin(a, b, ir::Expr::Eq),
        Expression::Greater(a, b) => bin(a, b, ir::Expr::Gt),
        Expression::GreaterOrEqual(a, b) => bin(a, b, ir::Expr::Ge),
        Expression::Less(a, b) => bin(a, b, ir::Expr::Lt),
        Expression::LessOrEqual(a, b) => bin(a, b, ir::Expr::Le),
        Expression::And(a, b) => bin(a, b, ir::Expr::And),
        Expression::Or(a, b) => bin(a, b, ir::Expr::Or),
        Expression::Not(a) => Ok(ir::Expr::Not(Box::new(lower_expr(a)?))),
        Expression::SameTerm(a, b) => bin(a, b, ir::Expr::Eq),
        other => Err(SparqlError::unsupported(format!(
            "FILTER expression `{}`",
            expr_kind(other)
        ))),
    }
}

fn bin(
    a: &Expression,
    b: &Expression,
    ctor: fn(Box<ir::Expr>, Box<ir::Expr>) -> ir::Expr,
) -> Result<ir::Expr> {
    Ok(ctor(Box::new(lower_expr(a)?), Box::new(lower_expr(b)?)))
}

fn expr_kind(e: &Expression) -> &'static str {
    match e {
        Expression::In(..) => "IN",
        Expression::Add(..) => "+",
        Expression::Subtract(..) => "-",
        Expression::Multiply(..) => "*",
        Expression::Divide(..) => "/",
        Expression::UnaryPlus(..) => "unary +",
        Expression::UnaryMinus(..) => "unary -",
        Expression::Exists(..) => "EXISTS",
        Expression::Bound(..) => "BOUND",
        Expression::If(..) => "IF",
        Expression::Coalesce(..) => "COALESCE",
        Expression::FunctionCall(..) => "function call",
        _ => "expression",
    }
}

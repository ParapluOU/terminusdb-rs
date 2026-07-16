//! Compile the [`crate::ir`] subset into a `terminusdb_woql2::Query`.
//!
//! The mapping is direct, because SPARQL and WOQL share the same triple/graph
//! data model:
//!
//! - a triple pattern `s p o` -> a WOQL `Triple(s, p, o)`,
//! - a basic graph pattern / `JOIN` -> the triples of an `And`,
//! - `OPTIONAL { ... }` -> `WoqlOptional`,
//! - `{ ... } UNION { ... }` -> `Or`,
//! - `FILTER(a op b)` -> the corresponding `Equals`/`Less`/`Greater`/... goal,
//! - the solution modifiers -> the canonical `Limit(Start(Distinct(Select(
//!   OrderBy(core)))))` nesting.
//!
//! ## IRI mapping
//!
//! SPARQL uses full IRIs; TerminusDB's instance graph uses `@schema:`-prefixed
//! predicates/classes and `rdf:type`. [`CompileOptions::schema_base`] names the
//! namespace whose members map onto `@schema:` (so `schema:name` ->
//! `@schema:name`, and `?p a schema:Person` -> `rdf:type @schema:Person`, since
//! SPARQL's `a` expands to the rdf:type IRI). Well-known `rdf:`/`rdfs:`/`xsd:`
//! IRIs collapse to their prefixed form; anything else (e.g. a full
//! `terminusdb:///data/...` instance IRI) passes through verbatim.

use decimal_rs::Decimal;
use terminusdb_format::prefix::{contract_iri, SCHEMA_IRI_DEFAULT};
use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::prelude::*;

use crate::error::{Result, SparqlError};
use crate::ir;

/// Options controlling how SPARQL IRIs map onto WOQL nodes/predicates.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// The namespace whose members are TerminusDB schema entities. IRIs under
    /// this base map onto the `@schema:` prefix (predicates and class names).
    /// Defaults to `http://terminusdb.com/schema#`.
    pub schema_base: String,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            schema_base: SCHEMA_IRI_DEFAULT.to_string(),
        }
    }
}

/// The result of compiling a SPARQL query.
#[derive(Debug, Clone)]
pub struct CompiledSparql {
    /// The WOQL query, ready to execute via `client.query(spec, compiled.query)`.
    pub query: Query,
    /// The projected variable names (bare, no `?`), in SELECT order. Result
    /// bindings are keyed by these names.
    pub variables: Vec<String>,
}

/// Compile an [`ir::SparqlQuery`] into WOQL.
pub fn compile(q: &ir::SparqlQuery, opts: &CompileOptions) -> Result<CompiledSparql> {
    let mut clauses = Vec::new();
    emit_pattern(&q.pattern, opts, &mut clauses)?;

    // Projection: explicit SELECT list, or every variable in the pattern for
    // `SELECT *`.
    let variables = if q.projection.is_empty() {
        collect_vars(&q.pattern)
    } else {
        q.projection.clone()
    };
    if variables.is_empty() {
        return Err(SparqlError::Empty);
    }

    // Compose the canonical nesting, inside-out:
    //   Limit( Start( Distinct( Select( OrderBy( And(core) ) ) ) ) )
    let mut query = and_of(clauses);

    if !q.order.is_empty() {
        let ordering = q
            .order
            .iter()
            .map(|k| OrderTemplate {
                variable: k.var.clone(),
                order: if k.desc { Order::Desc } else { Order::Asc },
            })
            .collect();
        query = Query::OrderBy(OrderBy {
            ordering,
            query: Box::new(query),
        });
    }

    query = Query::Select(Select {
        variables: variables.clone(),
        query: Box::new(query),
    });

    if q.distinct {
        query = Query::Distinct(Distinct {
            variables: variables.clone(),
            query: Box::new(query),
        });
    }

    if q.offset > 0 {
        query = Query::Start(Start {
            start: q.offset as u64,
            query: Box::new(query),
        });
    }

    if let Some(limit) = q.limit {
        query = Query::Limit(Limit {
            limit: limit as u64,
            query: Box::new(query),
        });
    }

    Ok(CompiledSparql { query, variables })
}

/// Combine a list of conjunctive goals into one `Query`: a lone goal is returned
/// as-is, an empty list becomes `True`, otherwise they are wrapped in `And`.
fn and_of(mut clauses: Vec<Query>) -> Query {
    match clauses.len() {
        0 => Query::True(True {}),
        1 => clauses.pop().unwrap(),
        _ => Query::And(And { and: clauses }),
    }
}

/// Emit the WOQL goals for a graph pattern, appending them to `clauses`.
fn emit_pattern(p: &ir::GraphPattern, opts: &CompileOptions, clauses: &mut Vec<Query>) -> Result<()> {
    match p {
        ir::GraphPattern::Bgp(triples) => {
            for t in triples {
                clauses.push(emit_triple(t, opts)?);
            }
            Ok(())
        }
        ir::GraphPattern::Join(left, right) => {
            emit_pattern(left, opts, clauses)?;
            emit_pattern(right, opts, clauses)?;
            Ok(())
        }
        ir::GraphPattern::Filter(expr, inner) => {
            emit_pattern(inner, opts, clauses)?;
            emit_filter(expr, opts, clauses)?;
            Ok(())
        }
        ir::GraphPattern::Optional(left, right, cond) => {
            emit_pattern(left, opts, clauses)?;
            let mut inner = Vec::new();
            emit_pattern(right, opts, &mut inner)?;
            if let Some(c) = cond {
                emit_filter(c, opts, &mut inner)?;
            }
            clauses.push(Query::WoqlOptional(WoqlOptional {
                query: Box::new(and_of(inner)),
            }));
            Ok(())
        }
        ir::GraphPattern::Union(left, right) => {
            let mut lc = Vec::new();
            emit_pattern(left, opts, &mut lc)?;
            let mut rc = Vec::new();
            emit_pattern(right, opts, &mut rc)?;
            clauses.push(Query::Or(Or {
                or: vec![and_of(lc), and_of(rc)],
            }));
            Ok(())
        }
        ir::GraphPattern::Empty => Ok(()),
    }
}

/// One triple pattern -> a WOQL `Triple`.
fn emit_triple(t: &ir::TriplePattern, opts: &CompileOptions) -> Result<Query> {
    Ok(Query::Triple(Triple {
        subject: node_value(&t.subject, opts, "subject")?,
        predicate: node_value(&t.predicate, opts, "predicate")?,
        object: object_value(&t.object, opts)?,
        // Left as the default graph; woql2's serialization fills the `instance`
        // default (see Query::to_woql_json).
        graph: None,
    }))
}

/// A subject/predicate term must be a node or a variable (never a literal).
fn node_value(term: &ir::Term, opts: &CompileOptions, position: &str) -> Result<NodeValue> {
    match term {
        ir::Term::Var(v) => Ok(NodeValue::Variable(v.clone())),
        ir::Term::Iri(iri) => Ok(NodeValue::Node(map_iri(iri, opts))),
        ir::Term::Literal(_) => Err(SparqlError::unsupported(format!(
            "literal in {position} position"
        ))),
    }
}

/// An object term can be a node, a variable, or a literal value.
fn object_value(term: &ir::Term, opts: &CompileOptions) -> Result<Value> {
    match term {
        ir::Term::Var(v) => Ok(Value::Variable(v.clone())),
        ir::Term::Iri(iri) => Ok(Value::Node(map_iri(iri, opts))),
        ir::Term::Literal(l) => Ok(Value::Data(literal_to_xsd(l)?)),
    }
}

/// Lower a boolean `FILTER` expression into one or more conjunctive WOQL goals.
fn emit_filter(e: &ir::Expr, opts: &CompileOptions, clauses: &mut Vec<Query>) -> Result<()> {
    match e {
        // A conjunction flattens into sibling goals of the enclosing And.
        ir::Expr::And(a, b) => {
            emit_filter(a, opts, clauses)?;
            emit_filter(b, opts, clauses)?;
            Ok(())
        }
        ir::Expr::Or(a, b) => {
            let mut ac = Vec::new();
            emit_filter(a, opts, &mut ac)?;
            let mut bc = Vec::new();
            emit_filter(b, opts, &mut bc)?;
            clauses.push(Query::Or(Or {
                or: vec![and_of(ac), and_of(bc)],
            }));
            Ok(())
        }
        ir::Expr::Not(inner) => {
            let mut ic = Vec::new();
            emit_filter(inner, opts, &mut ic)?;
            clauses.push(Query::Not(Not {
                query: Box::new(and_of(ic)),
            }));
            Ok(())
        }
        ir::Expr::Eq(a, b) => {
            clauses.push(equals_goal(a, b, opts)?);
            Ok(())
        }
        ir::Expr::Ne(a, b) => {
            clauses.push(Query::Not(Not {
                query: Box::new(equals_goal(a, b, opts)?),
            }));
            Ok(())
        }
        ir::Expr::Lt(a, b) => {
            let (left, right) = order_operands(a, b)?;
            clauses.push(Query::Less(Less { left, right }));
            Ok(())
        }
        ir::Expr::Le(a, b) => {
            let (left, right) = order_operands(a, b)?;
            clauses.push(Query::Lte(Lte { left, right }));
            Ok(())
        }
        ir::Expr::Gt(a, b) => {
            let (left, right) = order_operands(a, b)?;
            clauses.push(Query::Greater(Greater { left, right }));
            Ok(())
        }
        ir::Expr::Ge(a, b) => {
            let (left, right) = order_operands(a, b)?;
            clauses.push(Query::Gte(Gte { left, right }));
            Ok(())
        }
        // A bare term as a FILTER (e.g. `FILTER(?x)`) has no faithful two-valued
        // WOQL image without effective-boolean-value semantics.
        ir::Expr::Var(_) | ir::Expr::Lit(_) | ir::Expr::Iri(_) => Err(SparqlError::unsupported(
            "a bare term as a FILTER (expected a comparison or boolean combination)",
        )),
    }
}

/// `=` uses WOQL `Equals`, which takes node-aware `Value`s, so it works for
/// both `?x = <iri>` and `?x = "literal"`.
fn equals_goal(a: &ir::Expr, b: &ir::Expr, opts: &CompileOptions) -> Result<Query> {
    Ok(Query::Equals(Equals {
        left: expr_value(a, opts)?,
        right: expr_value(b, opts)?,
    }))
}

/// Ordered comparisons use `DataValue` (no node form).
fn order_operands(a: &ir::Expr, b: &ir::Expr) -> Result<(DataValue, DataValue)> {
    Ok((expr_data(a)?, expr_data(b)?))
}

fn expr_value(e: &ir::Expr, opts: &CompileOptions) -> Result<Value> {
    match e {
        ir::Expr::Var(v) => Ok(Value::Variable(v.clone())),
        ir::Expr::Lit(l) => Ok(Value::Data(literal_to_xsd(l)?)),
        ir::Expr::Iri(iri) => Ok(Value::Node(map_iri(iri, opts))),
        _ => Err(SparqlError::unsupported(
            "a compound expression inside `=` (only variables/constants compare)",
        )),
    }
}

fn expr_data(e: &ir::Expr) -> Result<DataValue> {
    match e {
        ir::Expr::Var(v) => Ok(DataValue::Variable(v.clone())),
        ir::Expr::Lit(l) => Ok(DataValue::Data(literal_to_xsd(l)?)),
        ir::Expr::Iri(_) => Err(SparqlError::unsupported(
            "an IRI in an ordered comparison (`<`, `>`, ...)",
        )),
        _ => Err(SparqlError::unsupported(
            "a compound expression in an ordered comparison",
        )),
    }
}

/// Map a SPARQL IRI onto the WOQL node/predicate string. Members of the schema
/// namespace contract to `@schema:`, the well-known `rdf:`/`rdfs:`/`xsd:`
/// namespaces to their prefixes, and anything else (a full
/// `terminusdb:///data/...` instance IRI, a foreign vocabulary) passes through
/// unchanged. See [`terminusdb_format::prefix::contract_iri`].
fn map_iri(iri: &str, opts: &CompileOptions) -> String {
    contract_iri(iri, &opts.schema_base)
}

fn literal_to_xsd(l: &ir::Literal) -> Result<XSDAnySimpleType> {
    Ok(match l {
        ir::Literal::Str(s) => XSDAnySimpleType::String(s.clone()),
        ir::Literal::Int(i) => XSDAnySimpleType::Integer(*i),
        ir::Literal::Double(f) => XSDAnySimpleType::Float(*f),
        ir::Literal::Bool(b) => XSDAnySimpleType::Boolean(*b),
        ir::Literal::Decimal(s) => s
            .parse::<Decimal>()
            .map(XSDAnySimpleType::Decimal)
            .map_err(|_| SparqlError::unsupported(format!("un-parseable xsd:decimal `{s}`")))?,
        // A typed literal we don't specifically model: keep the lexical value as
        // a string. WOQL comparisons still work for equality on the string form.
        ir::Literal::Typed(value, _dt) => XSDAnySimpleType::String(value.clone()),
    })
}

/// Collect every variable that appears in a pattern, in first-seen order
/// (used for `SELECT *`).
fn collect_vars(p: &ir::GraphPattern) -> Vec<String> {
    let mut out = Vec::new();
    collect_vars_into(p, &mut out);
    out
}

fn collect_vars_into(p: &ir::GraphPattern, out: &mut Vec<String>) {
    let push = |v: &str, out: &mut Vec<String>| {
        if !out.iter().any(|e| e == v) {
            out.push(v.to_string());
        }
    };
    match p {
        ir::GraphPattern::Bgp(triples) => {
            for t in triples {
                for term in [&t.subject, &t.predicate, &t.object] {
                    if let ir::Term::Var(v) = term {
                        push(v, out);
                    }
                }
            }
        }
        ir::GraphPattern::Join(a, b) | ir::GraphPattern::Union(a, b) => {
            collect_vars_into(a, out);
            collect_vars_into(b, out);
        }
        ir::GraphPattern::Optional(a, b, _) => {
            collect_vars_into(a, out);
            collect_vars_into(b, out);
        }
        ir::GraphPattern::Filter(_, inner) => collect_vars_into(inner, out),
        ir::GraphPattern::Empty => {}
    }
}

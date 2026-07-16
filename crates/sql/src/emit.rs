//! The emitter: a post-order walk of a DataFusion `LogicalPlan` producing WOQL.
//!
//! DataFusion has already resolved names and checked types, so the plan is a
//! trustworthy IR. We translate it to `terminusdb_woql2` AST directly (as
//! `terminusdb-xpath` does), and return [`SqlError::Unsupported`] for any node or
//! expression outside the v1 subset — never an approximation, never a dropped
//! predicate.
//!
//! ## SQL NULL vs datalog absence
//!
//! In WOQL a missing property is simply *no triple*: the row does not bind that
//! variable, so it is **absent** from the solution's binding map. That is datalog
//! absence, which is NOT the same as a SQL NULL cell. We reconcile the two only at
//! the projection boundary:
//!
//! 1. A nullable column that is *selected but not constrained* is emitted as its
//!    property triple wrapped in `Optional`, so the row still appears when the
//!    property is absent; the runner decodes the unbound variable as SQL NULL.
//! 2. A nullable column used by a WHERE/JOIN goal is emitted as a **required**
//!    (bare) triple — the predicate implies presence. We never emit optional +
//!    filtered, which would silently change which rows match. (To stay on the safe
//!    side of this rule we treat a column as constrained if *any* goal references a
//!    column of that name; see `collect_constrained`.)
//! 3. `IS NULL` / `IS NOT NULL` / `NOT IN` / a NULL literal in a predicate are
//!    rejected as `Unsupported` in v1: SQL three-valued logic has no faithful
//!    two-valued datalog image.

use std::collections::{HashMap, HashSet};

use datafusion_common::{Column as DfColumn, ScalarValue};
use datafusion_expr::expr::Sort as SortExpr;
use datafusion_expr::{BinaryExpr, Distinct, Expr, JoinType, LogicalPlan, Operator};
use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::prelude::{
    And, DataValue, Distinct as WoqlDistinct, Equals, Greater, Gte, Less, Limit, Lte, NodeValue,
    Not, Or, Order, OrderBy, OrderTemplate, Query, Select, Start, Triple, True, Value, WoqlOptional,
};

use crate::catalog::{schema_iri, Catalog};
use crate::error::{Result, SqlError};
use crate::meta::{ColumnKind, ColumnMeta, TableMeta};

/// The emitter's output: a WOQL query plus the ordered SQL output columns.
#[derive(Debug, Clone)]
pub struct SqlQuery {
    /// The compiled query (inner — NOT yet wrapped in `Using` for commit pinning).
    pub woql: Query,
    /// The SQL output columns, in SELECT order.
    pub projection: Vec<ProjCol>,
}

/// One SQL output column: its SQL name and the WOQL variable that carries it.
#[derive(Debug, Clone)]
pub struct ProjCol {
    pub sql_name: String,
    pub woql_var: String,
}

/// A fresh-variable allocator. Deterministic post-order allocation so golden tests
/// can pin the exact variable names.
struct Ctx {
    counter: u32,
}

impl Ctx {
    fn fresh(&mut self, prefix: &str) -> String {
        let v = format!("{prefix}{}", self.counter);
        self.counter += 1;
        v
    }
}

/// (qualifier table/alias, column name) → the variable bound to that column.
type VarKey = (Option<String>, String);

#[derive(Clone)]
struct ColBinding {
    var: String,
    /// True for `id`/object-ref columns, whose value is a node IRI (compared with
    /// `Equals`/node values, never with ordering operators).
    node_valued: bool,
}

/// The variable-binding environment, populated at table scans.
#[derive(Default)]
struct Env {
    map: HashMap<VarKey, ColBinding>,
}

impl Env {
    fn insert(&mut self, qual: Option<String>, name: &str, binding: ColBinding) {
        self.map.insert((qual, name.to_string()), binding);
    }

    fn lookup(&self, col: &DfColumn) -> Result<&ColBinding> {
        let qual = col.relation.as_ref().map(|r| r.table().to_string());
        if let Some(b) = self.map.get(&(qual.clone(), col.name.clone())) {
            return Ok(b);
        }
        // Fall back to a name-only match (unqualified reference in a single-table
        // query). DataFusion has already guaranteed the reference is unambiguous.
        let mut hits = self.map.iter().filter(|((_, n), _)| n == &col.name);
        match (hits.next(), hits.next()) {
            (Some((_, b)), None) => Ok(b),
            _ => Err(SqlError::unsupported(format!(
                "could not resolve column `{}` to a bound variable",
                col.name
            ))),
        }
    }
}

/// Compile a `LogicalPlan` into a [`SqlQuery`].
pub fn emit(plan: &LogicalPlan, catalog: &Catalog) -> Result<SqlQuery> {
    let mut ctx = Ctx { counter: 0 };

    // Which columns are referenced by a Filter/Join goal (so their triples must be
    // required, never optional). Keyed by name — the safe over-approximation.
    let mut constrained = HashSet::new();
    collect_constrained(plan, &mut constrained);

    // Which columns are referenced anywhere (projection / filter / join / sort).
    // Without the DataFusion optimizer, a TableScan exposes every column, so we
    // prune to the referenced set ourselves and bind only those.
    let mut referenced = HashSet::new();
    collect_referenced(plan, &mut referenced);

    // Peel the outer "shaping" nodes (projection / sort / limit / distinct) off the
    // relational core, in any order DataFusion layered them.
    let mut projection: Option<&[Expr]> = None;
    let mut sort_keys: Vec<&SortExpr> = Vec::new();
    let mut skip: u64 = 0;
    let mut fetch: Option<u64> = None;
    let mut distinct = false;
    // Aliases introduced by an INNER projection (`col AS out`). DataFusion inserts
    // such a projection e.g. for `ORDER BY <projected-away column>`, and the outer
    // projection then references the alias `out`. We record `out -> underlying
    // column` so the outer references resolve against the scan-bound variables.
    let mut alias_pairs: Vec<(String, DfColumn)> = Vec::new();
    let mut node = plan;
    loop {
        match node {
            LogicalPlan::Projection(p) => {
                if projection.is_none() {
                    projection = Some(&p.expr);
                } else {
                    // Inner projection: pass-through columns are fine (already
                    // bound by the core scan); aliases are recorded so the outer
                    // projection can resolve them.
                    for e in &p.expr {
                        collect_alias(e, &mut alias_pairs)?;
                    }
                }
                node = &p.input;
            }
            LogicalPlan::Sort(s) => {
                for key in &s.expr {
                    sort_keys.push(key);
                }
                if let Some(f) = s.fetch {
                    fetch = Some(fetch.map_or(f as u64, |e| e.min(f as u64)));
                }
                node = &s.input;
            }
            LogicalPlan::Limit(l) => {
                if let Some(n) = lit_u64_opt(l.skip.as_deref())? {
                    skip = skip.max(n);
                }
                if let Some(n) = lit_u64_opt(l.fetch.as_deref())? {
                    fetch = Some(fetch.map_or(n, |e| e.min(n)));
                }
                node = &l.input;
            }
            LogicalPlan::Distinct(Distinct::All(input)) => {
                distinct = true;
                node = input;
            }
            LogicalPlan::Distinct(Distinct::On(_)) => {
                return Err(SqlError::unsupported("DISTINCT ON"));
            }
            _ => break,
        }
    }

    let projection = projection.ok_or_else(|| SqlError::unsupported("query without a projection"))?;

    // Emit the relational core into one conjunction.
    let mut env = Env::default();
    let mut clauses: Vec<Query> = Vec::new();
    emit_relation(
        node,
        catalog,
        &mut ctx,
        &mut env,
        &constrained,
        &referenced,
        &mut clauses,
        None,
    )?;

    // Materialise inner-projection aliases: bind each output name to the same
    // variable as its underlying column, so the outer projection / sort can
    // reference it (e.g. `SELECT b.author AS link ... ORDER BY b.title`).
    for (name, underlying) in &alias_pairs {
        if let Ok(binding) = env.lookup(underlying) {
            let binding = binding.clone();
            env.insert(None, name, binding);
        }
    }

    // Resolve the SELECT list against the bound variables.
    let mut select_vars: Vec<String> = Vec::new();
    let mut proj_cols: Vec<ProjCol> = Vec::new();
    for e in projection {
        let (sql_name, binding) = resolve_projection_expr(e, &env)?;
        select_vars.push(binding.var.clone());
        proj_cols.push(ProjCol {
            sql_name,
            woql_var: binding.var,
        });
    }

    // Compose the canonical nesting, inside-out:
    //   Limit( Start( Distinct( Select( OrderBy( And(core) ) ) ) ) )
    let mut q = Query::And(And { and: clauses });

    if !sort_keys.is_empty() {
        let mut ordering = Vec::new();
        for key in &sort_keys {
            ordering.push(order_template(key, &env)?);
        }
        q = Query::OrderBy(OrderBy {
            ordering,
            query: Box::new(q),
        });
    }

    q = Query::Select(Select {
        variables: select_vars.clone(),
        query: Box::new(q),
    });

    if distinct {
        q = Query::Distinct(WoqlDistinct {
            variables: select_vars,
            query: Box::new(q),
        });
    }

    if skip > 0 {
        q = Query::Start(Start {
            start: skip,
            query: Box::new(q),
        });
    }

    if let Some(f) = fetch {
        q = Query::Limit(Limit {
            limit: f,
            query: Box::new(q),
        });
    }

    Ok(SqlQuery {
        woql: q,
        projection: proj_cols,
    })
}

/// Emit the relational core: table scans, filters, joins, and aliases. Appends
/// conjunction goals to `clauses` and binds columns into `env`.
///
/// `qualifier` overrides the qualifier used for a scan's columns — set to a
/// `SubqueryAlias`'s alias so upper column references (`p.name`) resolve.
#[allow(clippy::too_many_arguments)]
fn emit_relation(
    plan: &LogicalPlan,
    catalog: &Catalog,
    ctx: &mut Ctx,
    env: &mut Env,
    constrained: &HashSet<String>,
    referenced: &HashSet<String>,
    clauses: &mut Vec<Query>,
    qualifier: Option<&str>,
) -> Result<()> {
    match plan {
        LogicalPlan::TableScan(scan) => {
            let table_name = scan.table_name.table();
            let meta = catalog
                .table(table_name)
                .ok_or_else(|| SqlError::Plan(format!("table `{table_name}` vanished from catalog")))?;
            let qual = qualifier.unwrap_or(table_name).to_string();
            emit_table_scan(scan, meta, &qual, ctx, env, constrained, referenced, clauses);
            Ok(())
        }
        LogicalPlan::Filter(f) => {
            emit_relation(
                &f.input, catalog, ctx, env, constrained, referenced, clauses, qualifier,
            )?;
            predicate_clauses(&f.predicate, env, clauses)?;
            Ok(())
        }
        LogicalPlan::SubqueryAlias(sa) => emit_relation(
            &sa.input,
            catalog,
            ctx,
            env,
            constrained,
            referenced,
            clauses,
            Some(sa.alias.table()),
        ),
        LogicalPlan::Join(j) => emit_join(j, catalog, ctx, env, constrained, referenced, clauses),
        LogicalPlan::EmptyRelation(_) => Ok(()),
        other => Err(SqlError::unsupported(format!(
            "relational operator `{}`",
            plan_kind(other)
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_table_scan(
    scan: &datafusion_expr::TableScan,
    meta: &TableMeta,
    qual: &str,
    ctx: &mut Ctx,
    env: &mut Env,
    constrained: &HashSet<String>,
    referenced: &HashSet<String>,
    clauses: &mut Vec<Query>,
) {
    let subject = ctx.fresh("s");

    // Type filter: ?subject rdf:type @schema:Class
    clauses.push(Query::Triple(Triple {
        subject: NodeValue::Variable(subject.clone()),
        predicate: NodeValue::Node("rdf:type".to_string()),
        object: Value::Node(schema_iri(&meta.class_iri)),
        graph: None,
    }));

    for field in scan.projected_schema.fields() {
        let col_name = field.name();
        // Prune to referenced columns: without the optimizer, the scan exposes
        // every column, but we only bind/emit the ones the query actually uses.
        if !referenced.contains(col_name) {
            continue;
        }
        let Some(col) = meta.column(col_name) else {
            continue; // not a catalog column (should not happen post-planning)
        };
        if col.is_id() {
            // The id column binds the row subject; no triple of its own.
            env.insert(
                Some(qual.to_string()),
                col_name,
                ColBinding {
                    var: subject.clone(),
                    node_valued: true,
                },
            );
            continue;
        }

        let var = ctx.fresh("c");
        let triple = Triple {
            subject: NodeValue::Variable(subject.clone()),
            predicate: NodeValue::Node(col.predicate.clone().expect("non-id column has predicate")),
            object: Value::Variable(var.clone()),
            graph: None,
        };

        // Optional only when nullable AND not referenced by any goal (see the
        // NULL-vs-absence note at the top of this module).
        if col.nullable && !constrained.contains(col_name) {
            clauses.push(Query::WoqlOptional(WoqlOptional {
                query: Box::new(Query::Triple(triple)),
            }));
        } else {
            clauses.push(Query::Triple(triple));
        }

        env.insert(
            Some(qual.to_string()),
            col_name,
            ColBinding {
                var,
                node_valued: is_node_valued(col),
            },
        );
    }
}

fn emit_join(
    j: &datafusion_expr::logical_plan::Join,
    catalog: &Catalog,
    ctx: &mut Ctx,
    env: &mut Env,
    constrained: &HashSet<String>,
    referenced: &HashSet<String>,
    clauses: &mut Vec<Query>,
) -> Result<()> {
    match j.join_type {
        JoinType::Inner => {
            emit_relation(&j.left, catalog, ctx, env, constrained, referenced, clauses, None)?;
            emit_relation(&j.right, catalog, ctx, env, constrained, referenced, clauses, None)?;
            for (l, r) in &j.on {
                clauses.push(equijoin_goal(l, r, env)?);
            }
            if let Some(filter) = &j.filter {
                predicate_clauses(filter, env, clauses)?;
            }
            Ok(())
        }
        JoinType::Left => {
            emit_relation(&j.left, catalog, ctx, env, constrained, referenced, clauses, None)?;
            // The nullable side goes inside one Optional so absent matches still
            // yield the left row (with right columns unbound → SQL NULL).
            let mut right: Vec<Query> = Vec::new();
            emit_relation(&j.right, catalog, ctx, env, constrained, referenced, &mut right, None)?;
            for (l, r) in &j.on {
                right.push(equijoin_goal(l, r, env)?);
            }
            if let Some(filter) = &j.filter {
                predicate_clauses(filter, env, &mut right)?;
            }
            clauses.push(Query::WoqlOptional(WoqlOptional {
                query: Box::new(and_of(right)),
            }));
            Ok(())
        }
        other => Err(SqlError::unsupported(format!("{other:?} join"))),
    }
}

/// Build the unification goal for one equijoin key pair. Both sides must be
/// columns; they share a variable via `Equals`.
fn equijoin_goal(left: &Expr, right: &Expr, env: &Env) -> Result<Query> {
    let lv = column_var(left, env)?;
    let rv = column_var(right, env)?;
    Ok(Query::Equals(Equals {
        left: Value::Variable(lv),
        right: Value::Variable(rv),
    }))
}

fn column_var(expr: &Expr, env: &Env) -> Result<String> {
    match unwrap_alias(expr) {
        Expr::Column(c) => Ok(env.lookup(c)?.var.clone()),
        other => Err(SqlError::unsupported(format!(
            "join key must be a column, found `{other}`"
        ))),
    }
}

/// Translate a boolean predicate into conjunction goals, appending to `out`.
fn predicate_clauses(expr: &Expr, env: &Env, out: &mut Vec<Query>) -> Result<()> {
    match unwrap_alias(expr) {
        Expr::BinaryExpr(BinaryExpr { left, op: Operator::And, right }) => {
            predicate_clauses(left, env, out)?;
            predicate_clauses(right, env, out)?;
            Ok(())
        }
        Expr::BinaryExpr(BinaryExpr { left, op: Operator::Or, right }) => {
            let mut l = Vec::new();
            predicate_clauses(left, env, &mut l)?;
            let mut r = Vec::new();
            predicate_clauses(right, env, &mut r)?;
            out.push(Query::Or(Or {
                or: vec![and_of(l), and_of(r)],
            }));
            Ok(())
        }
        Expr::BinaryExpr(b) => {
            out.push(comparison_goal(b, env)?);
            Ok(())
        }
        Expr::Not(inner) => {
            let mut c = Vec::new();
            predicate_clauses(inner, env, &mut c)?;
            out.push(Query::Not(Not {
                query: Box::new(and_of(c)),
            }));
            Ok(())
        }
        Expr::IsNull(_) | Expr::IsNotNull(_) => Err(SqlError::unsupported(
            "IS NULL / IS NOT NULL (three-valued logic is out of v1 scope)",
        )),
        other => Err(SqlError::unsupported(format!(
            "predicate expression `{other}`"
        ))),
    }
}

/// Translate a single comparison `col <op> literal` (or `col = col`) into a goal.
fn comparison_goal(b: &BinaryExpr, env: &Env) -> Result<Query> {
    // Orient so the column is on the left, flipping the operator if needed.
    let (col_expr, lit_expr, op) = match (unwrap_alias(&b.left), unwrap_alias(&b.right)) {
        (Expr::Column(_), Expr::Column(_)) => {
            // Column = Column: a join-style equality (e.g. from a comma join).
            if b.op == Operator::Eq {
                return equijoin_goal(&b.left, &b.right, env);
            }
            return Err(SqlError::unsupported(
                "non-equality comparison between two columns",
            ));
        }
        (Expr::Column(_), _) => (&b.left, &b.right, b.op),
        (_, Expr::Column(_)) => (&b.right, &b.left, flip_operator(b.op)),
        _ => {
            return Err(SqlError::unsupported(
                "comparison without a column operand",
            ))
        }
    };

    let col = match unwrap_alias(col_expr) {
        Expr::Column(c) => c,
        _ => unreachable!("oriented column side is a column"),
    };
    let binding = env.lookup(col)?.clone();
    let var = binding.var;

    match op {
        Operator::Eq => Ok(Query::Equals(Equals {
            left: Value::Variable(var),
            right: literal_value(lit_expr, binding.node_valued)?,
        })),
        Operator::NotEq => Ok(Query::Not(Not {
            query: Box::new(Query::Equals(Equals {
                left: Value::Variable(var),
                right: literal_value(lit_expr, binding.node_valued)?,
            })),
        })),
        Operator::Lt | Operator::LtEq | Operator::Gt | Operator::GtEq => {
            if binding.node_valued {
                return Err(SqlError::unsupported(
                    "ordering comparison on an id/reference column",
                ));
            }
            let right = DataValue::Data(literal_xsd(lit_expr)?);
            let left = DataValue::Variable(var);
            Ok(match op {
                Operator::Lt => Query::Less(Less { left, right }),
                Operator::LtEq => Query::Lte(Lte { left, right }),
                Operator::Gt => Query::Greater(Greater { left, right }),
                Operator::GtEq => Query::Gte(Gte { left, right }),
                _ => unreachable!(),
            })
        }
        other => Err(SqlError::unsupported(format!("operator `{other}`"))),
    }
}

/// A literal for `Equals`: a node value for id/ref columns, else a data value.
fn literal_value(expr: &Expr, node_valued: bool) -> Result<Value> {
    if node_valued {
        Ok(Value::Node(literal_node(expr)?))
    } else {
        Ok(Value::Data(literal_xsd(expr)?))
    }
}

fn literal_node(expr: &Expr) -> Result<String> {
    match strip_cast(expr) {
        Expr::Literal(sv, _) => scalar_to_node(sv),
        other => Err(SqlError::unsupported(format!(
            "expected an IRI literal, found `{other}`"
        ))),
    }
}

fn literal_xsd(expr: &Expr) -> Result<XSDAnySimpleType> {
    match strip_cast(expr) {
        Expr::Literal(sv, _) => scalar_to_xsd(sv),
        other => Err(SqlError::unsupported(format!(
            "expected a literal, found `{other}`"
        ))),
    }
}

fn resolve_projection_expr<'a>(expr: &Expr, env: &'a Env) -> Result<(String, ColBinding)> {
    match expr {
        Expr::Column(c) => Ok((c.name.clone(), env.lookup(c)?.clone())),
        Expr::Alias(a) => match unwrap_alias(&a.expr) {
            Expr::Column(c) => Ok((a.name.clone(), env.lookup(c)?.clone())),
            _ => Err(SqlError::unsupported(
                "computed projection expression (only column projections are supported in v1)",
            )),
        },
        _ => Err(SqlError::unsupported(
            "computed projection expression (only column projections are supported in v1)",
        )),
    }
}

fn order_template(key: &SortExpr, env: &Env) -> Result<OrderTemplate> {
    let col = match unwrap_alias(&key.expr) {
        Expr::Column(c) => c,
        other => {
            return Err(SqlError::unsupported(format!(
                "ORDER BY expression `{other}` (only column sort keys are supported in v1)"
            )))
        }
    };
    Ok(OrderTemplate {
        variable: env.lookup(col)?.var.clone(),
        // Note: WOQL OrderBy has no nulls-first/last control; `key.nulls_first`
        // is not expressible and is ignored.
        order: if key.asc { Order::Asc } else { Order::Desc },
    })
}

// --- small helpers -------------------------------------------------------------

fn is_node_valued(col: &ColumnMeta) -> bool {
    matches!(col.kind, ColumnKind::Id | ColumnKind::ObjectRef { .. })
}

/// Combine goals into a single query (flattening a singleton, `True` if empty).
fn and_of(mut clauses: Vec<Query>) -> Query {
    match clauses.len() {
        0 => Query::True(True {}),
        1 => clauses.pop().unwrap(),
        _ => Query::And(And { and: clauses }),
    }
}

fn flip_operator(op: Operator) -> Operator {
    match op {
        Operator::Lt => Operator::Gt,
        Operator::LtEq => Operator::GtEq,
        Operator::Gt => Operator::Lt,
        Operator::GtEq => Operator::LtEq,
        other => other, // Eq / NotEq are symmetric
    }
}

fn unwrap_alias(expr: &Expr) -> &Expr {
    match expr {
        Expr::Alias(a) => unwrap_alias(&a.expr),
        other => other,
    }
}

/// See through a `Cast`/`TryCast` wrapping a literal (DataFusion inserts these for
/// mixed-type comparisons). A cast over anything else is not seen through here.
fn strip_cast(expr: &Expr) -> &Expr {
    match expr {
        Expr::Cast(c) => strip_cast(&c.expr),
        Expr::TryCast(c) => strip_cast(&c.expr),
        Expr::Alias(a) => strip_cast(&a.expr),
        other => other,
    }
}

/// Record an inner-projection expression: a plain column is already bound by the
/// core scan; an `Alias(Column) AS name` maps `name` to the underlying column so
/// the outer projection can resolve it. A *computed* inner projection is
/// unsupported.
fn collect_alias(expr: &Expr, out: &mut Vec<(String, DfColumn)>) -> Result<()> {
    match expr {
        Expr::Column(_) => Ok(()),
        Expr::Alias(a) => match unwrap_alias(&a.expr) {
            Expr::Column(c) => {
                out.push((a.name.clone(), c.clone()));
                Ok(())
            }
            _ => Err(SqlError::unsupported(
                "computed intermediate projection expression",
            )),
        },
        _ => Err(SqlError::unsupported(
            "computed intermediate projection expression",
        )),
    }
}

fn lit_u64_opt(expr: Option<&Expr>) -> Result<Option<u64>> {
    match expr {
        None => Ok(None),
        Some(e) => match strip_cast(e) {
            Expr::Literal(sv, _) => scalar_to_u64(sv).map(Some),
            other => Err(SqlError::unsupported(format!(
                "LIMIT/OFFSET must be a constant, found `{other}`"
            ))),
        },
    }
}

fn scalar_to_u64(sv: &ScalarValue) -> Result<u64> {
    let n: i128 = match sv {
        ScalarValue::Int8(Some(v)) => *v as i128,
        ScalarValue::Int16(Some(v)) => *v as i128,
        ScalarValue::Int32(Some(v)) => *v as i128,
        ScalarValue::Int64(Some(v)) => *v as i128,
        ScalarValue::UInt8(Some(v)) => *v as i128,
        ScalarValue::UInt16(Some(v)) => *v as i128,
        ScalarValue::UInt32(Some(v)) => *v as i128,
        ScalarValue::UInt64(Some(v)) => *v as i128,
        other => {
            return Err(SqlError::unsupported(format!(
                "non-integer LIMIT/OFFSET literal `{other:?}`"
            )))
        }
    };
    u64::try_from(n).map_err(|_| SqlError::unsupported("negative LIMIT/OFFSET"))
}

/// Convert a literal to an IRI string for a node-valued comparison.
fn scalar_to_node(sv: &ScalarValue) -> Result<String> {
    match sv {
        ScalarValue::Utf8(Some(s))
        | ScalarValue::LargeUtf8(Some(s))
        | ScalarValue::Utf8View(Some(s)) => Ok(s.clone()),
        other => Err(SqlError::unsupported(format!(
            "id/reference column can only be compared to a string IRI, found `{other:?}`"
        ))),
    }
}

/// Convert a DataFusion literal to a TerminusDB XSD value.
fn scalar_to_xsd(sv: &ScalarValue) -> Result<XSDAnySimpleType> {
    use XSDAnySimpleType as X;
    Ok(match sv {
        ScalarValue::Utf8(Some(s))
        | ScalarValue::LargeUtf8(Some(s))
        | ScalarValue::Utf8View(Some(s)) => X::String(s.clone()),
        ScalarValue::Boolean(Some(b)) => X::Boolean(*b),
        ScalarValue::Int8(Some(v)) => X::Integer(*v as i64),
        ScalarValue::Int16(Some(v)) => X::Integer(*v as i64),
        ScalarValue::Int32(Some(v)) => X::Integer(*v as i64),
        ScalarValue::Int64(Some(v)) => X::Integer(*v),
        ScalarValue::UInt8(Some(v)) => X::UnsignedInt(*v as usize),
        ScalarValue::UInt16(Some(v)) => X::UnsignedInt(*v as usize),
        ScalarValue::UInt32(Some(v)) => X::UnsignedInt(*v as usize),
        ScalarValue::UInt64(Some(v)) => X::UnsignedInt(*v as usize),
        ScalarValue::Float32(Some(v)) => X::Float(*v as f64),
        ScalarValue::Float64(Some(v)) => X::Float(*v),
        ScalarValue::Decimal128(Some(v), _p, scale) => X::Decimal(
            decimal_from_i128(*v, *scale)
                .ok_or_else(|| SqlError::unsupported("un-representable decimal literal"))?,
        ),
        ScalarValue::Date32(Some(d)) => X::Date(
            date_from_days(*d).ok_or_else(|| SqlError::unsupported("out-of-range date literal"))?,
        ),
        ScalarValue::TimestampSecond(Some(v), _) => X::DateTime(datetime_from_micros(*v * 1_000_000)?),
        ScalarValue::TimestampMillisecond(Some(v), _) => {
            X::DateTime(datetime_from_micros(*v * 1_000)?)
        }
        ScalarValue::TimestampMicrosecond(Some(v), _) => X::DateTime(datetime_from_micros(*v)?),
        ScalarValue::TimestampNanosecond(Some(v), _) => X::DateTime(datetime_from_micros(*v / 1_000)?),
        ScalarValue::Time64Microsecond(Some(v)) => X::Time(
            time_from_micros(*v).ok_or_else(|| SqlError::unsupported("out-of-range time literal"))?,
        ),
        ScalarValue::Null => {
            return Err(SqlError::unsupported(
                "NULL literal in a predicate (three-valued logic is out of v1 scope)",
            ))
        }
        other => {
            return Err(SqlError::unsupported(format!(
                "literal of type `{}`",
                other.data_type()
            )))
        }
    })
}

fn decimal_from_i128(v: i128, scale: i8) -> Option<decimal_rs::Decimal> {
    let s = if scale <= 0 {
        format!("{}{}", v, "0".repeat((-scale) as usize))
    } else {
        let neg = v < 0;
        let digits = v.unsigned_abs().to_string();
        let scale = scale as usize;
        let body = if digits.len() > scale {
            let point = digits.len() - scale;
            format!("{}.{}", &digits[..point], &digits[point..])
        } else {
            format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
        };
        if neg {
            format!("-{body}")
        } else {
            body
        }
    };
    s.parse().ok()
}

fn date_from_days(d: i32) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?.checked_add_signed(chrono::Duration::days(d as i64))
}

fn datetime_from_micros(us: i64) -> Result<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp_micros(us)
        .ok_or_else(|| SqlError::unsupported("out-of-range timestamp literal"))
}

fn time_from_micros(us: i64) -> Option<chrono::NaiveTime> {
    let us = u64::try_from(us).ok()?;
    let secs = (us / 1_000_000) as u32;
    let nanos = ((us % 1_000_000) * 1_000) as u32;
    chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos)
}

/// Collect every column name referenced anywhere in the plan (all node
/// expressions: projection, filter, join keys, sort). Name-based.
fn collect_referenced(plan: &LogicalPlan, out: &mut HashSet<String>) {
    for expr in plan.expressions() {
        for c in expr.column_refs() {
            out.insert(c.name.clone());
        }
    }
    for input in plan.inputs() {
        collect_referenced(input, out);
    }
}

/// A short human-readable name for an unsupported relational operator.
fn plan_kind(plan: &LogicalPlan) -> String {
    plan.display().to_string()
}

/// Collect the names of all columns referenced by Filter/Join goals (over the
/// whole plan). Name-based on purpose: over-approximating "constrained" keeps a
/// filtered column from ever being wrapped in `Optional`.
fn collect_constrained(plan: &LogicalPlan, out: &mut HashSet<String>) {
    match plan {
        LogicalPlan::Filter(f) => {
            for c in f.predicate.column_refs() {
                out.insert(c.name.clone());
            }
            collect_constrained(&f.input, out);
        }
        LogicalPlan::Join(j) => {
            for (l, r) in &j.on {
                for c in l.column_refs() {
                    out.insert(c.name.clone());
                }
                for c in r.column_refs() {
                    out.insert(c.name.clone());
                }
            }
            if let Some(f) = &j.filter {
                for c in f.column_refs() {
                    out.insert(c.name.clone());
                }
            }
            collect_constrained(&j.left, out);
            collect_constrained(&j.right, out);
        }
        other => {
            for input in other.inputs() {
                collect_constrained(input, out);
            }
        }
    }
}

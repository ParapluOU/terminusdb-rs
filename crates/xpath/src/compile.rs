//! Compile the [`ir`] subset into a `terminusdb_woql2::Query`.
//!
//! Each navigation step emits exactly one WOQL clause into an enclosing `And`:
//! - `Child`/`Attribute` -> a `Triple(subject, predicate, freshObject)`; the
//!   fresh object variable becomes the subject of the next step.
//! - `Descendant` (`//`) -> a `Path(subject, star(any)/pred, freshObject)`.
//!
//! The object variable of the final step is projected via `Select`.

use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::prelude::*;

use crate::error::{Result, XPathError};
use crate::ir;

/// Options controlling how XPath names map onto WOQL predicates.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Prefix applied to unqualified property names. TerminusDB user-defined
    /// properties live in the `@schema:` prefix (e.g. `@schema:name`), so an
    /// XPath step `name` becomes the WOQL predicate `@schema:name`. Names that
    /// already contain a `:` (e.g. `rdf:type`) are left untouched.
    pub property_prefix: String,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            property_prefix: "@schema:".to_string(),
        }
    }
}

/// The result of compiling an XPath expression.
#[derive(Debug, Clone)]
pub struct CompiledXPath {
    /// The WOQL query. Project variable [`Self::result_var`] holds the selected
    /// value(s). This query is *not* wrapped in `Using`; see [`Self::using_db`]
    /// and [`Self::into_using_query`].
    pub query: Query,
    /// The variable name (bare, no prefix) that holds the selected result.
    pub result_var: String,
    /// The database named by a leading `db("...")`, if any. Callers typically
    /// turn this into a `BranchSpec` when executing.
    pub using_db: Option<String>,
}

impl CompiledXPath {
    /// Wrap the query in a WOQL `Using(db, ...)` if a `db("...")` was given,
    /// producing a self-contained query. (Note: WOQL `Using` expects a resource
    /// descriptor; a bare database name may need qualification server-side.)
    pub fn into_using_query(self) -> Query {
        match self.using_db {
            Some(collection) => Query::Using(Using {
                collection,
                query: Box::new(self.query),
            }),
            None => self.query,
        }
    }

    /// Turn the query into one that reads the selected **node** as a full
    /// document, bound to `doc_var`.
    ///
    /// Use this when the path ends at a document/subdocument node (a child step
    /// or a bare `document(...)`) and you want the whole model back: run the
    /// returned query and deserialize each `doc_var` binding with
    /// `M::from_json` (`M: FromTDBInstance`). For paths that end at a value
    /// (`@attr`) the read will simply not bind.
    pub fn read_documents_query(&self, doc_var: &str) -> Query {
        // Reuse the existing pattern; append a ReadDocument on the result node
        // and re-project the document variable.
        let mut clauses = match &self.query {
            Query::Select(sel) => match &*sel.query {
                Query::And(and) => and.and.clone(),
                other => vec![other.clone()],
            },
            other => vec![other.clone()],
        };
        clauses.push(Query::ReadDocument(ReadDocument {
            identifier: NodeValue::Variable(self.result_var.clone()),
            document: Value::Variable(doc_var.to_string()),
        }));
        Query::Select(Select {
            variables: vec![doc_var.to_string()],
            query: Box::new(Query::And(And { and: clauses })),
        })
    }
}

/// Mints fresh, unique WOQL variable names for a single compilation.
struct Ctx {
    counter: u32,
}

impl Ctx {
    fn fresh(&mut self, prefix: &str) -> String {
        let name = format!("{prefix}{}", self.counter);
        self.counter += 1;
        name
    }
}

fn head_db(head: &ir::ContextHead) -> Option<String> {
    match head {
        ir::ContextHead::Document { db, .. } | ir::ContextHead::Root { db } => db.clone(),
    }
}

/// Build the `And`-clauses and the result variable for one path, minting fresh
/// variables from `ctx`. Sharing `ctx` across union branches keeps their
/// internal variables from colliding.
fn build_path(
    q: &ir::XPathQuery,
    opts: &CompileOptions,
    ctx: &mut Ctx,
) -> Result<(Vec<Query>, String)> {
    let mut subject = match &q.head {
        ir::ContextHead::Document { id, .. } => NodeValue::Node(id.clone()),
        ir::ContextHead::Root { .. } => NodeValue::Variable(ctx.fresh("root")),
    };

    if q.steps.is_empty() {
        // A bare `document("Id")` selects the document node itself: bind the
        // result variable to it (via `eq`) so it can be read as a full model.
        return match &q.head {
            ir::ContextHead::Document { id, .. } => {
                let r = ctx.fresh("x");
                Ok((
                    vec![Query::Equals(Equals {
                        left: Value::Variable(r.clone()),
                        right: Value::Node(id.clone()),
                    })],
                    r,
                ))
            }
            ir::ContextHead::Root { .. } => Err(XPathError::unsupported(
                "a bare db()/relative selection with no navigation steps selects nothing",
            )),
        };
    }

    let mut clauses = Vec::new();
    let mut result_var = String::new();
    for step in &q.steps {
        let obj = emit_step(ctx, &mut clauses, subject.clone(), step, opts)?;
        subject = NodeValue::Variable(obj.clone());
        result_var = obj;
    }
    Ok((clauses, result_var))
}

/// Compile an [`ir::XPathQuery`] into WOQL.
pub fn compile(q: &ir::XPathQuery, opts: &CompileOptions) -> Result<CompiledXPath> {
    let mut ctx = Ctx { counter: 0 };
    let (clauses, result_var) = build_path(q, opts, &mut ctx)?;
    let query = Query::Select(Select {
        variables: vec![result_var.clone()],
        query: Box::new(Query::And(And { and: clauses })),
    });
    Ok(CompiledXPath {
        query,
        result_var,
        using_db: head_db(&q.head),
    })
}

/// Compile a **union** of paths (`a | b | …`) into WOQL: every branch binds one
/// shared result variable, combined with `Or`. Branches share a variable
/// counter so their internal variables don't clash across the disjunction.
pub fn compile_union(branches: &[ir::XPathQuery], opts: &CompileOptions) -> Result<CompiledXPath> {
    match branches {
        [] => Err(XPathError::Empty),
        [single] => compile(single, opts),
        _ => {
            let mut ctx = Ctx { counter: 0 };
            let result = ctx.fresh("u");
            let mut or_branches = Vec::with_capacity(branches.len());
            for branch in branches {
                let (mut clauses, rv) = build_path(branch, opts, &mut ctx)?;
                clauses.push(Query::Equals(Equals {
                    left: Value::Variable(result.clone()),
                    right: Value::Variable(rv),
                }));
                or_branches.push(Query::And(And { and: clauses }));
            }
            let query = Query::Select(Select {
                variables: vec![result.clone()],
                query: Box::new(Query::Or(Or { or: or_branches })),
            });
            Ok(CompiledXPath {
                query,
                result_var: result,
                using_db: head_db(&branches[0].head),
            })
        }
    }
}

/// Emit the WOQL clause(s) for one step (the edge plus any predicates) and
/// return the fresh variable bound to the step's object node.
fn emit_step(
    ctx: &mut Ctx,
    clauses: &mut Vec<Query>,
    subject: NodeValue,
    step: &ir::Step,
    opts: &CompileOptions,
) -> Result<String> {
    let obj = ctx.fresh("x");

    match step.axis {
        ir::Axis::Child | ir::Axis::Attribute => {
            let predicate = match &step.test {
                ir::NodeTest::Name(n) => NodeValue::Node(predicate_iri(n, opts)),
                ir::NodeTest::Wildcard => NodeValue::Variable(ctx.fresh("p")),
            };
            clauses.push(Query::Triple(Triple {
                subject,
                predicate,
                object: Value::Variable(obj.clone()),
                // Intentionally left as the default graph; woql2's serialization
                // fills in the `instance` default (see Query::to_woql_json).
                graph: None,
            }));
        }
        ir::Axis::Descendant => {
            // `//foo` = reach `foo` via any chain of edges. The "any edge" star
            // is a `PathPredicate` with no predicate (an omitted predicate is the
            // any-predicate wildcard; woql2's serialization omits the null).
            let pattern = descendant_pattern(&step.test, opts);
            clauses.push(Query::Path(Path {
                subject: node_to_value(subject),
                pattern,
                object: Value::Variable(obj.clone()),
                path: None,
            }));
        }
    }

    for pred in &step.predicates {
        emit_predicate(ctx, clauses, &obj, pred, opts)?;
    }

    Ok(obj)
}

/// Emit clauses for the steps of a relative path, starting from `subject`,
/// returning the variable bound to the final node.
fn chain_steps(
    ctx: &mut Ctx,
    clauses: &mut Vec<Query>,
    mut subject: NodeValue,
    steps: &[ir::Step],
    opts: &CompileOptions,
) -> Result<String> {
    let mut last = match &subject {
        NodeValue::Variable(v) => v.clone(),
        NodeValue::Node(n) => n.clone(),
    };
    for step in steps {
        let obj = emit_step(ctx, clauses, subject.clone(), step, opts)?;
        subject = NodeValue::Variable(obj.clone());
        last = obj;
    }
    Ok(last)
}

fn emit_predicate(
    ctx: &mut Ctx,
    clauses: &mut Vec<Query>,
    subject_var: &str,
    pred: &ir::Predicate,
    opts: &CompileOptions,
) -> Result<()> {
    let subject = NodeValue::Variable(subject_var.to_string());
    match pred {
        ir::Predicate::Exists { rel } => {
            chain_steps(ctx, clauses, subject, rel, opts)?;
        }
        ir::Predicate::Compare { rel, op, value } => {
            let pv = chain_steps(ctx, clauses, subject, rel, opts)?;
            clauses.push(comparison(op, pv, value));
        }
    }
    Ok(())
}

fn comparison(op: &ir::CmpOp, var: String, value: &ir::Literal) -> Query {
    let xsd = literal_to_xsd(value);
    match op {
        ir::CmpOp::Eq => Query::Equals(Equals {
            left: Value::Variable(var),
            right: Value::Data(xsd),
        }),
        ir::CmpOp::Ne => Query::Not(Not {
            query: Box::new(Query::Equals(Equals {
                left: Value::Variable(var),
                right: Value::Data(xsd),
            })),
        }),
        ir::CmpOp::Lt => Query::Less(Less {
            left: DataValue::Variable(var),
            right: DataValue::Data(xsd),
        }),
        ir::CmpOp::Le => Query::Lte(Lte {
            left: DataValue::Variable(var),
            right: DataValue::Data(xsd),
        }),
        ir::CmpOp::Gt => Query::Greater(Greater {
            left: DataValue::Variable(var),
            right: DataValue::Data(xsd),
        }),
        ir::CmpOp::Ge => Query::Gte(Gte {
            left: DataValue::Variable(var),
            right: DataValue::Data(xsd),
        }),
    }
}

/// `//test` = descendant-or-self::node()/test. As a WOQL path pattern:
/// `Name(n)` -> `(any*) . n`; `*` -> `any+`. "any" is a `PathPredicate` with no
/// predicate (serialized as an omitted key — the any-predicate wildcard).
fn descendant_pattern(test: &ir::NodeTest, opts: &CompileOptions) -> PathPattern {
    let any_star = PathPattern::Star(PathStar {
        star: Box::new(PathPattern::Predicate(PathPredicate { predicate: None })),
    });
    match test {
        ir::NodeTest::Name(n) => PathPattern::Sequence(PathSequence {
            sequence: vec![
                any_star,
                PathPattern::Predicate(PathPredicate {
                    predicate: Some(predicate_iri(n, opts)),
                }),
            ],
        }),
        ir::NodeTest::Wildcard => PathPattern::Plus(PathPlus {
            plus: Box::new(PathPattern::Predicate(PathPredicate { predicate: None })),
        }),
    }
}

/// Map an XPath property name to a WOQL predicate IRI, applying the configured
/// schema prefix to unqualified names (names without a `:`).
fn predicate_iri(name: &str, opts: &CompileOptions) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("{}{}", opts.property_prefix, name)
    }
}

fn node_to_value(n: NodeValue) -> Value {
    match n {
        NodeValue::Node(s) => Value::Node(s),
        NodeValue::Variable(s) => Value::Variable(s),
    }
}

fn literal_to_xsd(lit: &ir::Literal) -> XSDAnySimpleType {
    match lit {
        ir::Literal::Str(s) => XSDAnySimpleType::String(s.clone()),
        ir::Literal::Int(i) => XSDAnySimpleType::Integer(*i),
        ir::Literal::Float(f) => XSDAnySimpleType::Float(*f),
    }
}

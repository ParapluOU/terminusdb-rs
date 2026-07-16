//! A small, explicit intermediate representation for the subset of SPARQL we
//! compile to WOQL.
//!
//! We deliberately do *not* lower the full `spargebra` algebra directly to WOQL.
//! Instead we first project it onto this narrow IR. That keeps the supported
//! subset obvious, decouples the compiler from spargebra's full SPARQL 1.1
//! generality, and gives a clean seam for testing (`str -> IR` and `IR -> Query`).
//!
//! The IR mirrors SPARQL's own algebra, restricted to the parts that have a
//! faithful WOQL image:
//!
//! - a [`GraphPattern`] tree (basic graph patterns, join, optional, union,
//!   filter) — the `WHERE` clause,
//! - a set of solution modifiers (projection, `DISTINCT`, `ORDER BY`,
//!   `LIMIT`/`OFFSET`) hoisted onto [`SparqlQuery`].

/// A compiled-down SPARQL `SELECT` query: a graph pattern plus solution
/// modifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct SparqlQuery {
    /// The `WHERE` graph pattern.
    pub pattern: GraphPattern,
    /// The projected variables (bare names, no leading `?`). Empty means
    /// `SELECT *`; the compiler then projects every variable in `pattern`.
    pub projection: Vec<String>,
    /// Whether `DISTINCT` was requested.
    pub distinct: bool,
    /// `ORDER BY` keys, outermost first.
    pub order: Vec<OrderKey>,
    /// `LIMIT`, if present.
    pub limit: Option<usize>,
    /// `OFFSET` (0 if absent).
    pub offset: usize,
}

/// A SPARQL graph pattern (the recursive `WHERE` algebra).
#[derive(Debug, Clone, PartialEq)]
pub enum GraphPattern {
    /// A basic graph pattern: a conjunction of triple patterns.
    Bgp(Vec<TriplePattern>),
    /// Conjunction of two patterns (`{ A } { B }`).
    Join(Box<GraphPattern>, Box<GraphPattern>),
    /// `A OPTIONAL { B }` — a left join, optionally guarded by a `FILTER`.
    Optional(Box<GraphPattern>, Box<GraphPattern>, Option<Expr>),
    /// `{ A } UNION { B }`.
    Union(Box<GraphPattern>, Box<GraphPattern>),
    /// `A FILTER(expr)`.
    Filter(Expr, Box<GraphPattern>),
    /// The empty pattern — matches exactly one (empty) solution.
    Empty,
}

/// A single triple pattern `subject predicate object`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriplePattern {
    pub subject: Term,
    pub predicate: Term,
    pub object: Term,
}

/// A term in a triple pattern: a variable, an IRI, or a literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// A query variable (bare name, no leading `?`).
    Var(String),
    /// An IRI / named node (as written; IRI→WOQL prefix mapping happens at
    /// compile time).
    Iri(String),
    /// A literal (only valid in object position).
    Literal(Literal),
}

/// A SPARQL literal, coarsely typed by the parts WOQL can represent.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// A plain / `xsd:string` (or language-tagged) string.
    Str(String),
    /// `xsd:integer` / `xsd:int` / `xsd:long` / ...
    Int(i64),
    /// `xsd:double` / `xsd:float`.
    Double(f64),
    /// `xsd:decimal` (kept lexical; parsed at compile time).
    Decimal(String),
    /// `xsd:boolean`.
    Bool(bool),
    /// Any other datatype, kept as `(lexical value, datatype IRI)`.
    Typed(String, String),
}

/// An `ORDER BY` key.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderKey {
    /// The variable to order by (bare name).
    pub var: String,
    /// `true` for `DESC(...)`, `false` for ascending.
    pub desc: bool,
}

/// A `FILTER` expression subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A variable reference.
    Var(String),
    /// A literal constant.
    Lit(Literal),
    /// An IRI constant.
    Iri(String),
    /// `a = b` (RDFterm-equal).
    Eq(Box<Expr>, Box<Expr>),
    /// `a != b`.
    Ne(Box<Expr>, Box<Expr>),
    /// `a < b`.
    Lt(Box<Expr>, Box<Expr>),
    /// `a <= b`.
    Le(Box<Expr>, Box<Expr>),
    /// `a > b`.
    Gt(Box<Expr>, Box<Expr>),
    /// `a >= b`.
    Ge(Box<Expr>, Box<Expr>),
    /// `a && b`.
    And(Box<Expr>, Box<Expr>),
    /// `a || b`.
    Or(Box<Expr>, Box<Expr>),
    /// `!a`.
    Not(Box<Expr>),
}

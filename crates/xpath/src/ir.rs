//! A small, explicit intermediate representation for the subset of XPath we
//! compile to WOQL.
//!
//! We deliberately do *not* lower the full `xee-xpath-ast` AST directly to WOQL.
//! Instead we first project it onto this narrow IR. That keeps the supported
//! subset obvious, decouples the compiler from xee's full XPath 3.1 generality,
//! and gives a clean seam for testing (`str -> IR` and `IR -> Query`).

/// A compiled-down XPath location path: a starting context followed by an
/// ordered sequence of navigation steps.
#[derive(Debug, Clone, PartialEq)]
pub struct XPathQuery {
    /// Where navigation begins (which database, which subject node).
    pub head: ContextHead,
    /// The navigation steps applied from the head, left to right.
    pub steps: Vec<Step>,
}

/// The leading "context" of a path, established by TerminusDB pseudo-functions
/// such as `db("name")` and `document("MyModel/1234")`.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextHead {
    /// Start from a specific document IRI, optionally within a named database.
    Document { db: Option<String>, id: String },
    /// Start from the (unbound) document set, optionally within a named database.
    /// Compiles to a fresh subject variable that matches any document.
    Root { db: Option<String> },
}

/// A single navigation step: an axis, a node test, and zero or more predicates.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub axis: Axis,
    pub test: NodeTest,
    pub predicates: Vec<Predicate>,
}

/// The supported axis subset. `//foo` folds the synthetic
/// `descendant-or-self::node()` step into the following step as [`Axis::Descendant`].
#[derive(Debug, Clone, PartialEq)]
pub enum Axis {
    /// `foo` — follow an object property (a graph hop / link).
    Child,
    /// `@foo` — read a value property (a literal / data edge).
    Attribute,
    /// `//foo` — reachable via any chain of edges then `foo`.
    Descendant,
}

/// The supported node-test subset.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeTest {
    /// A named property/predicate, e.g. `submodel`.
    Name(String),
    /// `*` — any property/predicate.
    Wildcard,
}

/// A predicate `[...]` attached to a step, evaluated against that step's node.
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    /// `[rel op literal]`, e.g. `[@name = "Jane"]` or `[age > 21]`.
    Compare {
        rel: Vec<Step>,
        op: CmpOp,
        value: Literal,
    },
    /// `[rel]` — the relative path must exist, e.g. `[submodel]`.
    Exists { rel: Vec<Step> },
}

/// Comparison operators supported in predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A literal value that can appear on the right-hand side of a predicate.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Int(i64),
    Float(f64),
}

impl From<&str> for Literal {
    fn from(s: &str) -> Self {
        Literal::Str(s.to_string())
    }
}
impl From<String> for Literal {
    fn from(s: String) -> Self {
        Literal::Str(s)
    }
}
impl From<i64> for Literal {
    fn from(i: i64) -> Self {
        Literal::Int(i)
    }
}
impl From<i32> for Literal {
    fn from(i: i32) -> Self {
        Literal::Int(i as i64)
    }
}
impl From<f64> for Literal {
    fn from(f: f64) -> Self {
        Literal::Float(f)
    }
}

//! A type-safe builder for XPath expressions, with `/` operator overloading.
//!
//! Construct paths in Rust without strings and compile them straight to WOQL
//! (bypassing the text parser). Navigation uses `/`; predicates use `.filter()`.
//!
//! ```ignore
//! use terminusdb_xpath::builder::{doc, child, attr, descendant};
//!
//! // document("Person/jane")/employer[@founded > 1990]/@name
//! let compiled = (doc::<Person>(jane_id)
//!     / child("employer").filter(attr("founded").gt(1990))
//!     / attr("name"))
//!     .compile()?;
//!
//! // a relative path: employer/@name
//! let compiled = (child("employer") / attr("name")).compile()?;
//!
//! // descendant (there is no `//` — it is a Rust line comment):
//! let compiled = (doc::<Company>(acme) / descendant("city")).compile()?;
//! ```
//!
//! `[...]` predicate syntax is deliberately *not* supported: Rust's `Index`
//! (`a[b]`) must return a reference into `a`, so it can't grow a builder. Use
//! `.filter(...)` instead.

use std::ops::{Div, Shr};

use terminusdb_schema::{EntityIDFor, ToTDBSchema};

use crate::compile::{CompileOptions, CompiledXPath};
use crate::error::Result;
use crate::ir;

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Start a path at a specific, **typed** document: `doc::<Person>(id)`.
///
/// `id` is anything convertible into `EntityIDFor<T>` (an `EntityIDFor<T>`
/// itself, most simply), so the starting IRI comes from the typed id — never a
/// hand-formatted string.
pub fn doc<T: ToTDBSchema>(id: impl Into<EntityIDFor<T>>) -> XPath {
    XPath {
        head: ir::ContextHead::Document {
            db: None,
            id: id.into().typed().to_string(),
        },
        steps: Vec::new(),
    }
}

/// A child step (object property): `child("employer")` ≡ `employer` / `/employer`.
pub fn child(name: impl Into<String>) -> XPath {
    relative(ir::Axis::Child, name)
}

/// An attribute step (value property): `attr("name")` ≡ `@name` / `/@name`.
pub fn attr(name: impl Into<String>) -> XPath {
    relative(ir::Axis::Attribute, name)
}

/// A descendant step: `descendant("city")` ≡ `//city`.
pub fn descendant(name: impl Into<String>) -> XPath {
    relative(ir::Axis::Descendant, name)
}

fn relative(axis: ir::Axis, name: impl Into<String>) -> XPath {
    XPath {
        head: ir::ContextHead::Root { db: None },
        steps: vec![named(axis, name)],
    }
}

// ---------------------------------------------------------------------------
// The path / fragment type
// ---------------------------------------------------------------------------

/// A path expression. Doubles as a *relative path fragment* (the right-hand side
/// of `/`) and as a *predicate relative path* (via [`XPath::eq`] etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct XPath {
    head: ir::ContextHead,
    steps: Vec<ir::Step>,
}

/// `a / b` — append `b`'s steps onto `a` (keeping `a`'s document/db context).
impl Div for XPath {
    type Output = XPath;
    fn div(mut self, rhs: XPath) -> XPath {
        self.steps.extend(rhs.steps);
        self
    }
}

/// `a >> b` — descendant append: like `/`, but `b`'s first step becomes a
/// descendant step. This is the operator form of `//` (which cannot be written
/// literally — `//` is a Rust line comment). `a >> child("city")` ≡ `a//city`.
///
/// `>>` binds looser than `/`, so `doc(id) / child("a") >> child("b") / attr("c")`
/// groups as `(doc(id)/a) >> (b/@c)` = `a//b/@c` — usually what you want.
impl Shr for XPath {
    type Output = XPath;
    fn shr(mut self, mut rhs: XPath) -> XPath {
        if let Some(first) = rhs.steps.first_mut() {
            first.axis = ir::Axis::Descendant;
        }
        self.steps.extend(rhs.steps);
        self
    }
}

impl XPath {
    /// Select the database this path runs against (`db("name")/...`).
    pub fn in_db(mut self, name: impl Into<String>) -> Self {
        match &mut self.head {
            ir::ContextHead::Document { db, .. } | ir::ContextHead::Root { db } => {
                *db = Some(name.into())
            }
        }
        self
    }

    /// Append a child step (method form of `/ child(name)`).
    pub fn child(mut self, name: impl Into<String>) -> Self {
        self.steps.push(named(ir::Axis::Child, name));
        self
    }

    /// Append an attribute step.
    pub fn attr(mut self, name: impl Into<String>) -> Self {
        self.steps.push(named(ir::Axis::Attribute, name));
        self
    }

    /// Append a descendant step.
    pub fn descendant(mut self, name: impl Into<String>) -> Self {
        self.steps.push(named(ir::Axis::Descendant, name));
        self
    }

    /// Attach a predicate to the most recent step (`.../name[predicate]`).
    pub fn filter(mut self, predicate: Predicate) -> Self {
        if let Some(last) = self.steps.last_mut() {
            last.predicates.push(predicate.0);
        }
        self
    }

    // --- predicate producers: treat this path's steps as a relative path and
    //     compare it to a literal (used inside `.filter(...)`) ---

    /// `path = value`
    pub fn eq(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Eq, value)
    }
    /// `path != value`
    pub fn ne(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Ne, value)
    }
    /// `path < value`
    pub fn lt(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Lt, value)
    }
    /// `path <= value`
    pub fn le(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Le, value)
    }
    /// `path > value`
    pub fn gt(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Gt, value)
    }
    /// `path >= value`
    pub fn ge(self, value: impl Into<ir::Literal>) -> Predicate {
        self.cmp(ir::CmpOp::Ge, value)
    }
    /// `[path]` — the relative path must exist.
    pub fn exists(self) -> Predicate {
        Predicate(ir::Predicate::Exists { rel: self.steps })
    }

    fn cmp(self, op: ir::CmpOp, value: impl Into<ir::Literal>) -> Predicate {
        Predicate(ir::Predicate::Compare {
            rel: self.steps,
            op,
            value: value.into(),
        })
    }

    /// The IR this builder represents.
    pub fn to_ir(&self) -> ir::XPathQuery {
        ir::XPathQuery {
            head: self.head.clone(),
            steps: self.steps.clone(),
        }
    }

    /// Compile to WOQL with default [`CompileOptions`].
    pub fn compile(&self) -> Result<CompiledXPath> {
        self.compile_with(&CompileOptions::default())
    }

    /// Compile to WOQL with explicit [`CompileOptions`].
    pub fn compile_with(&self, opts: &CompileOptions) -> Result<CompiledXPath> {
        crate::compile::compile(&self.to_ir(), opts)
    }
}

/// A predicate attached to a step via [`XPath::filter`].
#[derive(Debug, Clone, PartialEq)]
pub struct Predicate(ir::Predicate);

fn named(axis: ir::Axis, name: impl Into<String>) -> ir::Step {
    ir::Step {
        axis,
        test: ir::NodeTest::Name(name.into()),
        predicates: Vec::new(),
    }
}

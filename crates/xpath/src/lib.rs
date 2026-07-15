//! Compile XPath expressions into TerminusDB WOQL queries.
//!
//! TerminusDB's canonical query language is WOQL. This crate lets you navigate
//! TerminusDB documents/graphs with familiar XPath syntax by compiling an XPath
//! expression **entirely** into a [`terminusdb_woql2::prelude::Query`] value —
//! no Prolog, no server extension. The resulting query runs through the normal
//! client (`client.query(spec, compiled.query)`).
//!
//! # Pipeline
//!
//! ```text
//! XPath string --[xee-xpath-ast]--> xee AST --[lower]--> ir::XPathQuery --[compile]--> woql2::Query
//! ```
//!
//! We reuse the spec-grade [`xee-xpath-ast`](https://crates.io/crates/xee-xpath-ast)
//! parser (from the ParapluOU/x-rs bundle, vendored under `modules/x-rs`) rather
//! than hand-writing an XPath parser, then lower a well-defined subset onto WOQL.
//!
//! # Data model mapping
//!
//! - `document("MyModel/1234")` selects a starting document (subject) node.
//!   Both a short id (`MyModel/1234`) and a full IRI
//!   (`terminusdb:///data/MyModel/1234`) are accepted.
//! - `db("name")` selects the database (exposed as [`CompiledXPath::using_db`]).
//! - A child step `foo` follows an **object property** (a graph hop / link).
//! - An attribute step `@foo` reads a **value property** (a literal).
//! - `//foo` (descendant) matches `foo` reachable through any chain of edges
//!   (a WOQL path with a star over the any-predicate wildcard).
//!
//! See `ROADMAP.md` for the full spec-compliance status.
//!
//! # Example
//!
//! ```
//! let compiled = terminusdb_xpath::compile(
//!     r#"document("MyModel/1234")/submodel/@prop"#,
//! )
//! .unwrap();
//! // compiled.query is a woql2::Query ready to execute;
//! // compiled.result_var names the projected result variable.
//! ```

mod compile;
mod error;
mod lower;
mod parse;

pub mod builder;
pub mod debug;
pub mod ir;

pub use compile::{CompileOptions, CompiledXPath};
pub use debug::{explain, Explanation};
pub use error::{Result, XPathError};

/// Compile an XPath expression string into a WOQL query, using default
/// [`CompileOptions`] (the `@schema:` property prefix).
///
/// Returns [`XPathError::Parse`] if xee rejects the syntax, or
/// [`XPathError::Unsupported`] if it uses a construct outside the supported
/// subset (see `ROADMAP.md`).
pub fn compile(expr: &str) -> Result<CompiledXPath> {
    compile_with(expr, &CompileOptions::default())
}

/// Like [`compile`], but with explicit [`CompileOptions`].
pub fn compile_with(expr: &str, opts: &CompileOptions) -> Result<CompiledXPath> {
    let ast = parse::parse(expr)?;
    let query = lower::lower(&ast)?;
    compile::compile(&query, opts)
}

/// Format an XPath expression (like [`format!`]) and [`compile`] it, so the
/// syntax is **checked at runtime** — the result is `Result<CompiledXPath>`, and
/// a malformed or unsupported expression is an `Err`, never a panic.
///
/// This is the string-template escape hatch. For interpolating a document id,
/// prefer the type-safe [`builder`] (`doc::<T>(id)`); values spliced here are
/// formatted verbatim (mind XPath-significant characters).
///
/// ```
/// use terminusdb_xpath::xpath;
///
/// let compiled = xpath!(r#"document("{}")/submodel/@{}"#, "MyModel/1234", "prop")?;
/// assert_eq!(compiled.result_var, "x1");
/// # Ok::<(), terminusdb_xpath::XPathError>(())
/// ```
#[macro_export]
macro_rules! xpath {
    ($($arg:tt)*) => {
        $crate::compile(&format!($($arg)*))
    };
}

/// Parse and lower an XPath expression to the intermediate representation,
/// without compiling to WOQL. Useful for inspection and testing.
pub fn to_ir(expr: &str) -> Result<ir::XPathQuery> {
    let ast = parse::parse(expr)?;
    lower::lower(&ast)
}

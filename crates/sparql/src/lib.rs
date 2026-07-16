//! Compile SPARQL queries into TerminusDB WOQL queries.
//!
//! TerminusDB's canonical query language is WOQL. This crate lets you query
//! TerminusDB documents/graphs with familiar SPARQL syntax by compiling a SPARQL
//! `SELECT` **entirely** into a [`terminusdb_woql2::prelude::Query`] value — no
//! Prolog, no server extension. The resulting query runs through the normal
//! client (`client.query(spec, compiled.query)`).
//!
//! # Pipeline
//!
//! ```text
//! SPARQL string --[spargebra]--> algebra --[lower]--> ir::SparqlQuery --[compile]--> woql2::Query
//! ```
//!
//! We reuse the spec-grade [`spargebra`](https://crates.io/crates/spargebra)
//! parser (from the oxigraph ecosystem) rather than hand-writing a SPARQL
//! parser, then lower a well-defined subset of its algebra onto WOQL. SPARQL and
//! WOQL share the same triple/graph data model, so the mapping is direct: triple
//! patterns become WOQL triples, `OPTIONAL` becomes `Optional`, `UNION` becomes
//! `Or`, `FILTER` comparisons become `Equals`/`Less`/`Greater`/..., and the
//! solution modifiers become the `Select`/`Distinct`/`OrderBy`/`Limit`/`Start`
//! wrappers.
//!
//! # Data model mapping
//!
//! SPARQL uses full IRIs; TerminusDB's instance graph uses `@schema:`-prefixed
//! predicates/classes and `rdf:type`. By default, IRIs under
//! `http://terminusdb.com/schema#` map onto the `@schema:` prefix, so a natural
//! query reads:
//!
//! ```sparql
//! PREFIX schema: <http://terminusdb.com/schema#>
//! SELECT ?name WHERE {
//!   ?person a schema:Person .
//!   ?person schema:name ?name .
//! }
//! ```
//!
//! Here `a` (rdf:type) matches the class, and `schema:name` becomes the
//! `@schema:name` predicate. See [`CompileOptions`] to change the namespace and
//! `ROADMAP.md` for the full spec-coverage status.
//!
//! # Example
//!
//! ```
//! let compiled = terminusdb_sparql::compile(
//!     r#"PREFIX schema: <http://terminusdb.com/schema#>
//!        SELECT ?name WHERE { ?p a schema:Person . ?p schema:name ?name }"#,
//! )
//! .unwrap();
//! // compiled.query is a woql2::Query ready to execute;
//! // compiled.variables names the projected result variables.
//! assert_eq!(compiled.variables, vec!["name".to_string()]);
//! ```

mod compile;
mod error;
mod lower;
mod parse;

pub mod debug;
pub mod ir;

pub use compile::{compile as compile_ir, CompileOptions, CompiledSparql};
pub use debug::{explain, explain_with, Explanation};
pub use error::{Result, SparqlError};

/// Compile a SPARQL query string into a WOQL query, using default
/// [`CompileOptions`] (the `http://terminusdb.com/schema#` -> `@schema:`
/// mapping).
///
/// Returns [`SparqlError::Parse`] if spargebra rejects the syntax,
/// [`SparqlError::UnsupportedForm`] for a non-`SELECT` query, or
/// [`SparqlError::Unsupported`] for a construct outside the supported subset
/// (see `ROADMAP.md`).
pub fn compile(sparql: &str) -> Result<CompiledSparql> {
    compile_with(sparql, &CompileOptions::default())
}

/// Like [`compile`], but with explicit [`CompileOptions`].
pub fn compile_with(sparql: &str, opts: &CompileOptions) -> Result<CompiledSparql> {
    let ast = parse::parse(sparql, None)?;
    let query = lower::lower(&ast)?;
    compile::compile(&query, opts)
}

/// Parse and lower a SPARQL query to the intermediate representation, without
/// compiling to WOQL. Useful for inspection and testing.
pub fn to_ir(sparql: &str) -> Result<ir::SparqlQuery> {
    let ast = parse::parse(sparql, None)?;
    lower::lower(&ast)
}

/// Format a SPARQL query (like [`format!`]) and [`compile`] it, so the syntax is
/// **checked at runtime** — the result is `Result<CompiledSparql>`, and a
/// malformed or unsupported query is an `Err`, never a panic.
///
/// ```
/// use terminusdb_sparql::sparql;
///
/// let compiled = sparql!(
///     r#"PREFIX s: <http://terminusdb.com/schema#>
///        SELECT ?n WHERE {{ ?p a s:{} . ?p s:name ?n }}"#,
///     "Person"
/// )?;
/// assert_eq!(compiled.variables, vec!["n".to_string()]);
/// # Ok::<(), terminusdb_sparql::SparqlError>(())
/// ```
#[macro_export]
macro_rules! sparql {
    ($($arg:tt)*) => {
        $crate::compile(&format!($($arg)*))
    };
}

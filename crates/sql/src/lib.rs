//! Compile SQL queries into TerminusDB WOQL queries.
//!
//! TerminusDB's canonical query language is WOQL. This crate lets you query a
//! TerminusDB database with SQL by compiling a `SELECT` statement **entirely**
//! into a [`terminusdb_woql2::prelude::Query`] value — no SQL is executed
//! in-process; TerminusDB is the only engine. The resulting query runs through
//! the normal client, and result bindings are mapped back to SQL rows.
//!
//! # Pipeline
//!
//! ```text
//! SQL string --[datafusion-sql]--> LogicalPlan --[emit]--> woql2::Query --[client]--> rows
//! ```
//!
//! We reuse DataFusion's *logical* layer (`datafusion-sql`/`-expr`/`-common`) as
//! the parser, name-resolver, type-checker, and IR. DataFusion already rejects
//! unknown tables/columns, ambiguous columns, un-coercible comparisons, and wrong
//! arity — we do **not** reimplement any of that. We consume the resulting
//! `LogicalPlan` and translate it to WOQL, returning [`SqlError::Unsupported`] for
//! any construct we cannot faithfully translate.
//!
//! Our two responsibilities are:
//! 1. A **catalog loader** that mirrors the TerminusDB schema graph so
//!    DataFusion's checks are meaningful (each concrete class → a table).
//! 2. An **emitter** that maps a `LogicalPlan` to WOQL, never approximating.
//!
//! See `ROADMAP.md` for the supported SQL subset.

mod backend;
mod catalog;
mod context;
pub mod debug;
mod emit;
mod error;
mod mangle;
mod meta;
mod runner;
mod session;
mod typemap;

pub use backend::CatalogBackend;
pub use catalog::Catalog;
pub use debug::{explain, Explanation};
pub use emit::{ProjCol, SqlQuery};
pub use error::{Result, SqlError};
pub use meta::{ColumnKind, ColumnMeta, OmitReason, OmittedColumn, TableMeta, IRI_COLUMN};
pub use runner::{QueryResponse, SqlValue};
pub use session::Session;
pub use typemap::{RejectReason, Semantic};

/// Compile a SQL statement into WOQL against a loaded [`Catalog`]. DataFusion
/// plans and type-checks the statement; the emitter translates the plan to WOQL.
pub fn compile_sql(sql: &str, catalog: &Catalog) -> Result<SqlQuery> {
    let plan = catalog.plan(sql)?;
    emit::emit(&plan, catalog)
}

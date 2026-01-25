//! Strongly-typed Rust API for TerminusDB CLI operations.
//!
//! This module provides a type-safe, ergonomic interface to TerminusDB functionality
//! by wrapping the CLI with strongly-typed commands and builder patterns.
//!
//! # Example
//!
//! ```no_run
//! use terminusdb_bin::api::{TerminusDB, DbSpec, ServeOptions};
//!
//! let client = TerminusDB::new();
//!
//! // Start server in memory
//! client.serve(ServeOptions {
//!     memory: Some("root".into()),
//!     ..Default::default()
//! })?;
//!
//! // Create a database
//! let spec = DbSpec::new("admin", "mydb");
//! client.db().create(spec, Default::default())?;
//! # Ok::<(), std::io::Error>(())
//! ```

mod client;
mod commands;
mod options;
mod spec;
mod types;

// Re-export public API
pub use client::TerminusDB;
pub use options::*;
pub use spec::{BranchSpec, CommitSpec, DbSpec, GraphSpec, GraphType};
pub use types::{Author, CommitType, Message, RdfFormat, RoleAction, ScopeType};

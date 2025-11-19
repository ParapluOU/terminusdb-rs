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

mod spec;
mod types;
mod options;
mod client;
mod commands;

// Re-export public API
pub use spec::{DbSpec, GraphSpec, BranchSpec, CommitSpec, GraphType};
pub use types::{Author, Message, RoleAction, RdfFormat};
pub use options::*;
pub use client::TerminusDB;

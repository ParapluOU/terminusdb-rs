//! TerminusDB HTTP Client Module
//!
//! This module provides a modular HTTP client for interacting with TerminusDB.
//! The client is organized into logical submodules for better maintainability.
//!
//! ## Module Organization
//!
//! - `client`: Core client struct and constructors
//! - `database`: Database administration operations
//! - `schema`: Schema-related operations
//! - `document`: Untyped document CRUD operations
//! - `instance`: Strongly-typed instance operations
//! - `query`: Query execution and WOQL operations
//! - `log`: Log and commit tracking operations
//! - `response`: Response parsing utilities
//! - `url_builder`: URL construction utilities
//! - `helpers`: Helper functions

// Public modules
pub mod client;
pub mod database;
pub mod document;
pub mod helpers;
pub mod instance;
pub mod insert_result;
pub mod versions;
pub mod log;
pub mod query;
pub mod response;
pub mod schema;
pub mod url_builder;

// Re-export main types and traits
pub use client::TerminusDBHttpClient;
pub use document::DeleteOpts;
pub use helpers::{dedup_documents_by_id, dedup_instances_by_id, dump_failed_payload, dump_json, dump_schema, format_id};
pub use insert_result::InsertInstanceResult;
pub use url_builder::UrlBuilder;
pub use terminusdb_schema::TerminusDBModel;

// Re-export commonly used types
pub type EntityID = String;


#[derive(Debug, Clone, PartialEq)]
pub enum TDBInsertInstanceResult {
    /// inserted entity, returning ID
    Inserted(String),
    /// entity already exists, returning ID
    AlreadyExists(String),
}
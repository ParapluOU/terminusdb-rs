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
pub mod log;
pub mod query;
pub mod response;
pub mod schema;
pub mod url_builder;

// Re-export main types and traits
pub use client::TerminusDBHttpClient;
pub use helpers::{dedup_documents_by_id, dedup_instances_by_id, dump_failed_payload, dump_json, dump_schema, format_id};
pub use url_builder::UrlBuilder;

// Re-export commonly used types
pub type EntityID = String;

/// Trait alias for strongly typed TerminusDB models.
///
/// This represents any type that can be converted to a TerminusDB instance,
/// is debuggable, and serializable. Use this for functions that accept
/// TerminusDB models to make the API clearer and distinguish from untyped documents.
///
/// # Example
/// ```rust
/// use terminusdb_client::TerminusDBModel;
/// 
/// async fn insert_my_model<T: TerminusDBModel>(client: &TerminusDBHttpClient, model: &T) {
///     client.insert_instance(model, args).await.unwrap();
/// }
/// ```
pub trait TerminusDBModel = terminusdb_schema::ToTDBInstance + std::fmt::Debug + serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum TDBInsertInstanceResult {
    /// inserted entity, returning ID
    Inserted(String),
    /// entity already exists, returning ID
    AlreadyExists(String),
}
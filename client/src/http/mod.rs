//! TerminusDB HTTP Client Module
//!
//! This module provides a modular HTTP client for interacting with TerminusDB.
//! The client is organized into logical submodules for better maintainability.
//!
//! ## Module Organization
//!
//! - `client`: Core client struct and constructors
//! - `branch`: Branch management operations (squash, reset, rebase)
//! - `collaboration`: Collaboration operations (fetch, push, pull, clone)
//! - `database`: Database administration operations
//! - `diff`: Diff and patch operations
//! - `schema`: Schema-related operations
//! - `document`: Untyped document CRUD operations
//! - `instance`: Strongly-typed instance operations
//! - `query`: Query execution and WOQL operations
//! - `log`: Log and commit tracking operations
//! - `organization`: Organization management operations
//! - `remote`: Remote repository management
//! - `response`: Response parsing utilities
//! - `role`: Role management operations
//! - `url_builder`: URL construction utilities
//! - `user`: User management operations
//! - `helpers`: Helper functions
//! - `graphql`: GraphQL query execution and introspection

// Public modules
pub mod branch;
pub mod client;
pub mod collaboration;
pub mod database;
pub mod diff;
pub mod document;
pub mod graphql;
pub mod helpers;
pub mod insert_result;
pub mod instance;
pub mod log;
pub mod organization;
pub mod query;
pub mod remote;
pub mod response;
pub mod role;
pub mod schema;
pub mod url_builder;
pub mod user;
pub mod versions;

// Re-export main types and traits
pub use client::TerminusDBHttpClient;
pub use document::DeleteOpts;
pub use graphql::{GraphQLRequest, GraphQLResponse, GraphQLError};
pub use helpers::{
    dedup_documents_by_id, dedup_instances_by_id, dump_failed_payload, dump_json, dump_schema,
    format_id,
};
pub use insert_result::InsertInstanceResult;
pub use organization::{Organization, OrganizationMember};
pub use remote::{RemoteConfig, RemoteInfo};
pub use role::{Role, Permission};
pub use terminusdb_schema::TerminusDBModel;
pub use url_builder::UrlBuilder;
pub use user::User;

// Re-export commonly used types
pub type EntityID = String;

#[derive(Debug, Clone, PartialEq)]
pub enum TDBInsertInstanceResult {
    /// inserted entity, returning ID
    Inserted(String),
    /// entity already exists, returning ID
    AlreadyExists(String),
}

impl TDBInsertInstanceResult {
    /// Get the ID regardless of whether it was inserted or already existed
    pub fn id(&self) -> &str {
        match self {
            TDBInsertInstanceResult::Inserted(id) => id,
            TDBInsertInstanceResult::AlreadyExists(id) => id,
        }
    }
    
    /// Parse the ID into a TdbIRI
    pub fn get_iri(&self) -> anyhow::Result<terminusdb_schema::TdbIRI> {
        terminusdb_schema::TdbIRI::parse(self.id())
    }
    
    /// Extract the type name and ID parts
    /// Returns (type_name, id)
    pub fn get_parts(&self) -> anyhow::Result<(String, String)> {
        let iri = self.get_iri()?;
        Ok((iri.type_name().to_string(), iri.id().to_string()))
    }
}

//! GraphQL schema generation for TerminusDB models.
//!
//! This crate provides functionality to generate GraphQL schemas from
//! TerminusDB model definitions, leveraging the actual TerminusDB GraphQL
//! generation code from terminusdb-community.
//!
//! # Example
//!
//! ```ignore
//! use terminusdb_gql::generate_gql_schema;
//! use my_models::{Project, Ticket};
//!
//! // Generate GraphQL SDL from models
//! let sdl = generate_gql_schema::<(Project, Ticket)>();
//! println!("{}", sdl);
//! ```

pub mod codegen;
mod frames;
mod schema;

pub use codegen::{generate_all, generate_filter_impls, generate_filter_types, ModelConfig};
pub use frames::{schemas_to_allframes, schemas_vec_to_allframes};
pub use schema::{allframes_to_sdl, generate_gql_schema};

// Re-export key types from terminusdb-community
pub use terminusdb_community::graphql::filter;
pub use terminusdb_community::graphql::frame::{AllFrames, TypeDefinition};
pub use terminusdb_community::graphql::naming;

// Re-export filter/ordering traits from terminusdb-schema
pub use terminusdb_schema::{TdbGQLFilter, TdbGQLOrdering};

// The `live` introspection helpers compose deep async chains
// (server boot → schema insert → introspection → response parse) and the
// async-state-machine layout exceeds rustc's default query depth.
#![recursion_limit = "512"]

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
#[cfg(feature = "live")]
mod live;
mod render;
mod schema;

pub use codegen::{generate_all, generate_filter_impls, generate_filter_types, ModelConfig};
pub use frames::{schemas_to_allframes, schemas_vec_to_allframes};
#[cfg(feature = "live")]
pub use live::{introspect_schema_for, introspect_schema_sdl_for, with_introspected_schema};
pub use render::render_introspection_to_sdl;
pub use schema::{allframes_to_sdl, generate_gql_schema};

// Re-export key types from terminusdb-community
pub use terminusdb_community::graphql::filter;
pub use terminusdb_community::graphql::frame::{AllFrames, TypeDefinition};
pub use terminusdb_community::graphql::naming;

// Re-export filter/ordering traits from terminusdb-schema
pub use terminusdb_schema::{TdbGQLFilter, TdbGQLOrdering};

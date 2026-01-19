//! TerminusDB ORM Layer
//!
//! Provides an ActiveRecord-like API for querying TerminusDB with efficient
//! batch loading of related entities.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! let result = Comment::find_all(ids)
//!     .with::<Reply>()
//!     .execute(&spec)
//!     .await?;
//!
//! let comments: Vec<Comment> = result.get::<Comment>()?;
//! let replies: Vec<Reply> = result.get::<Reply>()?;
//! ```

mod client;
mod filter_query;
mod graphql_query;
mod multi_fetch;
mod multi_filter_query;
mod query;
mod relations;
mod resolver;
mod result;

pub mod prelude;

/// Test helpers for integration tests.
///
/// Enable with `#[cfg(test)]` or when the `testing` feature is enabled.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use client::{ClientAlreadyInitializedError, ClientProvider, GlobalClient, OrmClient};
pub use filter_query::{FilterExt, FilterQuery};
pub use graphql_query::{parse_id_response, IdQueryBuilder, IdQueryResult, RelationPath};
pub use multi_fetch::{fetch_by_ids, fetch_by_ids_default, FetchBuilder, MultiTypeFetch};
pub use multi_filter_query::MultiFilterQuery;
pub use query::{ModelExt, ModelQuery, OrmModel, RelationDirection, RelationSpec};
pub use relations::{
    BelongsTo, BelongsToField, DefaultField, ForwardRelation, HasMany, HasManyField, HasOne,
    HasOneField, ReverseRelation,
};
pub use resolver::{
    generate_graphql_query, BatchResolver, GraphQLRelationQuery, LoadStrategy, QueryPlan,
    RelationPlan, RelationResolution, RelationSelection, ResolvedRelations,
};
pub use result::OrmResult;

#[cfg(not(target_arch = "wasm32"))]
pub use graphql_query::execute_id_query;

#[cfg(not(target_arch = "wasm32"))]
pub use resolver::RelationResolver;

// Re-export commonly used types from dependencies
pub use terminusdb_client::{BranchSpec, GetOpts, TerminusDBHttpClient};
pub use terminusdb_gql::TdbGQLModel;
pub use terminusdb_relation::{RelationField, RelationFrom, RelationTo};
pub use terminusdb_schema::{
    json::InstanceFromJson, EntityIDFor, FromTDBInstance, Instance, TdbLazy, TerminusDBModel,
    ToSchemaClass, ToTDBSchema,
};

//! Prelude for convenient ORM imports.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! // Initialize global client
//! OrmClient::init(TerminusDBHttpClient::local_node().await?)?;
//!
//! // Fetch documents
//! let result = FetchBuilder::new()
//!     .add_ids(ids)
//!     .execute(&spec)
//!     .await?;
//!
//! let comments: Vec<Comment> = result.get()?;
//! ```

// ORM types
pub use crate::{
    BelongsTo, BelongsToField, ClientProvider, FetchBuilder, ForwardRelation, GlobalClient,
    HasMany, HasManyField, HasOne, HasOneField, IdQueryBuilder, IdQueryResult, ModelExt,
    ModelQuery, MultiTypeFetch, OrmClient, OrmModel, OrmResult, RelationBuilder, RelationDirection,
    RelationOpts, RelationPath, RelationSpec, ReverseRelation,
    // Filter query types
    FilterExt, FilterQuery, MultiFilterQuery, TdbGQLModel,
    // Composable query types
    ComposedQuery, ComposedResult, IntoQueryPart, Orm, QueryEntry,
    // Resolver types (GraphQL-based relation resolution)
    generate_graphql_query, BatchResolver, GraphQLRelationQuery, LoadStrategy, QueryPlan,
    RelationPlan, RelationResolution, RelationSelection, ResolvedRelations,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::{execute_id_query, RelationResolver};

// Convenience functions
pub use crate::{fetch_by_ids, fetch_by_ids_default};

// Client types
pub use crate::{BranchSpec, GetOpts, TerminusDBHttpClient};

// Relation types
pub use crate::{RelationField, RelationFrom, RelationTo};

// Schema types
pub use crate::{
    EntityIDFor, FromTDBInstance, Instance, InstanceFromJson, TdbLazy, TerminusDBModel,
    ToSchemaClass, ToTDBSchema,
};

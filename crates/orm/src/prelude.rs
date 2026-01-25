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
    // Resolver types (GraphQL-based relation resolution)
    generate_graphql_query,
    BatchResolver,
    BelongsTo,
    BelongsToField,
    ClientProvider,
    // Composable query types
    ComposedQuery,
    ComposedResult,
    FetchBuilder,
    ForwardRelation,
    GlobalClient,
    GraphQLRelationQuery,
    HasMany,
    HasManyField,
    HasOne,
    HasOneField,
    IdQueryBuilder,
    IdQueryResult,
    IntoQueryPart,
    LoadStrategy,
    ModelExt,
    ModelQuery,
    MultiTypeFetch,
    Orm,
    OrmClient,
    OrmModel,
    OrmResult,
    QueryEntry,
    QueryPlan,
    RelationBuilder,
    RelationDirection,
    RelationOpts,
    RelationPath,
    RelationPlan,
    RelationResolution,
    RelationSelection,
    RelationSpec,
    ResolvedRelations,
    ReverseRelation,
    TdbGQLFilter,
    TdbGQLOrdering,
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

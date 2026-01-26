//! Query builder for ORM operations.
//!
//! Provides the fluent API for building queries with eager loading of relations.
//!
//! # Forward vs Reverse Relations
//!
//! - **Forward relations** (HasMany/HasOne on self): Use `.with_field::<Target, StructFields::FieldName>()`
//! - **Reverse relations** (BelongsTo on target): Use `.with::<T>()` for all, or `.with_via::<T, TFields::FieldName>()`
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! // Reverse relation: Reply has BelongsTo<Comment>
//! let id = EntityIDFor::<Comment>::new("1")?;
//! let result = Comment::find_all([id])
//!     .with::<Reply>()  // All Replies referencing this Comment
//!     .execute(&spec)
//!     .await?;
//!
//! // Forward relation: Car has HasMany<Wheel> fields
//! let car_id = EntityIDFor::<Car>::new("car1")?;
//! let result = Car::find_all([car_id])
//!     .with_field::<Wheel, CarFields::Front>()
//!     .with_field::<Wheel, CarFields::Back>()
//!     .execute(&spec)
//!     .await?;
//! ```

use std::any::TypeId;
use std::marker::PhantomData;

use serde::Serialize;
use terminusdb_client::{BranchSpec, GetOpts};
use terminusdb_relation::RelationField;
use terminusdb_schema::{
    json::InstanceFromJson, EntityIDFor, FromTDBInstance, TerminusDBModel, ToSchemaClass,
    ToTDBSchema,
};

use crate::relations::{ForwardRelation, ReverseRelation};
use crate::{result::OrmResult, ClientProvider, GlobalClient, MultiTypeFetch};
use terminusdb_schema::{TdbGQLFilter, TdbGQLOrdering};

/// Convert a JSON value to GraphQL object literal syntax.
///
/// GraphQL object literals don't quote keys, but JSON does.
/// This converts `{"name": {"eq": "test"}}` to `{name: {eq: "test"}}`.
pub(crate) fn json_to_graphql(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_graphql).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(obj) => {
            let fields: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, json_to_graphql(v)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

/// Options for loading a relation with filtering/pagination.
///
/// The type parameters `F` and `O` are the filter and ordering types.
/// These are constrained via `TdbGQLFilter<M>` and `TdbGQLOrdering<M>` bounds
/// at the call site to ensure type safety.
///
/// # Example
/// ```ignore
/// use terminusdb_orm::prelude::*;
///
/// let result = Project::find(id)
///     .with_opts::<Ticket, _, _>(RelationOpts::new()
///         .filter(TicketFilter { status: Some(eq("open")), ..Default::default() })
///         .limit(5)
///         .order_by(TicketOrdering { created_at: Some(Ordering::Desc), ..Default::default() }))
///     .execute(&spec)
///     .await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct RelationOpts<F = (), O = ()> {
    /// Filter to apply to the relation.
    pub filter: Option<F>,
    /// Maximum number of related entities to load.
    pub limit: Option<i32>,
    /// Skip first N related entities.
    pub offset: Option<i32>,
    /// Ordering for related entities.
    pub order_by: Option<O>,
}

impl RelationOpts<(), ()> {
    /// Create new empty relation options.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<F, O> RelationOpts<F, O> {
    /// Set the filter for this relation.
    ///
    /// Returns a new `RelationOpts` with the filter type set.
    pub fn filter<F2>(self, filter: F2) -> RelationOpts<F2, O> {
        RelationOpts {
            filter: Some(filter),
            limit: self.limit,
            offset: self.offset,
            order_by: self.order_by,
        }
    }

    /// Set the maximum number of related entities to load.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the offset (skip first N related entities).
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set the ordering for related entities.
    ///
    /// Returns a new `RelationOpts` with the ordering type set.
    pub fn order_by<O2>(self, order_by: O2) -> RelationOpts<F, O2> {
        RelationOpts {
            filter: self.filter,
            limit: self.limit,
            offset: self.offset,
            order_by: Some(order_by),
        }
    }

    /// Convert filter and order_by to GraphQL strings for query generation.
    pub(crate) fn to_gql_strings(&self) -> (Option<String>, Option<String>)
    where
        F: Serialize,
        O: Serialize,
    {
        let filter_gql = self.filter.as_ref().map(|f| {
            let json = serde_json::to_value(f).unwrap_or_default();
            json_to_graphql(&json)
        });
        let order_by_gql = self.order_by.as_ref().map(|o| {
            let json = serde_json::to_value(o).unwrap_or_default();
            json_to_graphql(&json)
        });
        (filter_gql, order_by_gql)
    }
}

/// Specifies the direction of a relation query.
#[derive(Debug, Clone)]
pub enum RelationDirection {
    /// Forward relation: Self has a field pointing to Target (HasMany/HasOne).
    Forward {
        /// The field name on self that references the target.
        field_name: String,
    },
    /// Reverse relation: Target has a BelongsTo<Self> field.
    Reverse {
        /// Optional: specific BelongsTo field name. None = any field.
        via_field: Option<String>,
    },
}

impl Default for RelationDirection {
    fn default() -> Self {
        Self::Reverse { via_field: None }
    }
}

/// Specification for a relation to be loaded.
#[derive(Debug, Clone)]
pub struct RelationSpec {
    /// TypeId of the target type to load.
    pub target_type_id: TypeId,
    /// Type name (the TerminusDB schema class name, e.g., "BlogPost").
    pub target_type_name: String,
    /// Direction and field information.
    pub direction: RelationDirection,
    /// Nested relations to load under this relation.
    pub children: Vec<RelationSpec>,
    /// Serialized filter as GraphQL syntax (e.g., `{status: {eq: "open"}}`).
    pub filter_gql: Option<String>,
    /// Maximum number of related entities to load.
    pub limit: Option<i32>,
    /// Skip first N related entities.
    pub offset: Option<i32>,
    /// Serialized ordering as GraphQL syntax (e.g., `{created_at: Desc}`).
    pub order_by_gql: Option<String>,
}

/// Builder for configuring nested relations inside a `with_nested()` closure.
///
/// This builder accumulates relation specs that will be nested under a parent relation.
///
/// # Example
/// ```ignore
/// Writer::find(&id)
///     .with_nested::<Comment>(|b| {
///         b.with::<Reply>()      // Reply nested inside Comment
///          .with::<Like>()       // Like also nested inside Comment
///     })
///     .execute(&spec).await?;
/// ```
pub struct RelationBuilder<Parent> {
    /// The parent type this builder is for.
    _phantom: PhantomData<Parent>,
    /// Accumulated nested relations.
    pub(crate) relations: Vec<RelationSpec>,
}

impl<Parent: OrmModel> RelationBuilder<Parent> {
    /// Create a new relation builder.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            relations: Vec::new(),
        }
    }

    /// Add a reverse relation (target has `TdbLazy<Parent>` field).
    ///
    /// When the target type has exactly one `TdbLazy<Parent>` field, this method
    /// automatically uses that field name. If there are multiple fields, falls
    /// back to type-based inference (which may not work correctly - use `with_via` instead).
    ///
    /// # Example
    /// ```ignore
    /// // Comment has: writer: TdbLazy<Writer>
    /// // Inside with_nested::<Comment>, add Reply which has TdbLazy<Comment>
    /// b.with::<Reply>()
    /// ```
    pub fn with<R>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<Parent>,
    {
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Add a reverse relation with options (filter, limit, offset, order_by).
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    ///
    /// # Example
    /// ```ignore
    /// b.with_opts::<Reply, _, _>(RelationOpts::new().limit(5).filter(reply_filter))
    /// ```
    pub fn with_opts<R, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<Parent>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Add a reverse relation via a specific field.
    pub fn with_via<R, F>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        F: RelationField + 'static,
        R: ReverseRelation<Parent, F>,
    {
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: Some(F::field_name().to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Add a reverse relation via a specific field with options.
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    pub fn with_via_opts<R, Field, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        Field: RelationField + 'static,
        R: ReverseRelation<Parent, Field>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: Some(Field::field_name().to_string()),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Add a forward relation (Parent has field pointing to R).
    pub fn with_field<R, F>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        F: RelationField + 'static,
        Parent: ForwardRelation<R, F>,
    {
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Forward {
                field_name: F::field_name().to_string(),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Add a forward relation with options (filter, limit, offset, order_by).
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    pub fn with_field_opts<R, Field, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        Field: RelationField + 'static,
        Parent: ForwardRelation<R, Field>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Forward {
                field_name: Field::field_name().to_string(),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Add a nested relation with further nesting.
    pub fn with_nested<R, B>(mut self, builder_fn: B) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<Parent>,
        B: FnOnce(RelationBuilder<R>) -> RelationBuilder<R>,
    {
        let nested_builder = builder_fn(RelationBuilder::new());
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: nested_builder.relations,
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Add a nested relation with options and further nesting.
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    ///
    /// # Example
    /// ```ignore
    /// b.with_nested_opts::<Reply, _, _>(
    ///     RelationOpts::new().limit(5).filter(reply_filter),
    ///     |b| b.with::<Like>()
    /// )
    /// ```
    pub fn with_nested_opts<R, F, O, B>(mut self, opts: RelationOpts<F, O>, builder_fn: B) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<Parent>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
        B: FnOnce(RelationBuilder<R>) -> RelationBuilder<R>,
    {
        let nested_builder = builder_fn(RelationBuilder::new());
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: nested_builder.relations,
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }
}

impl<Parent: OrmModel> Default for RelationBuilder<Parent> {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker trait for types that can be queried via the ORM.
///
/// This is automatically implemented for types that derive `TerminusDBModel`.
pub trait OrmModel: TerminusDBModel + FromTDBInstance + InstanceFromJson + ToTDBSchema {}

// Blanket implementation for all qualifying types
impl<T> OrmModel for T where T: TerminusDBModel + FromTDBInstance + InstanceFromJson + ToTDBSchema {}

/// Extension trait that adds ORM query methods to models.
///
/// This trait provides the entry point for building ORM queries.
pub trait ModelExt: OrmModel + ToSchemaClass {
    /// Find all instances by their typed IDs.
    ///
    /// Takes `EntityIDFor<Self>` to ensure type safety - you can only query
    /// for Comment IDs when calling `Comment::find_all()`.
    ///
    /// # Example
    /// ```ignore
    /// let id1 = EntityIDFor::<Comment>::new("1")?;
    /// let id2 = EntityIDFor::<Comment>::new("2")?;
    /// let query = Comment::find_all([id1, id2]);
    /// ```
    fn find_all(ids: impl IntoIterator<Item = EntityIDFor<Self>>) -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        ModelQuery::new(ids.into_iter().map(|id| id.iri().to_string()))
    }

    /// Find all instances by string IDs (convenience method).
    ///
    /// Use this when you have string IDs and want to skip creating `EntityIDFor` values.
    /// Note: This bypasses type checking at compile time.
    ///
    /// # Example
    /// ```ignore
    /// let query = Comment::find_all_by_strings(["Comment/1", "Comment/2"]);
    /// ```
    fn find_all_by_strings(
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        ModelQuery::new(ids)
    }

    /// Find a single instance by its typed ID.
    ///
    /// # Example
    /// ```ignore
    /// let id = EntityIDFor::<Comment>::new("1")?;
    /// let query = Comment::find(id);
    /// ```
    fn find(id: EntityIDFor<Self>) -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        ModelQuery::new(std::iter::once(id.iri().to_string()))
    }

    /// Find a single instance by string ID (convenience method).
    ///
    /// # Example
    /// ```ignore
    /// let query = Comment::find_by_string("Comment/1");
    /// ```
    fn find_by_string(id: impl Into<String>) -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        ModelQuery::new(std::iter::once(id.into()))
    }

    /// Create a filter-based query (type-safe via TdbGQLFilter).
    ///
    /// This is the entry point for composable queries that can be combined
    /// with `Orm::and()`.
    ///
    /// # Example
    /// ```ignore
    /// let query = Project::query(ProjectFilter {
    ///     status: Some(eq("active")),
    ///     ..Default::default()
    /// }).limit(10).order_by(ProjectOrdering { name: Some(Ordering::Asc), ..Default::default() });
    /// ```
    fn query<F>(filter: F) -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
        F: TdbGQLFilter<Self> + Serialize,
    {
        ModelQuery::query(filter)
    }

    /// Query all records of this type as a composable query.
    ///
    /// This is the composable equivalent of `FilterExt::all()`. Use this when
    /// you need to combine multiple queries via `Orm::and()`.
    ///
    /// # Example
    /// ```ignore
    /// let query = Label::query_all().limit(100);
    /// let result = Orm::and(project_query, query).execute(&spec).await?;
    /// ```
    fn query_all() -> ModelQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        ModelQuery::all()
    }
}

// Blanket implementation for all OrmModel types
impl<T: OrmModel + ToSchemaClass> ModelExt for T {}

/// A type-erased query entry for multi-model composition.
///
/// This captures all the information needed to generate a GraphQL query fragment
/// for a single model type, including nested relations.
#[derive(Debug, Clone)]
pub struct QueryEntry {
    /// The GraphQL type name (root model).
    pub type_name: String,
    /// Filter serialized to GraphQL syntax.
    pub filter_gql: Option<String>,
    /// Maximum number of results.
    pub limit: Option<i32>,
    /// Skip first N results.
    pub offset: Option<i32>,
    /// Ordering serialized to GraphQL syntax.
    pub order_by_gql: Option<String>,
    /// Nested relation specs (from with/with_opts).
    pub relations: Vec<RelationSpec>,
}

/// Trait for queries that can be composed into a multi-query.
///
/// This enables type erasure so that `ModelQuery<Project>` and `ModelQuery<Ticket>`
/// can be combined into a single composed query.
pub trait IntoQueryPart {
    /// Convert this query into a type-erased QueryEntry.
    fn into_query_entry(self) -> QueryEntry;
}

/// A query builder for loading models with their relations.
///
/// Created via `Model::find_all()`, `Model::find()`, `Model::query()`, or `Model::all()`.
pub struct ModelQuery<T: OrmModel, C: ClientProvider = GlobalClient> {
    /// The primary IDs to fetch (for ID-based queries).
    pub(crate) primary_ids: Vec<String>,
    /// Relations to load (forward or reverse).
    pub(crate) with_relations: Vec<RelationSpec>,
    /// Get options.
    pub(crate) opts: GetOpts,
    /// Client to use.
    pub(crate) client: C,
    /// Marker for the primary type.
    pub(crate) _phantom: PhantomData<T>,

    // Filter-based query fields (for composable queries):
    /// Filter serialized to GraphQL syntax.
    pub(crate) filter_gql: Option<String>,
    /// Maximum number of results.
    pub(crate) limit: Option<i32>,
    /// Skip first N results.
    pub(crate) offset: Option<i32>,
    /// Ordering serialized to GraphQL syntax.
    pub(crate) order_by_gql: Option<String>,
}

impl<T: OrmModel> ModelQuery<T, GlobalClient> {
    /// Create a new query for the given IDs.
    pub fn new(ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            primary_ids: ids.into_iter().map(|id| id.into()).collect(),
            with_relations: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
            _phantom: PhantomData,
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        }
    }
}

impl<T: OrmModel + ToSchemaClass> ModelQuery<T, GlobalClient> {
    /// Create a query with a filter (type-safe via TdbGQLFilter).
    ///
    /// This creates a filter-based query for use with composable queries.
    /// The filter type is constrained via `TdbGQLFilter<T>`.
    ///
    /// # Example
    /// ```ignore
    /// let query = Project::query(ProjectFilter {
    ///     status: Some(eq("active")),
    ///     ..Default::default()
    /// });
    /// ```
    pub fn query<F>(filter: F) -> Self
    where
        F: TdbGQLFilter<T> + Serialize,
    {
        let filter_json = serde_json::to_value(&filter).unwrap_or_default();
        let filter_gql = json_to_graphql(&filter_json);
        Self {
            primary_ids: Vec::new(),
            with_relations: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
            _phantom: PhantomData,
            filter_gql: Some(filter_gql),
            limit: None,
            offset: None,
            order_by_gql: None,
        }
    }

    /// Query all records of this type (no filter).
    ///
    /// # Example
    /// ```ignore
    /// let all_labels = Label::all();
    /// ```
    pub fn all() -> Self {
        Self {
            primary_ids: Vec::new(),
            with_relations: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
            _phantom: PhantomData,
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        }
    }
}

impl<T: OrmModel, C: ClientProvider> ModelQuery<T, C> {
    /// Use a specific client instead of the global one.
    pub fn with_client<C2: ClientProvider>(self, client: C2) -> ModelQuery<T, C2> {
        ModelQuery {
            primary_ids: self.primary_ids,
            with_relations: self.with_relations,
            opts: self.opts,
            client,
            _phantom: PhantomData,
            filter_gql: self.filter_gql,
            limit: self.limit,
            offset: self.offset,
            order_by_gql: self.order_by_gql,
        }
    }

    /// Set the maximum number of results (for filter-based queries).
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the offset for pagination (for filter-based queries).
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set a timeout for the query execution.
    ///
    /// # Example
    /// ```ignore
    /// let result = Company::query_all()
    ///     .with_timeout(std::time::Duration::from_secs(60))
    ///     .with_client(&client)
    ///     .execute(&spec)
    ///     .await?;
    /// ```
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.opts.timeout = Some(timeout);
        self
    }

    /// Load related entities via a forward relation (HasMany/HasOne on self).
    ///
    /// This requires self to have a field pointing to the target type.
    /// The field marker type (e.g., `CarFields::Front`) specifies which field to traverse.
    ///
    /// # Example
    /// ```ignore
    /// // Car has fields: front: HasMany<Wheel>, back: HasMany<Wheel>
    /// let query = Car::find_all(ids)
    ///     .with_field::<Wheel, CarFields::Front>()
    ///     .with_field::<Wheel, CarFields::Back>();
    /// ```
    ///
    /// # Compile-time safety
    /// This will not compile if `T` does not implement `ForwardRelation<R, F>`.
    pub fn with_field<R, F>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        F: RelationField + 'static,
        T: ForwardRelation<R, F>,
    {
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Forward {
                field_name: F::field_name().to_string(),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Load related entities via a forward relation with options (filter, limit, offset, order_by).
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    pub fn with_field_opts<R, Field, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        Field: RelationField + 'static,
        T: ForwardRelation<R, Field>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Forward {
                field_name: Field::field_name().to_string(),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Load related entities via a reverse relation (BelongsTo on target).
    ///
    /// This loads all entities of type R that have a `BelongsTo<T>` field
    /// pointing to the primary entities.
    ///
    /// # Example
    /// ```ignore
    /// // Reply has: comment_id: BelongsTo<Comment>
    /// let query = Comment::find_all(ids)
    ///     .with::<Reply>();  // All Replies with any BelongsTo<Comment> field
    /// ```
    ///
    /// # Compile-time safety
    /// This will not compile if `R` does not implement `ReverseRelation<T>`.
    ///
    /// When `R` has exactly one `TdbLazy<T>` field, the correct field name is
    /// automatically used. If there are multiple fields, use `with_via` instead.
    pub fn with<R>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<T>,
    {
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Load related entities via a reverse relation with options (filter, limit, offset, order_by).
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    ///
    /// # Example
    /// ```ignore
    /// let query = Comment::find_all(ids)
    ///     .with_opts::<Reply, _, _>(RelationOpts::new()
    ///         .filter(ReplyFilter { ... })
    ///         .limit(5));
    /// ```
    pub fn with_opts<R, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<T>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Load related entities via a specific reverse relation field.
    ///
    /// Use this when the target type has multiple `BelongsTo<T>` fields
    /// and you want to filter by a specific one.
    ///
    /// # Example
    /// ```ignore
    /// // Document has: author: BelongsTo<User>, reviewer: BelongsTo<User>
    /// let query = User::find_all(ids)
    ///     .with_via::<Document, DocumentFields::Author>();  // Only docs where user is author
    /// ```
    ///
    /// # Compile-time safety
    /// This will not compile if `R` does not implement `ReverseRelation<T, F>`.
    pub fn with_via<R, F>(mut self) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        F: RelationField + 'static,
        R: ReverseRelation<T, F>,
    {
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: Some(F::field_name().to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Load related entities via a specific reverse relation field with options.
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    pub fn with_via_opts<R, Field, F, O>(mut self, opts: RelationOpts<F, O>) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        Field: RelationField + 'static,
        R: ReverseRelation<T, Field>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
    {
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: Some(Field::field_name().to_string()),
            },
            children: Vec::new(),
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Load related entities with nested relations.
    ///
    /// The closure receives a `RelationBuilder` to configure what relations
    /// should be loaded **inside** the target type.
    ///
    /// # Example
    /// ```ignore
    /// Writer::find(&id)
    ///     .with::<BlogPost>()
    ///     .with_nested::<Comment>(|b| {
    ///         b.with::<Reply>()   // Reply nested inside Comment
    ///          .with::<Like>()    // Like also nested inside Comment
    ///     })
    ///     .execute(&spec).await?;
    /// ```
    ///
    /// Generates GraphQL:
    /// ```graphql
    /// Writer {
    ///   _id
    ///   _writer_of_BlogPost { _id }
    ///   _comment_of_Writer {
    ///     _id
    ///     _reply_of_Comment { _id }
    ///     _like_of_Comment { _id }
    ///   }
    /// }
    /// ```
    pub fn with_nested<R, B>(mut self, builder_fn: B) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<T>,
        B: FnOnce(RelationBuilder<R>) -> RelationBuilder<R>,
    {
        let nested_builder = builder_fn(RelationBuilder::new());
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: nested_builder.relations,
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        });
        self
    }

    /// Load related entities with nested relations and options.
    ///
    /// The filter and ordering types are type-checked against the model R
    /// via `TdbGQLFilter<R>` and `TdbGQLOrdering<R>` bounds.
    ///
    /// # Example
    /// ```ignore
    /// Writer::find(&id)
    ///     .with_nested_opts::<Comment, _, _>(
    ///         RelationOpts::new().limit(5).filter(comment_filter),
    ///         |b| b.with::<Reply>()
    ///     )
    ///     .execute(&spec).await?;
    /// ```
    pub fn with_nested_opts<R, F, O, B>(mut self, opts: RelationOpts<F, O>, builder_fn: B) -> Self
    where
        R: OrmModel + ToSchemaClass + 'static,
        R: ReverseRelation<T>,
        F: TdbGQLFilter<R> + Serialize,
        O: TdbGQLOrdering<R> + Serialize,
        B: FnOnce(RelationBuilder<R>) -> RelationBuilder<R>,
    {
        let nested_builder = builder_fn(RelationBuilder::new());
        let (filter_gql, order_by_gql) = opts.to_gql_strings();
        self.with_relations.push(RelationSpec {
            target_type_id: TypeId::of::<R>(),
            target_type_name: R::to_class(),
            direction: RelationDirection::Reverse {
                via_field: R::default_field_name().map(|s| s.to_string()),
            },
            children: nested_builder.relations,
            filter_gql,
            limit: opts.limit,
            offset: opts.offset,
            order_by_gql,
        });
        self
    }

    /// Set the get options.
    pub fn opts(mut self, opts: GetOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Enable unfolding of nested documents.
    pub fn unfold(mut self) -> Self {
        self.opts.unfold = true;
        self
    }

    /// Check if this is a filter-based query (vs ID-based).
    pub fn is_filter_query(&self) -> bool {
        self.filter_gql.is_some() || (self.primary_ids.is_empty() && self.limit.is_some())
    }

    /// Get the primary IDs being queried.
    pub fn ids(&self) -> &[String] {
        &self.primary_ids
    }

    /// Get the number of primary IDs.
    pub fn len(&self) -> usize {
        self.primary_ids.len()
    }

    /// Check if no primary IDs have been added.
    pub fn is_empty(&self) -> bool {
        self.primary_ids.is_empty()
    }

    /// Get the relations that will be loaded.
    pub fn relations(&self) -> &[RelationSpec] {
        &self.with_relations
    }

    /// Execute the query and return all results.
    ///
    /// This will:
    /// 1. If primary IDs are provided: fetch documents by those IDs
    /// 2. If no primary IDs (filter-based or "all" query): first run a GraphQL
    ///    query to discover matching IDs, then fetch those documents
    /// 3. If relations were requested via `.with::<R>()`, `.with_field::<R, F>()`,
    ///    `.with_via::<R, F>()`, or `.with_nested::<R>(|b| ...)`:
    ///    - Execute ONE GraphQL query to collect all related `_id` values
    ///    - Execute ONE batch fetch to retrieve all documents by ID
    /// 4. Return an `OrmResult` containing all documents
    ///
    /// # Two-Phase Loading (NO N+1)
    ///
    /// Regardless of query complexity, this always results in exactly 2 database calls
    /// when relations are requested:
    /// 1. GraphQL query to traverse relations and collect IDs
    /// 2. Batch document fetch by all collected IDs
    pub async fn execute(mut self, spec: &BranchSpec) -> anyhow::Result<OrmResult>
    where
        C: MultiTypeFetch + Sync + ClientProvider,
        T: ToSchemaClass,
    {
        // If no primary IDs, we need to run a GraphQL query first to find them
        // This handles both filter-based queries and "query all" scenarios
        if self.primary_ids.is_empty() {
            self.primary_ids = self.discover_primary_ids(spec).await?;

            // If still no IDs after discovery, there are no matching documents
            if self.primary_ids.is_empty() {
                return Ok(OrmResult::empty());
            }
        }

        // If no relations, simple fetch
        if self.with_relations.is_empty() {
            return self
                .client
                .fetch_by_ids(self.primary_ids, spec, self.opts)
                .await;
        }

        // Two-phase relation loading:
        // Phase 1: GraphQL query to collect ALL related IDs
        let all_ids = self.collect_relation_ids(spec).await?;

        // Phase 2: Single batch fetch of all documents
        // Always enable unfold for relation queries to get full subdocument data
        // (users expect complete entities when loading relations, not reference strings)
        let mut fetch_opts = self.opts;
        fetch_opts.unfold = true;

        self.client.fetch_by_ids(all_ids, spec, fetch_opts).await
    }

    /// Discover primary IDs by executing a GraphQL query.
    ///
    /// This is used for filter-based queries and "query all" scenarios where
    /// we don't have explicit IDs upfront.
    async fn discover_primary_ids(&self, spec: &BranchSpec) -> anyhow::Result<Vec<String>>
    where
        C: ClientProvider,
        T: ToSchemaClass,
    {
        use terminusdb_client::graphql::GraphQLRequest;

        let type_name = T::to_class();

        // Build GraphQL query arguments
        let mut args = Vec::new();
        if let Some(filter) = &self.filter_gql {
            args.push(format!("filter: {}", filter));
        }
        if let Some(order_by) = &self.order_by_gql {
            args.push(format!("orderBy: {}", order_by));
        }
        if let Some(limit) = self.limit {
            args.push(format!("limit: {}", limit));
        }
        if let Some(offset) = self.offset {
            args.push(format!("offset: {}", offset));
        }

        let args_str = if args.is_empty() {
            String::new()
        } else {
            format!("({})", args.join(", "))
        };

        // Build the GraphQL query to fetch only _id fields
        let graphql_query = format!(
            "query {{\n  {}{} {{\n    _id\n  }}\n}}",
            type_name, args_str
        );

        // Execute GraphQL query
        let request = GraphQLRequest::new(&graphql_query);
        let response = self
            .client
            .client()
            .execute_graphql::<serde_json::Value>(&spec.db, spec.branch.as_deref(), request, None)
            .await?;

        // Check for errors
        if let Some(errors) = &response.errors {
            if !errors.is_empty() {
                let error_msgs: Vec<_> = errors.iter().map(|e| e.message.clone()).collect();
                return Err(anyhow::anyhow!("GraphQL errors: {:?}", error_msgs));
            }
        }

        let data = response
            .data
            .ok_or_else(|| anyhow::anyhow!("No GraphQL data returned"))?;

        // Extract IDs from the response
        let mut ids = Vec::new();
        if let Some(array) = data.get(&type_name).and_then(|v| v.as_array()) {
            for item in array {
                if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                    ids.push(id.to_string());
                }
            }
        }

        Ok(ids)
    }

    /// Collect all related entity IDs using a single GraphQL query.
    ///
    /// This generates a GraphQL query that traverses all requested relations
    /// (including nested ones) and extracts only the `_id` fields.
    async fn collect_relation_ids(&self, spec: &BranchSpec) -> anyhow::Result<Vec<String>>
    where
        C: ClientProvider,
    {
        use crate::resolver::{build_graphql_from_relation_specs, extract_ids_recursive};
        use terminusdb_client::graphql::GraphQLRequest;

        // Get the primary type name from the ID
        // IDs can be:
        // - "Writer/123" -> "Writer"
        // - "terminusdb:///data/Writer/123" -> "Writer"
        let primary_type = self
            .primary_ids
            .first()
            .and_then(|id| {
                // Handle both short and full URI formats
                if id.contains("///data/") {
                    // Full URI: terminusdb:///data/Writer/xxx
                    id.split("///data/").nth(1)?.split('/').next()
                } else {
                    // Short form: Writer/xxx
                    id.split('/').next()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Could not extract type from ID"))?;

        // Build GraphQL query from relation specs
        let graphql_query = build_graphql_from_relation_specs(
            primary_type,
            &self.primary_ids,
            &self.with_relations,
        );

        // Debug: print the generated GraphQL query
        eprintln!(
            "[ORM DEBUG] Executing relation GraphQL query:\n{}",
            graphql_query
        );

        // Execute GraphQL query
        let request = GraphQLRequest::new(&graphql_query);
        let response = self
            .client
            .client()
            .execute_graphql::<serde_json::Value>(&spec.db, spec.branch.as_deref(), request, None)
            .await?;

        // Check for errors
        if let Some(errors) = &response.errors {
            if !errors.is_empty() {
                let error_msgs: Vec<_> = errors.iter().map(|e| e.message.clone()).collect();
                return Err(anyhow::anyhow!("GraphQL errors: {:?}", error_msgs));
            }
        }

        let data = response
            .data
            .ok_or_else(|| anyhow::anyhow!("No GraphQL data returned"))?;

        // Extract all _id values from the response
        let mut ids = self.primary_ids.clone();
        extract_ids_recursive(&data, &mut ids);

        // Debug: print collected IDs count
        #[cfg(feature = "debug-graphql")]
        eprintln!("Collected {} IDs from relations", ids.len());

        Ok(ids)
    }

    /// Execute the query and return only the primary type.
    ///
    /// This is a convenience method that unwraps the result for the primary type.
    pub async fn execute_primary(self, spec: &BranchSpec) -> anyhow::Result<Vec<T>>
    where
        C: MultiTypeFetch + Sync + ClientProvider,
    {
        let result = self.execute(spec).await?;
        result.get::<T>()
    }

    /// Execute the query and return a single result.
    ///
    /// Returns an error if no results are found or if multiple results are found.
    pub async fn execute_one(self, spec: &BranchSpec) -> anyhow::Result<T>
    where
        C: MultiTypeFetch + Sync + ClientProvider,
    {
        let result = self.execute(spec).await?;
        result.get_one::<T>()?.ok_or_else(|| {
            anyhow::anyhow!("No {} found for the given ID", std::any::type_name::<T>())
        })
    }
}

/// Additional methods for queries with ordering.
impl<T, C> ModelQuery<T, C>
where
    T: OrmModel,
    C: ClientProvider,
{
    /// Set the ordering for results (type-safe via TdbGQLOrdering).
    ///
    /// # Example
    /// ```ignore
    /// let query = Project::query(filter)
    ///     .order_by(ProjectOrdering {
    ///         name: Some(Ordering::Asc),
    ///         ..Default::default()
    ///     });
    /// ```
    pub fn order_by<O>(mut self, order_by: O) -> Self
    where
        O: TdbGQLOrdering<T> + Serialize,
    {
        let order_json = serde_json::to_value(&order_by).unwrap_or_default();
        self.order_by_gql = Some(json_to_graphql(&order_json));
        self
    }
}

/// Implementation of IntoQueryPart for ModelQuery.
impl<T, C> IntoQueryPart for ModelQuery<T, C>
where
    T: OrmModel + ToSchemaClass,
    C: ClientProvider,
{
    fn into_query_entry(self) -> QueryEntry {
        QueryEntry {
            type_name: T::to_class(),
            filter_gql: self.filter_gql,
            limit: self.limit,
            offset: self.offset,
            order_by_gql: self.order_by_gql,
            relations: self.with_relations,
        }
    }
}

#[cfg(test)]
mod tests {
    // Note: Full tests require a running TerminusDB instance and proper model setup
    // These are structural tests only

    #[test]
    fn test_model_query_builder() {
        // Test that the builder can be constructed (compile-time test)
        // We can't actually test ModelQuery without a concrete OrmModel type
    }
}

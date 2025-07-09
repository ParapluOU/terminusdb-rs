use crate::{BranchSpec, TerminusDBHttpClient};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use tap::Pipe;
use terminusdb_schema::{FromTDBInstance, InstanceFromJson, TerminusDBModel, ToTDBSchema};
use terminusdb_woql2::prelude::Query;
use terminusdb_woql_builder::builder::WoqlBuilder;
use terminusdb_woql_builder::prelude::{node, Var};
use terminusdb_woql_builder::vars;

/// contract of a self-contained query
pub trait InstanceQueryable {
    /// the main user model it is returning;
    /// this can also be an ad-hoc deserializable type and is not neccessarily a TerminusDB model
    type Model: TerminusDBModel + InstanceFromJson;

    /// running the query with an adapter
    async fn apply(
        &self,
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<Self::Model>> {
        let query = self.query(limit, offset);

        let v_id = vars!("Subject");
        let v_doc = vars!("Doc");

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        res.bindings
            .into_iter()
            .filter_map(|mut b| {
                b.remove(&*v_doc)
                    .map(|v| <Self::Model as FromTDBInstance>::from_json(v))
            })
            .collect()
    }

    async fn count(
        &self,
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
    ) -> anyhow::Result<usize> {
        let query = self.query_count();

        let v_count = vars!("Count");

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        // The count query should return a single binding with the Count variable
        res.bindings
            .into_iter()
            .next()
            .and_then(|mut binding| binding.remove(&*v_count))
            .and_then(|value| {
                // Try to extract the count from @value field
                if let Some(obj) = value.as_object() {
                    if let Some(val) = obj.get("@value") {
                        return val.as_u64();
                    }
                }
                value.as_u64()
            })
            .map(|count| count as usize)
            .ok_or_else(|| anyhow::anyhow!("Failed to extract count from query result"))
    }

    /// user-implementation goes here
    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder;

    fn query_count(&self) -> Query {
        let v_id = vars!("Subject");
        let v_doc = vars!("Doc");
        let v_count = vars!("Count");

        let query = WoqlBuilder::new()
            // the triple was neccessary instead of the IsA
            .isa2::<Self::Model>(
                &v_id,
                // "rdf:type",
                // node(format!(
                //     "@schema:{}",
                //     <Self::Model as ToTDBSchema>::schema_name()
                // )),
            )
            // generate the implementation-specific query
            .pipe(|q| self.build(v_id.clone(), q))
            // important: this seems to HAVE to come after the triple
            .read_document(v_id.clone(), v_doc.clone())
            // .isa(v_id.clone(), node(T::schema_name())) // this didnt work
            .select(vec![v_doc.clone()])
            .count(v_count)
            .finalize();

        query
    }

    /// returning the query as a WOQL Query enum
    fn query(&self, limit: Option<usize>, offset: Option<usize>) -> Query {
        let v_id = vars!("Subject");
        let v_doc = vars!("Doc");

        WoqlBuilder::new()
            // the triple was neccessary instead of the IsA
            .isa2::<Self::Model>(
                &v_id,
                // "rdf:type",
                // node(format!(
                //     "@schema:{}",
                //     <Self::Model as ToTDBSchema>::schema_name()
                // )),
            )
            // generate the implementation-specific query
            .pipe(|q| self.build(v_id.clone(), q))
            // important: this seems to HAVE to come after the triple
            .read_document(v_id.clone(), v_doc.clone())
            // .isa(v_id.clone(), node(T::schema_name())) // this didnt work
            .select(vec![v_doc.clone()])
            .pipe(|q| match offset {
                None => q,
                Some(o) => q.start(o as u64),
            })
            .pipe(|q| match limit {
                None => q,
                Some(l) => q.limit(l as u64),
            })
            .finalize()
    }
}

/// default model litsing query that uses the instance query but adds no extra conditions
pub struct ListModels<T> {
    _ty: PhantomData<T>,
}

impl<T> Default for ListModels<T> {
    fn default() -> Self {
        Self {
            _ty: Default::default(),
        }
    }
}

impl<T: TerminusDBModel + InstanceFromJson> InstanceQueryable for ListModels<T> {
    type Model = T;

    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder {
        builder
    }
}

pub trait CanQuery<Q: InstanceQueryable> {
    fn get_query(&self) -> anyhow::Result<Q>;
}

#[derive(Default)]
pub enum QueryType {
    /// whether the result is reading some model through .read_document()
    #[default]
    Instance,
    /// or whether the returned type is ad-hoc created by triples and selects
    Custom,
}

/// Trait for raw WOQL queries that deserialize to custom structs.
///
/// Unlike `InstanceQueryable`, this trait doesn't assume you're querying for
/// TerminusDB models with read_document(). Instead, it allows arbitrary WOQL
/// queries that return custom result types.
///
/// # Example
/// ```rust
/// #[derive(Deserialize)]
/// struct PersonAge {
///     name: String,
///     age: i32,
/// }
///
/// struct AgeQuery;
///
/// impl RawQueryable for AgeQuery {
///     type Result = PersonAge;
///     
///     fn query(&self) -> Query {
///         WoqlBuilder::new()
///             .triple(vars!("Person"), "name", vars!("Name"))
///             .triple(vars!("Person"), "age", vars!("Age"))
///             .select(vec![vars!("Name"), vars!("Age")])
///             .finalize()
///     }
///     
///     fn extract_result(&self, binding: HashMap<String, serde_json::Value>) -> anyhow::Result<Self::Result> {
///         Ok(PersonAge {
///             name: serde_json::from_value(binding.get("Name").unwrap().clone())?,
///             age: serde_json::from_value(binding.get("Age").unwrap().clone())?,
///         })
///     }
/// }
/// ```
pub trait RawQueryable {
    /// The custom result type that query results will be deserialized into
    type Result: DeserializeOwned;

    /// Build the WOQL query
    fn query(&self) -> Query;

    /// Extract a single result from a binding row
    ///
    /// The default implementation assumes the binding can be directly deserialized
    /// into the Result type. Override this for custom extraction logic.
    fn extract_result(
        &self,
        binding: HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Self::Result> {
        serde_json::from_value(serde_json::to_value(binding)?)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize query result: {}", e))
    }

    /// Execute the query and return results
    async fn apply(
        &self,
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
    ) -> anyhow::Result<Vec<Self::Result>> {
        let query = self.query();

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        res.bindings
            .into_iter()
            .map(|binding| self.extract_result(binding))
            .collect()
    }
}

/// Builder for constructing raw WOQL queries with custom result types
pub struct RawWoqlQuery<T> {
    builder: WoqlBuilder,
    _phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> RawWoqlQuery<T> {
    /// Create a new raw query builder
    pub fn new() -> Self {
        Self {
            builder: WoqlBuilder::new(),
            _phantom: PhantomData,
        }
    }

    /// Access the underlying WoqlBuilder for query construction
    pub fn builder(self) -> WoqlBuilder {
        self.builder
    }
}

impl<T: DeserializeOwned> RawQueryable for RawWoqlQuery<T> {
    type Result = T;

    fn query(&self) -> Query {
        self.builder.clone().finalize()
    }
}

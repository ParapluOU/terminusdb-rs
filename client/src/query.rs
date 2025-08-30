use crate::{BranchSpec, TerminusDBHttpClient};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use tap::Pipe;
use terminusdb_schema::{FromTDBInstance, InstanceFromJson, TerminusDBModel, ToTDBSchema};
use terminusdb_woql2::misc::Count;
use terminusdb_woql2::prelude::{DataValue, Query};
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
            .pipe(|q| match offset {
                None => q,
                Some(o) => q.start(o as u64),
            })
            .pipe(|q| match limit {
                None => q,
                Some(l) => q.limit(l as u64),
            })
            .finalize();
        
        query
    }
}

/// Default model listing query - equivalent to FilteredListModels with no filters
pub type ListModels<T> = FilteredListModels<T>;

impl<T> Default for FilteredListModels<T> {
    fn default() -> Self {
        Self {
            filters: Vec::new(),
            _ty: PhantomData,
        }
    }
}

/// Model listing query with field-value filter conditions
pub struct FilteredListModels<T> {
    pub(crate) filters: Vec<(String, DataValue)>,
    _ty: PhantomData<T>,
}

impl<T> FilteredListModels<T> {
    /// Create a new filtered list query with the given field-value pairs
    pub fn new<I, K, V>(filters: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: terminusdb_woql2::prelude::IntoDataValue,
    {
        Self {
            filters: filters
                .into_iter()
                .map(|(k, v)| (k.into(), v.into_data_value()))
                .collect(),
            _ty: PhantomData,
        }
    }
    
    /// Create an empty filtered list query (lists all instances)
    pub fn empty() -> Self {
        Self::default()
    }
    
    /// Get the number of filters
    pub fn filter_count(&self) -> usize {
        self.filters.len()
    }
}

impl<T: TerminusDBModel + InstanceFromJson> InstanceQueryable for FilteredListModels<T> {
    type Model = T;

    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder {
        use terminusdb_woql_builder::prelude::WoqlInput;
        
        // Add a triple pattern for each filter condition
        self.filters.iter().fold(builder, |builder, (field, value)| {
            // Convert DataValue to WoqlInput which implements IntoWoql2
            let woql_value = match value {
                DataValue::Variable(v) => WoqlInput::Variable(Var::new(v)),
                DataValue::Data(d) => {
                    use terminusdb_schema::XSDAnySimpleType;
                    match d {
                        XSDAnySimpleType::String(s) => WoqlInput::String(s.clone()),
                        XSDAnySimpleType::Boolean(b) => WoqlInput::Boolean(*b),
                        XSDAnySimpleType::Decimal(d) => {
                            // For integer values stored as decimals, keep them as decimals
                            // TerminusDB might expect exact type matching
                            WoqlInput::Decimal(d.to_string())
                        }
                        XSDAnySimpleType::UnsignedInt(u) => WoqlInput::Integer(*u as i64),
                        XSDAnySimpleType::Integer(i) => WoqlInput::Integer(*i),
                        XSDAnySimpleType::Float(f) => {
                            // Convert float to decimal string for WOQL
                            WoqlInput::Decimal(f.to_string())
                        }
                        XSDAnySimpleType::DateTime(dt) => {
                            // Convert datetime to ISO 8601 string
                            WoqlInput::DateTime(dt.to_rfc3339())
                        }
                        XSDAnySimpleType::Date(d) => {
                            // Convert date to ISO 8601 string
                            WoqlInput::Date(d.to_string())
                        }
                        XSDAnySimpleType::Time(t) => {
                            // Convert time to ISO 8601 string
                            WoqlInput::Time(t.to_string())
                        }
                        XSDAnySimpleType::URI(uri) => {
                            // URIs are represented as nodes
                            WoqlInput::Node(uri.clone())
                        }
                        XSDAnySimpleType::HexBinary(hex) => {
                            // Store hex binary as string
                            WoqlInput::String(hex.clone())
                        }
                    }
                },
                DataValue::List(_) => {
                    panic!("List values are not supported in filters")
                }
            };
            
            // Properties need to be prefixed with @schema: for property lookups
            let qualified_field = format!("@schema:{}", field);
            builder.triple(subject.clone(), qualified_field.as_str(), woql_value)
        })
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

    /// Build a count query for this queryable
    /// 
    /// The default implementation first removes any pagination (Limit/Start) operations,
    /// then checks if the query is already a Count query. If it is, returns it as-is. 
    /// Otherwise, wraps the query in a Count operation.
    fn query_count(&self) -> Query {
        let query = self.query().unwrap_pagination();
        
        // Check if the query is already a Count
        match query {
            Query::Count(_) => query,
            _ => {
                // Wrap the query in a Count operation
                let v_count = vars!("Count");
                Query::Count(Count {
                    query: Box::new(query),
                    count: DataValue::Variable(v_count.name().to_string()),
                })
            }
        }
    }

    /// Execute the query and return the count of results
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
                        return val.as_u64().map(|v| v as usize);
                    }
                }
                // Fallback: try to parse the value directly as a number
                value.as_u64().map(|v| v as usize)
            })
            .ok_or_else(|| anyhow::anyhow!("Failed to extract count from query result"))
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

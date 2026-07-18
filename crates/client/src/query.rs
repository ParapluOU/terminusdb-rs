use crate::woql_helpers::isa_model;
use crate::{BranchSpec, TerminusDBHttpClient};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::marker::PhantomData;
use terminusdb_schema::{
    FromTDBInstance, GraphType, InstanceFromJson, TerminusDBModel, XSDAnySimpleType,
};
use terminusdb_woql2::macros::IntoNodeValue;
use terminusdb_woql2::misc::{Count, Limit, Start};
use terminusdb_woql2::prelude::{
    And, DataValue, NodeValue, Query, ReadDocument, Select, Triple, True, Value,
};

/// contract of a self-contained query
// These trait futures are only ever driven by our own native HTTP client, so the
// missing `Send` bound that `async fn` in a trait can't express is not a concern.
#[allow(async_fn_in_trait)]
pub trait InstanceQueryable {
    /// the main user model it is returning;
    /// this can also be an ad-hoc deserializable type and is not neccessarily a TerminusDB model
    type Model: TerminusDBModel + InstanceFromJson;

    /// The variable name used to bind the read document
    const READ_DOCUMENT_BINDING: &'static str = "Doc";

    /// Returns the woql2 `Value` variable used to bind the read document.
    fn doc_var() -> Value {
        Value::Variable(Self::READ_DOCUMENT_BINDING.to_string())
    }

    /// running the query with an adapter
    async fn apply(
        &self,
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<Self::Model>> {
        let query = self.query(limit, offset);

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        res.bindings
            .into_iter()
            .filter_map(|mut b| {
                b.remove(Self::READ_DOCUMENT_BINDING)
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

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        // The count query should return a single binding with the Count variable
        res.bindings
            .into_iter()
            .next()
            .and_then(|mut binding| binding.remove("Count"))
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
    ///
    /// Returns the implementation-specific constraints as a WOQL `Query`, given
    /// the subject variable that binds the matched instance. Return
    /// `Query::True` when there are no constraints.
    fn build(&self, subject: &Value) -> Query;

    /// Combine the `rdf:type` constraint for `Self::Model` with the
    /// implementation-specific `build` constraints into a single flat `And`
    /// (or just the type triple when there are no extra constraints).
    fn base_query(&self, subject: &Value) -> Query {
        let isa = isa_model::<Self::Model>(subject);
        match self.build(subject) {
            Query::True(_) => isa,
            Query::And(And { and }) => {
                let mut parts = Vec::with_capacity(and.len() + 1);
                parts.push(isa);
                parts.extend(and);
                Query::And(And { and: parts })
            }
            other => Query::And(And {
                and: vec![isa, other],
            }),
        }
    }

    fn query_count(&self) -> Query {
        let subject = Value::Variable("Subject".to_string());

        Query::Count(Count {
            query: Box::new(self.base_query(&subject)),
            count: DataValue::Variable("Count".to_string()),
        })
    }

    /// returning the query as a WOQL Query enum
    fn query(&self, limit: Option<usize>, offset: Option<usize>) -> Query {
        let subject = Value::Variable("Subject".to_string());
        let doc = Self::doc_var();

        // type triple + implementation constraints
        let base = self.base_query(&subject);

        // important: read_document has to come after the type triple
        let read = Query::ReadDocument(ReadDocument {
            identifier: subject.clone().into_node_value(),
            document: doc.clone(),
        });
        let inner = match base {
            Query::And(And { mut and }) => {
                and.push(read);
                Query::And(And { and })
            }
            other => Query::And(And {
                and: vec![other, read],
            }),
        };

        let selected = Query::Select(Select {
            variables: vec![Self::READ_DOCUMENT_BINDING.to_string()],
            query: Box::new(inner),
        });

        // pagination order: Limit { Start { Select { .. } } }
        let with_offset = match offset {
            None => selected,
            Some(o) => Query::Start(Start {
                start: o as u64,
                query: Box::new(selected),
            }),
        };

        match limit {
            None => with_offset,
            Some(l) => Query::Limit(Limit {
                limit: l as u64,
                query: Box::new(with_offset),
            }),
        }
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

    fn build(&self, subject: &Value) -> Query {
        // One triple pattern per filter condition. The DataValue -> object
        // mapping mirrors the previous builder conversion exactly (URI ->
        // Node, HexBinary -> String, UnsignedInt -> Integer, Float -> Decimal,
        // everything else -> the same xsd variant).
        let triples: Vec<Query> = self
            .filters
            .iter()
            .map(|(field, value)| {
                let object: Value = match value {
                    DataValue::Variable(v) => Value::Variable(v.clone()),
                    DataValue::Data(d) => match d {
                        // URIs are represented as nodes
                        XSDAnySimpleType::URI(uri) => Value::Node(uri.clone()),
                        // Store hex binary as string
                        XSDAnySimpleType::HexBinary(hex) => {
                            Value::Data(XSDAnySimpleType::String(hex.clone()))
                        }
                        // Unsigned integers are emitted as (signed) integers
                        XSDAnySimpleType::UnsignedInt(u) => {
                            Value::Data(XSDAnySimpleType::Integer(*u as i64))
                        }
                        // Floats are emitted as decimals via their string form
                        XSDAnySimpleType::Float(f) => Value::Data(XSDAnySimpleType::Decimal(
                            f.to_string().parse().expect("Invalid decimal string format"),
                        )),
                        // String/Boolean/Integer/Decimal/DateTime/Date/Time pass
                        // through unchanged.
                        other => Value::Data(other.clone()),
                    },
                    DataValue::List(_) => {
                        panic!("List values are not supported in filters")
                    }
                };

                // Properties need to be prefixed with @schema: for property lookups
                Query::Triple(Triple {
                    subject: subject.clone().into_node_value(),
                    predicate: NodeValue::Node(format!("@schema:{}", field)),
                    object,
                    graph: Some(GraphType::Instance),
                })
            })
            .collect();

        match triples.len() {
            0 => Query::True(True {}),
            1 => triples.into_iter().next().unwrap(),
            _ => Query::And(And { and: triples }),
        }
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
///         use terminusdb_woql2::prelude::*;
///         select!([Name, Age], and!(
///             triple!(var!(Person), "name", var!(Name)),
///             triple!(var!(Person), "age", var!(Age)),
///         ))
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
// See `InstanceQueryable`: these futures are only driven by our own native client.
#[allow(async_fn_in_trait)]
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
                Query::Count(Count {
                    query: Box::new(query),
                    count: DataValue::Variable("Count".to_string()),
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

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        // The count query should return a single binding with the Count variable
        res.bindings
            .into_iter()
            .next()
            .and_then(|mut binding| binding.remove("Count"))
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

/// Wrapper pairing a raw woql2 [`Query`] with a custom result type
pub struct RawWoqlQuery<T> {
    query: Query,
    _phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> RawWoqlQuery<T> {
    /// Create a new raw query from a fully-constructed woql2 [`Query`]
    pub fn new(query: Query) -> Self {
        Self {
            query,
            _phantom: PhantomData,
        }
    }
}

impl<T: DeserializeOwned> RawQueryable for RawWoqlQuery<T> {
    type Result = T;

    fn query(&self) -> Query {
        self.query.clone()
    }
}

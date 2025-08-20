//! Query execution and WOQL operations

use {
    crate::{
        spec::BranchSpec, InstanceFromJson, InstanceQueryable, ListModels, RawQueryable,
        TerminusDBModel, WOQLResult,
    },
    ::tracing::{instrument, trace},
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize},
    serde_json::{json, Value},
    std::{collections::HashMap, fmt::Debug},
    terminusdb_schema::{ToJson, ToTDBInstance, ToTDBSchema},
    terminusdb_woql2::prelude::Query as Woql2Query,
    terminusdb_woql_builder::prelude::{vars, WoqlBuilder},
};

use crate::result::ResponseWithHeaders;

/// Query execution methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Executes a WOQL query and returns typed results.
    ///
    /// This function executes Web Object Query Language (WOQL) queries against TerminusDB
    /// and deserializes the results into the specified type.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize query results into
    ///
    /// # Arguments
    /// * `db` - Optional branch specification (if None, uses client's default)
    /// * `query` - The WOQL query to execute
    ///
    /// # Returns
    /// WOQL query results deserialized to type `T`
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_woql2::Query;
    ///
    /// let query = Query::select().triple("v:Subject", "v:Predicate", "v:Object");
    /// let results: WOQLResult<HashMap<String, Value>> = client.query(Some(spec), query).await?;
    /// ```
    #[instrument(
        name = "terminus.query.execute",
        skip(self, query),
        fields(
            db = db.as_ref().map(|s| s.db.as_str()).unwrap_or("default"),
            branch = ?db.as_ref().and_then(|s| s.branch.as_ref())
        ),
        err
    )]
    pub async fn query<T: Debug + DeserializeOwned>(
        &self,
        db: Option<BranchSpec>,
        query: Woql2Query, // Changed input type
    ) -> anyhow::Result<WOQLResult<T>> {
        // Serialize the query to JSON-LD here
        let json_query = query.to_instance(None).to_json();
        let woql_context = crate::Context::woql().to_json();
        // self.query_raw(db, Value::Array(vec!(woql_context, json_query))).await // Pass serialized JSON to query_raw
        self.query_raw(db, json_query).await // Pass serialized JSON to query_raw
    }

    // query_raw remains the same, accepting serde_json::Value
    #[instrument(
        name = "terminus.query.execute_raw",
        skip(self, query),
        fields(
            db = spec.as_ref().map(|s| s.db.as_str()).unwrap_or("default"),
            branch = ?spec.as_ref().and_then(|s| s.branch.as_ref())
        ),
        err
    )]
    pub async fn query_raw<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<WOQLResult<T>> {
        let uri = match spec {
            None => self.build_url().endpoint("woql").build(),
            Some(spc) => self
                .build_url()
                .endpoint("woql")
                .simple_database(&spc.db)
                .build(),
        };

        //eprintln!("querying at {}...: {:#?}", &uri, &query);

        let json = json!({
            "query": query
        });

        let json = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        let json = self.parse_response(res).await?;

        trace!("query result: {:#?}", &json);

        Ok(json)
    }

    /// Execute a query from a string that can be either WOQL DSL or JSON-LD format.
    /// 
    /// # Arguments
    /// * `spec` - Optional database and branch specification
    /// * `query_string` - The query as a string (either WOQL DSL or JSON-LD)
    /// 
    /// # Examples
    /// 
    /// ```ignore
    /// // Using WOQL DSL syntax
    /// let results = client.query_string(
    ///     Some(spec),
    ///     r#"select([$Subject, $Predicate, $Object], triple($Subject, $Predicate, $Object))"#
    /// ).await?;
    /// 
    /// // Using JSON-LD format
    /// let results = client.query_string(
    ///     Some(spec),
    ///     r#"{"@type": "Select", "variables": ["Subject"], "query": {"@type": "Triple", ...}}"#
    /// ).await?;
    /// ```
    #[instrument(
        name = "terminus.query.execute_string",
        skip(self, query_string),
        fields(
            db = spec.as_ref().map(|s| s.db.as_str()).unwrap_or("default"),
            branch = ?spec.as_ref().and_then(|s| s.branch.as_ref()),
            format = %if serde_json::from_str::<serde_json::Value>(query_string).is_ok() { "json-ld" } else { "dsl" }
        ),
        err
    )]
    pub async fn query_string<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query_string: &str,
    ) -> anyhow::Result<WOQLResult<T>> {
        // Try to parse as JSON-LD first, then fall back to DSL
        let json_query = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(query_string) {
            // If it's valid JSON, use it directly as the query payload
            json_value
        } else {
            // If it's not valid JSON, parse as WOQL DSL and convert to JSON
            let query = terminusdb_woql_dsl::parse_woql_dsl(query_string)?;
            query.to_json()
        };
        
        self.query_raw(spec, json_query).await
    }

    // query_raw_with_headers - similar to query_raw but captures TerminusDB-Data-Version header
    #[instrument(
        name = "terminus.query.execute_raw_with_headers",
        skip(self, query),
        fields(
            db = spec.as_ref().map(|s| s.db.as_str()).unwrap_or("default"),
            branch = ?spec.as_ref().and_then(|s| s.branch.as_ref())
        ),
        err
    )]
    pub async fn query_raw_with_headers<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<ResponseWithHeaders<WOQLResult<T>>> {
        let uri = match spec {
            None => self.build_url().endpoint("woql").build(),
            Some(spc) => self
                .build_url()
                .endpoint("woql")
                .simple_database(&spc.db)
                .build(),
        };

        //eprintln!("querying at {}...: {:#?}", &uri, &query);

        let json = json!({
            "query": query
        });

        let json = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        let json = self.parse_response_with_headers(res).await?;

        trace!("query result: {:#?}", &json);

        Ok(json)
    }

    /// Execute a query from a string (WOQL DSL or JSON-LD) and capture response headers.
    /// 
    /// Similar to `query_string` but also returns the TerminusDB-Data-Version header
    /// which contains commit information.
    /// 
    /// # Arguments
    /// * `spec` - Optional database and branch specification
    /// * `query_string` - The query as a string (either WOQL DSL or JSON-LD)
    /// 
    /// # Returns
    /// A `ResponseWithHeaders` containing the query results and optional commit_id header
    #[instrument(
        name = "terminus.query.execute_string_with_headers",
        skip(self, query_string),
        fields(
            db = spec.as_ref().map(|s| s.db.as_str()).unwrap_or("default"),
            branch = ?spec.as_ref().and_then(|s| s.branch.as_ref()),
            format = %if serde_json::from_str::<serde_json::Value>(query_string).is_ok() { "json-ld" } else { "dsl" }
        ),
        err
    )]
    pub async fn query_string_with_headers<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query_string: &str,
    ) -> anyhow::Result<ResponseWithHeaders<WOQLResult<T>>> {
        // Try to parse as JSON-LD first, then fall back to DSL
        let json_query = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(query_string) {
            // If it's valid JSON, use it directly as the query payload
            json_value
        } else {
            // If it's not valid JSON, parse as WOQL DSL and convert to JSON
            let query = terminusdb_woql_dsl::parse_woql_dsl(query_string)?;
            query.to_json()
        };
        
        self.query_raw_with_headers(spec, json_query).await
    }

    // todo: roll into ORM-like model
    #[instrument(
        name = "terminus.query.query_instances",
        skip(self, query),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            limit = limit,
            offset = offset
        ),
        err
    )]
    pub async fn query_instances<T: TerminusDBModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
        query: impl InstanceQueryable<Model = T>,
    ) -> anyhow::Result<Vec<T>> {
        query.apply(self, spec, limit, offset).await
    }

    #[instrument(
        name = "terminus.query.query_instances_count",
        skip(self, query),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name()
        ),
        err
    )]
    pub async fn query_instances_count<T: TerminusDBModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        query: impl InstanceQueryable<Model = T>,
    ) -> anyhow::Result<usize> {
        query.count(self, spec).await
    }

    // todo: roll into ORM-like model
    #[instrument(
        name = "terminus.query.list_instances",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            limit = limit,
            offset = offset
        ),
        err
    )]
    pub async fn list_instances<T: TerminusDBModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<T>> {
        self.query_instances(spec, limit, offset, ListModels::<T>::default())
            .await
    }

    /// Count the total number of instances of a specific type in the database
    #[instrument(
        name = "terminus.query.count_instances",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name()
        ),
        err
    )]
    pub async fn count_instances<T: ToTDBSchema>(
        &self,
        spec: &BranchSpec,
    ) -> anyhow::Result<usize> {
        let count_var = vars!("Count");
        let instance_var = vars!("Instance");

        // Build a query to count instances of the specific type using the isa2 shortcut
        let query = WoqlBuilder::new()
            .isa2::<T>(&instance_var)
            .count(count_var.clone())
            .select(vec![count_var.clone()])
            .finalize();

        #[derive(Deserialize, Debug)]
        struct CountResultBinding {
            #[serde(rename = "@value")]
            value: u64,
        }

        // Execute the query
        let result = self
            .query::<std::collections::HashMap<String, CountResultBinding>>(
                Some(spec.clone()),
                query,
            )
            .await?;

        // Extract count from the result
        /*
            parsed typed response: Success(
            WOQLResult {
                api_status: Success,
                api_variable_names: [
                    "Count",
                ],
                bindings: [
                    {
                        "Count": Object {
                            "@type": String("xsd:decimal"),
                            "@value": Number(1),
                        },
                    },
                ],
                deletes: 0,
                inserts: 0,
                transaction_retry_count: 0,
            },
        )
         */
        if let Some(binding) = result.bindings.first() {
            let CountResultBinding { value } = binding
                .get(&*count_var)
                .ok_or_else(|| anyhow::anyhow!("Count variable not found in result"))?;

            return Ok(*value as usize);
        }

        Ok(0)
    }

    /// Execute a raw WOQL query that returns custom result types.
    ///
    /// This method provides a convenient way to execute queries that implement
    /// the `RawQueryable` trait, allowing for custom deserialization logic.
    ///
    /// # Type Parameters
    /// * `Q` - A type implementing `RawQueryable`
    ///
    /// # Arguments
    /// * `spec` - Branch specification for the query
    /// * `query` - The query implementation
    ///
    /// # Returns
    /// A vector of custom result types as defined by the query
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_client::{RawQueryable, RawWoqlQuery};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct PersonSummary {
    ///     name: String,
    ///     total_orders: i32,
    /// }
    ///
    /// struct OrderSummaryQuery;
    ///
    /// impl RawQueryable for OrderSummaryQuery {
    ///     type Result = PersonSummary;
    ///     
    ///     fn query(&self) -> Query {
    ///         WoqlBuilder::new()
    ///             .triple(vars!("Person"), "name", vars!("Name"))
    ///             .triple(vars!("Person"), "orders", vars!("Orders"))
    ///             .count(vars!("TotalOrders"), vars!("Orders"))
    ///             .select(vec![vars!("Name"), vars!("TotalOrders")])
    ///             .finalize()
    ///     }
    /// }
    ///
    /// let summaries = client.execute_raw_query(&spec, OrderSummaryQuery).await?;
    /// ```
    #[instrument(
        name = "terminus.query.execute_raw_custom",
        skip(self, query),
        fields(
            db = %spec.db,
            branch = ?spec.branch
        ),
        err
    )]
    pub async fn execute_raw_query<Q: RawQueryable>(
        &self,
        spec: &BranchSpec,
        query: Q,
    ) -> anyhow::Result<Vec<Q::Result>> {
        query.apply(self, spec).await
    }
}

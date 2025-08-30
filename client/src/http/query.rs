//! Query execution and WOQL operations

use {
    crate::{
        spec::BranchSpec, InstanceFromJson, InstanceQueryable, ListModels, RawQueryable,
        TerminusDBModel, WOQLResult,
        debug::{OperationEntry, OperationType, QueryLogEntry},
    },
    ::tracing::{instrument, trace},
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize},
    serde_json::{json, Value},
    std::{collections::HashMap, fmt::Debug, time::Instant},
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
        let start_time = Instant::now();
        
        // Serialize the query to JSON-LD here
        let json_query = query.to_instance(None).to_json();
        let woql_context = crate::Context::woql().to_json();
        
        // Create operation entry with the query
        let mut operation = OperationEntry::new(
            OperationType::Query,
            format!("/api/woql/{}", db.as_ref().map(|s| s.db.as_str()).unwrap_or("default"))
        ).with_context(
            db.as_ref().map(|s| s.db.clone()),
            db.as_ref().and_then(|s| s.branch.clone())
        ).with_query(query.clone());
        
        // Execute the query
        let result = self.query_raw(db.clone(), json_query.clone()).await;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        // Update operation entry based on result
        match &result {
            Ok(res) => {
                let result_count = res.bindings.len();
                operation = operation.success(Some(result_count), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "query".to_string(),
                        database: db.as_ref().map(|s| s.db.clone()),
                        branch: db.as_ref().and_then(|s| s.branch.clone()),
                        endpoint: operation.endpoint.clone(),
                        details: json_query,
                        success: true,
                        result_count: Some(result_count),
                        duration_ms,
                        error: None,
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
            Err(e) => {
                operation = operation.failure(e.to_string(), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "query".to_string(),
                        database: db.as_ref().map(|s| s.db.clone()),
                        branch: db.as_ref().and_then(|s| s.branch.clone()),
                        endpoint: operation.endpoint.clone(),
                        details: json_query,
                        success: false,
                        result_count: None,
                        duration_ms,
                        error: Some(e.to_string()),
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
        }
        
        // Add to operation log
        self.operation_log.push(operation);
        
        result
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
        let start_time = Instant::now();
        
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

        let json_string = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json_string);

        let res = self
            .http
            .post(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json_string)
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
        let start_time = Instant::now();
        
        // Try to parse as JSON-LD first, then fall back to DSL
        let (json_query, parsed_query) = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(query_string) {
            // If it's valid JSON, use it directly as the query payload
            // Try to parse it back to a Query for storage, but don't fail if it can't be parsed
            let query_opt = serde_json::from_value::<Woql2Query>(json_value.clone()).ok();
            (json_value, query_opt)
        } else {
            // If it's not valid JSON, parse as WOQL DSL and convert to JSON
            let query = terminusdb_woql_dsl::parse_woql_dsl(query_string)?;
            let json = query.to_json();
            (json, Some(query))
        };
        
        // Create operation entry
        let mut operation = OperationEntry::new(
            OperationType::Query,
            format!("/api/woql/{}", spec.as_ref().map(|s| s.db.as_str()).unwrap_or("default"))
        ).with_context(
            spec.as_ref().map(|s| s.db.clone()),
            spec.as_ref().and_then(|s| s.branch.clone())
        );
        
        // Add the parsed query if we have one
        if let Some(query) = parsed_query.clone() {
            operation = operation.with_query(query);
        }
        
        let result = self.query_raw(spec.clone(), json_query.clone()).await;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        // Update operation entry based on result
        match &result {
            Ok(res) => {
                let result_count = res.bindings.len();
                operation = operation.success(Some(result_count), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "query_string".to_string(),
                        database: spec.as_ref().map(|s| s.db.clone()),
                        branch: spec.as_ref().and_then(|s| s.branch.clone()),
                        endpoint: operation.endpoint.clone(),
                        details: json!({"query_string": query_string, "parsed": json_query}),
                        success: true,
                        result_count: Some(result_count),
                        duration_ms,
                        error: None,
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
            Err(e) => {
                operation = operation.failure(e.to_string(), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "query_string".to_string(),
                        database: spec.as_ref().map(|s| s.db.clone()),
                        branch: spec.as_ref().and_then(|s| s.branch.clone()),
                        endpoint: operation.endpoint.clone(),
                        details: json!({"query_string": query_string}),
                        success: false,
                        result_count: None,
                        duration_ms,
                        error: Some(e.to_string()),
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
        }
        
        self.operation_log.push(operation);
        
        result
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
        let (json_query, parsed_query) = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(query_string) {
            // If it's valid JSON, use it directly as the query payload
            // Try to parse it back to a Query for storage, but don't fail if it can't be parsed
            let query_opt = serde_json::from_value::<Woql2Query>(json_value.clone()).ok();
            (json_value, query_opt)
        } else {
            // If it's not valid JSON, parse as WOQL DSL and convert to JSON
            let query = terminusdb_woql_dsl::parse_woql_dsl(query_string)?;
            let json = query.to_json();
            (json, Some(query))
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

    /// List instances of a specific type with field-value filter conditions.
    ///
    /// This method provides server-side filtering by translating the filter conditions
    /// into WOQL triple patterns, which is much more efficient than client-side filtering.
    ///
    /// # Type Parameters
    /// * `T` - The TerminusDB model type to query
    /// * `I` - Iterator type for filters
    /// * `K` - Field name type (anything that converts to String)
    /// * `V` - Value type (anything implementing IntoDataValue)
    ///
    /// # Arguments
    /// * `spec` - Database and branch specification
    /// * `limit` - Maximum number of results to return
    /// * `offset` - Number of results to skip
    /// * `filters` - Iterator of (field_name, value) pairs for filtering
    ///
    /// # Returns
    /// A vector of instances matching all filter conditions
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_client::prelude::*;
    /// 
    /// // Filter with various data types
    /// let active_adults = client.list_instances_where::<Person>(
    ///     &spec,
    ///     Some(10),  // limit
    ///     None,      // offset
    ///     vec![
    ///         ("status", "active"),      // String
    ///         ("age", 25),               // Integer
    ///         ("verified", true),        // Boolean
    ///     ],
    /// ).await?;
    /// 
    /// // Or use data! macro for explicit types
    /// let recent_users = client.list_instances_where::<User>(
    ///     &spec,
    ///     None,
    ///     None,
    ///     vec![
    ///         ("created_at", datetime!("2024-01-01T00:00:00Z")),
    ///         ("role", "admin"),
    ///     ],
    /// ).await?;
    /// ```
    #[instrument(
        name = "terminus.query.list_instances_where",
        skip(self, filters),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            limit = limit,
            offset = offset,
            filter_count = tracing::field::Empty
        ),
        err
    )]
    pub async fn list_instances_where<T, I, K, V>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
        filters: I,
    ) -> anyhow::Result<Vec<T>>
    where
        T: TerminusDBModel + InstanceFromJson,
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: terminusdb_woql2::prelude::IntoDataValue,
    {
        use crate::query::FilteredListModels;
        
        let query = FilteredListModels::<T>::new(filters);
        let filter_count = query.filters.len();
        
        // Record the filter count in the trace
        tracing::Span::current().record("filter_count", filter_count);
        
        self.query_instances(spec, limit, offset, query).await
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

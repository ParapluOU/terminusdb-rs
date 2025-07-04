//! Query execution and WOQL operations

use {
    crate::{
        spec::BranchSpec, 
        WOQLResult, 
        InstanceFromJson, 
        TerminusDBModel, 
        InstanceQueryable, 
        ListModels,
    },
    ::log::trace,
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize},
    serde_json::{json, Value},
    std::{collections::HashMap, fmt::Debug},
    terminusdb_schema::{ToTDBInstance, ToTDBSchema, ToJson},
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
    pub async fn query_raw<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<WOQLResult<T>> {
        let uri = match spec {
            None => {
                self.build_url().endpoint("woql").build()
            }
            Some(spc) => {
                self.build_url().endpoint("woql").simple_database(&spc.db).build()
            }
        };

        eprintln!("querying at {}...: {:#?}", &uri, &query);

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

    // query_raw_with_headers - similar to query_raw but captures TerminusDB-Data-Version header
    pub async fn query_raw_with_headers<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<ResponseWithHeaders<WOQLResult<T>>> {
        let uri = match spec {
            None => {
                self.build_url().endpoint("woql").build()
            }
            Some(spc) => {
                self.build_url().endpoint("woql").simple_database(&spc.db).build()
            }
        };

        eprintln!("querying at {}...: {:#?}", &uri, &query);

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

    // todo: roll into ORM-like model
    pub async fn query_instances<T: TerminusDBModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
        query: impl InstanceQueryable<Model = T>,
    ) -> anyhow::Result<Vec<T>> {
        query.apply(self, spec, limit, offset).await
    }

    // todo: roll into ORM-like model
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
}
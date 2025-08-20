//! Core HTTP client struct and constructors

#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;

use {
    crate::{Info, TerminusDBAdapterError},
    ::tracing::{debug, instrument},
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::{fmt::Debug, sync::{Arc, RwLock}},
    terminusdb_schema::{ToTDBInstance, ToJson},
    terminusdb_woql2::prelude::Query,
    url::Url,
};

use super::url_builder::UrlBuilder;

#[derive(Clone, Debug)]
pub struct TerminusDBHttpClient {
    pub endpoint: Url,
    // Use conditional compilation for the http client
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) http: Client,
    /// user auth for this user
    pub(crate) user: String,
    /// this user's password
    pub(crate) pass: String,
    /// organization that we are logging in for
    pub(crate) org: String,
    /// stores the last executed WOQL query for debugging purposes
    last_query: Arc<RwLock<Option<Query>>>,
}

// Wrap the entire impl block with a conditional compilation attribute
#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Creates a client connected to a local TerminusDB instance.
    ///
    /// This is a convenience constructor that connects to `http://localhost:6363`
    /// using default admin credentials. Ideal for development and testing.
    ///
    /// # Returns
    /// A client instance connected to the local TerminusDB server
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// ```
    ///
    /// # Equivalent to
    /// ```rust
    /// TerminusDBHttpClient::new(
    ///     Url::parse("http://localhost:6363").unwrap(),
    ///     "admin", "root", "admin"
    /// ).await.unwrap()
    /// ```
    #[instrument(name = "terminus.client.local_node")]
    pub async fn local_node() -> Self {
        Self::new(
            Url::parse("http://localhost:6363").unwrap(),
            "admin",
            "root",
            "admin",
        )
        .await
        .unwrap()
    }

    #[instrument(name = "terminus.client.local_node_with_database", fields(db = %db))]
    pub async fn local_node_with_database(db: &str) -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database(db).await
    }

    /// Creates a client connected to a local TerminusDB instance with a test database.
    ///
    /// This is a convenience constructor that connects to a local TerminusDB server
    /// and ensures a "test" database exists. Ideal for integration tests and development.
    ///
    /// # Returns
    /// A client instance connected to the local TerminusDB server with "test" database
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node_test().await?;
    /// // Ready to use with "test" database
    /// ```
    #[instrument(name = "terminus.client.local_node_test")]
    pub async fn local_node_test() -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database("test").await
    }

    /// Creates a new TerminusDB HTTP client with custom connection parameters.
    ///
    /// # Arguments
    /// * `endpoint` - The TerminusDB server endpoint URL (will have "/api" appended)
    /// * `user` - Username for authentication
    /// * `pass` - Password for authentication  
    /// * `org` - Organization name
    ///
    /// # Returns
    /// A configured client instance
    ///
    /// # Example
    /// ```rust
    /// use url::Url;
    ///
    /// let client = TerminusDBHttpClient::new(
    ///     Url::parse("https://my-terminusdb.com").unwrap(),
    ///     "my_user",
    ///     "my_password",
    ///     "my_org"
    /// ).await?;
    /// ```
    #[instrument(
        name = "terminus.client.new",
        skip(pass),
        fields(
            endpoint = %endpoint,
            user = %user,
            org = %org
        ),
        err
    )]
    pub async fn new(mut endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        let err = format!("Cannot modify segments for endpoint: {}", &endpoint);

        endpoint.path_segments_mut().expect(&err).push("api");

        Ok(Self {
            user: user.to_string(),
            pass: pass.to_string(),
            endpoint,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            org: org.to_string(),
            last_query: Arc::new(RwLock::new(None)),
        })
    }

    #[instrument(
        name = "terminus.client.new_with_database",
        skip(pass),
        fields(
            endpoint = %endpoint,
            user = %user,
            org = %org,
            db = %db
        ),
        err
    )]
    pub async fn new_with_database(
        endpoint: Url,
        user: &str,
        pass: &str,
        db: &str,
        org: &str,
    ) -> anyhow::Result<Self> {
        let client = Self::new(endpoint, user, pass, org).await?;
        client.ensure_database(db).await
    }

    /// Returns a clone of the last executed WOQL query for debugging purposes.
    ///
    /// This method provides access to the most recently executed query, which can be
    /// useful for debugging, logging, or re-executing queries.
    ///
    /// # Returns
    /// `Some(Query)` if a query has been executed, `None` otherwise
    ///
    /// # Example
    /// ```ignore
    /// let client = TerminusDBHttpClient::local_node().await;
    /// 
    /// // Execute a query
    /// let query = Query::select().triple("v:Subject", "rdf:type", "owl:Class");
    /// client.query(Some(spec), query).await?;
    /// 
    /// // Retrieve the last executed query
    /// if let Some(last_query) = client.last_query() {
    ///     println!("Last query: {:?}", last_query);
    /// }
    /// ```
    pub fn last_query(&self) -> Option<Query> {
        self.last_query.read().ok().and_then(|guard| guard.clone())
    }

    /// Returns the last executed WOQL query as JSON for debugging purposes.
    ///
    /// This method converts the last executed query to its JSON-LD representation,
    /// which can be useful for debugging, API inspection, or external tools.
    ///
    /// # Returns
    /// `Some(serde_json::Value)` if a query has been executed, `None` otherwise
    ///
    /// # Example
    /// ```ignore
    /// let client = TerminusDBHttpClient::local_node().await;
    /// 
    /// // Execute a query
    /// let query = Query::select().triple("v:Subject", "rdf:type", "owl:Class");
    /// client.query(Some(spec), query).await?;
    /// 
    /// // Retrieve the last executed query as JSON
    /// if let Some(last_query_json) = client.last_query_json() {
    ///     println!("Last query JSON: {}", serde_json::to_string_pretty(&last_query_json).unwrap());
    /// }
    /// ```
    pub fn last_query_json(&self) -> Option<serde_json::Value> {
        self.last_query().map(|query| query.to_instance(None).to_json())
    }

    /// Internal method to store a query for debugging purposes
    pub(crate) fn store_last_query(&self, query: Query) {
        if let Ok(mut last_query) = self.last_query.write() {
            *last_query = Some(query);
        }
    }

    /// Test-only method to store a query for debugging purposes
    #[cfg(test)]
    pub fn test_store_last_query(&self, query: Query) {
        self.store_last_query(query);
    }

    /// Centralized URL builder for TerminusDB API endpoints.
    /// Handles all URL construction patterns and eliminates duplication.
    pub(crate) fn build_url(&self) -> UrlBuilder {
        UrlBuilder::new(&self.endpoint, &self.org)
    }

    #[instrument(
        name = "terminus.client.info",
        skip(self),
        fields(
            endpoint = %self.endpoint,
            org = %self.org
        ),
        err
    )]
    #[pseudonym::alias(verify_connection)]
    pub async fn info(&self) -> anyhow::Result<Info> {
        let uri = self.build_url().endpoint("info").build();
        debug!(
            "📡 Making HTTP request to TerminusDB info endpoint: {}",
            &uri
        );

        let res = self
            .http
            .get(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context(format!("failed to parse response for {}", &uri))?;

        debug!("📨 Received response from TerminusDB, parsing...");
        self.parse_response(res).await
    }

    #[instrument(
        name = "terminus.client.is_running",
        skip(self),
        fields(
            endpoint = %self.endpoint
        )
    )]
    pub async fn is_running(&self) -> bool {
        self.info().await.is_ok()
    }
}

// Add a separate impl block for WASM
#[cfg(target_arch = "wasm32")]
impl TerminusDBHttpClient {
    // Implement a stub or alternative implementation for WASM
    // This is just a basic example, you'll need to adjust based on your needs
    pub async fn new(endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint,
            user: user.to_string(),
            pass: pass.to_string(),
            org: org.to_string(),
        })
    }

    // Implement other methods as needed for WASM
}

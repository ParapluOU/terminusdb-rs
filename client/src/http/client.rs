//! Core HTTP client struct and constructors

#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;

use {
    crate::{Info, TerminusDBAdapterError},
    ::log::debug,
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::fmt::Debug,
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
        })
    }

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

    /// Centralized URL builder for TerminusDB API endpoints.
    /// Handles all URL construction patterns and eliminates duplication.
    pub(crate) fn build_url(&self) -> UrlBuilder {
        UrlBuilder::new(&self.endpoint, &self.org)
    }

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

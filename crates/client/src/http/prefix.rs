//! Prefix management API client (TerminusDB 12): CRUD over the JSON-LD context
//! prefixes of a database at `/api/prefix/{org}/{db}/{prefix}` and the whole
//! context at `/api/prefixes/{org}/{db}`.
//!
//! See `docs/terminusdb/prefix-management.md`.

use anyhow::Context;
use serde::Deserialize;

use crate::spec::BranchSpec;
use crate::TerminusDBHttpClient;

/// A successful prefix response (`api:PrefixResponse` / `api:PrefixAddResponse`
/// / `api:PrefixUpdateResponse` / `api:PrefixDeleteResponse`).
#[derive(Debug, Clone, Deserialize)]
pub struct PrefixResponse {
    #[serde(rename = "api:status")]
    pub status: String,
    #[serde(rename = "api:prefix_name")]
    pub prefix_name: Option<String>,
    #[serde(rename = "api:prefix_uri")]
    pub prefix_uri: Option<String>,
}

impl PrefixResponse {
    pub fn is_success(&self) -> bool {
        self.status == "api:success"
    }
}

impl TerminusDBHttpClient {
    fn prefix_url(&self, spec: &BranchSpec, name: &str) -> String {
        self.build_url()
            .endpoint("prefix")
            .simple_database(&spec.db)
            .add_path(name)
            .build()
    }

    /// GET the IRI a prefix expands to. Errors if the prefix does not exist.
    pub async fn get_prefix(&self, spec: &BranchSpec, name: &str) -> anyhow::Result<String> {
        let res = self
            .http
            .get(self.prefix_url(spec, name))
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to send prefix GET")?;
        let parsed: PrefixResponse = self.parse_response(res).await?;
        parsed
            .prefix_uri
            .ok_or_else(|| anyhow::anyhow!("prefix response missing api:prefix_uri"))
    }

    /// POST a new prefix. Fails if the prefix already exists (use
    /// [`Self::upsert_prefix`] for create-or-update).
    pub async fn add_prefix(
        &self,
        spec: &BranchSpec,
        name: &str,
        uri: &str,
    ) -> anyhow::Result<PrefixResponse> {
        let _permit = self.acquire_write_permit().await;
        let res = self
            .http
            .post(self.prefix_url(spec, name))
            .basic_auth(&self.user, Some(&self.pass))
            .json(&serde_json::json!({ "uri": uri }))
            .send()
            .await
            .context("failed to send prefix POST")?;
        self.parse_response(res).await
    }

    /// PUT an existing prefix to a new IRI. With `create`, creates it if absent
    /// (`?create=true`, always succeeds).
    async fn put_prefix(
        &self,
        spec: &BranchSpec,
        name: &str,
        uri: &str,
        create: bool,
    ) -> anyhow::Result<PrefixResponse> {
        let mut builder = self
            .build_url()
            .endpoint("prefix")
            .simple_database(&spec.db)
            .add_path(name);
        if create {
            builder = builder.query("create", "true");
        }
        let _permit = self.acquire_write_permit().await;
        let res = self
            .http
            .put(builder.build())
            .basic_auth(&self.user, Some(&self.pass))
            .json(&serde_json::json!({ "uri": uri }))
            .send()
            .await
            .context("failed to send prefix PUT")?;
        self.parse_response(res).await
    }

    /// Update an existing prefix. Fails if the prefix does not exist.
    pub async fn update_prefix(
        &self,
        spec: &BranchSpec,
        name: &str,
        uri: &str,
    ) -> anyhow::Result<PrefixResponse> {
        self.put_prefix(spec, name, uri, false).await
    }

    /// Create or update a prefix (idempotent; always succeeds).
    pub async fn upsert_prefix(
        &self,
        spec: &BranchSpec,
        name: &str,
        uri: &str,
    ) -> anyhow::Result<PrefixResponse> {
        self.put_prefix(spec, name, uri, true).await
    }

    /// DELETE a prefix.
    pub async fn delete_prefix(
        &self,
        spec: &BranchSpec,
        name: &str,
    ) -> anyhow::Result<PrefixResponse> {
        let _permit = self.acquire_write_permit().await;
        let res = self
            .http
            .delete(self.prefix_url(spec, name))
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to send prefix DELETE")?;
        self.parse_response(res).await
    }
}

// The full JSON-LD context (all prefixes) at `/api/prefixes/{org}/{db}` is
// available via the pre-existing `get_prefixes(path)` on the database client.

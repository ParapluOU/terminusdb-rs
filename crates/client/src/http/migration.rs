//! Schema migration API client (`POST /api/migration/{org}/{db}`), new in
//! TerminusDB 12. Lets you evolve a schema — add/rename/retype/re-key
//! properties and classes — as an ordered list of operations, optionally with
//! `dry_run` validation. This replaces the "drop the database on schema
//! failure" workaround for schema changes on databases that already hold data.
//!
//! See `docs/terminusdb/schema-migration-reference-guide.md`.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::spec::BranchSpec;
use crate::TerminusDBHttpClient;

/// A single schema-migration operation. Serializes to the `@type`-tagged JSON
/// the migration API expects, e.g. `{"@type":"DeleteClass","class":"Product"}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum MigrationOperation {
    /// Add a new class from a full class document (weakening).
    CreateClass { class_document: Value },
    /// Remove a class (its properties must already be dropped) (strengthening).
    DeleteClass { class: String },
    /// Rename a class.
    MoveClass { from: String, to: String },
    /// Replace a class's `@metadata` (weakening).
    ReplaceClassMetadata { class: String, metadata: Value },
    /// Replace a class's `@documentation` (weakening).
    ReplaceClassDocumentation { class: String, documentation: Value },
    /// Replace the schema `@context`.
    ReplaceContext { context: Value },
    /// Add values to an existing enum (weakening).
    ExpandEnum {
        #[serde(rename = "enum")]
        enum_name: String,
        values: Vec<String>,
    },
    /// Add a property to a class. `default` is required when the property is
    /// mandatory (a strengthening that must fill existing instances).
    CreateClassProperty {
        class: String,
        property: String,
        #[serde(rename = "type")]
        property_type: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<Value>,
    },
    /// Remove a property from a class (strengthening).
    DeleteClassProperty { class: String, property: String },
    /// Rename a property on a class.
    MoveClassProperty {
        class: String,
        from: String,
        to: String,
    },
    /// Widen a property's type without touching instance data (weakening).
    UpcastClassProperty {
        class: String,
        property: String,
        #[serde(rename = "type")]
        property_type: Value,
    },
    /// Change a property's type, transforming instance data. `default` is a
    /// `{"@type":"Default","value":..}` or `{"@type":"Error"}`.
    CastClassProperty {
        class: String,
        property: String,
        #[serde(rename = "type")]
        property_type: Value,
        default: Value,
    },
    /// Change a class's key strategy. `fields` applies to Lexical/Hash keys.
    ChangeKey {
        class: String,
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<String>>,
    },
}

/// Options controlling how a migration is applied.
#[derive(Debug, Clone, Default)]
pub struct MigrationOptions {
    /// Validate the operations without committing any change.
    pub dry_run: bool,
    /// Ask the server for a verbose report of what the migration did.
    pub verbose: bool,
}

impl MigrationOptions {
    /// A dry-run (validate only, commit nothing).
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            verbose: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MigrationRequest<'a> {
    author: &'a str,
    message: &'a str,
    operations: &'a [MigrationOperation],
}

/// The `api:MigrationResponse` envelope. Extra server fields (e.g. a verbose
/// report, the schema-operation count) are preserved in `extra`.
#[derive(Debug, Clone, Deserialize)]
pub struct MigrationResponse {
    #[serde(rename = "api:status")]
    pub status: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl MigrationResponse {
    /// Whether the server reported `api:success`.
    pub fn is_success(&self) -> bool {
        self.status == "api:success"
    }
}

impl TerminusDBHttpClient {
    /// Apply an ordered list of schema-migration operations to the database in
    /// `spec`. With `options.dry_run`, the server validates the operations and
    /// commits nothing.
    ///
    /// Operations are order-dependent; different orderings can produce different
    /// instance-data transformations.
    pub async fn migrate_schema(
        &self,
        spec: &BranchSpec,
        author: &str,
        message: &str,
        operations: Vec<MigrationOperation>,
        options: MigrationOptions,
    ) -> anyhow::Result<MigrationResponse> {
        let uri = self
            .build_url()
            .endpoint("migration")
            .database(spec)
            .build();

        let body = MigrationRequest {
            author,
            message,
            operations: &operations,
        };

        // Boolean query params are only sent when set (the server defaults both
        // to false).
        let mut query: Vec<(&str, &str)> = Vec::new();
        if options.dry_run {
            query.push(("dry_run", "true"));
        }
        if options.verbose {
            query.push(("verbose", "true"));
        }

        let _permit = self.acquire_write_permit().await;

        let request = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .query(&query)
            .json(&body);

        let res = request
            .send()
            .await
            .context("failed to send schema migration request")?;

        self.parse_response::<MigrationResponse>(res).await
    }
}

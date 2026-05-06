//! Source-of-truth GraphQL schema fetch, via a live TerminusDB instance.
//!
//! Hand-emitting SDL from Rust models (`generate_gql_schema`) is fast but
//! drifts from what TDB actually serves: bugs in the emission code, or
//! changes in TDB's filter/ordering shapes, surface as malformed SDL with
//! dangling type references. This module avoids that by spinning up a
//! TerminusDB instance, inserting the requested model schema tuple, and
//! running the standard GraphQL introspection query against it. The
//! response is the canonical schema TDB would expose to clients.
//!
//! Trade-off: needs `terminusdb-bin` to be on PATH (or built into the
//! workspace) and the boot/teardown takes a few hundred milliseconds —
//! fine for build steps and tests, not for hot paths.
//!
//! ```no_run
//! # use terminusdb_schema_derive::TerminusDBModel;
//! # use terminusdb_schema::ToTDBInstance;
//! # #[derive(TerminusDBModel, Clone, Debug)]
//! # struct Project { name: String }
//! # async fn demo() -> anyhow::Result<()> {
//! let json = terminusdb_gql::introspect_schema_for::<(Project,)>("schema-dump").await?;
//! // `json` is the standard `__Schema` introspection envelope, suitable
//! // for piping into apollo-compiler, GraphiQL, schema diff tools, etc.
//! # Ok(()) }
//! ```

use serde_json::Value;
use std::time::Duration;
use terminusdb_bin::TerminusDBServer;
use terminusdb_schema::ToTDBSchemas;

/// Insert the schema tuple `T` into a temporary database on a shared test
/// TerminusDB instance, run a GraphQL introspection query, and return the
/// raw `data` field of the response (the standard `__Schema` envelope).
///
/// `prefix` is prepended to the temporary database name (a UUID is added
/// so parallel calls don't collide). The database is automatically dropped
/// when this returns, even on error.
///
/// The shared test instance is reused across calls in the same process,
/// so paying the boot cost is a once-per-process expense.
pub async fn introspect_schema_for<T: ToTDBSchemas>(
    prefix: &str,
) -> anyhow::Result<Value> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<T, _, _, _>(prefix, |client, spec| async move {
            // 60s timeout is generous; introspection is cheap but the
            // first request after schema insertion can be slow if TDB is
            // still indexing on a cold cache.
            let result = client
                .introspect_schema(
                    &spec.db,
                    spec.branch.as_deref(),
                    Some(Duration::from_secs(60)),
                )
                .await
                .map_err(|e| anyhow::anyhow!("introspection failed: {e}"))?;
            Ok(result)
        })
        .await
}

/// Convenience: introspect, then render the result as SDL. Equivalent to
/// `render_introspection_to_sdl(introspect_schema_for(prefix).await?)?`
/// but in a single call. The returned SDL is internally consistent — it
/// came out of a live server, and cynic just printed it back — so it
/// parses cleanly with apollo-compiler.
pub async fn introspect_schema_sdl_for<T: ToTDBSchemas>(
    prefix: &str,
) -> anyhow::Result<String> {
    let json = introspect_schema_for::<T>(prefix).await?;
    crate::render::render_introspection_to_sdl(&json)
}

/// Same as `introspect_schema_for` but takes a connected schema closure
/// instead of returning the JSON, so the caller can issue follow-up
/// queries against the same temporary database.
///
/// Useful for rendering SDL via apollo-compiler, comparing the introspected
/// schema against a stored fixture, or running test queries to verify a
/// resolver shape works against the canonical filter/ordering inputs.
pub async fn with_introspected_schema<T, F, Fut, R>(
    prefix: &str,
    f: F,
) -> anyhow::Result<R>
where
    T: ToTDBSchemas,
    F: FnOnce(
        terminusdb_client::TerminusDBHttpClient,
        terminusdb_client::BranchSpec,
        Value,
    ) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<R>>,
{
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<T, _, _, _>(prefix, |client, spec| async move {
            let introspection = client
                .introspect_schema(
                    &spec.db,
                    spec.branch.as_deref(),
                    Some(Duration::from_secs(60)),
                )
                .await
                .map_err(|e| anyhow::anyhow!("introspection failed: {e}"))?;
            f(client, spec, introspection).await
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    // The TerminusDBModel derive expands to method calls on traits that
    // aren't in the prelude — pull them into scope here.
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    // ToTDBSchemas tuple impls start at arity 2, so we deliberately use a
    // two-element fixture. Both types are tiny to keep schema insertion fast.
    #[derive(TerminusDBModel, Clone, Debug)]
    struct Project {
        pub name: String,
    }

    #[derive(TerminusDBModel, Clone, Debug)]
    struct Ticket {
        pub title: String,
    }

    /// Smoke test: confirms the helper round-trips against a real server
    /// and produces an introspection envelope with `__schema.types`.
    #[tokio::test]
    async fn introspects_simple_schema() -> anyhow::Result<()> {
        let result = introspect_schema_for::<(Project, Ticket)>("introspect_smoke").await?;
        let schema = result
            .get("__schema")
            .ok_or_else(|| anyhow::anyhow!("missing __schema in introspection: {result}"))?;
        let types = schema
            .get("types")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing __schema.types"))?;
        // Don't assert on specific Project fields — TDB augments every
        // type with `_id` / `_type` / `_json` plus filter / ordering
        // inputs. Presence of our model classes by name is enough.
        assert!(
            types.iter().any(|t| t.get("name").and_then(|n| n.as_str()) == Some("Project")),
            "Project type missing from introspection result"
        );
        assert!(
            types.iter().any(|t| t.get("name").and_then(|n| n.as_str()) == Some("Ticket")),
            "Ticket type missing from introspection result"
        );
        Ok(())
    }
}

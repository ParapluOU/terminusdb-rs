#![recursion_limit = "256"]
//! Live test of the TerminusDB 12 GraphQL `_count` field via the client's
//! `count_documents`: it returns the number of matching documents without
//! fetching them.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Widget {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[tokio::test]
    async fn test_v12_graphql_count() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("v12_count", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                for i in 0..5 {
                    client
                        .insert_instance(
                            &Widget {
                                id: EntityIDFor::new(&format!("w{i}")).unwrap(),
                                name: format!("Widget {i}"),
                            },
                            args.clone(),
                        )
                        .await?;
                }

                // Server-side count of all Widgets — no rows fetched.
                let count = client
                    .count_documents(&spec.db, spec.branch.as_deref(), "Widget", "{}")
                    .await?;
                assert_eq!(count, 5, "server _count should report 5 Widgets");
                Ok(())
            })
            .await
    }
}

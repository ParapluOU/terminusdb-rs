#![recursion_limit = "256"]
//! Live test of the data-version diff form: diff two commits (identified by the
//! TerminusDB-Data-Version returned on each write) for a single document.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Thing {
        id: EntityIDFor<Self>,
        val: String,
    }

    #[tokio::test]
    async fn test_v12_diff_data_versions() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Thing,), _, _, _>("v12_diff_versions", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Commit 1: val = "one".
                let r1 = client
                    .insert_instance(
                        &Thing { id: EntityIDFor::new("x").unwrap(), val: "one".to_string() },
                        args.clone(),
                    )
                    .await?;
                let v1 = r1.commit_id.clone().expect("data-version 1").to_string();

                // Commit 2: val = "two" (update).
                let r2 = client
                    .insert_instance(
                        &Thing { id: EntityIDFor::new("x").unwrap(), val: "two".to_string() },
                        args.clone(),
                    )
                    .await?;
                let v2 = r2.commit_id.clone().expect("data-version 2").to_string();

                // Diff the single document between the two data versions.
                let diff = client
                    .diff_data_versions(&spec, &v1, &v2, Some("Thing/x"), DiffOptions::default())
                    .await?;

                let text = serde_json::to_string(&diff)?;
                println!("data-version diff: {text}");
                assert!(!diff.is_null(), "diff should not be null");
                // The change from "one" to "two" must appear somewhere in the diff.
                assert!(text.contains("two"), "diff should reflect the new value");
                Ok(())
            })
            .await
    }
}

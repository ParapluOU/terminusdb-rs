#![recursion_limit = "256"]
//! Live test of the apply endpoint: create a feature branch, add a document on
//! it, then apply the main→feature diff onto main (squash-merge) and confirm the
//! document lands on main.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Note {
        id: EntityIDFor<Self>,
        text: String,
    }

    #[tokio::test]
    async fn test_v12_apply_cherry_pick() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Note,), _, _, _>("v12_apply", |client, spec| async move {
                let org = "admin";
                let db = &spec.db;

                // Create a "feature" branch off main.
                client
                    .create_branch(
                        &format!("{org}/{db}/local/branch/feature"),
                        &format!("{org}/{db}/local/branch/main"),
                    )
                    .await?;

                // Add a Note only on feature.
                let feature = BranchSpec::with_branch(db.clone(), "feature");
                client
                    .insert_instance(
                        &Note {
                            id: EntityIDFor::new("n1").unwrap(),
                            text: "from feature".to_string(),
                        },
                        DocumentInsertArgs::from(feature.clone()),
                    )
                    .await?;

                // main does not have it yet.
                let main = BranchSpec::with_branch(db.clone(), "main");
                assert!(
                    client
                        .get_document("Note/n1", &main, GetOpts::default())
                        .await
                        .is_err(),
                    "Note/n1 should not be on main before apply"
                );

                // Apply the main→feature diff onto main.
                client
                    .apply_commit_diff(
                        &main,
                        "main",
                        "feature",
                        "tester",
                        "squash-merge feature into main",
                        ApplyOptions::default(),
                    )
                    .await?;

                // Now it's on main.
                let doc = client
                    .get_document("Note/n1", &main, GetOpts::default())
                    .await?;
                assert_eq!(doc.get("text").and_then(|v| v.as_str()), Some("from feature"));
                Ok(())
            })
            .await
    }
}

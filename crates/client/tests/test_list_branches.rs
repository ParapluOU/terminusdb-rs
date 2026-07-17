//! Live tests for `list_branches` / `branch_exists`.
//!
//! Verifies branch enumeration against a real embedded TerminusDB: a fresh
//! database reports `["main"]`, and the list reflects `create_branch` /
//! `delete_branch` immediately.
#![recursion_limit = "256"]

use terminusdb_bin::TerminusDBServer;

#[tokio::test]
async fn list_branches_reflects_create_and_delete() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_list_branches", |client, spec| async move {
            let db = spec.db.clone();
            let org = client.org().to_string();

            // A fresh database has exactly `main`.
            let branches = client.list_branches(&db).await?;
            assert_eq!(branches, vec!["main".to_string()], "fresh db → [main]");
            assert!(client.branch_exists(&db, "main").await?);
            assert!(!client.branch_exists(&db, "feature").await?);

            // Create a branch off main; enumeration reflects it immediately.
            client
                .create_branch(
                    &format!("{org}/{db}/local/branch/feature"),
                    &format!("{org}/{db}/local/branch/main"),
                )
                .await?;
            let mut branches = client.list_branches(&db).await?;
            branches.sort();
            assert_eq!(branches, vec!["feature".to_string(), "main".to_string()]);
            assert!(client.branch_exists(&db, "feature").await?);

            // Delete it; we're back to just main.
            client
                .delete_branch(&format!("{org}/{db}/local/branch/feature"))
                .await?;
            assert_eq!(client.list_branches(&db).await?, vec!["main".to_string()]);
            assert!(!client.branch_exists(&db, "feature").await?);

            Ok(())
        })
        .await
}

#[tokio::test]
async fn list_branches_errors_for_missing_db() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    let client = server.client().await?;
    let result = client.list_branches("no_such_database_xyz").await;
    assert!(result.is_err(), "missing db should error, got {result:?}");
    Ok(())
}

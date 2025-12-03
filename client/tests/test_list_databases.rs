#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;

    #[tokio::test]
    async fn test_list_databases() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // List all databases without branches and verbose info
        let databases = client.list_databases(false, false).await?;

        // Should have at least the system database
        assert!(!databases.is_empty(), "Should have at least one database");

        // Check that we got valid database objects
        for db in &databases {
            // The actual response uses 'path' as the main identifier
            assert!(db.path.is_some(), "Database path should be present");

            let path = db.path.as_ref().unwrap();
            println!("Found database: {}", path);

            // Additional fields may be present when verbose=true
            if let Some(name) = &db.name {
                println!("  Name: {}", name);
            }
            if let Some(id) = &db.id {
                println!("  ID: {}", id);
            }
            if let Some(db_type) = &db.database_type {
                println!("  Type: {}", db_type);
            }
            if let Some(comment) = &db.comment {
                println!("  Comment: {}", comment);
            }
            if let Some(state) = &db.state {
                println!("  State: {}", state);
            }
            if let Some(branches) = &db.branches {
                println!("  Branches: {:?}", branches);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_databases_simple() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Use the simple method
        let databases = client.list_databases_simple().await?;

        assert!(!databases.is_empty(), "Should have at least one database");

        // Check that all databases have paths
        for db in &databases {
            assert!(db.path.is_some(), "Database path should be present");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_databases_with_branches() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // List databases with branch information
        let databases = client.list_databases(true, false).await?;

        assert!(!databases.is_empty(), "Should have at least one database");

        // With branches=true, the response should include branch information
        for db in &databases {
            assert!(db.path.is_some(), "Database path should be present");

            // When branches=true, we should get branch information
            if let Some(branches) = &db.branches {
                println!("Database {} has branches: {:?}", db.path.as_ref().unwrap(), branches);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_databases_verbose() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // List databases with verbose information
        let databases = client.list_databases(false, true).await?;

        assert!(!databases.is_empty(), "Should have at least one database");

        // With verbose=true, we should get all available fields populated
        for db in &databases {
            println!("Verbose database info: {:?}", db);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_databases_after_creation() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server.with_tmp_db("test_list_db", |client, _spec| async move {
            // List databases and check if our test database is there
            let databases = client.list_databases_simple().await?;

            // The database was just created by with_tmp_db, so it should exist
            let test_db = databases.iter()
                .find(|db| db.path.as_ref().map(|p| p.contains("test_list_db")).unwrap_or(false));

            assert!(test_db.is_some(), "Should find the test database we just created");

            Ok(())
        }).await
    }
}

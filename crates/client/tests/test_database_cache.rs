#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;

    #[tokio::test]
    async fn test_database_cache() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Clear cache to start fresh
        client.clear_database_cache()?;

        // Check cache is empty
        let cached = client.get_cached_databases()?;
        assert!(cached.is_empty(), "Cache should be empty after clearing");

        // Ensure a database
        client.ensure_database("test_cache_db").await?;

        // Check cache contains the database
        let cached = client.get_cached_databases()?;
        assert_eq!(cached.len(), 1, "Cache should contain one database");
        assert!(
            cached.contains(&"test_cache_db".to_string()),
            "Cache should contain test_cache_db"
        );

        // Ensure same database again (should use cache, not hit server)
        let start = std::time::Instant::now();
        client.ensure_database("test_cache_db").await?;
        let elapsed = start.elapsed();
        println!(
            "Second ensure_database took: {:?} (should be very fast due to cache)",
            elapsed
        );

        // Delete the database
        client.delete_database("test_cache_db").await?;

        // Check cache is empty after deletion
        let cached = client.get_cached_databases()?;
        assert!(cached.is_empty(), "Cache should be empty after deletion");

        // Test reset_database
        client.reset_database("test_cache_db2").await?;
        let cached = client.get_cached_databases()?;
        assert!(
            cached.contains(&"test_cache_db2".to_string()),
            "Cache should contain test_cache_db2 after reset"
        );

        // Clean up
        client.delete_database("test_cache_db2").await?;

        Ok(())
    }
}

//! Test file to verify the new bulk document retrieval functionality
//! 
//! These tests verify that the new get_documents() and get_instances() methods
//! work correctly with the enhanced GetOpts structure.

#[cfg(test)]
mod tests {
    use terminusdb_client::{
        GetOpts,
        BranchSpec,
        TerminusDBHttpClient,
    };

    #[test]
    fn test_get_opts_builder_pattern() {
        // Test that GetOpts can be built using the fluent builder pattern
        let opts = GetOpts::default()
            .with_skip(10)
            .with_count(5)
            .with_type_filter("Person")
            .with_unfold(true)
            .with_as_list(true);

        assert_eq!(opts.skip, Some(10));
        assert_eq!(opts.count, Some(5));
        assert_eq!(opts.type_filter, Some("Person".to_string()));
        assert_eq!(opts.unfold, true);
        assert_eq!(opts.as_list, true);
    }

    #[test]
    fn test_get_opts_constructors() {
        // Test paginated constructor
        let paginated = GetOpts::paginated(5, 20);
        assert_eq!(paginated.skip, Some(5));
        assert_eq!(paginated.count, Some(20));
        assert_eq!(paginated.unfold, false);
        assert_eq!(paginated.as_list, false);

        // Test type-filtered constructor
        let filtered = GetOpts::filtered_by_type("User");
        assert_eq!(filtered.type_filter, Some("User".to_string()));
        assert_eq!(filtered.skip, None);
        assert_eq!(filtered.count, None);
    }

    #[test]
    fn test_default_get_opts() {
        let opts = GetOpts::default();
        assert_eq!(opts.skip, None);
        assert_eq!(opts.count, None);
        assert_eq!(opts.type_filter, None);
        assert_eq!(opts.unfold, false);
        assert_eq!(opts.as_list, false);
    }

    // Note: The following would be integration tests that require a running TerminusDB instance
    // They are commented out to avoid test failures in CI
    
    /*
    #[tokio::test]
    async fn test_get_documents_integration() {
        // This would test the actual API call
        let client = TerminusDBHttpClient::local_node();
        let spec = BranchSpec::new("admin", "test_db", Some("main"));
        
        let ids = vec!["Person/alice".to_string(), "Person/bob".to_string()];
        let opts = GetOpts::default().with_unfold(true);
        
        let result = client.get_documents(ids, &spec, opts).await;
        // Would assert based on expected data
    }

    #[tokio::test] 
    async fn test_get_instances_integration() {
        // This would test the typed API call
        let client = TerminusDBHttpClient::local_node();
        let spec = BranchSpec::new("admin", "test_db", Some("main"));
        let mut deserializer = DefaultDeserializer::new();
        
        let ids = vec!["alice_id".to_string(), "bob_id".to_string()];
        let opts = GetOpts::paginated(0, 10);
        
        let result: Result<Vec<Person>, _> = client.get_instances(ids, &spec, opts, &mut deserializer).await;
        // Would assert based on expected data
    }
    */
}
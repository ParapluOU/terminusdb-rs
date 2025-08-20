#[cfg(test)]
mod tests {
    use terminusdb_woql2::prelude::*;
    use terminusdb_client::TerminusDBHttpClient;
    use std::sync::{Arc, RwLock};
    
    #[tokio::test]
    async fn test_last_query_initial_state() {
        let client = TerminusDBHttpClient::local_node().await;
        
        // Initially, there should be no last query
        assert!(client.last_query().is_none());
        assert!(client.last_query_json().is_none());
    }

    #[tokio::test] 
    async fn test_query_string_dsl_storage() {
        let client = TerminusDBHttpClient::local_node().await;
        
        // Initially no query should be stored
        assert!(client.last_query().is_none());
        
        // Try to execute a DSL query string (will fail due to no database, but should store the parsed query)
        let dsl_query = r#"and()"#; // Simple empty And query
        let _result = client.query_string::<serde_json::Value>(None, dsl_query).await;
        
        // The query should have been parsed and stored even if execution failed
        let stored_query = client.last_query();
        assert!(stored_query.is_some());
        
        // Verify it's an And query
        if let Some(Query::And(and_query)) = stored_query {
            assert!(and_query.and.is_empty());
        } else {
            panic!("Expected And query, got: {:?}", stored_query);
        }
        
        // Verify JSON representation
        let stored_json = client.last_query_json();
        assert!(stored_json.is_some());
        
        let json_value = stored_json.unwrap();
        assert!(json_value.is_object());
        assert_eq!(json_value.get("@type").and_then(|v| v.as_str()), Some("And"));
    }

    #[tokio::test]
    async fn test_query_string_format_detection() {
        // Test JSON-LD detection
        let json_query = r#"{"@type": "Select", "variables": ["Subject"]}"#;
        let json_result = serde_json::from_str::<serde_json::Value>(json_query);
        assert!(json_result.is_ok());
        
        // Test DSL detection (should fail JSON parsing)
        let dsl_query = r#"select([$Subject], triple($Subject, "rdf:type", "owl:Class"))"#;
        let dsl_result = serde_json::from_str::<serde_json::Value>(dsl_query);
        assert!(dsl_result.is_err());
    }

    #[tokio::test] 
    async fn test_last_query_clone_sharing() {
        let client = TerminusDBHttpClient::local_node().await;
        let client_clone = client.clone();
        
        // Initially both should have no last query
        assert!(client.last_query().is_none());
        assert!(client_clone.last_query().is_none());
        
        // Execute a query on one client
        let dsl_query = r#"and()"#;
        let _result = client.query_string::<serde_json::Value>(None, dsl_query).await;
        
        // Both client and clone should see the same query (shared Arc)
        let query1 = client.last_query();
        let query2 = client_clone.last_query();
        
        assert!(query1.is_some());
        assert!(query2.is_some());
        assert_eq!(query1, query2);
    }
}
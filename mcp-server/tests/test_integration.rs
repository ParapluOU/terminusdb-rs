use serde_json::json;
use std::process::Stdio;
use tokio::process::Command;

/// Integration tests that verify the MCP server behavior
/// Run with: cargo test --package terminusdb-mcp-server -- --ignored
#[cfg(test)]
mod integration_tests {
    use super::*;

    async fn is_terminusdb_running() -> bool {
        // Try to connect to TerminusDB
        let response = reqwest::get("http://localhost:6363/api/")
            .await
            .ok()
            .and_then(|r| {
                if r.status().is_success() {
                    Some(())
                } else {
                    None
                }
            });

        response.is_some()
    }

    #[tokio::test]
    async fn test_mcp_server_binary_starts() {
        // Test that the server binary can be started
        let mut child = Command::new("cargo")
            .args(&["run", "--quiet", "--package", "terminusdb-mcp-server"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start MCP server");

        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check if it's still running
        match child.try_wait() {
            Ok(None) => {
                // Process is still running, good
                child.kill().await.ok();
            }
            Ok(Some(status)) => {
                panic!("Server exited immediately with status: {}", status);
            }
            Err(e) => {
                panic!("Error checking server status: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires manual testing with MCP client"]
    async fn test_mcp_protocol_flow() {
        // This test demonstrates the expected MCP protocol flow
        // In practice, you would test this with an actual MCP client

        println!("Expected MCP Protocol Flow:");
        println!("1. Client sends initialize request");
        println!("2. Server responds with capabilities");
        println!("3. Client sends initialized notification");
        println!("4. Client can now call tools");

        // Example messages that would be exchanged
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {
                    "name": "TestClient",
                    "version": "1.0.0"
                }
            }
        });

        let init_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "terminusdb-mcp-server",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        println!(
            "\nInit request: {}",
            serde_json::to_string_pretty(&init_request).unwrap()
        );
        println!(
            "\nExpected response: {}",
            serde_json::to_string_pretty(&init_response).unwrap()
        );
    }

    #[tokio::test]
    #[ignore = "requires running TerminusDB instance"]
    async fn test_with_real_terminusdb() {
        if !is_terminusdb_running().await {
            eprintln!("TerminusDB is not running on localhost:6363, skipping test");
            return;
        }

        println!("TerminusDB is running, MCP server would be able to connect");

        // In a real test scenario, you would:
        // 1. Start the MCP server
        // 2. Send initialization sequence
        // 3. Call tools and verify responses
        // 4. Check error handling
    }
}

/// Example WOQL queries for testing
#[cfg(test)]
mod query_examples {
    use super::*;

    #[test]
    fn test_woql_query_json_format() {
        // Examples of how WOQL queries would be sent to the MCP server
        let queries = vec![
            json!({
                "query": "select ?x ?y where { ?x a ?y }",
                "database": "mydb"
            }),
            json!({
                "query": "select ?name where { ?p a Person; name ?name }",
                "database": "mydb",
                "branch": "development"
            }),
            json!({
                "query": "insert { person1 a Person; name 'Alice' }",
                "database": "mydb"
            }),
            json!({
                "query": "select ?x where { ?x a Person }",
                "database": "mydb",
                "commit": "abc123" // Time-travel query
            }),
        ];

        for query in queries {
            assert!(query["query"].is_string());
            assert!(query["database"].is_string());
            println!(
                "Query example: {}",
                serde_json::to_string_pretty(&query).unwrap()
            );
        }
    }
}

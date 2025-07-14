use serde_json::{json, Value};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_woql_tool_request_format() {
        // Test the format of a tool call request
        let tool_call_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "execute_woql",
                "arguments": {
                    "query": "select ?x ?y where { ?x a ?y }",
                    "database": "test_db"
                }
            }
        });

        // Verify request structure
        assert_eq!(tool_call_request["method"], "tools/call");
        assert_eq!(tool_call_request["params"]["name"], "execute_woql");
        assert!(tool_call_request["params"]["arguments"]["query"].is_string());
        assert!(tool_call_request["params"]["arguments"]["database"].is_string());
    }

    #[test]
    fn test_execute_woql_with_optional_params() {
        // Test with all optional parameters
        let tool_call_with_options = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "execute_woql",
                "arguments": {
                    "query": "select ?x where { ?x a Person }",
                    "database": "test_db",
                    "branch": "development",
                    "commit": "abc123",
                    "connection": {
                        "host": "http://custom:6363",
                        "user": "custom_user",
                        "password": "custom_pass"
                    }
                }
            }
        });

        // Verify optional parameters are included
        assert!(tool_call_with_options["params"]["arguments"]["branch"].is_string());
        assert!(tool_call_with_options["params"]["arguments"]["commit"].is_string());
        assert!(tool_call_with_options["params"]["arguments"]["connection"].is_object());
    }

    #[test]
    fn test_execute_woql_error_response_format() {
        // Example error response for invalid WOQL
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "error": {
                "code": -32000,
                "message": "Tool execution failed",
                "data": {
                    "details": "Expected identifier but found 'invalid'"
                }
            }
        });

        assert!(error_response["error"].is_object());
        assert_eq!(error_response["error"]["code"], -32000);
    }

    #[test]
    fn test_get_schema_tool_request() {
        let schema_request = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "get_schema",
                "arguments": {
                    "database": "mydb"
                }
            }
        });

        assert_eq!(schema_request["params"]["name"], "get_schema");
        assert!(schema_request["params"]["arguments"]["database"].is_string());
    }

    #[test]
    fn test_list_databases_tool_request() {
        let list_db_request = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "list_databases",
                "arguments": {}
            }
        });

        assert_eq!(list_db_request["params"]["name"], "list_databases");
        // No required arguments for this tool
        assert!(list_db_request["params"]["arguments"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_list_databases_response_format() {
        // Test the expected response format for list_databases
        let expected_response = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"databases\": [\n    {\n      \"path\": \"admin/test\",\n      \"name\": \"test\",\n      \"organization\": \"admin\"\n    }\n  ],\n  \"count\": 1\n}"
                    }
                ]
            }
        });

        // Verify response structure
        assert!(expected_response["result"]["content"].is_array());
        let content = &expected_response["result"]["content"][0];
        assert_eq!(content["type"], "text");
        
        // Parse the JSON text content to verify structure
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value = serde_json::from_str(text).expect("Should be valid JSON");
            assert!(parsed["databases"].is_array());
            assert!(parsed["count"].is_number());
        }
    }

    #[test]
    fn test_list_databases_with_connection_params() {
        // Test list_databases with custom connection parameters
        let list_db_with_conn = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "list_databases",
                "arguments": {
                    "connection": {
                        "host": "http://custom-host:6363",
                        "user": "custom_user",
                        "password": "custom_pass"
                    }
                }
            }
        });

        assert_eq!(list_db_with_conn["params"]["name"], "list_databases");
        assert!(list_db_with_conn["params"]["arguments"]["connection"].is_object());
        assert_eq!(
            list_db_with_conn["params"]["arguments"]["connection"]["host"],
            "http://custom-host:6363"
        );
    }

    #[test]
    fn test_tool_response_content_format() {
        // Expected response format for successful tool execution
        let success_response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "Query Results:\\n[\\n  {\\\"x\\\": \\\"doc:1\\\", \\\"y\\\": \\\"Person\\\"}\\n]"
                    }
                ]
            }
        });

        assert!(success_response["result"]["content"].is_array());
        let content = &success_response["result"]["content"][0];
        assert_eq!(content["type"], "text");
        assert!(content["text"].is_string());
    }

    #[test]
    fn test_woql_query_examples() {
        // Test various WOQL query formats
        let queries = vec![
            ("simple select", "select ?x where { ?x a Person }"),
            (
                "with filter",
                "select ?name where { ?p a Person; name ?name } filter(?name = 'John')",
            ),
            ("count", "count ?c where { ?x a ?type } group by ?type"),
            ("insert", "insert { person1 a Person; name 'Test' }"),
            (
                "delete",
                "delete { ?p name ?n } where { ?p a Person; name ?n }",
            ),
        ];

        for (name, query) in queries {
            let request = json!({
                "params": {
                    "arguments": {
                        "query": query,
                        "database": "test"
                    }
                }
            });

            assert!(
                request["params"]["arguments"]["query"].is_string(),
                "Query '{}' should be a valid string",
                name
            );
        }
    }
}

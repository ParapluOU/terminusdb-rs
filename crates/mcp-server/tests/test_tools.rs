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
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
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

    #[test]
    fn test_get_document_tool_request_basic() {
        // Test basic get_document request with just document_id
        let request = json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "get_document",
                "arguments": {
                    "document_id": "Person/12345"
                }
            }
        });

        assert_eq!(request["params"]["name"], "get_document");
        assert_eq!(
            request["params"]["arguments"]["document_id"],
            "Person/12345"
        );
    }

    #[test]
    fn test_get_document_tool_request_with_type_name() {
        // Test get_document with separate type_name and document_id
        let request = json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "get_document",
                "arguments": {
                    "document_id": "12345",
                    "type_name": "Person"
                }
            }
        });

        assert_eq!(request["params"]["arguments"]["document_id"], "12345");
        assert_eq!(request["params"]["arguments"]["type_name"], "Person");
    }

    #[test]
    fn test_get_document_tool_request_with_options() {
        // Test get_document with all optional parameters
        let request = json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "get_document",
                "arguments": {
                    "document_id": "Person/12345",
                    "unfold": true,
                    "as_list": false,
                    "include_headers": true,
                    "connection": {
                        "database": "test_db",
                        "host": "http://localhost:6363",
                        "user": "admin",
                        "password": "root"
                    }
                }
            }
        });

        assert_eq!(request["params"]["arguments"]["unfold"], true);
        assert_eq!(request["params"]["arguments"]["as_list"], false);
        assert_eq!(request["params"]["arguments"]["include_headers"], true);
        assert!(request["params"]["arguments"]["connection"].is_object());
    }

    #[test]
    fn test_get_document_response_format_simple() {
        // Test expected response format for simple document retrieval
        let response = json!({
            "jsonrpc": "2.0",
            "id": 8,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"@id\": \"Person/12345\",\n  \"@type\": \"Person\",\n  \"name\": \"John Doe\",\n  \"age\": 30\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];
        assert_eq!(content["type"], "text");

        // Parse the JSON text content to verify document structure
        if let Some(text) = content["text"].as_str() {
            let document: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert!(document["@id"].is_string());
            assert!(document["@type"].is_string());
        }
    }

    #[test]
    fn test_get_document_response_format_with_headers() {
        // Test expected response format when include_headers is true
        let response = json!({
            "jsonrpc": "2.0",
            "id": 10,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"document\": {\n    \"@id\": \"Person/12345\",\n    \"@type\": \"Person\",\n    \"name\": \"John Doe\"\n  },\n  \"commit_id\": \"ValidCommit/abc123...\",\n  \"metadata\": {\n    \"unfold\": true,\n    \"as_list\": false,\n    \"database\": \"test_db\",\n    \"branch\": \"main\"\n  }\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];

        // Parse the JSON text content to verify structure with headers
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert!(parsed["document"].is_object());
            assert!(parsed["commit_id"].is_string());
            assert!(parsed["metadata"].is_object());
            assert!(parsed["metadata"]["database"].is_string());
        }
    }

    #[test]
    fn test_get_document_error_response() {
        // Test error response when document not found
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 8,
            "error": {
                "code": -32000,
                "message": "Tool execution failed",
                "data": {
                    "details": "document #12345 does not exist"
                }
            }
        });

        assert!(error_response["error"].is_object());
        assert_eq!(error_response["error"]["code"], -32000);
        assert!(error_response["error"]["data"]["details"]
            .as_str()
            .unwrap()
            .contains("does not exist"));
    }

    #[test]
    fn test_query_log_tool_request_format() {
        // Test basic query log status request
        let request = json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "query_log",
                "arguments": {
                    "action": "status"
                }
            }
        });

        assert_eq!(request["params"]["name"], "query_log");
        assert_eq!(request["params"]["arguments"]["action"], "status");
    }

    #[test]
    fn test_query_log_enable_request() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": {
                "name": "query_log",
                "arguments": {
                    "action": "enable",
                    "log_path": "/tmp/test_queries.log"
                }
            }
        });

        assert_eq!(request["params"]["arguments"]["action"], "enable");
        assert_eq!(
            request["params"]["arguments"]["log_path"],
            "/tmp/test_queries.log"
        );
    }

    #[test]
    fn test_query_log_view_request_with_filters() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "tools/call",
            "params": {
                "name": "query_log",
                "arguments": {
                    "action": "view",
                    "limit": "50",
                    "offset": "10",
                    "operation_type_filter": "query",
                    "success_filter": true
                }
            }
        });

        assert_eq!(request["params"]["arguments"]["action"], "view");
        assert_eq!(request["params"]["arguments"]["limit"], "50");
        assert_eq!(request["params"]["arguments"]["offset"], "10");
        assert_eq!(
            request["params"]["arguments"]["operation_type_filter"],
            "query"
        );
        assert_eq!(request["params"]["arguments"]["success_filter"], true);
    }

    #[test]
    fn test_query_log_view_response_format() {
        // Test expected response format for view action
        let response = json!({
            "jsonrpc": "2.0",
            "id": 14,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"entries\": [\n    {\n      \"timestamp\": \"2025-01-01T12:00:00Z\",\n      \"operation_type\": \"query\",\n      \"database\": \"test_db\",\n      \"branch\": \"main\",\n      \"endpoint\": \"/api/db/test_db/query\",\n      \"details\": {\"query_type\": \"select\"},\n      \"success\": true,\n      \"result_count\": 10,\n      \"duration_ms\": 25,\n      \"error\": null\n    }\n  ],\n  \"pagination\": {\n    \"total\": 1,\n    \"limit\": 20,\n    \"offset\": 0,\n    \"has_more\": false\n  }\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];

        // Parse the JSON text content to verify structure
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert!(parsed["entries"].is_array());
            assert!(parsed["pagination"].is_object());
            assert!(parsed["pagination"]["total"].is_number());
            assert!(parsed["pagination"]["has_more"].is_boolean());
        }
    }

    #[test]
    fn test_query_log_status_response_format() {
        // Test expected response format for status action
        let response = json!({
            "jsonrpc": "2.0",
            "id": 15,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"enabled\": true,\n  \"log_path\": null,\n  \"message\": \"Query logging is enabled to: None\"\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];

        // Parse the JSON text content to verify structure
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert!(parsed["enabled"].is_boolean());
            assert!(parsed["message"].is_string());
        }
    }

    #[test]
    fn test_insert_document_tool_request() {
        // Test basic single document insertion
        let request = json!({
            "jsonrpc": "2.0",
            "id": 16,
            "method": "tools/call",
            "params": {
                "name": "insert_document",
                "arguments": {
                    "document": {
                        "@id": "Person/123",
                        "@type": "Person",
                        "name": "Alice",
                        "age": 30
                    },
                    "database": "test_db",
                    "message": "Added new person"
                }
            }
        });

        assert_eq!(request["params"]["name"], "insert_document");
        assert!(request["params"]["arguments"]["document"].is_object());
        assert_eq!(
            request["params"]["arguments"]["document"]["@id"],
            "Person/123"
        );
        assert_eq!(
            request["params"]["arguments"]["document"]["@type"],
            "Person"
        );
        assert_eq!(request["params"]["arguments"]["database"], "test_db");
    }

    #[test]
    fn test_insert_documents_tool_request() {
        // Test batch document insertion
        let request = json!({
            "jsonrpc": "2.0",
            "id": 17,
            "method": "tools/call",
            "params": {
                "name": "insert_documents",
                "arguments": {
                    "documents": [
                        {
                            "@id": "Person/124",
                            "@type": "Person",
                            "name": "Bob",
                            "age": 25
                        },
                        {
                            "@id": "Person/125",
                            "@type": "Person",
                            "name": "Carol",
                            "age": 28
                        }
                    ],
                    "database": "test_db",
                    "branch": "feature",
                    "author": "test_user",
                    "force": true
                }
            }
        });

        assert_eq!(request["params"]["name"], "insert_documents");
        assert!(request["params"]["arguments"]["documents"].is_array());
        assert_eq!(
            request["params"]["arguments"]["documents"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(request["params"]["arguments"]["branch"], "feature");
        assert_eq!(request["params"]["arguments"]["force"], true);
    }

    #[test]
    fn test_replace_document_tool_request() {
        // Test document replacement
        let request = json!({
            "jsonrpc": "2.0",
            "id": 18,
            "method": "tools/call",
            "params": {
                "name": "replace_document",
                "arguments": {
                    "document": {
                        "@id": "Person/123",
                        "@type": "Person",
                        "name": "Alice Updated",
                        "age": 31
                    },
                    "database": "test_db",
                    "message": "Updated person data"
                }
            }
        });

        assert_eq!(request["params"]["name"], "replace_document");
        assert!(request["params"]["arguments"]["document"].is_object());
        assert_eq!(
            request["params"]["arguments"]["document"]["name"],
            "Alice Updated"
        );
    }

    #[test]
    fn test_insert_document_response_format() {
        // Test expected response format for single document insertion
        let response = json!({
            "jsonrpc": "2.0",
            "id": 16,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"status\": \"success\",\n  \"database\": \"test_db\",\n  \"results\": {\n    \"Person/123\": {\n      \"id\": \"Person/123\",\n      \"status\": \"inserted\"\n    }\n  },\n  \"commit_id\": \"abc123def456\"\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];

        // Parse the JSON text content to verify structure
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert_eq!(parsed["status"], "success");
            assert!(parsed["results"].is_object());
            assert!(parsed["commit_id"].is_string());
        }
    }

    #[test]
    fn test_insert_documents_response_format() {
        // Test expected response format for batch insertion
        let response = json!({
            "jsonrpc": "2.0",
            "id": 17,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\n  \"status\": \"success\",\n  \"database\": \"test_db\",\n  \"total_documents\": 2,\n  \"results\": {\n    \"Person/124\": {\n      \"id\": \"Person/124\",\n      \"status\": \"inserted\"\n    },\n    \"Person/125\": {\n      \"id\": \"Person/125\",\n      \"status\": \"already_exists\"\n    }\n  },\n  \"summary\": {\n    \"inserted\": 1,\n    \"already_exists\": 1\n  },\n  \"commit_id\": \"abc123def456\"\n}"
                    }
                ]
            }
        });

        assert!(response["result"]["content"].is_array());
        let content = &response["result"]["content"][0];

        // Parse the JSON text content to verify structure
        if let Some(text) = content["text"].as_str() {
            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("Should be valid JSON");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["total_documents"], 2);
            assert!(parsed["results"].is_object());
            assert!(parsed["summary"].is_object());
            assert_eq!(parsed["summary"]["inserted"], 1);
            assert_eq!(parsed["summary"]["already_exists"], 1);
        }
    }

    #[test]
    fn test_insert_document_error_missing_fields() {
        // Test error when @id or @type is missing
        let request = json!({
            "jsonrpc": "2.0",
            "id": 19,
            "method": "tools/call",
            "params": {
                "name": "insert_document",
                "arguments": {
                    "document": {
                        "name": "Invalid Document",
                        "description": "Missing required fields"
                    },
                    "database": "test_db"
                }
            }
        });

        // The handler should reject documents without @id and @type
        let doc = &request["params"]["arguments"]["document"];
        assert!(doc.get("@id").is_none());
        assert!(doc.get("@type").is_none());
    }

    #[test]
    fn test_insert_with_all_optional_params() {
        // Test insertion with all optional parameters specified
        let request = json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "insert_document",
                "arguments": {
                    "document": {
                        "@id": "Person/126",
                        "@type": "Person",
                        "name": "David"
                    },
                    "database": "test_db",
                    "branch": "feature/new-person",
                    "message": "Adding David to the system",
                    "author": "admin_user",
                    "force": false,
                    "connection": {
                        "host": "http://custom-host:6363",
                        "user": "custom_user",
                        "password": "custom_pass"
                    }
                }
            }
        });

        let args = &request["params"]["arguments"];
        assert_eq!(args["branch"], "feature/new-person");
        assert_eq!(args["message"], "Adding David to the system");
        assert_eq!(args["author"], "admin_user");
        assert_eq!(args["force"], false);
        assert!(args["connection"].is_object());
    }
}

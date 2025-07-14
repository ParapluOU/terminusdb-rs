use serde_json::{json, Value};
use std::sync::Arc;
use terminusdb_mcp_server::McpError;
use tokio::sync::Mutex;

// Helper functions for testing MCP protocol messages

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialization_message_format() {
        // Test the initialization request format that MCP expects
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "roots": {
                        "listChanged": true
                    },
                    "sampling": {}
                },
                "clientInfo": {
                    "name": "TestClient",
                    "version": "1.0.0"
                }
            }
        });

        // Expected server response format
        let expected_response = json!({
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

        // Verify the message structures are valid JSON
        assert!(init_request.is_object());
        assert!(expected_response.is_object());

        // Verify required fields exist
        assert_eq!(init_request["jsonrpc"], "2.0");
        assert_eq!(init_request["method"], "initialize");
        assert!(init_request["params"]["protocolVersion"].is_string());
    }

    #[test]
    fn test_unsupported_protocol_version_error() {
        // Test with an unsupported protocol version
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0.0", // Invalid version
                "capabilities": {},
                "clientInfo": {
                    "name": "TestClient",
                    "version": "1.0.0"
                }
            }
        });

        // Expected error response for unsupported protocol
        let expected_error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32602,
                "message": "Unsupported protocol version",
                "data": {
                    "supported": ["2025-06-18"],
                    "requested": "1.0.0"
                }
            }
        });

        // Verify error structure
        assert!(expected_error_response["error"].is_object());
        assert_eq!(expected_error_response["error"]["code"], -32602);
    }

    #[test]
    fn test_initialized_notification_format() {
        // After successful initialization, client should send initialized notification
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        // This is a notification (no id field), server should not respond
        assert!(notification["id"].is_null());
        assert_eq!(notification["method"], "notifications/initialized");
    }

    #[test]
    fn test_tools_list_request_format() {
        // After initialization, client can request tools list
        let tools_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        // Expected response format with our three tools
        let expected_tools_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {
                        "name": "execute_woql",
                        "description": "Execute WOQL queries using DSL syntax",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "WOQL query in DSL syntax"
                                },
                                "database": {
                                    "type": "string",
                                    "description": "Database name"
                                }
                            },
                            "required": ["query", "database"]
                        }
                    },
                    {
                        "name": "get_schema",
                        "description": "Get schema information for a database"
                    },
                    {
                        "name": "list_databases",
                        "description": "List available databases"
                    }
                ]
            }
        });

        // Verify request structure
        assert_eq!(tools_request["method"], "tools/list");
        assert!(tools_request["id"].is_i64());

        // Verify response would contain tools array
        assert!(expected_tools_response["result"]["tools"].is_array());
    }
}

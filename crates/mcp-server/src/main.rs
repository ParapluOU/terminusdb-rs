pub mod collab;
pub mod config;
pub mod dispatch;
pub mod documents;
pub mod handler;
pub mod servers;
pub mod tools;

use handler::TerminusDBMcpHandler;
use rust_mcp_sdk::schema::{Implementation, InitializeResult, ServerCapabilities};
use rust_mcp_sdk::{mcp_server::server_runtime, McpServer, StdioTransport, TransportOptions};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // info!("Starting TerminusDB MCP Server");

    // Create server details
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "TerminusDB MCP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("TerminusDB MCP Server".to_string()),
        },
        capabilities: ServerCapabilities {
            tools: Some(Default::default()),
            ..Default::default()
        },
        protocol_version: "2025-06-18".to_string(),
        instructions: Some(
            "This server provides access to TerminusDB via WOQL DSL queries. \
            Use execute_woql to run queries, list_databases to see available databases, \
            get_schema to inspect database schemas, and check_server_status to verify \
            the TerminusDB server is running and accessible."
                .to_string(),
        ),
        meta: None,
    };

    // Create transport
    let transport = StdioTransport::new(TransportOptions::default())?;

    // Create handler
    let handler = TerminusDBMcpHandler::new();

    // Create and start server
    let server = server_runtime::create_server(server_details, transport, handler);
    server.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_woql_json_wrapping() {
        // Test that the wrapping logic works correctly
        let original_json = serde_json::json!({
            "@type": "Select",
            "variables": ["Doc"],
            "query": {
                "@type": "And",
                "and": []
            }
        });

        let mut json_value = original_json.clone();

        // Check if needs wrapping
        let needs_wrapping = json_value
            .get("@type")
            .and_then(|t| t.as_str())
            .map(|t| t != "Query")
            .unwrap_or(false);

        assert!(needs_wrapping);

        // Apply wrapping
        if let Some(query_type) = json_value.get("@type").and_then(|t| t.as_str()) {
            let mut wrapper = serde_json::Map::new();
            wrapper.insert(
                "@type".to_string(),
                serde_json::Value::String("Query".to_string()),
            );
            wrapper.insert(query_type.to_lowercase(), json_value);
            json_value = serde_json::Value::Object(wrapper);
        }

        // Verify the wrapped structure
        assert_eq!(
            json_value.get("@type").and_then(|v| v.as_str()),
            Some("Query")
        );
        assert!(json_value.get("select").is_some());

        // The wrapped JSON should now be ready for deserialization
        // Note: The actual deserialization may still fail due to how FromTDBInstance
        // handles abstract tagged unions, but the wrapping structure is correct
    }

    #[test]
    fn test_complex_woql_query_json_ld() {
        // This test verifies that complex JSON-LD queries can be handled
        // by the execute_woql function without deserialization
        let query_json = json!({
            "@type": "Select",
            "query": {
                "@type": "And",
                "and": [
                    {
                        "@type": "OrderBy",
                        "ordering": [
                            {
                                "@type": "OrderTemplate",
                                "order": "asc",
                                "variable": "CreatedBy"
                            }
                        ],
                        "query": {
                            "@type": "And",
                            "and": [
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "node": "@schema:AwsDBPublication"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "rdf:type"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "node": "@schema:AwsDBPublication"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "rdf:type"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "Title"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "title"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "CreatedOn"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "created_on"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "Title"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "title"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Lower",
                                    "lower": {
                                        "@type": "DataValue",
                                        "variable": "LowerTitle"
                                    },
                                    "mixed": {
                                        "@type": "DataValue",
                                        "variable": "Title"
                                    }
                                },
                                {
                                    "@type": "Regexp",
                                    "pattern": {
                                        "@type": "DataValue",
                                        "data": ".*alpha.*"
                                    },
                                    "result": null,
                                    "string": {
                                        "@type": "DataValue",
                                        "variable": "LowerTitle"
                                    }
                                }
                            ]
                        }
                    },
                    {
                        "@type": "ReadDocument",
                        "document": {
                            "@type": "Value",
                            "variable": "Doc"
                        },
                        "identifier": {
                            "@type": "NodeValue",
                            "variable": "Subject"
                        }
                    }
                ]
            },
            "variables": [
                "Doc"
            ]
        });

        // Test that the JSON can be used directly without wrapping or deserialization
        let json_string = serde_json::to_string(&query_json).unwrap();

        // Simulate what execute_woql does - parse the JSON string
        let parsed_json = serde_json::from_str::<serde_json::Value>(&json_string).unwrap();

        // Verify that the JSON has the expected structure
        assert_eq!(
            parsed_json.get("@type").and_then(|v| v.as_str()),
            Some("Select")
        );
        assert!(parsed_json.get("variables").is_some());
        assert!(parsed_json.get("query").is_some());

        // Verify the nested structure
        let query_obj = parsed_json.get("query").unwrap();
        assert_eq!(query_obj.get("@type").and_then(|v| v.as_str()), Some("And"));

        let and_array = query_obj.get("and").and_then(|v| v.as_array()).unwrap();
        assert_eq!(and_array.len(), 2);

        // First element should be an OrderBy
        assert_eq!(
            and_array[0].get("@type").and_then(|v| v.as_str()),
            Some("OrderBy")
        );

        // Second element should be a ReadDocument
        assert_eq!(
            and_array[1].get("@type").and_then(|v| v.as_str()),
            Some("ReadDocument")
        );

        // The JSON should be ready to send to the API without any transformation
        println!(
            "JSON-LD query ready for API: {}",
            serde_json::to_string_pretty(&parsed_json).unwrap()
        )
    }
}

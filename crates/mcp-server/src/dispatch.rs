//! MCP `ServerHandler` trait implementation: tool listing and call dispatch.

use crate::handler::TerminusDBMcpHandler;
use crate::tools::*;
use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolRequest, CallToolResult, ListToolsRequest, ListToolsResult,
    RpcError, TextContent,
};
use rust_mcp_sdk::McpServer;
use std::fmt;

// Simple error wrapper for anyhow::Error
#[derive(Debug)]
struct McpError(anyhow::Error);

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for McpError {}

#[async_trait]
impl ServerHandler for TerminusDBMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: &dyn McpServer,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![
                ConnectTool::tool(),
                ExecuteWoqlTool::tool(),
                ListDatabasesTool::tool(),
                GetSchemaTool::tool(),
                CheckServerStatusTool::tool(),
                ResetDatabaseTool::tool(),
                GetDocumentTool::tool(),
                QueryLogTool::tool(),
                DeleteClassesTool::tool(),
                SquashTool::tool(),
                ResetTool::tool(),
                OptimizeTool::tool(),
                GetGraphQLSchemaTool::tool(),
                // Collaboration operations
                CloneTool::tool(),
                FetchTool::tool(),
                PushTool::tool(),
                PullTool::tool(),
                // Remote management operations
                AddRemoteTool::tool(),
                GetRemoteTool::tool(),
                UpdateRemoteTool::tool(),
                DeleteRemoteTool::tool(),
                // Local server management
                StartLocalServerTool::tool(),
                StopLocalServerTool::tool(),
                ListLocalServersTool::tool(),
                // Temporarily disabled due to serde_json::Value schema issues
                // InsertDocumentTool::tool(),
                // InsertDocumentsTool::tool(),
                // ReplaceDocumentTool::tool(),
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: &dyn McpServer,
    ) -> Result<CallToolResult, CallToolError> {
        let tool_name = &request.params.name;
        let args = request.params.arguments.clone().unwrap_or_default();

        match tool_name.as_str() {
            name if name == ConnectTool::tool_name() => {
                let tool_request: ConnectTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.connect(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ExecuteWoqlTool::tool_name() => {
                let tool_request: ExecuteWoqlTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.execute_woql(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ListDatabasesTool::tool_name() => {
                let tool_request: ListDatabasesTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.list_databases(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetSchemaTool::tool_name() => {
                let tool_request: GetSchemaTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.get_schema(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == CheckServerStatusTool::tool_name() => {
                let tool_request: CheckServerStatusTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.check_server_status(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ResetDatabaseTool::tool_name() => {
                let tool_request: ResetDatabaseTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.reset_database(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetDocumentTool::tool_name() => {
                let tool_request: GetDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.get_document(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == QueryLogTool::tool_name() => {
                let tool_request: QueryLogTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_query_log(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == InsertDocumentTool::tool_name() => {
                let tool_request: InsertDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_insert_document(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == InsertDocumentsTool::tool_name() => {
                let tool_request: InsertDocumentsTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_insert_documents(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ReplaceDocumentTool::tool_name() => {
                let tool_request: ReplaceDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_replace_document(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == DeleteClassesTool::tool_name() => {
                let tool_request: DeleteClassesTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_delete_classes(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == SquashTool::tool_name() => {
                let tool_request: SquashTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_squash(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ResetTool::tool_name() => {
                let tool_request: ResetTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_reset(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == OptimizeTool::tool_name() => {
                let tool_request: OptimizeTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_optimize(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetGraphQLSchemaTool::tool_name() => {
                let tool_request: GetGraphQLSchemaTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_get_graphql_schema(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == CloneTool::tool_name() => {
                let tool_request: CloneTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_clone(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == FetchTool::tool_name() => {
                let tool_request: FetchTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_fetch(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == PushTool::tool_name() => {
                let tool_request: PushTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_push(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == PullTool::tool_name() => {
                let tool_request: PullTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_pull(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == AddRemoteTool::tool_name() => {
                let tool_request: AddRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_add_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetRemoteTool::tool_name() => {
                let tool_request: GetRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_get_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == UpdateRemoteTool::tool_name() => {
                let tool_request: UpdateRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_update_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == DeleteRemoteTool::tool_name() => {
                let tool_request: DeleteRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_delete_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == StartLocalServerTool::tool_name() => {
                let tool_request: StartLocalServerTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_start_local_server(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == StopLocalServerTool::tool_name() => {
                let tool_request: StopLocalServerTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_stop_local_server(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ListLocalServersTool::tool_name() => {
                let tool_request: ListLocalServersTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_list_local_servers(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            _ => Err(CallToolError::unknown_tool(tool_name.to_string())),
        }
    }
}

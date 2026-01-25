use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use serde::Serialize;

use crate::models::{AddRemoteRequest, CommitInfo, DatabaseInfo, ModelInfo, RemoteInfo};
use crate::state::AppState;

/// Response for database operations
#[derive(Debug, Serialize)]
pub struct DatabaseResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List all databases on a node with metadata (from cache)
#[get("/nodes/<node_id>/databases")]
pub async fn list_databases(
    state: &State<AppState>,
    node_id: String,
) -> Json<DatabaseResponse<Vec<DatabaseInfo>>> {
    // Read database information from cached NodeStatus (populated by background poller)
    if let Some(status) = state.get_status(&node_id) {
        Json(DatabaseResponse {
            success: true,
            data: Some(status.databases),
            error: None,
        })
    } else {
        Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Node status not available for: {}", node_id)),
        })
    }
}

/// Get schema/models for a database
#[get("/nodes/<node_id>/databases/<database>/schema")]
pub async fn get_database_schema(
    state: &State<AppState>,
    node_id: String,
    database: String,
) -> Json<DatabaseResponse<Vec<ModelInfo>>> {
    match state.get_or_create_client(&node_id).await {
        Ok(client) => {
            // Extract just the database name without the organization prefix
            let db_name_only = database.split('/').nth(1).unwrap_or(&database).to_string();

            // Get the schema using GraphQL introspection
            match client.introspect_schema(&db_name_only, None, None).await {
                Ok(schema) => {
                    // Parse the schema JSON to extract model names
                    // The introspection returns a complex GraphQL schema structure
                    // For now, we'll extract the types from the schema
                    let model_infos: Vec<ModelInfo> = if let Some(obj) = schema.as_object() {
                        if let Some(data) = obj.get("__schema").and_then(|s| s.as_object()) {
                            if let Some(types) = data.get("types").and_then(|t| t.as_array()) {
                                types
                                    .iter()
                                    .filter_map(|type_obj| {
                                        type_obj
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .map(|s| s.to_string())
                                    })
                                    .filter(|name| {
                                        !name.starts_with("__")
                                            && name != "Query"
                                            && name != "Mutation"
                                    })
                                    .map(|name| ModelInfo {
                                        name: name.clone(),
                                        instance_count: 0, // Will be populated by counting instances
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    Json(DatabaseResponse {
                        success: true,
                        data: Some(model_infos),
                        error: None,
                    })
                }
                Err(e) => Json(DatabaseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to get schema: {}", e)),
                }),
            }
        }
        Err(e) => Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to connect to node: {}", e)),
        }),
    }
}

/// Get commit history for a database
#[get("/nodes/<node_id>/databases/<database>/commits")]
pub async fn get_database_commits(
    state: &State<AppState>,
    node_id: String,
    database: String,
) -> Json<DatabaseResponse<Vec<CommitInfo>>> {
    match state.get_or_create_client(&node_id).await {
        Ok(client) => {
            // Extract just the database name without the organization prefix
            let db_name_only = database.split('/').nth(1).unwrap_or(&database).to_string();

            // Create BranchSpec and LogOpts for log query
            let spec = terminusdb_client::BranchSpec {
                db: db_name_only,
                branch: None,
                ref_commit: None,
            };
            let log_opts = terminusdb_client::LogOpts {
                offset: None,
                count: Some(100), // Get last 100 commits
                verbose: false,
            };

            match client.log(&spec, log_opts).await {
                Ok(commits) => {
                    let commit_infos: Vec<CommitInfo> = commits
                        .iter()
                        .map(|c| CommitInfo {
                            id: c.identifier.clone(),
                            author: c.author.clone(),
                            message: c.message.clone(),
                            timestamp: c.timestamp.to_string(),
                        })
                        .collect();

                    Json(DatabaseResponse {
                        success: true,
                        data: Some(commit_infos),
                        error: None,
                    })
                }
                Err(e) => Json(DatabaseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to get commits: {}", e)),
                }),
            }
        }
        Err(e) => Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to connect to node: {}", e)),
        }),
    }
}

/// List remotes for a database
#[get("/nodes/<node_id>/databases/<database>/remotes")]
pub async fn list_remotes(
    state: &State<AppState>,
    node_id: String,
    database: String,
) -> Json<DatabaseResponse<Vec<RemoteInfo>>> {
    match state.get_or_create_client(&node_id).await {
        Ok(client) => {
            // Extract just the database name without the organization prefix
            let db_name_only = database.split('/').nth(1).unwrap_or(&database).to_string();

            match client.list_remotes(&db_name_only).await {
                Ok(remotes) => {
                    let remote_infos: Vec<RemoteInfo> = remotes
                        .iter()
                        .map(|r| {
                            // Try to match URL to a known node
                            let target_node_id = state
                                .get_nodes()
                                .iter()
                                .find(|n| r.remote_url.contains(&n.host))
                                .map(|n| n.id.clone());

                            RemoteInfo {
                                database: database.clone(),
                                remote_name: r.name.clone(),
                                remote_url: r.remote_url.clone(),
                                target_node_id,
                            }
                        })
                        .collect();

                    Json(DatabaseResponse {
                        success: true,
                        data: Some(remote_infos),
                        error: None,
                    })
                }
                Err(e) => Json(DatabaseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to list remotes: {}", e)),
                }),
            }
        }
        Err(e) => Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to connect to node: {}", e)),
        }),
    }
}

/// Add a remote to a database
#[post("/nodes/<node_id>/databases/<database>/remotes", data = "<request>")]
pub async fn add_remote(
    state: &State<AppState>,
    node_id: String,
    database: String,
    request: Json<AddRemoteRequest>,
) -> Json<DatabaseResponse<()>> {
    match state.get_or_create_client(&node_id).await {
        Ok(client) => {
            // Extract just the database name without the organization prefix
            let db_name_only = database.split('/').nth(1).unwrap_or(&database).to_string();

            match client
                .add_remote(&db_name_only, &request.remote_name, &request.remote_url)
                .await
            {
                Ok(_) => Json(DatabaseResponse {
                    success: true,
                    data: Some(()),
                    error: None,
                }),
                Err(e) => Json(DatabaseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to add remote: {}", e)),
                }),
            }
        }
        Err(e) => Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to connect to node: {}", e)),
        }),
    }
}

/// Delete a remote from a database
#[delete("/nodes/<node_id>/databases/<database>/remotes/<remote_name>")]
pub async fn delete_remote(
    state: &State<AppState>,
    node_id: String,
    database: String,
    remote_name: String,
) -> Json<DatabaseResponse<()>> {
    match state.get_or_create_client(&node_id).await {
        Ok(client) => {
            // Extract just the database name without the organization prefix
            let db_name_only = database.split('/').nth(1).unwrap_or(&database).to_string();

            match client.delete_remote(&db_name_only, &remote_name).await {
                Ok(_) => Json(DatabaseResponse {
                    success: true,
                    data: Some(()),
                    error: None,
                }),
                Err(e) => Json(DatabaseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to delete remote: {}", e)),
                }),
            }
        }
        Err(e) => Json(DatabaseResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to connect to node: {}", e)),
        }),
    }
}

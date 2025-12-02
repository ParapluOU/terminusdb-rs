use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use serde::{Deserialize, Serialize};

use crate::models::NodeConfig;
use crate::state::AppState;

/// Request to create a new node
#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub label: String,
    pub host: String,
    pub port: u32,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub ssh_enabled: bool,
    #[serde(default)]
    pub position_x: f64,
    #[serde(default)]
    pub position_y: f64,
}

/// Request to update a node
#[derive(Debug, Deserialize)]
pub struct UpdateNodeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_y: Option<f64>,
}

/// Response for node operations
#[derive(Debug, Serialize)]
pub struct NodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List all nodes
#[get("/nodes")]
pub fn list_nodes(state: &State<AppState>) -> Json<Vec<NodeConfig>> {
    Json(state.get_nodes())
}

/// Create a new node
#[post("/nodes", data = "<request>")]
pub async fn create_node(
    state: &State<AppState>,
    request: Json<CreateNodeRequest>,
) -> Json<NodeResponse> {
    // Generate a unique ID
    let id = format!("node_{}", uuid::Uuid::new_v4());

    let node = NodeConfig {
        id,
        label: request.label.clone(),
        host: request.host.clone(),
        port: request.port,
        username: request.username.clone(),
        password: request.password.clone(),
        ssh_enabled: request.ssh_enabled,
        position_x: request.position_x,
        position_y: request.position_y,
    };

    match state.save_node(&node).await {
        Ok(()) => Json(NodeResponse {
            success: true,
            node: Some(node),
            error: None,
        }),
        Err(e) => Json(NodeResponse {
            success: false,
            node: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Update a node
#[put("/nodes/<id>", data = "<request>")]
pub async fn update_node(
    state: &State<AppState>,
    id: String,
    request: Json<UpdateNodeRequest>,
) -> Json<NodeResponse> {
    let mut node = match state.get_node(&id) {
        Some(node) => node,
        None => {
            return Json(NodeResponse {
                success: false,
                node: None,
                error: Some("Node not found".to_string()),
            })
        }
    };

    // Update fields if provided
    if let Some(label) = &request.label {
        node.label = label.clone();
    }
    if let Some(host) = &request.host {
        node.host = host.clone();
    }
    if let Some(port) = request.port {
        node.port = port;
    }
    if let Some(username) = &request.username {
        node.username = username.clone();
    }
    if let Some(password) = &request.password {
        node.password = password.clone();
    }
    if let Some(ssh_enabled) = request.ssh_enabled {
        node.ssh_enabled = ssh_enabled;
    }
    if let Some(x) = request.position_x {
        node.position_x = x;
    }
    if let Some(y) = request.position_y {
        node.position_y = y;
    }

    match state.update_node(node.clone()).await {
        Ok(()) => Json(NodeResponse {
            success: true,
            node: Some(node),
            error: None,
        }),
        Err(e) => Json(NodeResponse {
            success: false,
            node: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Delete a node
#[delete("/nodes/<id>")]
pub async fn delete_node(state: &State<AppState>, id: String) -> Json<NodeResponse> {
    match state.delete_node(&id).await {
        Ok(()) => Json(NodeResponse {
            success: true,
            node: None,
            error: None,
        }),
        Err(e) => Json(NodeResponse {
            success: false,
            node: None,
            error: Some(e.to_string()),
        }),
    }
}

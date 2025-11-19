use rocket::serde::json::Json;
use rocket::{get, post, State};
use serde::Serialize;

use crate::state::AppState;

/// Information about the local instance
#[derive(Debug, Serialize)]
pub struct LocalInstanceInfo {
    pub port: u16,
    pub running: bool,
    pub url: String,
}

/// Response for instance operations
#[derive(Debug, Serialize)]
pub struct InstanceResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Get information about the local instance
#[get("/instance/local")]
pub fn get_local_instance(state: &State<AppState>) -> Json<LocalInstanceInfo> {
    let manager = state.manager();
    let port = manager.port();

    Json(LocalInstanceInfo {
        port,
        running: manager.is_running(),
        url: format!("http://localhost:{}", port),
    })
}

/// Restart the local instance
#[post("/instance/local/restart")]
pub async fn restart_local_instance(state: &State<AppState>) -> Json<InstanceResponse> {
    let manager = state.manager();

    match manager.ensure_running().await {
        Ok(()) => Json(InstanceResponse {
            success: true,
            error: None,
        }),
        Err(e) => Json(InstanceResponse {
            success: false,
            error: Some(e.to_string()),
        }),
    }
}

use rocket::serde::json::Json;
use rocket::{get, State};

use crate::models::NodeStatus;
use crate::state::AppState;

/// Get statuses for all nodes
#[get("/status")]
pub fn get_all_statuses(state: &State<AppState>) -> Json<Vec<NodeStatus>> {
    Json(state.get_statuses())
}

/// Get status for a specific node
#[get("/status/<id>")]
pub fn get_node_status(state: &State<AppState>, id: String) -> Option<Json<NodeStatus>> {
    state.get_status(&id).map(Json)
}

use rocket::{launch, routes, get};
use terminusdb_manager::{AppState, poller, assets};

/// Serve index.html
#[get("/")]
fn index() -> assets::AssetResponse {
    assets::get_asset("index.html")
}

/// Serve static files (CSS, JS, etc.)
#[get("/<file..>")]
fn files(file: std::path::PathBuf) -> assets::AssetResponse {
    let path = file.to_str().unwrap_or("index.html");
    assets::get_asset(path)
}

#[launch]
async fn rocket() -> _ {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting TerminusDB Manager");

    // Initialize application state
    let state = AppState::new().await
        .expect("Failed to initialize application state");

    // Spawn background poller
    let _poller_handle = poller::spawn_poller(state.clone());

    tracing::info!("TerminusDB Manager initialized successfully");

    rocket::build()
        .manage(state)
        .mount("/api", routes![
            // Node endpoints
            terminusdb_manager::api::list_nodes,
            terminusdb_manager::api::create_node,
            terminusdb_manager::api::update_node,
            terminusdb_manager::api::delete_node,
            // Status endpoints
            terminusdb_manager::api::get_all_statuses,
            terminusdb_manager::api::get_node_status,
            // Instance endpoints
            terminusdb_manager::api::get_local_instance,
            terminusdb_manager::api::restart_local_instance,
        ])
        .mount("/", routes![index, files])
}

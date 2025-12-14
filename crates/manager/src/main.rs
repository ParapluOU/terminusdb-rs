use rocket::{launch, routes, get};
use terminusdb_manager::{AppState, assets};

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

    // Start per-node pollers for all configured nodes
    state.start_all_pollers();

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
            // Database endpoints
            terminusdb_manager::api::list_databases,
            terminusdb_manager::api::get_database_schema,
            terminusdb_manager::api::get_database_commits,
            terminusdb_manager::api::list_remotes,
            terminusdb_manager::api::add_remote,
            terminusdb_manager::api::delete_remote,
        ])
        .mount("/", routes![index, files])
}

use rocket::{launch, routes, get};
use rocket::response::content::RawHtml;
use terminusdb_manager::{AppState, poller};

#[get("/")]
fn index() -> RawHtml<&'static str> {
    RawHtml(r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TerminusDB Manager</title>
    <style>
        body {
            font-family: system-ui, -apple-system, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        h1 {
            color: #333;
        }
        .info {
            background: white;
            padding: 20px;
            border-radius: 8px;
            margin: 20px 0;
        }
        .endpoint {
            background: #e8f4f8;
            padding: 10px;
            margin: 10px 0;
            border-radius: 4px;
            font-family: monospace;
        }
    </style>
</head>
<body>
    <h1>TerminusDB Manager</h1>
    <div class="info">
        <h2>Welcome</h2>
        <p>The TerminusDB Manager backend is running. The Elm frontend is under development.</p>

        <h3>Available API Endpoints:</h3>

        <div class="endpoint">GET /api/nodes - List all nodes</div>
        <div class="endpoint">POST /api/nodes - Create a new node</div>
        <div class="endpoint">PUT /api/nodes/:id - Update a node</div>
        <div class="endpoint">DELETE /api/nodes/:id - Delete a node</div>

        <div class="endpoint">GET /api/status - Get all node statuses</div>
        <div class="endpoint">GET /api/status/:id - Get specific node status</div>

        <div class="endpoint">GET /api/instance/local - Get local instance info</div>
        <div class="endpoint">POST /api/instance/local/restart - Restart local instance</div>
    </div>
</body>
</html>
    "#)
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
        .mount("/", routes![index])
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
}

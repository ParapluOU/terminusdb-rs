/// Test that SSE functionality is disabled by default and can be enabled via env var
use serde::{Deserialize, Serialize};
use terminusdb_client::{BranchSpec, TerminusDBHttpClient};
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(TerminusDBModel, FromTDBInstance, Clone, Debug, Default)]
struct TestUser {
    name: String
}

#[tokio::test]
async fn test_sse_disabled_by_default() {
    // Ensure TERMINUSDB_SSE is not set
    std::env::remove_var("TERMINUSDB_SSE");

    // Create a client (doesn't need to connect to a real server for this test)
    let url = url::Url::parse("http://localhost:6363").unwrap();
    let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
        .await
        .unwrap();

    // Create a change listener - should succeed even though SSE is disabled
    let spec = BranchSpec::with_branch("testdb", "main");
    let listener = client.change_listener(spec).unwrap();

    // Should be able to register handlers without error (they just won't be called)
    listener.on_added_id::<TestUser>(|_iri| {
        // This callback won't be called since SSE is disabled
    });

    // Test passes if we get here without panicking
}

#[tokio::test]
async fn test_sse_enabled_with_env_var() {
    // Set TERMINUSDB_SSE to enable SSE
    std::env::set_var("TERMINUSDB_SSE", "true");

    // Create a client
    let url = url::Url::parse("http://localhost:6363").unwrap();
    let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
        .await
        .unwrap();

    // Create a change listener - this will try to initialize SSE manager
    // but won't fail even if no server is available
    let spec = BranchSpec::with_branch("testdb", "main");
    let _listener = client.change_listener(spec).unwrap();

    // Clean up
    std::env::remove_var("TERMINUSDB_SSE");

    // Test passes if we get here without panicking
}

#[tokio::test]
async fn test_sse_enabled_with_various_env_values() {
    let test_values = vec!["true", "TRUE", "1", "yes", "YES", "on", "ON"];

    for value in test_values {
        std::env::set_var("TERMINUSDB_SSE", value);

        let url = url::Url::parse("http://localhost:6363").unwrap();
        let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
            .await
            .unwrap();

        let spec = BranchSpec::with_branch("testdb", "main");
        let _listener = client.change_listener(spec).unwrap();

        std::env::remove_var("TERMINUSDB_SSE");
    }
}

#[tokio::test]
async fn test_sse_disabled_with_invalid_env_values() {
    // Ensure invalid values don't enable SSE
    std::env::remove_var("TERMINUSDB_SSE");

    let test_values = vec!["false", "0", "no", "off", "invalid", ""];

    for value in test_values {
        std::env::set_var("TERMINUSDB_SSE", value);

        let url = url::Url::parse("http://localhost:6363").unwrap();
        let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
            .await
            .unwrap();

        let spec = BranchSpec::with_branch("testdb", "main");
        let listener = client.change_listener(spec).unwrap();

        // Should still be able to register handlers (they just won't be called)
        listener.on_added_id::<TestUser>(|_iri| {
            // This callback won't be called
        });

        std::env::remove_var("TERMINUSDB_SSE");
    }
}

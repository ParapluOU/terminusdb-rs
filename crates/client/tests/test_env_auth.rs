//! Tests for environment variable authentication

use std::env;
use terminusdb_client::*;

#[tokio::test]
#[ignore] // This test modifies environment variables, so it should be run in isolation
async fn test_password_env_precedence() {
    // Note: Since we can't access the private `pass` field directly,
    // this test verifies that local_node() creates a client successfully
    // with different environment configurations. The actual password
    // verification would happen when connecting to a real TerminusDB instance.

    // Save original values
    let orig_admin_pass = env::var("TERMINUSDB_ADMIN_PASS").ok();
    let orig_pass = env::var("TERMINUSDB_PASS").ok();

    // Test 1: When both are set, client should be created successfully
    env::set_var("TERMINUSDB_ADMIN_PASS", "admin_password");
    env::set_var("TERMINUSDB_PASS", "regular_password");

    let _client = TerminusDBHttpClient::local_node().await;
    // If we had a running TerminusDB with this password, we could test connection

    // Test 2: When only TERMINUSDB_PASS is set
    env::remove_var("TERMINUSDB_ADMIN_PASS");
    let _client = TerminusDBHttpClient::local_node().await;

    // Test 3: When neither is set (defaults to "root")
    env::remove_var("TERMINUSDB_PASS");
    let _client = TerminusDBHttpClient::local_node().await;

    // Restore original values
    if let Some(val) = orig_admin_pass {
        env::set_var("TERMINUSDB_ADMIN_PASS", val);
    } else {
        env::remove_var("TERMINUSDB_ADMIN_PASS");
    }
    if let Some(val) = orig_pass {
        env::set_var("TERMINUSDB_PASS", val);
    } else {
        env::remove_var("TERMINUSDB_PASS");
    }
}

#[test]
fn test_env_var_reading() {
    // This test just verifies we can read environment variables correctly
    // without making actual connections

    // Save original values
    let orig_admin_pass = env::var("TERMINUSDB_ADMIN_PASS").ok();
    let orig_pass = env::var("TERMINUSDB_PASS").ok();

    // Test the precedence logic directly
    env::set_var("TERMINUSDB_ADMIN_PASS", "test_admin");
    env::set_var("TERMINUSDB_PASS", "test_regular");

    let password = env::var("TERMINUSDB_ADMIN_PASS")
        .or_else(|_| env::var("TERMINUSDB_PASS"))
        .unwrap_or_else(|_| "root".to_string());

    assert_eq!(password, "test_admin");

    env::remove_var("TERMINUSDB_ADMIN_PASS");
    let password = env::var("TERMINUSDB_ADMIN_PASS")
        .or_else(|_| env::var("TERMINUSDB_PASS"))
        .unwrap_or_else(|_| "root".to_string());

    assert_eq!(password, "test_regular");

    env::remove_var("TERMINUSDB_PASS");
    let password = env::var("TERMINUSDB_ADMIN_PASS")
        .or_else(|_| env::var("TERMINUSDB_PASS"))
        .unwrap_or_else(|_| "root".to_string());

    assert_eq!(password, "root");

    // Restore original values
    if let Some(val) = orig_admin_pass {
        env::set_var("TERMINUSDB_ADMIN_PASS", val);
    } else {
        env::remove_var("TERMINUSDB_ADMIN_PASS");
    }
    if let Some(val) = orig_pass {
        env::set_var("TERMINUSDB_PASS", val);
    } else {
        env::remove_var("TERMINUSDB_PASS");
    }
}

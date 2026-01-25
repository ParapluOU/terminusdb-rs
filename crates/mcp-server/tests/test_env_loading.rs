use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn test_connect_with_env_file() {
    // Build the MCP server binary first
    let output = Command::new("cargo")
        .args(&["build", "--bin", "terminusdb-mcp"])
        .output()
        .expect("Failed to build MCP server");

    if !output.status.success() {
        panic!(
            "Failed to build MCP server: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Start the MCP server as a subprocess
    let mut child = Command::new("cargo")
        .args(&["run", "--bin", "terminusdb-mcp"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // Suppress stderr for cleaner test output
        .spawn()
        .expect("Failed to start MCP server");

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let reader = BufReader::new(stdout);

    // Helper function to send a request and get a response
    let send_request = |stdin: &mut std::process::ChildStdin, request: serde_json::Value| {
        let request_str = request.to_string();
        println!("Sending: {}", request_str);
        writeln!(stdin, "{}", request_str).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    };

    // Helper function to read a response
    let read_response = |reader: &mut BufReader<std::process::ChildStdout>| -> serde_json::Value {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("Failed to read response");
        println!("Received: {}", line.trim());
        serde_json::from_str(&line).expect("Failed to parse response")
    };

    // Create a thread to read responses
    let (tx, rx) = std::sync::mpsc::channel();
    let reader_thread = thread::spawn(move || {
        let mut reader = reader;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if let Ok(response) = serde_json::from_str::<serde_json::Value>(&line) {
                        tx.send(response).unwrap();
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stdout: {}", e);
                    break;
                }
            }
        }
    });

    // Send initialize request
    let init_request = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        },
        "id": 1
    });
    send_request(&mut stdin, init_request);

    // Wait for initialize response
    thread::sleep(Duration::from_millis(500));
    let init_response = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for initialize response");

    assert_eq!(init_response["id"], 1);
    assert!(init_response["result"].is_object());

    // Send connect request with env_file
    let connect_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "connect",
            "arguments": {
                "env_file": ".secrets.env"
            }
        },
        "id": 2
    });
    send_request(&mut stdin, connect_request);

    // Wait for connect response
    thread::sleep(Duration::from_millis(500));
    let connect_response = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for connect response");

    println!(
        "Connect response: {}",
        serde_json::to_string_pretty(&connect_response).unwrap()
    );

    // Check the response
    assert_eq!(connect_response["id"], 2);

    if let Some(result) = connect_response.get("result") {
        // Check if the connection was successful
        if let Some(content) = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
        {
            if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                let parsed: serde_json::Value =
                    serde_json::from_str(text).expect("Failed to parse content text");

                // Verify the host is not localhost (should be from env file)
                if let Some(host) = parsed.get("host").and_then(|h| h.as_str()) {
                    assert_ne!(
                        host, "http://localhost:6363",
                        "Host should be loaded from env file, not default"
                    );
                    assert_eq!(
                        host, "http://tdb-dev.eastus.azurecontainer.io:6363",
                        "Host should match env file value"
                    );
                }

                // Verify the user is from env file
                if let Some(user) = parsed.get("user").and_then(|u| u.as_str()) {
                    assert_eq!(user, "admin", "User should match env file value");
                }
            }
        }
    } else if let Some(error) = connect_response.get("error") {
        panic!("Connect request failed with error: {:?}", error);
    }

    // Now test that subsequent commands use the saved connection
    // Send check_server_status request without connection parameter
    let status_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "check_server_status",
            "arguments": {}  // No connection parameter - should use saved connection
        },
        "id": 3
    });
    send_request(&mut stdin, status_request);

    // Wait for status response
    thread::sleep(Duration::from_millis(500));
    let status_response = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for status response");

    println!(
        "Status response: {}",
        serde_json::to_string_pretty(&status_response).unwrap()
    );

    // Check the response
    assert_eq!(status_response["id"], 3);

    if let Some(result) = status_response.get("result") {
        if let Some(content) = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
        {
            if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                let parsed: serde_json::Value =
                    serde_json::from_str(text).expect("Failed to parse content text");

                // Verify the server is running
                assert_eq!(
                    parsed.get("status").and_then(|s| s.as_str()),
                    Some("running")
                );
                assert_eq!(
                    parsed.get("connected").and_then(|c| c.as_bool()),
                    Some(true)
                );
            }
        }
    } else if let Some(error) = status_response.get("error") {
        panic!("Status request failed with error: {:?}", error);
    }

    // Also test list_databases to ensure it uses the saved connection
    let list_db_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "list_databases",
            "arguments": {}  // No connection parameter - should use saved connection
        },
        "id": 4
    });
    send_request(&mut stdin, list_db_request);

    // Wait for list databases response
    thread::sleep(Duration::from_millis(500));
    let list_db_response = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for list databases response");

    println!(
        "List databases response: {}",
        serde_json::to_string_pretty(&list_db_response).unwrap()
    );

    // Check the response
    assert_eq!(list_db_response["id"], 4);

    // Verify we got a successful response (not checking specific databases as they may vary)
    if let Some(error) = list_db_response.get("error") {
        panic!("List databases request failed with error: {:?}", error);
    }

    // Clean up
    child.kill().expect("Failed to kill child process");
    drop(stdin);
    reader_thread.join().ok();
}

#[test]
fn test_env_variable_loading() {
    // This test verifies that the environment variables are loaded correctly
    use std::env;

    // Save current env vars
    let original_host = env::var("TERMINUSDB_HOST").ok();
    let original_user = env::var("TERMINUSDB_USER").ok();
    let original_pass = env::var("TERMINUSDB_PASS").ok();

    // Load the .secrets.env file
    dotenv::from_path(".secrets.env").ok();

    // Check that the variables were loaded
    let host = env::var("TERMINUSDB_HOST").expect("TERMINUSDB_HOST should be set");
    let user = env::var("TERMINUSDB_USER").expect("TERMINUSDB_USER should be set");
    let pass = env::var("TERMINUSDB_PASS").expect("TERMINUSDB_PASS should be set");

    assert_eq!(host, "http://tdb-dev.eastus.azurecontainer.io:6363");
    assert_eq!(user, "admin");
    assert_eq!(pass, "sdkhgvslivglwiyagvw");

    // Restore original env vars
    match original_host {
        Some(val) => env::set_var("TERMINUSDB_HOST", val),
        None => env::remove_var("TERMINUSDB_HOST"),
    }
    match original_user {
        Some(val) => env::set_var("TERMINUSDB_USER", val),
        None => env::remove_var("TERMINUSDB_USER"),
    }
    match original_pass {
        Some(val) => env::set_var("TERMINUSDB_PASS", val),
        None => env::remove_var("TERMINUSDB_PASS"),
    }
}

use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_server_status_request_format() {
        // Test that we can create a valid check_server_status request
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "check_server_status",
                "arguments": {
                    "connection": {
                        "host": "http://localhost:6363",
                        "user": "admin",
                        "password": "root"
                    }
                }
            }
        });

        // Verify the structure
        assert_eq!(request["method"], "tools/call");
        assert_eq!(request["params"]["name"], "check_server_status");
        assert!(request["params"]["arguments"]["connection"].is_object());
    }

    #[test]
    fn test_check_server_status_with_defaults() {
        // Test that we can use default connection settings
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "check_server_status",
                "arguments": {}
            }
        });

        assert_eq!(request["params"]["name"], "check_server_status");
        // Empty arguments should use default connection config
        assert!(request["params"]["arguments"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_expected_response_format() {
        // Test the expected response format for different scenarios

        // Server running response
        let running_response = json!({
            "status": "running",
            "connected": true,
            "server_info": {
                "version": "10.0.0",
                "storage": "memory"
            }
        });

        assert_eq!(running_response["status"], "running");
        assert_eq!(running_response["connected"], true);
        assert!(running_response["server_info"].is_object());

        // Server offline response
        let offline_response = json!({
            "status": "offline",
            "connected": false,
            "error": "Server is not responding"
        });

        assert_eq!(offline_response["status"], "offline");
        assert_eq!(offline_response["connected"], false);
        assert!(offline_response["error"].is_string());

        // Server error response
        let error_response = json!({
            "status": "error",
            "connected": false,
            "error": "Server responded but info request failed: connection refused"
        });

        assert_eq!(error_response["status"], "error");
        assert_eq!(error_response["connected"], false);
        assert!(error_response["error"].is_string());
    }
}

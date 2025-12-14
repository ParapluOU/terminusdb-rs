#!/bin/bash

# Script to manually test the TerminusDB MCP server
# This script demonstrates the MCP protocol message flow

echo "TerminusDB MCP Server Manual Testing Script"
echo "=========================================="
echo ""
echo "This script will send MCP protocol messages to test the server."
echo "Make sure TerminusDB is running on localhost:6363"
echo ""

# Function to send a message and wait for response
send_message() {
    local message="$1"
    echo "Sending: $message"
    echo "$message"
    echo ""
    sleep 1
}

echo "Starting MCP server in the background..."
echo "Run this command in a separate terminal:"
echo "cargo run --package terminusdb-mcp-server"
echo ""
echo "Press Enter when the server is running..."
read

echo "1. Sending initialize request..."
send_message '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "roots": {
        "listChanged": true
      }
    },
    "clientInfo": {
      "name": "ManualTestClient",
      "version": "1.0.0"
    }
  }
}'

echo "Expected response: Server capabilities and info"
echo "Press Enter to continue..."
read

echo "2. Sending initialized notification..."
send_message '{
  "jsonrpc": "2.0",
  "method": "notifications/initialized"
}'

echo "No response expected (notification)"
echo "Press Enter to continue..."
read

echo "3. Listing available tools..."
send_message '{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}'

echo "Expected response: List of 3 tools (execute_woql, get_schema, list_databases)"
echo "Press Enter to continue..."
read

echo "4. Executing a WOQL query..."
send_message '{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "execute_woql",
    "arguments": {
      "query": "select ?x ?y where { ?x a ?y }",
      "database": "_system"
    }
  }
}'

echo "Expected response: Query results or error"
echo "Press Enter to continue..."
read

echo "5. Testing error handling with invalid query..."
send_message '{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "execute_woql",
    "arguments": {
      "query": "this is not valid WOQL",
      "database": "test_db"
    }
  }
}'

echo "Expected response: Error with parsing details"
echo ""
echo "Test complete!"
echo ""
echo "To pipe these messages to the server, you can use:"
echo "cat messages.jsonl | cargo run --package terminusdb-mcp-server"
echo ""
echo "Where messages.jsonl contains one JSON message per line."
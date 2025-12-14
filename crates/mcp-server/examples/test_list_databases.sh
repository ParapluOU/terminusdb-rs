#!/bin/bash

# Test the list_databases functionality of the MCP server

echo "Testing list_databases tool..."
echo

# Start the MCP server and send a list_databases request
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_databases","arguments":{}}}' | cargo run --package terminusdb-mcp-server --quiet 2>/dev/null | jq .

echo
echo "Note: This test requires a running TerminusDB instance at http://localhost:6363"
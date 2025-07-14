# TerminusDB MCP Server Tests

This directory contains tests for the TerminusDB MCP (Model Context Protocol) server.

## Test Structure

- `test_mcp_messages.rs` - Examples of MCP protocol messages and expected formats
- `test_protocol.rs` - Tests for MCP protocol initialization and handshake
- `test_tools.rs` - Tests for the three MCP tools (execute_woql, get_schema, list_databases)
- `test_integration.rs` - Integration tests that require a running TerminusDB instance

## Running Tests

### Unit Tests
Run all unit tests:
```bash
cargo test --package terminusdb-mcp-server
```

### Integration Tests
Integration tests require a running TerminusDB instance on `localhost:6363`.

Start TerminusDB first:
```bash
docker run -p 6363:6363 terminusdb/terminusdb-server:latest
```

Then run integration tests:
```bash
cargo test --package terminusdb-mcp-server -- --ignored
```

### Understanding MCP Protocol

Run the message format tests to see examples of MCP protocol messages:
```bash
cargo test --package terminusdb-mcp-server test_mcp_messages -- --nocapture
```

## MCP Protocol Initialization Sequence

The MCP protocol requires a specific initialization sequence:

1. **Client sends `initialize` request** with:
   - Protocol version (must be "2024-11-05")
   - Client capabilities
   - Client info (name and version)

2. **Server responds** with:
   - Supported protocol version
   - Server capabilities (tools, resources, etc.)
   - Server info

3. **Client sends `notifications/initialized`** to indicate ready state

4. **Normal operations begin** - client can now:
   - List tools with `tools/list`
   - Call tools with `tools/call`
   - Subscribe to notifications

## Testing the MCP Server Manually

You can test the server manually using the MCP SDK test client:

```bash
# Start the server
cargo run --package terminusdb-mcp-server

# In another terminal, send JSON-RPC messages via stdin
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' | cargo run --package terminusdb-mcp-server
```

## Environment Variables

The tests use these environment variables (with defaults):
- `TERMINUSDB_HOST` - TerminusDB server URL (default: http://localhost:6363)
- `TERMINUSDB_USER` - Username (default: admin)
- `TERMINUSDB_PASSWORD` - Password (default: root)

## Common Test Scenarios

### Valid WOQL Query
```rust
let args = json!({
    "query": "select ?x ?y where { ?x a ?y }",
    "database": "mydb"
});
```

### Time-Travel Query
```rust
let args = json!({
    "query": "select ?x where { ?x a Person }",
    "database": "mydb",
    "commit": "abc123def456"  // Query at specific commit
});
```

### Custom Connection
```rust
let args = json!({
    "query": "select ?x where { ?x a Person }",
    "database": "mydb",
    "connection": {
        "host": "http://custom-host:6363",
        "user": "custom_user",
        "password": "custom_pass"
    }
});
```
# TerminusDB Client Debug Logging

This document describes the debug logging functionality added to the TerminusDB Rust client.

## Features

### 1. In-Memory Operation Log
- Maintains a ring buffer of the last 50 operations (configurable)
- Tracks operation type, timing, success/failure, and result counts
- Accessible via `client.get_operation_log()` and `client.get_recent_operations(n)`

### 2. Persistent Query Log
- Appends all operations to a log file in newline-delimited JSON format
- Includes timestamps, operation details, and results (no sensitive data)
- Supports log rotation
- Enable with `client.enable_query_log("/path/to/log.json")`

## Usage Example

```rust
use terminusdb_client::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    
    // Enable query logging
    client.enable_query_log("/var/log/terminusdb/queries.log").await?;
    
    // Your operations here...
    let spec = BranchSpec::from("mydb");
    let results = client.query(Some(spec), my_query).await?;
    
    // Check recent operations
    for op in client.get_recent_operations(10) {
        println!("{}: {} - {} results in {}ms", 
            op.timestamp, op.operation_type, 
            op.result_count.unwrap_or(0), op.duration_ms);
    }
    
    // Rotate log file
    client.rotate_query_log().await?;
    
    Ok(())
}
```

## Query Log Format

Each line in the query log is a JSON object:

```json
{
  "timestamp": "2024-01-21T10:30:00Z",
  "operation_type": "query",
  "database": "mydb",
  "branch": "main",
  "endpoint": "/api/woql/mydb",
  "details": {"query_type": "select", "variables": ["Subject", "Predicate", "Object"]},
  "success": true,
  "result_count": 42,
  "duration_ms": 125,
  "error": null
}
```

## Operations Tracked

- **Queries**: WOQL queries with result counts
- **Instance Operations**: Insert, update, delete with entity information
- **Database Operations**: Create, delete database operations
- **Schema Operations**: Schema insertions and updates (when integrated)

## Configuration

The debug functionality is enabled by default via the `debug-logging` feature flag.
To disable at compile time:

```toml
[dependencies]
terminusdb-client = { version = "0.1", default-features = false }
```

## Performance Considerations

- Operation log has minimal overhead (in-memory ring buffer)
- Query log uses async I/O to avoid blocking operations
- Logging can be disabled at runtime via `client.disable_query_log()`

## Testing

Run the debug logging tests:

```bash
cargo test test_debug_logging
```

Run the example:

```bash
cargo run --example debug_logging
```
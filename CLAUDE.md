# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Build and Development Commands

```bash
# Build all crates in the workspace
cargo build

# Build release version
cargo build --release

# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p terminusdb-client
cargo test -p terminusdb-schema
cargo test -p terminusdb-woql-builder

# Run a specific test
cargo test test_name

# Run ignored tests (requires running TerminusDB instance)
cargo test -- --ignored

# Generate OpenAPI client (from client directory)
cd client && make generate-client

# Check code formatting
cargo fmt -- --check

# Run clippy lints
cargo clippy -- -D warnings

# Build documentation
cargo doc --no-deps --open
```

## Architecture Overview

This is a Rust client library for TerminusDB, organized as a Cargo workspace
with multiple interconnected crates:

### Core Crates

1. **terminusdb-client**: HTTP client for TerminusDB operations
   - Async operations using tokio
   - Document CRUD operations in `client/src/document/`
   - Query execution via `client/src/query.rs`
   - Commit tracking and instance management in `client/src/log/`
   - Local development uses `TerminusDBHttpClient::local_node()` for
     `http://localhost:6363`

2. **terminusdb-schema**: Type system and schema definitions
   - Core traits and types for TerminusDB documents
   - JSON serialization/deserialization
   - Instance validation and management
   - Type implementations for primitives, collections, and custom types

3. **terminusdb-schema-derive**: Procedural macros for deriving schema traits
   - Simplifies creation of TerminusDB-compatible types
   - Automatically implements required traits

4. **terminusdb-woql2**: WOQL query language implementation
   - Query operations (control flow, data manipulation, logic, math, string ops)
   - Graph traversal and triple store operations
   - Path queries

5. **terminusdb-woql-builder**: Builder pattern for constructing WOQL queries
   - Type-safe query construction
   - Fluent API for building complex queries

### Key Design Patterns

- **Async-first**: All network operations use async/await
- **Type safety**: Strong typing throughout, especially in schema and query
  building
- **Error handling**: Comprehensive error types using `thiserror` and `anyhow`
- **Platform support**: Conditional compilation for WASM targets
- **Feature flags**: Uses nightly Rust features (specialization,
  associated_type_defaults)

### Testing Approach

- Unit tests are inline with source code
- Integration tests in `tests/` directories
- Async tests use `#[tokio::test]`
- **Prefer `TerminusDBServer` for integration tests** - no `#[ignore]` needed, runs in parallel

### Writing Parallel Integration Tests with TerminusDBServer

Use `terminusdb_bin::TerminusDBServer` for tests that need a real database. Each process gets its own server on a unique port, enabling parallel test execution without conflicts.

**Best Practice Pattern:**
```rust
use terminusdb_bin::TerminusDBServer;

#[tokio::test]
async fn test_my_feature() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    // Use with_tmp_db for automatic cleanup and unique database names
    server.with_tmp_db("test_my_feature", |client, spec| async move {
        // Each test gets a unique database like "test_my_feature_<uuid>"
        // Database is automatically deleted when closure returns

        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_instance(&my_model, args).await?;

        Ok(())
    }).await
}
```

**With Pre-inserted Schemas:**
```rust
#[tokio::test]
async fn test_with_models() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    // Schemas for MyModel are automatically inserted
    server.with_db_schema::<(MyModel,)>("test_models", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_instance(&my_instance, args).await?;
        Ok(())
    }).await
}
```

**Key Points:**
- `test_instance()` returns a shared server per-process (efficient for multiple tests)
- `with_tmp_db()` creates a unique database name using UUID (no conflicts between parallel tests)
- `with_db_schema::<T>()` pre-inserts schemas before running the test
- Databases are automatically cleaned up even if the test fails/panics
- No `#[ignore]` needed - tests run automatically with `cargo test`
- Each test process gets its own server on a unique port via `TERMINUSDB_SERVER_PORT`

### Important Notes

- Current branch: main
- Recent work focuses on instance tracking and commit ID functionality
- HTTP client uses `reqwest` for native targets only (not available in WASM)
- OpenAPI client generation available via Docker in client directory

### TerminusDB-Data-Version Header Support

The client now automatically captures the `TerminusDB-Data-Version` header from HTTP responses:

- **Transparent wrapper**: All insert functions return `ResponseWithHeaders<T>` which implements `Deref<Target=T>` for backward compatibility
- **Header access**: Use `.commit_id` field to access the commit ID from responses
- **Efficient commit tracking**: `insert_instance_with_commit_id()` now uses headers instead of commit log iteration
- **Fallback support**: Falls back to commit log search if header is not present or for existing instances

Example usage:
```rust
let result = client.insert_instance(&model, args).await?;
// Works as before due to Deref implementation
let ids = result.values().collect::<Vec<_>>();

// Access header information
if let Some(header_value) = &result.commit_id {
    println!("Full header: {}", header_value); // e.g., "branch:abc123..."
    // The insert_instance_with_commit_id() function automatically extracts just the commit ID part
}

// Or use the convenience function that extracts the commit ID automatically
let (instance_id, commit_id) = client.insert_instance_with_commit_id(&model, args).await?;
println!("Commit ID: {}", commit_id); // Just "abc123..." without the "branch:" prefix
```

## Common Troubleshooting

- When TerminusDB returns an error indicating a "Schema failure", it most often means we have changed a model's shape after inserting its schema. This can be resolved by dropping the database using the client::delete_database() function.

## Testing and Development Insights

- When writing tests for WOQL functionality, nothing is proving it is "working" until the WOQL is tested against an actual database. Whether a WOQL functionality works can only be determined based on the result of an actual query with it. The client can be called in a unit/integration test, but Claude can also use the TerminusDB MCP server to realtime test/debug queries

## Model Serialization and Trait Implementation Notes

- If serializing TerminusDB models (structs/enums deriving TerminusDBModel) have issues with how they are serialized, then DONT try to fix it with a custom Serialize, Deserialize impl, as these are not used by our implementation. If the struct is supposed to be a "model", it should be converteable to an Instance, so derive TerminusDBModel. If the layout of it has to change use the tdb() proc-macro attributes defined in the schema/derive. If structs represent pritimitive values, they need implementations of ToInstanceProperty instead so that they are convertable to field values without representing a model.
- tests should NOT manually implement ToTDBSchema. if there are import conflicts because the derive hardcodes terminusdb_schema, create a crate alias like 'use crate as terminusdb_schema'
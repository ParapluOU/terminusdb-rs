# TerminusDB Rust Client

A Rust client library for [TerminusDB](https://terminusdb.com/), a document and
graph database built for the web age.

## Overview

This repository contains multiple crates that provide comprehensive Rust support
for TerminusDB:

- **`terminusdb-client`** - High-level client for interacting with TerminusDB
- **`terminusdb-schema`** - Schema definitions and validation for TerminusDB
- **`terminusdb-schema-derive`** - Derive macros for TerminusDB schema types
- **`terminusdb-woql`** - WOQL (Web Object Query Language) support
- **`terminusdb-woql2`** - Enhanced WOQL functionality
- **`terminusdb-woql-builder`** - Builder pattern for constructing WOQL queries

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
terminusdb-client = "0.1.0"
```

## Quick Start

```rust
use terminusdb_client::*;
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Serialize, Deserialize};

// Define your data model
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    age: i32,
    email: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to TerminusDB
    let client = TerminusDBHttpClient::local_node().await?;
    
    // Create database
    let db_name = "my_app";
    client.ensure_database(db_name).await?;
    
    // Insert schema
    let branch = BranchSpec::from(db_name);
    let args = DocumentInsertArgs::from(branch.clone());
    client.schema::<Person>(args.clone()).await?;
    
    // Create and insert data
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        email: Some("alice@example.com".to_string()),
    };
    
    client.insert(&person, args).await?;
    
    Ok(())
}
```

## Working with TerminusDB Models

### Creating TDB Models

Define your data structures using the `#[derive(TerminusDBModel)]` attribute:

```rust
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Serialize, Deserialize};

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct User {
    name: String,
    age: i32,
    active: bool,
}
```

### Model Attributes

The `#[tdb]` attribute system provides powerful customization options:

#### Struct-Level Attributes

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(
    class_name = "CustomUser",           // Custom class name in database
    base = "http://example.org/",        // Base URI for the schema
    key = "hash",                        // Key type: "hash", "random", or "lexical"
    doc = "User profile information",    // Documentation string
    id_field = "user_id"                // Use specific field as ID (see ID Fields section)
)]
struct User {
    user_id: String,  // When using id_field, this becomes the document ID
    name: String,
    age: i32,
}
```

#### Field-Level Attributes

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Person {
    #[tdb(name = "fullName")]                    // Custom property name
    name: String,
    
    #[tdb(name = "userAge", doc = "Age in years")]  // Custom name + documentation
    age: i32,
    
    #[tdb(subdocument = true)]                   // Embed as nested document
    address: Address,
    
    #[tdb(name = "emailAddress", class = "xsd:string")]  // Custom property name + type
    email: Option<String>,
}
```

#### Enum Attributes

```rust
// Simple enum (becomes TerminusDB Enum)
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(doc = "User status enumeration")]
enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

// Tagged union (becomes TerminusDB TaggedUnion)
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(unfoldable = true)]  // Enable unfoldable tagged union
enum ContactInfo {
    Email(String),
    Phone(String),
    Address { street: String, city: String },
}
```

### ID Fields and Random Keys

By default, TerminusDB generates random IDs for documents. You can control this behavior:

#### Using Generated IDs (Default)
```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Document {
    title: String,
    content: String,
}
// TerminusDB will generate a random ID like "Document_abc123def456"
```

#### Using Custom ID Fields
```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(id_field = "id")]  // Use the 'id' field as the document identifier
struct Document {
    id: String,      // This field becomes the document ID
    title: String,
    content: String,
}

// Usage:
let doc = Document {
    id: "my-custom-id".to_string(),
    title: "My Document".to_string(),
    content: "Document content".to_string(),
};
```

**Important:** When using `id_field`, the specified field value becomes the document's unique identifier in the database. Ensure these values are unique to avoid conflicts.

### Schema vs Instance vs Document

Understanding the terminology is crucial:

- **Schema**: The structural definition of your data model. Created using `client.schema::<T>()` or `client.insert_entity_schema::<T>()`.

- **Instance**: A strongly-typed Rust struct that implements `TerminusDBModel`. Use `*_instance` methods like `client.insert()`, `client.get()`, `client.has()`.

- **Document**: An untyped JSON-like structure (`serde_json::Value`). Use `*_document` methods for working with raw JSON data.

### Schema Insertion Workflow

Before inserting data, you must insert the schema:

```rust
// 1. Connect to database
let client = TerminusDBHttpClient::local_node().await?;
let db_name = "my_database";
client.ensure_database(db_name).await?;

// 2. Insert schema for your models
let branch = BranchSpec::from(db_name);
let args = DocumentInsertArgs::from(branch.clone());

client.schema::<User>(args.clone()).await?;  // Insert User schema
client.schema::<Address>(args.clone()).await?;  // Insert related schemas

// 3. Now you can insert instances
let user = User { /* ... */ };
client.insert(&user, args).await?;
```

### Model Insertion and Retrieval

```rust
// Insert a model instance
let user = User {
    name: "Alice".to_string(),
    age: 30,
    active: true,
};

let result = client.insert(&user, args.clone()).await?;
println!("Inserted with ID: {:?}", result);

// Check if an instance exists
let exists = client.has::<User>("user_id", &branch).await?;

// Retrieve an instance
let retrieved_user = client.get::<User>("user_id", &branch).await?;

// Insert multiple instances
let users = vec![user1, user2, user3];
client.insert_many(&users, args).await?;
```

### Query Construction

Use WOQL (Web Object Query Language) for advanced queries:

```rust
use terminusdb_woql_builder::prelude::*;

// Build a query using the WOQL builder
let v_id = vars!("id");
let v_name = vars!("name");

let query = WoqlBuilder::new()
    .triple(v_id.clone(), "name", v_name.clone())
    .isa(v_id.clone(), node("User"))
    .select(vec![v_name.clone()])
    .finalize();

// Execute the query
let response = client.query::<HashMap<String, String>>(
    Some(branch), 
    query
).await?;

for binding in response.bindings {
    println!("Found user: {}", binding.get("name").unwrap());
}
```

### Error Handling and Troubleshooting

#### Schema Failures
```rust
// If you get schema failures, reset the database:
client.reset_database(db_name).await?;

// Schema failures typically occur when:
// 1. Model structure changed after inserting schema
// 2. Field types don't match previously inserted schema
// 3. Required fields are missing
```

#### Common Patterns
```rust
// Always ensure database exists before operations
client.ensure_database(db_name).await?;

// Insert schemas before inserting data
client.schema::<MyModel>(args.clone()).await?;

// Use the commit tracking for version control
let result = client.insert_instance_with_commit_id(&model, args).await?;
println!("Data version: {}", result.commit_id);
```

### Special Types Support

TerminusDB-rs supports various Rust types:

```rust
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct AdvancedModel {
    id: Uuid,                                    // UUID -> xsd:string
    created_at: DateTime<Utc>,                   // DateTime -> xsd:dateTime
    metadata: HashMap<String, serde_json::Value>, // HashMap -> sys:JSON
    tags: Vec<String>,                           // Vec -> List type
    data: serde_json::Value,                     // JSON -> sys:JSON
}
```

## Features

- **Async/await support** - Built with `tokio` for modern async Rust
- **Type-safe queries** - Compile-time query validation with WOQL
- **Schema validation** - Strong typing for TerminusDB documents
- **Cross-platform** - Supports both native and WASM targets
- **Version tracking** - Built-in commit ID tracking with headers
- **Flexible modeling** - Support for enums, tagged unions, and nested structures

## Development

This is a Cargo workspace. To build all crates:

```bash
cargo build
```

To run tests:

```bash
cargo test
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Future Development

### JavaScript Client Reference

For future development and feature parity, developers should reference the official JavaScript client:
- Repository: https://github.com/terminusdb/terminusdb-client-js
- Key files to study:
  - `lib/woqlClient.js` - Main client implementation
  - `lib/connectionConfig.js` - URL construction patterns
  - `lib/query/` - Query building functionality

Features to port from JS client:
- [ ] Advanced query building
- [ ] Schema migration tools
- [ ] Branch management operations
- [ ] Remote database operations (push/pull/clone)
- [ ] Advanced authentication methods
- [ ] Streaming operations
- [ ] Patch/diff operations

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

### ID Field Configuration

The `id_field` attribute specifies which field holds the document's ID. The field type you use depends on your key strategy:

#### Key Strategy and ID Field Types

| Key Strategy | ID Field Type | Description |
|-------------|---------------|-------------|
| `random` | `EntityIDFor<Self>` or `String` | Client provides or generates ID |
| `lexical` | `ServerIDFor<Self>` | Server computes ID from `key_fields` |
| `hash` | `ServerIDFor<Self>` | Server computes hash-based ID |
| `value_hash` | `ServerIDFor<Self>` | Server computes content hash ID |

#### ServerIDFor (Server-Assigned IDs)

Use `ServerIDFor<Self>` when the server computes the ID (lexical, hash, value_hash keys):

```rust
use terminusdb_schema::ServerIDFor;

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
struct User {
    id: ServerIDFor<Self>,  // Empty until server assigns
    email: String,
    name: String,
}

// Create with empty ID placeholder
let user = User {
    id: ServerIDFor::new(),
    email: "alice@example.com".to_string(),
    name: "Alice".to_string(),
};

// After insertion, retrieve to get the server-assigned ID
let (saved_user, _) = client.insert_instance_and_retrieve(&user, args).await?;
println!("Server-assigned ID: {}", saved_user.id.as_ref().unwrap().id());
```

#### EntityIDFor (Client-Provided IDs)

Use `EntityIDFor<Self>` when you want to control the ID (typically with random keys):

```rust
use terminusdb_schema::EntityIDFor;

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(key = "random", id_field = "id")]
struct Document {
    id: EntityIDFor<Self>,
    title: String,
    content: String,
}

// Create with custom ID
let doc = Document {
    id: EntityIDFor::new("my-custom-id")?,
    title: "My Document".to_string(),
    content: "Content here".to_string(),
};

// Or generate a random UUID-based ID
let doc_random = Document {
    id: EntityIDFor::random(),
    title: "Random Doc".to_string(),
    content: "Content".to_string(),
};
```

#### Using String for Simple Cases

For random keys only, you can use a plain `String`:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(key = "random", id_field = "id")]
struct SimpleDoc {
    id: String,
    content: String,
}

let doc = SimpleDoc {
    id: "my-id".to_string(),
    content: "Hello".to_string(),
};
```

### Entity IDs (`EntityIDFor<T>`)

`EntityIDFor<T>` is a strongly-typed ID wrapper that ensures type-safe references between models. It validates that IDs match the expected type at runtime.

#### Creating IDs

```rust
use terminusdb_schema::EntityIDFor;

// From a simple ID (auto-prefixes with type name)
let id = EntityIDFor::<Person>::new("123")?;  // → "Person/123"

// Generate random UUID-based ID
let id = EntityIDFor::<Person>::random();  // → "Person/550e8400-e29b-..."

// From full typed path
let id = EntityIDFor::<Person>::new("Person/123")?;

// From full IRI (for advanced use)
let id = EntityIDFor::<Person>::new_unchecked("terminusdb://data#Person/123")?;
```

#### Cross-Model References

Reference other models type-safely:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct User {
    name: String,
    email: String,
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Post {
    title: String,
    content: String,
    author_id: EntityIDFor<User>,  // Type-safe reference to User
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Comment {
    text: String,
    post_id: EntityIDFor<Post>,      // Reference to Post
    author_id: EntityIDFor<User>,    // Reference to User
}

// Usage
let comment = Comment {
    text: "Great post!".to_string(),
    post_id: EntityIDFor::new("post-123")?,
    author_id: EntityIDFor::new("user-456")?,
};
```

#### TaggedUnion IDs

For tagged unions, use `new_variant()` to specify the concrete variant type:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
enum PaymentMethod {
    CreditCard { card_number: String, cvv: String },
    BankTransfer { account: String, routing: String },
}

// Create ID for a specific variant
let id: EntityIDFor<PaymentMethod> =
    EntityIDFor::new_variant::<PaymentMethodCreditCard>("cc_123")?;
// Result: "PaymentMethodCreditCard/cc_123"
```

#### ID Accessors

```rust
let id = EntityIDFor::<Person>::new("terminusdb://data#Person/123")?;

id.id()            // "123" - just the ID part
id.typed()         // "Person/123" - type-prefixed
id.iri()           // "terminusdb://data#Person/123" - full IRI
id.get_type_name() // "Person" - type name only
id.get_base_uri()  // Some("terminusdb://data") - base URI if present
```

### Lazy Loading (`TdbLazy<T>`)

`TdbLazy<T>` provides lazy-loading for relationships, storing either an ID reference or the loaded data. Unlike `EntityIDFor<T>`, it creates actual document links in the schema.

#### When to Use Each

| Type | Schema Link | Lazy Loading | Use Case |
|------|-------------|--------------|----------|
| `EntityIDFor<T>` | No | Manual | Lightweight foreign key references |
| `TdbLazy<T>` | Yes | Built-in | Full relationships with auto-loading |

#### Basic Usage

```rust
use terminusdb_schema::TdbLazy;

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Writer {
    name: String,
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct BlogPost {
    title: String,
    writer: TdbLazy<Writer>,  // Lazy-loaded relationship
}

// Create with ID reference only (not loaded)
let post = BlogPost {
    title: "My Post".to_string(),
    writer: TdbLazy::new_id("writer-123")?,
};

// Or create with loaded data
let writer = Writer { name: "Alice".to_string() };
let post = BlogPost {
    title: "My Post".to_string(),
    writer: TdbLazy::from(writer),
};
```

#### Key Methods

```rust
// Check if data is loaded
if lazy_ref.is_loaded() {
    let data = lazy_ref.get_expect();  // Get loaded data (panics if not loaded)
}

// Get the ID reference
let id: &EntityIDFor<Writer> = lazy_ref.id();

// Lazy-load from database
let data = lazy_ref.get(&client)?;  // Fetches if not already loaded

// Convert to reference-only (discard loaded data)
lazy_ref.make_ref();  // Useful to avoid re-saving nested documents
```

#### Serialization Behavior

- **When loaded**: Serializes as the full nested object
- **When ID-only**: Serializes as just the ID string

```rust
// ID-only serializes as: "Writer/writer-123"
// Loaded serializes as: { "name": "Alice", ... }
```

### Subdocuments

Subdocuments are embedded documents without independent identity—they exist only within their parent document and are stored inline.

#### When to Use Subdocuments

- **Value objects**: Addresses, coordinates, configuration blocks
- **Tightly coupled data**: Data that has no meaning outside its parent
- **Performance**: Avoid separate database lookups for related data

#### Struct-Level Subdocument

Mark an entire type to always be embedded:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(subdocument = true, key = "value_hash")]
struct Address {
    street: String,
    city: String,
    country: String,
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    home_address: Address,  // Always embedded (Address is a subdocument type)
}
```

#### Field-Level Subdocument

Mark specific fields to be embedded, even if the type isn't always a subdocument:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    #[tdb(subdocument = true)]
    home_address: Address,     // Embedded subdocument
    employer: Company,         // Regular document reference (separate entity)
}
```

#### Subdocument Collections

Embed collections of subdocuments:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(subdocument = true)]
struct LineItem {
    product: String,
    quantity: i32,
    price: f64,
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Order {
    order_number: String,
    #[tdb(subdocument = true)]
    items: Vec<LineItem>,  // Vec of embedded subdocuments
}
```

#### TaggedUnion Subdocuments

When a tagged union is marked as subdocument, all its variants are also embedded:

```rust
#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(subdocument = true)]
enum ContactMethod {
    Email { address: String, verified: bool },
    Phone { number: String, country_code: String },
    Address { street: String, city: String },
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    #[tdb(subdocument = true)]
    contacts: Vec<ContactMethod>,  // All variants are embedded
}
```

#### Subdocuments vs Regular Documents

| Aspect | Subdocuments | Regular Documents |
|--------|--------------|-------------------|
| Identity | Path-based (Parent/123/field/Child/456) | Own ID (Child/456) |
| Storage | Embedded in parent JSON | Separate document |
| Queries | Must query through parent | Directly queryable |
| Flattening | Never flattened to references | Flattened when serializing |
| Use Case | Value objects, embedded data | Standalone entities |

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

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

## Usage

```rust
use terminusdb_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://localhost:6363")?;
    
    // Your TerminusDB operations here
    
    Ok(())
}
```

## Features

- **Async/await support** - Built with `tokio` for modern async Rust
- **Type-safe queries** - Compile-time query validation with WOQL
- **Schema validation** - Strong typing for TerminusDB documents
- **Cross-platform** - Supports both native and WASM targets

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

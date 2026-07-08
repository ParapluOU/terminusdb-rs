---
tags:
  - tutorial
  - documents
  - curl
  - rust
  - intermediate
title: Rust Client — Getting Started
nextjs:
  metadata:
    title: Rust Client — Getting Started
    description: Connect to TerminusDB from Rust using the community-contributed terminusdb-client crate.
    keywords: terminusdb, document database, documents, json-ld, rust client — getting started
    alternates:
      canonical: https://terminusdb.org/docs/rust-client-quickstart/
---

{% callout type="warning" title="Community contribution — incubating" %}
The Rust client is maintained by [ParapluOU](https://github.com/ParapluOU/terminusdb-rs) — thank you to the community authors. It requires **Rust nightly** and is not yet published on crates.io.
{% /callout %}

## Prerequisites

- TerminusDB running locally — see [Install guide](/docs/install-terminusdb-as-a-docker-container)
- Rust nightly: `rustup install nightly && rustup default nightly`

## Add the dependency

```toml
[dependencies]
terminusdb-client = { git = "https://github.com/ParapluOU/terminusdb-rs" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

Add a `rust-toolchain.toml` to your project root:

```toml
[toolchain]
channel = "nightly"
```

## Connect

Set environment variables:

```bash
export TERMINUSDB_HOST="http://localhost:6363"
export TERMINUSDB_USER="admin"
export TERMINUSDB_PASS="root"
```

```rust
use terminusdb_client::TerminusDBHttpClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::from_env().await?;
    println!("Connected to TerminusDB");
    Ok(())
}
```

Run with `cargo run`. You should see `Connected to TerminusDB`.

## Next steps

- Browse the [crate source and examples](https://github.com/ParapluOU/terminusdb-rs) on GitHub
- For the full HTTP API, see the [HTTP Document API](/docs/http-documents-api/) — all operations available in the Rust client map to HTTP endpoints
- For the complete git-for-data workflow (branch, diff, merge), see the [15-minute tutorial](/docs/first-15-minutes/) using curl

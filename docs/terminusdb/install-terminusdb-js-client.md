---
tags:
  - how-to
  - installation
  - typescript
  - python
title: Install a TerminusDB Client Library
nextjs:
  metadata:
    title: Install a TerminusDB Client Library
    description: Install the TerminusDB client for TypeScript, Python, or Rust. One command to start building.
    keywords: terminusdb, install, client, npm, pip, cargo, typescript, python, rust, setup, getting started
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/install-terminusdb-js-client/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have the TerminusDB client library installed and ready to use in your project.
{% /callout %}

This page covers installing the **client library** — the SDK you use to talk to TerminusDB from your application code. If you need to install the TerminusDB **server** itself, see [Install TerminusDB (Docker)](/docs/install-terminusdb-as-a-docker-container/).

## Install the client

{% code-tabs %}
{% code-tab label="TypeScript / Node.js" %}

**Requirements:** Node.js 18+

```bash
npm install terminusdb
```

This adds `terminusdb` to your `package.json`. You can also use it in a browser via CDN:

```html
<script src="https://unpkg.com/terminusdb/dist/terminusdb-client.min.js"></script>
```

**Verify:**

```bash
node -e "const T = require('terminusdb'); console.log('terminusdb', T.version || 'installed')"
```

{% /code-tab %}
{% code-tab label="Python" %}

**Requirements:** Python 3.9+

It is recommended to install in a virtual environment:

```bash
python3 -m venv ~/.virtualenvs/terminusdb
source ~/.virtualenvs/terminusdb/bin/activate
```

Then install:

```bash
python3 -m pip install terminusdb-client
```

**Verify:**

```bash
python3 -c "import terminusdb_client; print('terminusdb-client', terminusdb_client.__version__)"
```

{% /code-tab %}
{% code-tab label="Rust" %}

**Requirements:** Rust nightly toolchain

```bash
rustup install nightly && rustup default nightly
```

Add to your `Cargo.toml`:

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

{% /code-tab %}
{% /code-tabs %}

## Next steps

- [**TypeScript Quickstart**](/docs/connect-with-the-javascript-client/) — Branch, edit, diff, and merge documents
- [**Python Quickstart**](/docs/connect-with-python-client/) — Same workflow in Python
- [**Rust Quickstart**](/docs/rust-client-quickstart/) — Connect and query from Rust
- [**First 10 Minutes (curl)**](/docs/get-started/) — Try TerminusDB without installing any SDK

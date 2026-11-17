---
title: Git-like version control for JSON data
nextjs:
  metadata:
    title: Git-like version control for JSON data — diff, patch, and branching for JSON documents
    description: TerminusDB gives JSON documents Git-like version control — structural diff, patch, branches, and full commit history. Store schemaless JSON with automatic deduplication. Tutorial with curl examples.
    keywords: json diff and patch, git-for-data, git-like version control, branches, version control, json store, document versioning, terminusdb, branching, historical diff, patch, data versioning, schemaless, deduplication, content-addressed storage, json version control, raw json, upsert, document history, json database, immutable data, audit trail, change tracking
    openGraph:
      title: Git-like version control for JSON data — TerminusDB
      description: Git-like version control for JSON documents. Structural diff, patch, branching, and full commit history with automatic content deduplication.
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/version-controlled-json/
tags:
  - tutorial
  - diff-patch
  - version-control
  - beginner
  - curl
---

TerminusDB is a document database that provides Git-like version control for JSON. Every write is an immutable commit. Every document has full history. Branches, diffs, and patches work exactly as they do in Git — but for structured data instead of source code.

**What is JSON diff?** A structural comparison of two JSON objects that returns the precise field-level changes between them — not a line-based text diff.

**What is JSON patch?** An operation that applies a diff to a JSON object, transforming it into the target state. Diffs and patches are reversible and composable.

**What is git-for-data?** A version control model where every data change is recorded as an immutable commit with author, timestamp, and message. Branches isolate work. Diffs show exactly what changed. History is permanent and queryable.

This tutorial walks through all of these operations with working curl examples against a local TerminusDB instance. No schema is required — store any valid JSON and get versioning for free. Open the interactive walkthrough to run every step — simulated responses work without a database; run live to execute against your own TerminusDB.

{% callout type="note" %}
**What you will build**
By the end of this tutorial you will have:
1. Understood how JSON diff and patch work (conceptually)
2. Stored a schemaless JSON document in TerminusDB
3. Created a branch and used upsert (`PUT ?create=true`) to update it
4. Compared two branches with a structural diff
5. Compared the current state against a historical point
{% /callout %}

## Step 1 — Try a JSON diff {% step %}

Before touching the database, understand what structural diff and patch mean for JSON.

**Diff** takes two JSON objects and returns a precise description of what changed — field by field, value by value. This is not a line-by-line text diff. It is a semantic operation that knows the structure of your data.

Try it now against the public endpoint (no authentication, no database required):

```bash
curl -s -X POST "https://data.terminusdb.org/api/diff" \
  -H "Content-Type: application/json" \
  -d '{
    "before": {"name": "Alice", "role": "engineer", "level": 3},
    "after":  {"name": "Alice", "role": "lead engineer", "level": 4}
  }'
```
{% http-expected %}
{"level":{"@after":4,"@before":3,"@op":"SwapValue"},"role":{"@after":"lead engineer","@before":"engineer","@op":"SwapValue"}}
{% /http-expected %}

TerminusDB detected exactly two field changes: `role` and `level`. The `name` field is identical, so it does not appear in the diff. Each change is a `SwapValue` operation — it records both the old and new value.

## Step 2 — Apply the diff as a patch {% step %}

**Patch** takes one JSON object and a diff, then applies the changes to produce the other object:

```bash
curl -s -X POST "https://data.terminusdb.org/api/patch" \
  -H "Content-Type: application/json" \
  -d '{
    "before": {"name": "Alice", "role": "engineer", "level": 3},
    "patch":  {"level":{"@after":4,"@before":3,"@op":"SwapValue"},"role":{"@after":"lead engineer","@before":"engineer","@op":"SwapValue"}}
  }'
```
{% http-expected %}
{"level":4,"name":"Alice","role":"lead engineer"}
{% /http-expected %}

The patch transformed the `before` document into the `after` document. This is the foundation of TerminusDB's version control — every change is recorded as a reversible, composable patch.

{% callout type="note" title="Key insight" %}
Diff and patch are **standalone operations** — they work on any two JSON objects you supply. When you store documents in TerminusDB, the database computes and stores these diffs automatically on every change. That is how you get full history with zero effort.
{% /callout %}

## Step 3 — Start TerminusDB {% step %}

You need [Docker](https://docs.docker.com/get-docker/) installed. Then start TerminusDB:

```bash
docker run --pull always -d -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  --name terminusdb terminusdb/terminusdb-server
```

Confirm it is running:

```bash
curl -s -u admin:root http://localhost:6363/api/info
```
{% http-expected %}
{"@type":"api:InfoResponse","api:info":{"authority":"terminusdb://system/data/User/admin","edition":"community","storage":{"version":"2"},"terminusdb":{"version":"12.1.0-rc2"},"terminusdb_store":{"version":"0.19.8"}},"api:status":"api:success"}
{% /http-expected %}

{% callout type="warning" title="Already have a container named terminusdb?" %}
If you see `container name "terminusdb" is already in use`, stop and remove it first: `docker rm -f terminusdb`
{% /callout %}

## Step 4 — Create the inventory database {% step %}

Store a JSON document with no schema at all — first, the database:

{% http-example method="POST" path="/api/db/admin/inventory" %}
{"label": "Inventory", "comment": "Product inventory — schemaless JSON store"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## Step 5 — Insert a document {% step %}

Insert a product document. The `raw_json=true` parameter tells TerminusDB to accept any valid JSON — no schema required. Supplying an `@id` field gives the document a stable identity; if you omit it, the server mints a random `JSONDocument/` ID for you:

{% http-example method="POST" path="/api/document/admin/inventory?author=admin&message=Add+first+product&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 42, "tags": ["peripherals", "input"]}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

TerminusDB stored the document exactly as you sent it and returned its identifier. Every change from this point is version-controlled automatically.

{% callout type="note" title="Built-in content deduplication" %}
TerminusDB uses content-addressed storage — identical structures are stored only once, regardless of how many documents or versions share them. If you store thousands of similar JSON documents (common in configuration management, product catalogues, or regulatory filings), repeated fields and sub-objects are automatically deduplicated. Storage grows with the amount of *unique* content, not the number of documents. Combined with structural diffs that record only what changed between versions, this makes TerminusDB exceptionally space-efficient for large collections of similar JSON.
{% /callout %}

## Step 6 — Read it back {% step %}

{% http-example method="GET" path="/api/document/admin/inventory?id=product/SKU-001&raw_json=true" /%}

{% http-expected %}
{"@id":"product/SKU-001","name":"Mechanical Keyboard","price":149.99,"stock":42,"tags":["peripherals","input"]}
{% /http-expected %}

## Step 7 — Restock it {% step %}

Change `stock` from 42 to 87:

{% http-example method="PUT" path="/api/document/admin/inventory?author=admin&message=Restock+SKU-001&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 87, "tags": ["peripherals", "input"]}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

TerminusDB replaced the document and recorded the change. The previous version (stock: 42) is preserved in history.

## Step 8 — Delete it {% step %}

{% http-example method="DELETE" path="/api/document/admin/inventory?id=product/SKU-001&author=admin&message=Remove+SKU-001&raw_json=true" /%}

{% http-expected %}
["product/SKU-001"]
{% /http-expected %}

The document is gone from the current state, but its full history remains accessible.

## Step 9 — Re-add it {% step %}

For the rest of this tutorial, re-insert the document so we have data to work with:

{% http-example method="POST" path="/api/document/admin/inventory?author=admin&message=Re-add+product&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 87, "tags": ["peripherals", "input"]}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

## Step 10 — Create a price-update branch {% step %}

Branches let you make changes in isolation before committing them to your main line:

{% http-example method="POST" path="/api/branch/admin/inventory/local/branch/price-update" %}
{"origin": "admin/inventory/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

You now have two branches: `main` (unchanged) and `price-update` (a copy, ready for edits).

## Step 11 — Upsert on the branch {% step layout="longform" %}

The `create=true` parameter makes PUT behave as an **upsert** — it creates the document if it does not exist, or replaces it if it does. This is the idiomatic way to ensure a document exists in a known state:

{% http-example method="PUT" path="/api/document/admin/inventory/local/branch/price-update?author=admin&message=Price+increase&raw_json=true&create=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 179.99, "stock": 87, "tags": ["peripherals", "input", "premium"]}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

Two changes on the branch: the price increased from 149.99 to 179.99, and a `"premium"` tag was added. The `main` branch is untouched.

{% callout type="note" title="Why use create=true?" %}
Without `create=true`, PUT fails if the document does not exist. With it, you can safely write a document regardless of whether it was previously created — useful in scripts, migrations, and automation where you want idempotent writes.
{% /callout %}

## Step 12 — Diff main against the branch {% step %}

Compare `main` and `price-update` to see exactly what diverged:

{% http-example method="POST" path="/api/diff/admin/inventory" %}
{"before_data_version": "main", "after_data_version": "price-update"}
{% http-expected %}
[{"@id":"terminusdb:///data/product/SKU-001","price":{"@op":"SwapValue","@before":149.99,"@after":179.99},"tags":{"@op":"CopyList","@to":2,"@rest":{"@op":"SwapList","@before":[],"@after":["premium"],"@rest":{"@op":"KeepList"}}}}]
{% /http-expected %}
{% /http-example %}

{% callout type="note" %}
**Reading the diff**

TerminusDB detected two changes:
1. **`price`** — a `SwapValue` from 149.99 to 179.99
2. **`tags`** — a `CopyList` operation that preserves the first two elements (`"peripherals"`, `"input"`) then appends `"premium"`

Fields that did not change (`name`, `stock`) are absent from the diff. TerminusDB tracks the minimal structural delta — no manual change tracking required.
{% /callout %}

You can also diff a single document by adding `document_id` to the request body:

```bash
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/diff/admin/inventory" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "main",
    "after_data_version": "price-update",
    "document_id": "product/SKU-001"
  }'
```
{% http-expected %}
[{"@id":"terminusdb:///data/product/SKU-001","price":{"@op":"SwapValue","@before":149.99,"@after":179.99},"tags":{"@op":"CopyList","@to":2,"@rest":{"@op":"SwapList","@before":[],"@after":["premium"],"@rest":{"@op":"KeepList"}}}}]
{% /http-expected %}

## Step 13 — Commit another change {% step %}

Update the stock level so there is a clear change to detect when comparing against history:

{% http-example method="PUT" path="/api/document/admin/inventory?author=admin&message=Stock+adjustment&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 93, "tags": ["peripherals", "input"]}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

This creates a new commit on `main` — stock changed from 87 to 93. You now have multiple commits in history to compare against.

## Step 14 — Get the document's history {% step %}

The `/api/history` endpoint returns every commit that touched a document, with an inline diff showing what changed in each commit. Add `diff=true` to include the diffs:

```bash publishes="commit_identifiers" publishColumn="identifier" publishLabel="message"
curl -s -u admin:root \
  "http://localhost:6363/api/history/admin/inventory/local/branch/main?id=product/SKU-001&diff=true"
```
{% http-expected %}
[
  {
    "author": "admin",
    "message": "Stock adjustment",
    "identifier": "26ptm8el...",
    "timestamp": 1747581234.567,
    "diff": {
      "@id": "terminusdb:///data/product/SKU-001",
      "stock": {"@op": "SwapValue", "@before": 87, "@after": 93}
    }
  },
  {
    "author": "admin",
    "message": "Re-add product",
    "identifier": "6gj2ud03...",
    "timestamp": 1747581230.123,
    "diff": {
      "@op": "Insert",
      "@insert": {
        "@id": "terminusdb:///data/product/SKU-001",
        "@type": "http://terminusdb.com/schema/sys#JSONDocument",
        "name": "Mechanical Keyboard",
        "price": 149.99,
        "stock": 87,
        "tags": ["peripherals", "input"]
      }
    }
  },
  {
    "author": "admin",
    "message": "Remove SKU-001",
    "identifier": "v3gq229x..."
  },
  {
    "author": "admin",
    "message": "Restock SKU-001",
    "identifier": "ttti5b21...",
    "timestamp": 1747581228.456,
    "diff": {
      "@id": "terminusdb:///data/product/SKU-001",
      "stock": {"@op": "SwapValue", "@before": 42, "@after": 87}
    }
  },
  {
    "author": "admin",
    "message": "Add first product",
    "identifier": "ov8v014j...",
    "timestamp": 1747581226.789,
    "diff": {
      "@op": "Insert",
      "@insert": {
        "@id": "terminusdb:///data/product/SKU-001",
        "@type": "http://terminusdb.com/schema/sys#JSONDocument",
        "name": "Mechanical Keyboard",
        "price": 149.99,
        "stock": 42,
        "tags": ["peripherals", "input"]
      }
    }
  }
]
{% /http-expected %}

{% callout type="note" title="Reading the history" %}
Each entry includes commit metadata (`author`, `message`, `timestamp`, `identifier`) and a `diff` field showing the structural patch for that commit:

- **SwapValue** — a field changed between two values (e.g. stock 87 → 93)
- **Insert** — the document was created (first commit, or re-added after deletion)
- **No diff field** — the document was deleted in that commit

The full history of this document shows all five operations from the tutorial: create, update, delete, re-create, and update again.
{% /callout %}

{% callout type="note" title="One call, full audit trail" %}
With `diff=true`, a single GET request gives you the complete change history for any document — no need to manually fetch commit IDs and diff them one by one. This is the recommended approach for document-level audit trails.
{% /callout %}

## Step 15 — Get the commit log {% step %}

You can also compare any two points in history manually — useful when you need to diff across branches or between arbitrary commits (not just sequential ones). Get the commit log to find identifiers:

{% http-example method="GET" path="/api/log/admin/inventory/local/branch/main?count=3" /%}

{% http-expected %}
[
  {"author": "admin", "identifier": "26ptm8el...", "message": "Stock adjustment", "timestamp": 1747581234.567},
  {"author": "admin", "identifier": "6gj2ud03...", "message": "Re-add product", "timestamp": 1747581230.123},
  {"author": "admin", "identifier": "v3gq229x...", "message": "Remove SKU-001", "timestamp": 1747581228.456}
]
{% /http-expected %}

{% callout type="note" title="Use an identifier from the response above" %}
Each entry in the log has an `identifier` field — a short string like `dcbrirs75l6c...`. The next step uses the identifier of the **oldest** commit (the "Add first product" entry) as `before_data_version`, diffing the current state against the original.

**In the browser:** the identifier flows into the next request automatically when you run the walkthrough — no copy-paste needed.
{% /callout %}

## Step 16 — Diff against a historical point {% step %}

Diff between two data versions, using an identifier from the history above:

```bash slot="commit_identifiers" placeholder="DATA_VERSION"
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/diff/admin/inventory" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "DATA_VERSION",
    "after_data_version": "main",
    "document_id": "product/SKU-001"
  }'
```
{% http-expected %}
{"stock":{"@op":"SwapValue","@before":42,"@after":93}}
{% /http-expected %}

{% callout type="note" title="What you can compare" %}
The `before_data_version` and `after_data_version` fields accept:
- **Branch names** — `"main"`, `"price-update"`
- **Commit identifiers** — the `identifier` value from the log or history endpoints

Mix and match: compare a branch against a commit, a commit against another commit, or any combination.
{% /callout %}

## Step 17 — Clean up {% step %}

Delete the database when you are done experimenting:

{% http-example method="DELETE" path="/api/db/admin/inventory" /%}

{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

To stop TerminusDB entirely:

```bash
docker stop terminusdb && docker rm terminusdb
```

Your data persists in the `terminusdb_storage` Docker volume. To remove it entirely: `docker volume rm terminusdb_storage`.

## What you just learned

| Concept | What it means |
|---------|---------------|
| **Schemaless storage** | `raw_json=true` stores any valid JSON — no schema definition needed |
| **Automatic versioning** | Every POST, PUT, and DELETE is a versioned commit with author and message |
| **Structural diff** | TerminusDB computes field-level semantic diffs, not line-based text diffs |
| **Branches** | Isolate changes, then compare or merge — exactly like Git branches |
| **Upsert** | `PUT ?create=true` creates or updates idempotently |
| **Historical comparison** | Diff any document against any point in its history |
| **Deduplicated storage** | Unchanged fields are stored once, not copied per version |

## Next steps

- **[Your First 10 Minutes (clone)](/docs/get-started/)** — Start from a pre-populated dataset instead of building from scratch
- **[JSON Diff and Patch (deep dive)](/docs/json-diff-and-patch/)** — Full reference for all patch operations (`SwapValue`, `CopyList`, `SwapList`, and more)
- **[HTTP Document API](/docs/http-documents-api/)** — Complete reference for document CRUD endpoints
- **[Branching and Merging](/docs/branch-howto/)** — Advanced branching workflows, merge conflicts, and strategies
- **[Your First Schema](/docs/schema-reference-guide/)** — Add type safety when you are ready for it

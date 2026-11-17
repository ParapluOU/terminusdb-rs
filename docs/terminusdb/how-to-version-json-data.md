---
tags:
  - how-to
  - version-control
  - beginner
  - typescript
  - curl
title: How to Version JSON Data in an Application Database
nextjs:
  metadata:
    title: How to Version JSON Data in an Application Database — TerminusDB
    description: Version JSON documents with Git-style commits, branches, diffs and full history. Step-by-step guide with runnable curl and TypeScript examples against TerminusDB.
    keywords: version json data, json versioning, json version control, document versioning, database history, data versioning, git for data, versioned database, json diff, immutable database, audit trail, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/how-to-version-json-data/
media: []
---

The most reliable way to version JSON data in an application database is to store it in a database that versions every write for you. In TerminusDB, each insert, update and delete becomes an immutable commit — with an author, a timestamp and a message — so you can view the full history of a document, diff any two states, branch for safe changes, and roll back, without writing any versioning code yourself.

## Your options for versioning JSON

Three patterns cover nearly every approach to versioning JSON in an application:

- **Audit columns or history tables** — you copy old versions into a shadow table or a `previous_version` column yourself. It works, but every query and migration carries the extra schema, and computing diffs is your problem.
- **Event sourcing** — you store every change as an event and rebuild state by replaying them. Powerful, but you now maintain an event store, projections and replay logic on top of the database.
- **A versioned database** — the database keeps the history for you. TerminusDB takes this path: writes are commits on an append-only store, so versioning is a property of the storage engine rather than your application code.

This guide walks through the third option end to end — create a database, version a JSON document, inspect its history, and diff two versions. Every curl command runs as-is against a local TerminusDB.

{% prerequisites-connected /%}

## 1. Create a database {% step %}

A TerminusDB database is a versioned container for JSON documents — the rough equivalent of a Git repository.

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "JSON versioning demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## 2. Insert a JSON document — your first commit {% step %}

Every write to TerminusDB is a commit. The `raw_json=true` parameter accepts any valid JSON document — no schema required — and the `author` and `message` parameters are recorded in the commit log, exactly like `git commit -m`.

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=jane@example.com&message=Add+first+product&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 42}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

The `@id` gives the document a stable identity across versions — the response echoes it back as a full IRI. Omit `@id` and the server mints a random ID instead; the versioning works the same either way.

## 3. Update the document — a second version {% step %}

Replace the document with a new state. TerminusDB stores the new version alongside the old one rather than overwriting it.

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=jane@example.com&message=Restock+SKU-001&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 149.99, "stock": 87}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

The document now reads `stock: 87`, and the previous version (`stock: 42`) remains in history — permanently retrievable. Updates return the short `@id` form rather than the full IRI.

## 4. Read the commit history {% step %}

The commit log lists every version of the database, newest first — the equivalent of `git log`.

{% http-example method="GET" path="/api/log/admin/MyDatabase?count=5" /%}
{% http-expected %}
[ {"@id":"ValidCommit/e2ilt9l172wppv0pst4c540hy4oxfo0","@type":"ValidCommit","author":"jane@example.com","identifier":"e2ilt9l172wppv0pst4c540hy4oxfo0","instance":"layer_data:Layer_90c3fcc363b46ed226220f123a04c9a79922a9772be035496a4b817da14eeb1d","message":"Restock SKU-001","parent":"ValidCommit/xfdwgd2786qj8gm5sbqnedbsh0nndyc","schema":"layer_data:Layer_d6f3a3fb1a138ae6a03ba27adce3e524c405a1f8c2ac2b1bbb188568659fd1a2","timestamp":1793755630.417351,"user":"terminusdb://system/data/User/admin"},  {"@id":"ValidCommit/xfdwgd2786qj8gm5sbqnedbsh0nndyc","@type":"ValidCommit","author":"jane@example.com","identifier":"xfdwgd2786qj8gm5sbqnedbsh0nndyc","instance":"layer_data:Layer_e9e2fa57470e051593e306b2ddb87e1ecb2adbbecae8cc3dea7c0bc60fdbbb0f","message":"Add first product","schema":"layer_data:Layer_d6f3a3fb1a138ae6a03ba27adce3e524c405a1f8c2ac2b1bbb188568659fd1a2","timestamp":1793755630.378798,"user":"terminusdb://system/data/User/admin"} ]
{% /http-expected %}

Example output — one entry per commit, each with author, message, timestamp and the commit's identifier:

{% http-expected %}
[
  {
    "@type": "ValidCommit",
    "author": "jane@example.com",
    "identifier": "rlg1k3q...",
    "message": "Restock SKU-001",
    "timestamp": 1760000000.0
  },
  {
    "@type": "ValidCommit",
    "author": "jane@example.com",
    "identifier": "9fhw2a1...",
    "message": "Add first product",
    "timestamp": 1760000001.0
  }
]
{% /http-expected %}

A commit identifier can be pinned into any API path as `local/commit/<id>` for time-travel reads — see [Time Travel](/docs/time-travel-howto/) for the full pattern.

## 5. Isolate a change on a branch {% step %}

Branches let you stage JSON changes without touching production state — the equivalent of `git branch`. Create a `reprice` branch, then write to it by putting the branch name in the API path.

{% http-example method="POST" path="/api/branch/admin/MyDatabase/local/branch/reprice" %}
{"origin": "admin/MyDatabase/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

{% http-example method="PUT" path="/api/document/admin/MyDatabase/local/branch/reprice?author=jane@example.com&message=Reprice+SKU-001&raw_json=true" %}
{"@id": "product/SKU-001", "name": "Mechanical Keyboard", "price": 179.99, "stock": 87}
{% http-expected %}
["product/SKU-001"]
{% /http-expected %}
{% /http-example %}

The price change lives only on `reprice`. `main` still shows the old price.

## 6. Diff two versions {% step %}

The diff endpoint compares any two states — two branches, two commits, or a branch and a commit — and returns the precise field-level changes.

{% http-example method="POST" path="/api/diff/admin/MyDatabase" %}
{"before_data_version": "main", "after_data_version": "reprice"}
{% http-expected %}
[{"@id":"terminusdb:///data/product/SKU-001","price":{"@after":179.99,"@before":149.99,"@op":"SwapValue"}}]
{% /http-expected %}
{% /http-example %}

The diff is a structural operation, not a text diff: it reports `price` as a `SwapValue` from 149.99 to 179.99 and says nothing about fields that did not change. `before_data_version` and `after_data_version` also accept commit IDs as `"commit:<id>"` — the format returned in the `TerminusDB-Data-Version` response header on every write.

## 7. Same workflow in TypeScript {% step %}

The JavaScript client exposes the same operations — insert, update, history, branch and diff — as method calls.

```typescript test-example file="examples/version-json-data.example.ts"
```

## 8. Clean up {% step %}

Delete the demo database — including its history — when you are done.

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## What you get for free

Versioning JSON in TerminusDB brings properties that hand-rolled approaches rarely reach: provenance on every commit (author, message, timestamp — a built-in audit trail), time-travel reads at any commit identifier, three-way merges via `POST /api/apply` that report conflicts instead of silently overwriting, content-addressed deduplication that keeps long histories compact, and clone/push/pull between instances. The full model is covered in [A Graph Database with Git-like Version Control](/docs/graph-database-with-git-like-version-control/).

## See also

- [Git-like Version Control for JSON](/docs/version-controlled-json/) — the concept tutorial, including schemaless storage in more depth
- [How to Branch Your Database](/docs/branch-howto/) — branch, merge and delete with curl, TypeScript and Python
- [Diff & Patch Operations](/docs/diff-and-patch-operations/) — the diff and patch endpoints in detail
- [Time Travel How-To](/docs/time-travel-howto/) — query the database at any historical commit
- [Version Control Operations Reference](/docs/version-control-operations/) — the full HTTP API surface

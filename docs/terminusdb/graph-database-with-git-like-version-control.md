---
tags:
  - explanation
  - version-control
  - knowledge-graph
  - typescript
  - curl
title: A Graph Database with Git-like Version Control for Production Data
nextjs:
  metadata:
    title: Database Version Control with Git-like Branches — TerminusDB
    description: Database version control for production data — Git-style commits, branches, diffs, merges and clone/push/pull, built into the TerminusDB graph database engine.
    keywords: database version control, git-like database, database with version control, git-like version control database, git for data, versioned database, graph database, branching database, production data versioning, immutable database, data provenance, terminusdb, document graph database
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/graph-database-with-git-like-version-control/
media: []
---

TerminusDB is a git-like database: a graph database with version control built into its storage engine rather than layered on top. It stores production data as linked JSON documents, and every write — insert, update, delete, schema change — becomes an immutable commit with an author, timestamp and message. Branches, structural diffs, merges, time travel and clone/push/pull between servers all work the way a Git user expects, but on live application data. If you are evaluating database version control for production, this is the model implemented end to end.

## What "Git-like version control" should mean for a database

Many tools claim version control for data; few deliver the parts that matter in production. A reasonable checklist:

- **Immutable commits** — every change is appended, never overwritten, so history cannot be silently rewritten.
- **Cheap branches** — a branch isolates in-flight work without copying the dataset.
- **Structural diff and merge** — changes are compared at the field level and merged three-way, with genuine conflict detection.
- **Provenance** — each commit records who changed what, when and why, which doubles as an audit trail.
- **Time travel** — any earlier commit is directly readable, for reports, debugging and reproducible pipelines.
- **Distribution** — databases clone, push and pull between instances the way Git repositories do.

TerminusDB implements all six — as properties of the storage engine, not application code or triggers.

## How TerminusDB maps to Git

| Git concept | TerminusDB equivalent |
|---|---|
| Repository | Database — a versioned container of JSON documents |
| Commit | Immutable commit on an append-only layer store |
| Branch | Named line of history, addressed in the API path |
| `git diff` | Structural diff of documents and triples |
| `git merge` | `apply` — three-way merge with conflict reports |
| `git log` | Commit log — queryable history per branch |
| `git clone` / `push` / `pull` | Clone, push and pull between TerminusDB instances |

The one deliberate difference is granularity: Git versions lines in files, TerminusDB versions individual fields and graph edges. A diff tells you *which fact changed*, not which line moved.

## A production workflow in seven calls

The examples below run unchanged against a local TerminusDB — the same operations your application would issue in production: schema, write, branch, change, review, merge.

### 1. Create a database {% step %}

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Production versioning demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 2. Define a schema {% step %}

Production data carries a schema: a `Product` class with typed fields. Schema writes are commits too — they appear in the same history and diff the same way.

{% http-example method="POST" path="/api/document/admin/MyDatabase?graph_type=schema&author=ops@example.com&message=Add+Product+schema" %}
{"@type": "Class", "@id": "Product", "name": "xsd:string", "price": "xsd:decimal", "stock": "xsd:integer"}
{% http-expected %}
["Product"]
{% /http-expected %}
{% /http-example %}

{% callout type="note" title="Merges need a schema" %}
Three-way merge (`apply`, step 7) schema-checks the result. On a schemaless `raw_json` database the merge fails validation because documents have no class — define a schema before relying on merges in production.
{% /callout %}

### 3. Write data — every write is a commit {% step %}

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=ops@example.com&message=Add+product" %}
{"@id": "Product/SKU-001", "@type": "Product", "name": "Mechanical Keyboard", "price": 149.99, "stock": 42}
{% http-expected %}
["Product/SKU-001"]
{% /http-expected %}
{% /http-example %}

Each response carries a `TerminusDB-Data-Version` header with the resulting commit ID — capture it if you need a merge base later.

### 4. Branch before changing production data {% step %}

{% http-example method="POST" path="/api/branch/admin/MyDatabase/local/branch/reprice" %}
{"origin": "admin/MyDatabase/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 5. Change data on the branch — main is untouched {% step %}

{% http-example method="PUT" path="/api/document/admin/MyDatabase/local/branch/reprice?author=ops@example.com&message=Reprice+SKU-001" %}
{"@id": "Product/SKU-001", "@type": "Product", "name": "Mechanical Keyboard", "price": 179.99, "stock": 42}
{% http-expected %}
["Product/SKU-001"]
{% /http-expected %}
{% /http-example %}

### 6. Review the diff before merging {% step %}

{% http-example method="POST" path="/api/diff/admin/MyDatabase" %}
{"before_data_version": "main", "after_data_version": "reprice"}
{% http-expected %}
[{"@id":"Product/SKU-001","price":{"@after":179.99,"@before":149.99,"@op":"SwapValue"}}]
{% /http-expected %}
{% /http-example %}

### 7. Merge the branch into main {% step %}

`apply` performs a three-way merge. `before_commit` is the common ancestor — here `main` has not moved since the branch was created, so its head is the correct merge base. In a live application, capture the head commit at branch-creation time rather than re-reading `main`.

{% http-example method="POST" path="/api/apply/admin/MyDatabase/local/branch/main" %}
{"before_commit": "main", "after_commit": "reprice", "commit_info": {"author": "ops@example.com", "message": "Merge reprice into main"}}
{% http-expected %}
{"@type":"api:ApplyResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

If two branches had changed the same field, `apply` would return HTTP 409 with a conflict report — TerminusDB never silently picks a winner.

Verify the merge landed on main:

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=Product/SKU-001" /%}
{% http-expected %}
{"@id":"Product/SKU-001","@type":"Product","name":"Mechanical Keyboard","price":179.99,"stock":42}
{% /http-expected %}

### Same workflow in TypeScript {% step %}

```typescript test-example file="examples/production-workflow.example.ts"
```

### Clean up {% step %}

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## Built for production, not just demos

The version control model is backed by properties production systems need:

- **ACID transactions** — a commit either lands in full or not at all; concurrent writers never produce partial states.
- **Schema when you want it** — store schemaless JSON, or enforce classes, types and cardinality; schema changes are versioned commits too.
- **Queryable graph** — the same documents form a knowledge graph you can traverse with WOQL, GraphQL or path queries.
- **Rollback and recovery** — reset a branch to any earlier commit to undo a bad deploy or a bad batch.
- **Space-efficient history** — content-addressed, deduplicated storage keeps long histories compact.

## Where it runs

TerminusDB is open source and self-hostable — a single Docker container gives you the full versioned database locally or on your own infrastructure: see [Docker installation](/docs/install-terminusdb-as-a-docker-container/). For a managed service, contact [Gettier](https://gettier.ai) who host TerminusDB data products with the same API.

## When it is not the right tool

Version control for data pays off when your JSON documents are connected, change over time, and carry audit or collaboration requirements. It is not a replacement for a blob store, a fast-moving metrics timeseries database beyond 10-20k docs per second, or a high-throughput append-only log — those workloads do not need this kind of history, branching or merges. We suggest InfluxDB and ClickHouse for time-series data, and S3 or similar for blob storage.

## See also

- [Knowledge Graph Database with Git-like Version Control](/docs/knowledge-graph-database-version-control/) — the underlying concepts in more depth
- [How to Version JSON Data](/docs/how-to-version-json-data/) — step-by-step versioning how-to
- [Merge How-To](/docs/merge-howto/) — merge strategies, rebase and conflict handling
- [Clone, Push, Pull](/docs/use-the-collaboration-features/) — distributing databases between instances
- [ACID Transactions](/docs/acid-transactions-explanation/) — the transactional guarantees behind each commit

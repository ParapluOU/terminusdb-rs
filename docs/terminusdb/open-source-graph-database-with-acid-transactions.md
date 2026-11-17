---
tags:
  - explanation
  - knowledge-graph
  - beginner
  - typescript
  - curl
title: An Open-Source Graph Database with ACID Transactions
nextjs:
  metadata:
    title: An Open-Source Graph Database with ACID Transactions — TerminusDB
    description: "TerminusDB is an Apache 2.0 open-source graph database where every write is an ACID commit: atomic, schema-checked, isolated and durable. With runnable proof."
    keywords: open source graph database, acid transactions, graph database acid, acid compliant database, document graph database, apache 2.0 database, atomic transactions, schema validation, immutable database, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/open-source-graph-database-with-acid-transactions/
media: []
---

TerminusDB is an open-source graph database — Apache 2.0 licensed — with full ACID transactions on every write. It stores linked JSON documents as a property graph, and each insert, update or delete lands as a single atomic commit: schema-validated, isolated from concurrent readers, and durable on an append-only layer store. As a bonus, the same commit machinery gives you Git-style branching, diffs and history on your data.

## Why ACID matters for a graph database

Graph writes rarely touch one record — a realistic update spans several documents and the edges between them. Without atomicity, a write that fails halfway leaves dangling relationships: an order without its customer, a membership edge pointing at a deleted node. Without schema-checked consistency, invalid documents accumulate until queries return garbage. ACID is what lets you treat a multi-document graph update as one operation instead of a cleanup job.

## How TerminusDB delivers each property

| Property | Mechanism |
|---|---|
| **Atomicity** | Every request is one commit — a new immutable layer containing the complete change, applied all at once. A rejected request stores nothing. |
| **Consistency** | When a schema exists, every commit is validated against it: types, required fields and cardinality are enforced before the commit lands. |
| **Isolation** | Readers see the immutable layer at commit boundaries — snapshot isolation by construction, so long-running reads never observe a half-written state. |
| **Durability** | Commits are appended to a content-addressed layer store; a failed or partial commit never modifies existing layers, so prior states stay intact and recoverable. |

For the internals, see [ACID Transactions](/docs/acid-transactions-explanation/) — this page demonstrates the behaviour instead.

## Prove atomicity in five calls

The steps below run unchanged against a local TerminusDB. The demonstration: a batch insert where one document violates the schema — the whole request is rejected and *nothing* is stored.

{% prerequisites-connected /%}

### 1. Create a database {% step %}

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "ACID demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 2. Define a schema with a required field {% step %}

A `Product` class where `price` is mandatory — this is the consistency rule the transaction will be checked against.

{% http-example method="POST" path="/api/document/admin/MyDatabase?graph_type=schema&author=ops@example.com&message=Add+Product+schema" %}
{"@type": "Class", "@id": "Product", "name": "xsd:string", "price": "xsd:decimal", "stock": "xsd:integer"}
{% http-expected %}
["Product"]
{% /http-expected %}
{% /http-example %}

### 3. Send a batch where one document violates the schema {% step %}

`Product/A` is valid; `Product/B` is missing the required `price`. One request, two documents — the invalid one rejects the whole commit.

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=ops@example.com&message=Batch+insert" runnable=false %}
[
  {"@id": "Product/A", "@type": "Product", "name": "Gadget A", "price": 10, "stock": 1},
  {"@id": "Product/B", "@type": "Product", "name": "Gadget B", "stock": 2}
]
{% http-expected %}
{
  "@type": "api:InsertDocumentErrorResponse",
  "api:status": "api:failure",
  "api:message": "Schema check failure",
  "api:error": {
    "@type": "api:SchemaCheckFailure",
    "api:witnesses": [
      {
        "@type": "required_field_does_not_exist_in_document",
        "document": {"@id": "terminusdb:///data/Product/B"},
        "field": "terminusdb:///schema#price"
      }
    ]
  }
}
{% /http-expected %}
{% /http-example %}

The request fails with a schema-check failure naming the offending document and field.

### 4. Verify nothing landed — including the valid document {% step %}

This is the atomicity guarantee: the valid `Product/A` was not stored either.

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=Product/A" runnable=false %}
{% http-expected %}
{
  "@type": "api:GetDocumentErrorResponse",
  "api:status": "api:not_found",
  "api:error": {"@type": "api:DocumentNotFound", "api:document_id": "Product/A"},
  "api:message": "Document not found: 'Product/A'"
}
{% /http-expected %}
{% /http-example %}

HTTP 404 — the database is exactly as it was before the batch. No partial state, no cleanup query.

### 5. Retry with a valid batch — one commit, both documents {% step %}

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=ops@example.com&message=Batch+insert+fixed" %}
[
  {"@id": "Product/A", "@type": "Product", "name": "Gadget A", "price": 10, "stock": 1},
  {"@id": "Product/B", "@type": "Product", "name": "Gadget B", "price": 20, "stock": 2}
]
{% http-expected %}
["Product/A","Product/B"]
{% /http-expected %}
{% /http-example %}

Both documents now exist — written in a single commit, so a reader never sees `Product/A` without `Product/B`.

### Clean up {% step %}

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## Same demo in TypeScript {% step %}

The JavaScript client surfaces the same guarantee — a rejected `addDocument` throws, and the earlier state is untouched.

```typescript test-example file="examples/acid-atomicity.example.ts"
```

## What else the commit model gives you

Because every ACID commit is also a named point in history, the same machinery provides branches, time travel, a per-commit audit trail, and clone/push/pull between instances — the Git-like version control model covered in [A Graph Database with Git-like Version Control](/docs/graph-database-with-git-like-version-control/).

## Where it runs

TerminusDB runs as a single Docker container or a compiled binary — see [Docker installation](/docs/install-terminusdb-as-a-docker-container/) — and the [source is on GitHub](https://github.com/terminusdb/terminusdb) under Apache 2.0. For managed hosting, contact [Gettier](https://gettier.ai/connect) to host TerminusDB data products with the same API.

## See also

- [ACID Transactions](/docs/acid-transactions-explanation/) — how each property is implemented internally
- [Immutability](/docs/immutability-explanation/) — the append-only layer store that makes snapshot isolation free
- [A Graph Database with Git-like Version Control](/docs/graph-database-with-git-like-version-control/) — branching and merging on production data
- [How to Version JSON Data](/docs/how-to-version-json-data/) — the versioning workflow step by step
- [Document Insertion](/docs/document-insertion/) — full Document API reference

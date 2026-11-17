---
tags:
  - how-to
  - vectorlink
  - schema
  - graphql
title: Embeddings Field Guide
nextjs:
  metadata:
    title: Embeddings Field Guide
    description: How TerminusDB turns schema metadata and documents into embedding text — and the schema details that decide whether indexing works at all
    keywords: embeddings, vector search, store_indices, handlebars, graphql, versioned search, vectorlink
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-embeddings/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

Every indexed document starts as text. TerminusDB builds that text in two steps — a GraphQL query fetches the fields you care about, a Handlebars template renders them into a sentence — and the embedding model turns the sentence into a vector. This guide collects the schema details that decide whether that pipeline produces good embeddings, or quietly produces nothing.

## The pipeline, briefly

1. A commit lands on a branch with `store_indices` enabled.
2. For each changed document of an embedding-enabled type, the indexer runs the class's GraphQL `query` against the document *as it exists in that commit*.
3. The query result feeds the `template`, which renders plain text.
4. The text goes to the embedding model. For the default `multilingual-e5-base`, documents are prefixed with `passage:` and search queries with `query:` — the engine does this for you.
5. The vector lands in the index, keyed by the document's TerminusDB ID.

## Schema checklist — the parts that bite

These are ordered by how confusing the failure is when you get them wrong.

### Use the document-API array format

The schema you load through `/api/document` must be a JSON array where every class is an object with its own `@id`:

```json
[
  { "@type": "@context", "@base": "terminusdb:///data/", "@schema": "terminusdb:///schema#", "@metadata": { "terminusdb": { "options": ["store_indices"] } } },
  { "@type": "Class", "@id": "Article", "@key": { "@type": "Lexical", "@fields": ["slug"] }, "slug": "xsd:string", "title": "xsd:string" }
]
```

A keyed-object schema (`{ "Article": { ... } }`) is silently misread: each class is treated as a property value and you get a `TypeError` that names neither the file nor the class.

### `store_indices` is the indexing gate

Embedding metadata on a class is not enough. The `@context` needs `"store_indices"` in `@metadata.terminusdb.options`, or no commit ever reaches the indexer. `store_clustering` is a separate flag for the clustering feature — it does not imply indexing. Wanting both is common:

```json
"options": ["store_indices", "store_clustering"]
```

### No commas in the GraphQL query

TerminusDB's GraphQL parser rejects commas between fields. `Article(id: $id) { title, body }` fails validation; `Article(id: $id) { title body }` works. Write queries without commas, always.

### Match `@type` cardinality to your `{{#if}}` guards

If your template guards a value with `{{#if size}}`, the property must be declared optional:

```json
"size": { "@type": "Optional", "@class": "ProductSize" }
```

A bare `"size": "ProductSize"` makes the property required — any document written without it fails schema validation and takes its whole commit down with it. This shows up when you load data from a source that legitimately omits fields, or when you hand-insert a test document. The template and the schema must agree about what may be absent; the template is usually the better statement of intent.

Related: subdocuments like `ProductSize` should use `@key: {"@type": "ValueHash"}` and `"@subdocument": []` unless you have a reason to identify them.

### `_id` gives you the document IRI in text

The GraphQL field `_id` resolves to the document's TerminusDB ID. Adding it to the query and the end of the template puts a stable handle in the embedding text:

```json
"query": "query($id: ID){ Product(id: $id) { _id name sku } }",
"template": "Product: {{name}}. SKU: {{sku}}. ID: {{_id}}."
```

Useful for provenance and debugging. Do not expect it to make documents retrievable *by* ID — see the limits section below.

## Versioned rendering: the schema is historical too

Indexing renders each commit with the schema that was in effect **at that commit** — not the schema at branch HEAD. This is deliberate: the index is a deterministic replay of history, and a document is embedded with the query and template of its own era.

Two consequences:

- **Documents embed under the schema of their commit.** A document saved while the template said `{{title}}` carries that text forever — editing the schema does not update embeddings that already exist.
- **Get the template right before bulk load.** If you want every document's text to include a field (say, `_id`), it must be in the template before those documents land. To restyle existing documents, change the schema — see "Re-embedding a type" below.

Schema changes still matter going forward: once a type has embedding metadata, every future commit that touches its documents embeds automatically.

## Re-embedding a type

Changing the query or template does not silently reach already-indexed documents — but a schema commit that changes a type's `@metadata.embedding` `query` or `template` re-embeds that type automatically. The schema commit's own push stream emits a `Changed` upsert for every document of the type, rendered with that commit's schema, so live indexing and history replay produce identical results. No endpoint call, no bookkeeping — the change detection reads the commit's schema-layer delta directly.

The same mechanism purges: if a schema commit removes a type's embedding config, drops either `query` or `template` without replacement, or removes the class, it emits `Deleted` ops for all documents of that type — an embedding is only valid with both fields.

Detection is precise about what counts as a change. Because metadata is stored as content-addressed JSON, the delta only reports a type when the `sys:metadata → json#embedding → json#query|json#template` chain actually churned. Re-committing an identical schema, adding an unrelated class property, or editing another metadata key emits no document operations at all.

Heads-up on volume: a template edit on a large type re-embeds all of its documents in one commit — on a million-doc type that is a million embedding operations streamed through the normal pipeline, so expect a long push and a real model pass per document (the cache cannot hit on genuinely new text).

Triggering index on a branch (`POST /api/index/...`) wipes the engine's domain — all indexed documents, tags, and bookkeeping — then replays every commit from the oldest forward. It does not reject when indexing is already running: it aborts the in-flight run and restarts.

What to expect while it runs:

- `searchable_documents` drops and climbs back. A cancelled pipeline may write a few last rows before the wipe takes effect — a transient, not corruption.
- The embedding cache survives the wipe (it is keyed by model + rendered text, not by index), so re-embedding identical text is a cache hit. A changed template produces new text and is a real recompute — a full reindex after a template change is a full pass through the embedding model.

## What embeddings cannot do

Semantic similarity is not exact match. A random GUID or SKU carries no semantics, and no embedding model will reliably rank `Product/bb5d75b8-…` top for a query of that GUID. For find-by-ID, use document lookup or WOQL on the `@id`/key fields — that is what they are for.

Embeddings answer "what is this about", not "which row is this".

## Operational notes

- **Commits index in order.** Each commit is one push; the engine tags it only after every expected document is received. A truncated stream is retried, never partially indexed.
- **Cache sizing.** `VECTORLINK_EMBED_CACHE_SIZE` defaults to 20,000 entries. Bulk-ingesting millions of documents on a slow local model warrants millions of entries (sled + zstd on the data-directory disk; millions of entries ≈ gigabytes). `none` disables the cache.
- **Model.** The default `multilingual-e5-base` (278M params, ~100 languages, MIT) is served as a first-party Q8_0 GGUF from `hf.co/vectorlink/multilingual-e5-base-GGUF`. Queries get the `query:` prefix, documents `passage:` — automatic, but visible if you read the cache keys.

## See also

- [Indexing by Push](/docs/versioned-search-indexing) — the commit-to-index pipeline this page's text feeds
- [Embedded Engine](/docs/versioned-search-embedded) — running VectorLink in-process
- [Searching](/docs/versioned-search-querying) — where the vectors end up

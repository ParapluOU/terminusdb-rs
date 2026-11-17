---
tags:
  - how-to
  - vectorlink
  - schema
title: Clustering Embeddings
nextjs:
  metadata:
    title: Clustering Embeddings
    description: How to enable store_clustering in your schema to generate clustering embeddings for projection, deduplication, and entity resolution
    keywords: terminusdb, versioned search, clustering, embeddings, store_clustering, schema, options, projection, umap
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-clustering/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

Every chunk gets one embedding by default. That is enough for search and similarity — the two things the engine does out of the box. But some workloads need a second, separate embedding vector per chunk, computed with a different role, to support projection visualizations, near-duplicate grouping, and entity resolution with clustering distances. This page covers how to turn that on and what happens when you do.

## The option

Clustering is a schema-level flag. You set it in the `@context` document — the first object in your schema — under `@metadata.terminusdb.options`. The value is an **array of strings**, not a boolean or a single string:

```json
[
  {
    "@type": "@context",
    "@base": "terminusdb:///data/",
    "@schema": "terminusdb:///schema#",
    "@metadata": {
      "terminusdb": {
        "options": ["store_clustering"]
      }
    }
  },
  {
    "@id": "Article",
    "@type": "Class",
    "@key": { "@type": "Lexical", "@fields": ["title"] },
    "title": "xsd:string",
    "body": "xsd:string",
    "@metadata": {
      "embedding": {
        "query": "query($id: ID){ Article(id: $id) { title body } }",
        "template": "{{title}}. {{body}}"
      }
    }
  }
]
```

A few things to keep in mind:

- The array can hold other option strings. Currently, `"store_clustering"` and `"store_indices"` are supported. `"store_indices"` enables automatic indexing on every commit, while `"store_clustering"` enables the secondary clustering embeddings described on this page.
- If `@metadata`, `terminusdb`, or `options` is missing — or the string just is not there — clustering is off. The default is always `false`.
- TerminusDB reads this at commit time. On schema-change commits, it checks the flag directly from the in-memory schema objects. On document-only commits, it skips the check entirely and relies on the Rust-side cache to decide whether indexing is needed. No restart, no environment variable.

## What changes at index time

With clustering on, each chunk is embedded **twice**:

- Once with `EmbeddingRole::Document` — the primary vector, ANN-indexed, used by `/search` and `/similar`.
- Once with `EmbeddingRole::Clustering` — stored in a separate `clustering_embedding` column, never ANN-indexed, retrieved on demand by `/embeddings` and `/candidates`.

That is double the embedding calls. For a 2,000-document corpus with one chunk each, you go from 2,000 model invocations to 4,000. The trade-off is bandwidth and latency at index time for richer downstream queries.

With clustering off, the second column is filled with zero vectors of the same dimensionality. No second call is made. The column always exists — it is just empty.

## What changes at query time

The `/embeddings` endpoint returns both sets when clustering is enabled:

```bash
curl -u admin:root 'http://localhost:7373/api/plugin/search-embeddings/admin/my_db/local/branch/main'
```

```json
{
  "doc_embeddings": {
    "terminusdb:///data/Article/1": [0.12, -0.04, 0.33, ...]
  },
  "clustering_embeddings": {
    "terminusdb:///data/Article/1": [0.08, 0.21, -0.15, ...]
  },
  "store_clustering": true,
  "served_commit": "abc123"
}
```

The `store_clustering` field in the response tells you whether clustering data is present. When `false`, `clustering_embeddings` is an empty object.

The `/candidates` endpoint uses the clustering column for a dual KNN gather. This produces a `clustering_distance` alongside the standard document distance — two signals that, together, separate true duplicates from semantic neighbors that merely happen to be close.

## Checking the current state

The index status endpoint reports whether clustering is active for a domain:

```bash
curl -u admin:root 'http://localhost:7373/api/index/admin/my_db/local/branch/main'
```

The `engine.store_clustering` field is `true`, `false`, or `null`. The `null` case means the engine has no indexed data for that domain yet — it has never heard of it.

## Turning it on after the first index

Clustering is set on first push. If you indexed without it and later want to enable it, a full reindex is required. There is no in-place upgrade path — the clustering column would be all zeros for every existing chunk, and the only way to populate it is to re-embed everything.

The steps are:

1. Update the schema's `@metadata.terminusdb.options` to include `"store_clustering"`.
2. Delete the domain's search footprint from the engine.
3. Re-push the full index.

Going the other direction — turning clustering off after it was on — also requires a reindex. The zero-vector fallback only applies to data indexed from scratch with clustering disabled.

---

Next: [History & Branching](/docs/versioned-search-history/).

---
tags:
  - reference
  - vectorlink
title: Versioned Search API Reference
nextjs:
  metadata:
    title: Versioned Search API Reference
    description: Complete HTTP endpoint reference for the VectorLink 2.0 search engine
    keywords: terminusdb, versioned search, api reference, http, endpoints, vectorlink, openapi
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-api-reference/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

The full HTTP endpoint reference for the Versioned Search engine. Every request carries the admin secret as HTTP Basic auth (`-u admin:root` by default). Health probes are the only unauthenticated endpoints.

## Authentication

Every functional endpoint requires a shared admin secret over HTTP Basic. Missing or wrong secret returns `401` on every endpoint.

This is authentication only — no per-user authorisation (RBAC). The engine is a trusted component: run it on a private network with TerminusDB as the front door.

## Health

### `GET /health/live`

Process liveness. Answers immediately, no auth required.

```bash
curl -fsS http://localhost:7372/health/live
# {"status":"ok"}
```

### `GET /health/ready`

Per-capability readiness. No auth required.

```bash
curl -fsS http://localhost:7372/health/ready | jq
# { "ready": true, "index": true, "search": true }
```

- `index` — store reachable, can accept `/push`.
- `search` — store and embedding backend warm, can serve `/search`.

While the embedding backend is warming, `/search` returns `503` with `Retry-After`.

## Indexing

### `GET /last-indexed`

Returns the last-indexed commit for a `(domain, branch)`.

| Param | Required | Description |
|-------|----------|-------------|
| `domain` | Yes | Data product (graphspec). |
| `branch` | Yes | Branch name. |

```bash
curl -u admin:root 'http://localhost:7372/last-indexed?domain=admin/star_wars&branch=main'
# { "branch": "main", "commit": "c1", "version": 7 }
```

### `POST /push`

Push an NDJSON delta for a commit. Returns a task id (indexing runs asynchronously).

| Param | Required | Description |
|-------|----------|-------------|
| `domain` | Yes | Data product (graphspec). |
| `branch` | Yes | Branch name. |
| `target_commit` | Yes | The commit to index. |
| `parent_commit` | No | The commit diffed from. Omit for first index of a lineage. |

Body: `application/x-ndjson` — one operation per line.

```bash
curl -u admin:root -X POST \
  'http://localhost:7372/push?domain=admin/star_wars&branch=main&target_commit=c1' \
  -H 'Content-Type: application/x-ndjson' \
  --data-binary @delta.ndjson
# task-7f3a9c
```

A commit can only be indexed once: pushing a `target_commit` that is already in-flight or already indexed is rejected with `409` — the first push wins. A failed push releases the reservation, so retrying a failed commit is fine.

### `GET /check`

Poll an indexing task.

| Param | Required | Description |
|-------|----------|-------------|
| `task_id` | Yes | Task id from `/push` response. |

```bash
curl -u admin:root 'http://localhost:7372/check?task_id=task-7f3a9c'
# { "status": "Complete", "indexed_documents": 2, "skipped": [] }
```

### `POST /assign`

Point a commit at an existing snapshot (no recompute).

| Param | Required | Description |
|-------|----------|-------------|
| `domain` | Yes | Data product (graphspec). |
| `source_commit` | Yes | The commit whose snapshot to reuse. |
| `target_commit` | Yes | The commit to bind. |

```bash
curl -u admin:root -X POST \
  'http://localhost:7372/assign?domain=admin/star_wars&source_commit=c2&target_commit=c3'
# 204
```

### `DELETE /domain`

Purge a data product's entire search footprint. Idempotent.

| Param | Required | Description |
|-------|----------|-------------|
| `domain` | Yes | Data product to delete. |

```bash
curl -u admin:root -X DELETE 'http://localhost:7372/domain?domain=admin/star_wars'
# 204
```

## Search

### `GET /search`

Read-only, cacheable search. Query text is the `q` parameter.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `domain` | Yes | — | Data product (graphspec). |
| `commit` | Yes | — | Snapshot to search. |
| `q` | Yes | — | Query text. |
| `mode` | No | `hybrid` | `vector` \| `fts` \| `hybrid` |
| `start` | No | `0` | Zero-based offset. |
| `count` | No | `50` | Page size. |
| `doc_type` | No | — | Restrict to types (repeat param). |
| `doc_id` | No | — | Restrict to IRIs (repeat param). |
| `ancestor` | No | — | Nearest-first ancestor window (repeat param). |
| `snippet` | No | `false` | Include matched chunk text. |

```bash
curl -u admin:root 'http://localhost:7372/search?domain=admin/star_wars&commit=c1&q=wise+old+man'
```

Response: `200` with a JSON array of hits, nearest first. Staleness reported via `TerminusDB-Data-Version` header.

### `POST /search`

Structured search via JSON body. Body fields override query params.

```bash
curl -u admin:root -X POST 'http://localhost:7372/search' \
  -H 'Content-Type: application/json' \
  -d '{"domain":"admin/star_wars","commit":"c1","q":"wise old man","mode":"hybrid","count":5}'
```

### `GET /similar`

Find documents similar to a known document.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `domain` | Yes | — | Data product. |
| `commit` | Yes | — | Snapshot to search. |
| `id` | Yes | — | Source document IRI. |
| `start` | No | `0` | Offset. |
| `count` | No | `10` | Page size. |
| `doc_type` | No | — | Filter result types (repeat param). |
| `snippet` | No | `false` | Include chunk text. |

```bash
curl -u admin:root 'http://localhost:7372/similar?domain=admin/star_wars&commit=c1&id=terminusdb:///star-wars/People/20'
```

### `POST /similar`

Find documents similar to a text string (engine embeds the text).

```bash
curl -u admin:root -X POST 'http://localhost:7372/similar' \
  -H 'Content-Type: application/json' \
  -d '{"domain":"admin/star_wars","commit":"c1","text":"bounty hunter"}'
```

### `GET /duplicates`

Near-duplicate groups within a population or across two record sets.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `domain` | Yes | — | Data product. |
| `commit` | Yes | — | Snapshot. |
| `threshold` | No | `0.0` | Max distance for a pair. |
| `start` | No | `0` | Offset. |
| `count` | No | `MAX` | Page size. |
| `doc_type` | No | — | Set-side type filter (repeat). |
| `doc_id` | No | — | Set-side IRI filter (repeat). |
| `target_doc_type` | No | — | Target-side type filter (repeat). |
| `target_doc_id` | No | — | Target-side IRI filter (repeat). |
| `snippet` | No | `false` | Include chunk text. |

```bash
curl -u admin:root 'http://localhost:7372/duplicates?domain=admin/er&commit=c1&threshold=0.1&doc_type=Abt&target_doc_type=Buy'
```

### `POST /candidates`

Raw KNN gather across two record sets, without tau filtering.

```bash
curl -u admin:root -X POST 'http://localhost:7372/candidates' \
  -H 'Content-Type: application/json' \
  -d '{
    "domain": "admin/er",
    "commit": "c1",
    "set_doc_types": ["Abt"],
    "target_doc_types": ["Buy"],
    "k": 5,
    "threshold_set": 0.3,
    "threshold_target": 0.3,
    "include": "embeddings,content"
  }'
```

### `GET /suggest`

Typeahead assist (FTS-only, no embedding). Optimised for sub-100ms responses.

| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| `domain` | Yes | — | Data product. |
| `commit` | Yes | — | Snapshot. |
| `q` | Yes | — | Partial query string. |
| `count` | No | `11` | Number of document IDs to return. |
| `doc_type` | No | — | Filter types (repeat). |
| `doc_id` | No | — | Filter IRIs (repeat). |

```bash
curl -u admin:root 'http://localhost:7372/suggest?domain=admin/star_wars&commit=c1&q=wise&count=10'
```

### `POST /compare`

Stateless text distance comparison. No index lookup — embeds both texts and returns the distance.

```bash
curl -u admin:root -X POST 'http://localhost:7372/compare' \
  -H 'Content-Type: application/json' \
  -d '{"source":"wise old Jedi master","target":"ancient Jedi teacher"}'
```

## Operations

### `GET /statistics`

Advisory counters for the engine.

```bash
curl -u admin:root 'http://localhost:7372/statistics' | jq
# {
#   "domains": 3,
#   "branches": 9,
#   "indexed_commits": 412,
#   "documents": 18044,
#   "chunks": 51234
# }
```

Counters are best-effort (may be approximate under concurrency). Don't gate logic on exact values.

## Error responses

| Situation | Response |
|-----------|----------|
| Missing/wrong admin secret | `401` |
| Missing/invalid parameter | `400`, naming the parameter |
| Branch has no indexed ancestor | `404` |
| Embedding backend cold/unreachable | `503` + `Retry-After` |
| Query embedding dimension ≠ index | `409` |
| Provider error / store read failure | `5xx` with the cause in the body |

An empty result array always means "genuinely nothing matched" — never a hidden failure.

## Configuration

All settings are environment variables; there are no command-line flags. Invalid or missing-required values fail at startup.

| Setting | Env | Default |
|---------|-----|---------|
| storage directory | `VECTORLINK_DATA_DIR` | `/data` |
| listen port | `VECTORLINK_PORT` | `7372` |
| embedding provider | `VECTORLINK_EMBED_PROVIDER` | `openai_compatible` |
| provider URL | `VECTORLINK_EMBED_URL` | `http://localhost:11434` |
| embedding model | `VECTORLINK_MODEL` | `multilingual-e5-base` |
| embedding dimension | `VECTORLINK_DIM` | `768` (fixed per index once created) |
| provider key | `VECTORLINK_EMBED_API_KEY` | — |
| admin user | `VECTORLINK_ADMIN_USER` | `admin` |
| admin secret | `VECTORLINK_ADMIN_SECRET` | _required — no default_ |

The embedding dimension is fixed when an index is first created and is immutable. A query whose embedding dimension doesn't match the index fails loudly (`409`) — it is never silently padded.

> The default `multilingual-e5-base` is fixed at 768 dimensions. If you switch to a Matryoshka-trained model such as `nomic-embed-text-v2-moe`, a smaller `VECTORLINK_DIM` (256/512/128/64) is valid at first index and saves storage and latency at scale. Whatever you choose, the dimension is set once before the first push — changing it later requires a full reindex.

## Local and cloud deployment

The engine runs the same binary locally and in the cloud for the **embedding** side: set `--provider openai`, `--embed-url https://api.openai.com`, and `--embed-key sk-...` (or `OPENAI_API_KEY`) to use a hosted model instead of the local Ollama sidecar. The **storage** side (`--directory`) is local-disk only today. LanceDB supports S3, GCS, and Azure Blob natively, but those backends are disabled in the current build to eliminate ~87 transitive dependencies and reduce binary size by ~40%. For cloud deployments today, mount a persistent volume and point `--directory` at it.

## Asymmetric model prefix handling

The default model — `multilingual-e5-base` — is an **asymmetric** embedding model. It expects a task prefix prepended to the input text, and the correct prefix depends on whether the text is being indexed (corpus side) or queried (retrieval side). Getting this wrong silently degrades retrieval quality — you still get results, they are just worse, and nothing tells you why.

The engine applies the correct prefix automatically based on the endpoint and operation context. You never write `query:` or `passage:` (or a nomic `search_query:`) yourself — the engine maps each endpoint to a role, then applies the model's prefix for that role.

| Endpoint | Role | e5 prefix | nomic prefix | Why |
|----------|------|-----------|--------------|-----|
| `POST /push` | Document | `passage: ` | `search_document: ` | Text is being indexed into the corpus. |
| `GET /search`, `POST /search` | Query | `query: ` | `search_query: ` | Text is a retrieval query against the corpus. |
| `POST /similar` (by text) | Document | `passage: ` | `search_document: ` | Text is treated as a document probe for document-to-document similarity. |
| `GET /similar` (by id) | — | none | none | No embedding needed — reuses the stored vector. |
| `GET /duplicates` | Clustering | `query: ` | `clustering: ` | Symmetric comparison for near-duplicate grouping. |
| `POST /candidates` | Clustering | `query: ` | `clustering: ` | Symmetric KNN gather for entity resolution. |
| `POST /compare` | Configurable | `query:` → `passage:` | `search_query:` → `search_document:` | Asymmetric by default. Pass `role=clustering`, `role=classification`, or `role=document` for symmetric comparison. |

Models without known prefixes (any model not in the table below) are embedded as-is — no prefix is applied.

| Model | Prefixes recognised |
|-------|---------------------|
| `intfloat/multilingual-e5-base`, `multilingual-e5-base` | `passage:` (document), `query:` (query, clustering, classification) |
| `nomic-ai/nomic-embed-text-v2-moe`, `nomic-embed-text-v2-moe` | `search_document:`, `search_query:`, `clustering:`, `classification:` |
| `nomic-embed-v2`, `nomic-ai/nomic-embed-text-v1.5` | same as above |

The full machine-readable contract is the OpenAPI document in the `vectorlink` repository (`openapi.yaml`).

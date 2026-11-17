---
tags:
  - how-to
  - vectorlink
  - self-hosted
title: Embedded VectorLink Search Engine
nextjs:
  metadata:
    title: Embedded VectorLink Search Engine
    description: Run the VectorLink search engine in-process inside TerminusDB — no separate service to deploy
    keywords: versioned search, version control, semantic search, self-hosted, vector search, embeddings,
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-embedded/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will understand the embedded VectorLink deployment mode, know how to enable and configure it, and be able to verify that the in-process engine is indexing and serving search.
{% /callout %}

TerminusDB 12.1 can run the VectorLink search engine **in-process** — compiled into the server itself as a builtin plugin. There is no separate vectorlink service to deploy, no socket, and no endpoint to configure.

## Deployment modes at a glance

`TERMINUSDB_INDEXER_BACKEND` selects how TerminusDB talks to a search engine. Exactly one backend is active at a time.

| Value | Engine | Use when |
|-------|--------|----------|
| `none` (default) | — | Indexing and search endpoints disabled |
| `vectorlink` | **Embedded** — VectorLink runs inside TerminusDB | Simplest deployment; single host; no extra service to manage |
| `http_vectorlink` | Remote VectorLink 2.0 service over HTTP(`TERMINUSDB_VECTORLINK_ENDPOINT`) | V2 engine on another host or scaled separately |
| `http_legacy_vectorlink` | Remote legacy VectorLink service | Existing v1 deployments |

The selector fails loud at startup on unrecognised values, and on endpoints configured for an inactive backend — a typo can never silently disable indexing.

## Enabling the embedded engine

Set the backend before starting TerminusDB:

```bash
export TERMINUSDB_INDEXER_BACKEND=vectorlink
./terminusdb serve
```

Two requirements:

- **Rust webserver backend** — embedded dispatch runs on the server's tokio runtime. `TERMINUSDB_SERVER_BACKEND=rust` is the default in 12.1; the classic SWI-Prolog HTTP server cannot host the engine.
- **An embedding provider** — the engine indexes full text search and embeds document text and queries through a configurable provider. It defaults to an OpenAI-compatible HTTP endpoint, so a local [Ollama](https://ollama.ai) serving `multilingual-e5-base` works out of the box and keeps the whole stack offline. Point it elsewhere with the provider variables below.

On startup you should see:

```text
[INFO] [vectorlink] embedded engine initialized | data_dir=... | model=multilingual-e5-base
```

If indexing is enabled (`store_indices` in the schema — see [TerminusDB Push Indexing](/docs/versioned-search-indexing/)), every commit pushes an NDJSON delta to the in-process engine exactly as it would to a remote one.

## Configuration reference

| Variable | Purpose | Default |
|----------|---------|---------|
| `TERMINUSDB_INDEXER_BACKEND` | Backend selector — set to `vectorlink` for embedded mode | `none` |
| `TERMINUSDB_VECTORLINK_DATA_DIR` | Where the engine keeps its Lance vector store | `<storage>/vectorlink` |
| `TERMINUSDB_SEARCH_ADMIN_USER` / `TERMINUSDB_SEARCH_ADMIN_SECRET` | Credentials TerminusDB uses to authenticate against the engine — HTTP modes only; unused in embedded mode | `admin` / `root` |
| `VECTORLINK_EMBED_PROVIDER` | `openai`, `openai_compatible`, or `generic_http` | `openai_compatible` |
| `VECTORLINK_EMBED_URL` | Embedding endpoint base URL — the engine POSTs to `<url>/v1/embeddings` | `http://localhost:11434` (local Ollama) |
| `VECTORLINK_MODEL` | Embedding model name | `multilingual-e5-base` |
| `VECTORLINK_DIM` | Embedding dimensionality | `768` |
| `VECTORLINK_EMBED_API_KEY` | Bearer token for providers that require one (e.g. OpenAI). Optional and never persisted | unset |
| `VECTORLINK_TOKENIZER_PATH` | Tokenizer file for chunking — must match the embedding model. Ships at `builtin_plugins/assets/tokenizer.json.bz2` and is found automatically | shipped asset for the default model |
| `VECTORLINK_EMBED_CACHE_SIZE` | Max entries in the disk-backed embedding cache; `none` disables it | `20000` |

Any endpoint that answers `POST /v1/embeddings` with an OpenAI-shaped response works. For a local Ollama sidecar use `VECTORLINK_EMBED_URL=http://<ollama-host>:11434` with `VECTORLINK_MODEL=multilingual-e5-base` and `VECTORLINK_DIM=768`. For OpenAI use `VECTORLINK_EMBED_PROVIDER=openai`, `VECTORLINK_EMBED_URL=https://api.openai.com`, `VECTORLINK_MODEL=text-embedding-3-small`, `VECTORLINK_DIM=1536`, and set `VECTORLINK_EMBED_API_KEY` to your key.

### Embedding cache

Every embedded text is cached on disk (zstd-compressed vectors in a sled store, keyed by the SHA-256 of model name + prefixed text). A text that was embedded before — including in an earlier commit or after a reindex — returns from cache without hitting the provider.

With a local model on CPU, embedding is the slowest part of ingestion by far. For a bulk load, set `VECTORLINK_EMBED_CACHE_SIZE` to cover the number of distinct texts you expect to embed — repeated templates, unchanged fields across reindex runs, and retried pushes then cost nothing. Each entry holds one compressed 768-dim vector, so a few million entries cost a few GB: size the cache to the corpus, and leave room for it on the same disk as `TERMINUSDB_VECTORLINK_DATA_DIR` (or `VECTORLINK_DATA_DIR` standalone). Set it to `none` to disable caching entirely.

### Embedding parameters are locked

The engine writes `vectorlink-params.json` at the root of its store on first initialization, recording the provider, URL, model, and dimension. Every stored vector is only valid for the parameter set that produced it, so on each subsequent start the engine verifies the configured parameters against the lock file. A mismatch aborts engine initialization with a message that prints both parameter sets — either restore the original values, or recreate the store so every document is re-embedded:

- **Embedded mode** — stop TerminusDB, delete `<storage>/vectorlink`, restart. The store is recreated with the new parameters and reindexing repopulates it.
- **Standalone vectorlink** — `vectorlink drop-store --yes` while the server is stopped deletes the entire store; the next start recreates it.

Over HTTP the engine sits behind a shared admin secret (`TERMINUSDB_SEARCH_ADMIN_*`) and is never exposed directly to end users. In embedded mode there are no credentials at all — the router is built without the auth gate, since every request has already been authorised by TerminusDB's `/api/*` layer before forwarding.

## How requests flow

Both deployment modes share the same Prolog route handlers, URL construction, and response handling. The only difference is the transport:

- **Remote mode** — TerminusDB calls the engine over HTTP at `TERMINUSDB_VECTORLINK_ENDPOINT`.
- **Embedded mode** — the same calls are dispatched in-process to the engine's router — no socket, no separate server. Request and response bodies stay streaming, so large NDJSON pushes and multi-chunk responses behave exactly as they do over HTTP.

Either way, the full indexing pipeline flows through the same endpoints — `/last-indexed`, `/push`, task polling, and `DELETE /domain` on database deletion.

## Builtin plugins vs user plugins

The embedded engine arrives through a structural change: TerminusDB now loads plugins from two directories.

- `builtin_plugins/` — shipped with the server, always loaded, versioned with the release. This is where the vectorlink fronting plugin (`vectorlink.pl`), `search_resolve.pl`, the webserver plugins, and `auto-optimize.pl` live. Path override: `TERMINUSDB_BUILTIN_PLUGINS_PATH`.
- `plugins/` — operator-installed extensions under the storage path. Path override: `TERMINUSDB_PLUGINS_PATH`.

Being builtin does not mean always active. `auto-optimize` runs by default — optimisation should happen in production — but test harnesses set `TERMINUSDB_ENABLE_OPTIMIZER=0` because it optimises probabilistically after commits and can perturb timing-sensitive tests. VectorLink follows a different pattern: the plugin is always present, but it only registers its search behaviour when the `vectorlink` backend is selected.

## Embedded vs remote: choosing

Choose **embedded** when you want the fewest moving parts — one binary, one storage tree, no service discovery, no port to secure. It is the right default for self-hosted single-node deployments, development, and evaluation.

Choose **remote** (`http_vectorlink`) when the engine needs its own host: separating embedding CPU load from the database, sharing one engine between several TerminusDB instances, or keeping an existing vectorlink service.

{% callout type="warning" %}
The embedded store and a remote engine's store are independent. Switching modes does not migrate indexes — trigger a reindex with `POST /api/index/{org}/{db}` after switching. They are compatible though!
{% /callout %}

## Verifying it works

With the backend enabled, the standard index status endpoint reflects engine state:

```bash
curl -u admin:root 'http://localhost:6363/api/index/admin/star_wars'
# {
#   "branch": "main",
#   "status": "completed",
#   "last_indexed_commit": "hzrm59olwuyy6n2wt1uuwt1wqekwg2z",
#   "engine": { "searchable_documents": 2, "text_segments_indexed": 2, ... },
#   "error": null
# }
```

The `engine` block is populated from the in-process engine's `/statistics` — `searchable_documents` counts what is actually indexed. Search behaves identically to remote mode:

```bash
curl -u admin:root 'http://localhost:6363/api/search/admin/star_wars?q=wise+old+man&snippet=true'
```

All fronting endpoints work unchanged: `/api/search`, `/api/suggest`, `/api/similar`, `/api/duplicates`, `/api/compare`, `/api/plugin/search-resolve`, and `/api/plugin/search-embeddings`.

## Troubleshooting

- **`vectorlink endpoint is not configured`** — the backend is still `none`. Export `TERMINUSDB_INDEXER_BACKEND=vectorlink` before starting.
- **Init failure mentioning tokio** — the classic webserver backend is in use. Set `TERMINUSDB_SERVER_BACKEND=rust`.
- **Search returns an engine error while index status is fine** — the embedding provider is unreachable. Check `VECTORLINK_EMBED_URL`/`VECTORLINK_MODEL` and that the provider (e.g. Ollama) is running.
- **Init fails with "embedding parameters mismatch"** — the `VECTORLINK_EMBED_*` values changed after the store was created. The error prints the locked and current parameters; restore the original values, or recreate the store as described under [Embedding parameters are locked](#embedding-parameters-are-locked).
- **Provider returns 401** — the endpoint requires a key. Set `VECTORLINK_EMBED_API_KEY`.
- **Data directory** — engine data lives under `<storage>/vectorlink`. Deleting it removes all indexed vectors; TerminusDB reindexes on the next push trigger.

---

Next: [API Reference](/docs/versioned-search-api-reference/).

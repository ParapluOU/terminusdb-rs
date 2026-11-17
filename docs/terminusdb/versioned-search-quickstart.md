---
tags:
  - how-to
  - vectorlink
title: Versioned Search Quickstart
nextjs:
  metadata:
    title: Versioned Search Quickstart
    description: Bring up the stack, index a commit, and run a search in five minutes
    keywords: terminusdb, versioned search, quickstart, vectorlink, semantic search, getting started
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-quickstart/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

{% callout type="note" %}
**Prerequisites**
- Docker and Docker Compose installed
- No external API keys required — the default embedding model runs locally on CPU
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have the search engine running, one commit indexed, and a semantic search returning results — all over HTTP with `curl`.
{% /callout %}

Bring up the stack, index one commit, and run a search. The whole thing takes about five minutes once the embedding model is pulled.

{% callout type="note" %}
**Running the engine embedded?** TerminusDB 12.1 can host VectorLink in-process — set `TERMINUSDB_INDEXER_BACKEND=vectorlink` and skip the `vectorlink` service entirely. You still need an embedding provider (the `embeddings` service in this quickstart works). See [Embedded VectorLink Engine](/docs/versioned-search-embedded/).
{% /callout %}

## 1. Start the stack

One command — no clone, no build. All three services use pre-built images:

```bash
curl -fsSL https://raw.githubusercontent.com/terminusdb-org/vectorlink/main/docker-compose.quickstart.yml \
  | docker compose -p vectorlink-quickstart -f - up -d
```

This starts three services on CPU, with no external network after the one-time model pull:

- `terminusdb` — a TerminusDB server on `:6365` (with the indexer plugin that auto-pushes to VectorLink)
- `vectorlink` — the search engine (HTTP API on `:7372`)
- `embeddings` — a local embedding model server (Ollama serving `multilingual-e5-base`)

Stop everything with `docker compose -p vectorlink-quickstart down`.

Wait until the engine is ready:

```bash
curl -fsS http://localhost:7372/health/ready | jq
# { "ready": true, "index": true, "search": true }
```

`index: true` means it can accept pushes. `search: true` means the embedding backend is warm. Poll until both are true — don't sleep a fixed amount.

```bash
until curl -fsS http://localhost:7372/health/ready | jq -e '.index and .search' >/dev/null; do
  echo "waiting for engine…"; sleep 2
done
echo "ready"
```

## 2. Index a commit

Find where the engine is up to (empty on first use):

```bash
curl -u admin:root 'http://localhost:7372/last-indexed?domain=admin/star_wars&branch=main'
# { "branch": "main", "commit": null, "version": 0 }
```

Push a small NDJSON delta as commit `c1` — one operation per line. Each `Inserted` carries the rendered text the engine will chunk and embed:

```bash publishes="task_id"
curl -u admin:root -X POST \
  'http://localhost:7372/push?domain=admin/star_wars&branch=main&target_commit=c1' \
  -H 'Content-Type: application/x-ndjson' \
  -d '{"op":"Inserted","id":"terminusdb:///star-wars/People/20","string":"This person is named Yoda. A wise old Jedi master, small and green."}
{"op":"Inserted","id":"terminusdb:///star-wars/Species/8","string":"The Mon Calamari are an amphibious species resembling squid, known as skilled starship engineers."}'
# task-7f3a9c
```

{% callout type="note" %}
The `string` field above is hand-written for this tutorial. In production, TerminusDB generates it automatically from your schema's embedding template — a GraphQL query selects fields, a Handlebars template renders them to text. For example, a `Person` class with `name` and `bio` fields might use `"template": "{{name}}. {{bio}}"`, producing the same kind of plain-text string you see above. See [TerminusDB Push Indexing](/docs/versioned-search-indexing/) for the full schema setup.
{% /callout %}

Poll the task until complete:

```bash slot="task_id" placeholder="task-7f3a9c"
curl -u admin:root 'http://localhost:7372/check?task_id=task-7f3a9c'
# { "status": "Complete", "indexed_documents": 2, "skipped": [] }
```

## 3. Search

GET — simple and cacheable:

```bash
curl -u admin:root 'http://localhost:7372/search?domain=admin/star_wars&commit=c1&q=wise+old+man&snippet=true'
# [
#   {
#     "id": "terminusdb:///star-wars/People/20",
#     "distance": 0.0823,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 14, "location": 0.0,
#       "snippet": "This person is named Yoda. A wise old Jedi master, small and green." }
#   }
# ]
```

POST — structured JSON:

```bash
curl -u admin:root -X POST 'http://localhost:7372/search' \
  -H 'Content-Type: application/json' \
  -d '{"domain":"admin/star_wars","commit":"c1","q":"who are the squid people","snippet":true}'
# [
#   {
#     "id": "terminusdb:///star-wars/Species/8",
#     "distance": 0.0941,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 16, "location": 0.0,
#       "snippet": "The Mon Calamari are an amphibious species resembling squid, known as skilled starship engineers." }
#   }
# ]
```

Expected (hybrid, the default): `People/20` (Yoda) tops "wise old man". `Species/8` (Mon Calamari) tops "squid people" — even though neither rendered text contains those exact words. The `snippet` field in each hit shows the chunk text that was actually matched, making it easy to see why the engine returned that document. That is the semantic payoff.

## What just happened

1. You pushed two documents as an NDJSON stream to the engine.
2. The engine chunked each document's text, embedded the chunks with the local model, and stored them as a versioned snapshot tagged `commit:c1`.
3. You searched that snapshot. The engine embedded your query, ran hybrid (vector + full-text) search, deduplicated chunk hits back to documents, and returned the nearest match first.

In production, TerminusDB performs steps 1–2 automatically after each commit. The `curl` calls above are how you drive the engine standalone for testing or debugging.

## Asymmetric model prefixes (automatic)

The default embedding model — `multilingual-e5-base` — is an **asymmetric** model. It expects different text prefixes depending on whether you are indexing a document or running a query:

- Documents being indexed get `passage: ` prepended automatically.
- Search queries get `query: ` prepended automatically.
- Duplicate detection and entity resolution use `clustering: ` automatically.

You never write `query:` or `passage:` yourself — the engine applies the correct prefix based on which endpoint you call. Getting this wrong is the most common silent quality killer with asymmetric models (e5, bge, and nomic all share this failure mode), so the engine handles it for you.

## Cleanup

Delete the indexed domain so you can rerun the quickstart from scratch:

```bash
curl -u admin:root -X DELETE 'http://localhost:7372/domain?domain=admin/star_wars'
```

---

Next: [TerminusDB Push Indexing](/docs/versioned-search-indexing/).

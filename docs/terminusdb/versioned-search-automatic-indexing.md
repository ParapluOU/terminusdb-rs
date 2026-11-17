---
tags:
  - how-to
  - vectorlink
title: How automatic indexing works
nextjs:
  metadata:
    title: How automatic indexing works in TerminusDB
    description: The three things TerminusDB 12.1 needs before it embeds and indexes your documents by itself, what happens automatically, what does not yet, and how to fix a search that returns nothing.
    keywords: terminusdb, automatic indexing, embeddings, vectorlink, store_indices, store_clustering, embedding metadata, reindex, TERMINUSDB_INDEXER_BACKEND
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-automatic-indexing/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

TerminusDB 12.1 can embed and index your documents by itself, so you can search them with [VectorLink](/docs/versioned-search/). But it only does this when three things are in place. If one of them is missing, nothing is indexed, and search returns no hits. TerminusDB does not tell you why.

This page explains the three things, what happens automatically after that, and what to expect from the first search in the release candidate.

## The three things you need

### 1. The server runs with the VectorLink backend

Nothing is indexed unless the server runs with this environment variable:

```bash
TERMINUSDB_INDEXER_BACKEND=vectorlink
```

The default is `none`. With `none`, commits work as normal, but no document is ever sent for indexing.

The `docker-compose.yml` in the [vectorsearch folder](https://github.com/terminusdb/terminusdb/tree/main/vectorsearch) of the TerminusDB repository sets this for you. If you start TerminusDB another way (for example with a plain `docker run` or the CLI), set the variable yourself.

### 2. The database turns indexing on

At database level, you turn indexing on in the schema `@context` document, under `@metadata.terminusdb.options`:

```json
{
  "@type": "@context",
  "@base": "terminusdb:///data/",
  "@schema": "terminusdb:///schema#",
  "@metadata": {
    "terminusdb": {
      "options": ["store_indices"]
    }
  }
}
```

- `store_indices` turns on automatic indexing on every commit.
- `store_clustering` is a sibling option. It adds the second, clustering embeddings. See [Clustering Embeddings](/docs/versioned-search-clustering/).

A commit that changes the schema reads these options and turns indexing on or off for the database.

With the embedded VectorLink backend, the engine also indexes classes that have embedding metadata (see the next step). We still suggest you set `store_indices`, so that your intent is clear in the schema.

### 3. Each class says what to embed

At class level, each class that you want to search needs an `@metadata.embedding` block with two parts:

- `query` — a GraphQL query that fetches the fields of one document.
- `template` — a Handlebars template that turns those fields into the text that is embedded.

A class without this block is never embedded. Here is the `Person` class from the [AI agent prompt](/docs/get-started/#set-it-up-with-an-ai-agent), together with a `@context` that turns on `store_indices`:

```json
[
  {
    "@type": "@context",
    "@base": "terminusdb:///data/",
    "@schema": "terminusdb:///schema#",
    "@metadata": { "terminusdb": { "options": ["store_indices"] } }
  },
  {
    "@type": "Class",
    "@id": "Person",
    "name": "xsd:string",
    "friends": { "@type": "Set", "@class": "Person" },
    "@metadata": {
      "embedding": {
        "query": "query($id: ID){ Person(id: $id) { name } }",
        "template": "{{name}}"
      }
    }
  }
]
```

Send it to the schema graph with `full_replace=true`, because it includes the `@context`:

```bash
curl -u admin:$PASS -X POST \
  'http://localhost:6363/api/document/admin/agent_demo?graph_type=schema&author=me&message=Add+schema&full_replace=true' \
  -H 'Content-Type: application/json' \
  -d @schema.json
```

**Who adds this block?** If you write your schema through the TerminusDB MCP server, it adds the embedding metadata for you (you can also pass an `indexing` option to control it). If you write the schema through the plain HTTP document API, or a client library, nothing is added for you. You must put `@metadata.embedding` in the schema yourself.

For help with good templates, see the [Embeddings Field Guide](/docs/versioned-search-embeddings/).

## What happens automatically

When all three things are in place:

- **Every new commit is queued for indexing.** The work runs in the background, so your commit returns at once. The index catches up a moment later.
- **The first search catches up the index.** If the index is behind the head commit, the search asks the indexer to catch up. This also indexes documents that you committed before the setup was complete. The first search may return no results, or only some. Wait a short time and search again.

You can see the state of the index at any time:

```bash
curl -u admin:$PASS http://localhost:6363/api/index/admin/agent_demo
```

Look at `searchable_documents`. It should match the number of documents you expect to find.

## What to know about the first search

There is one small gap in the release candidate:

- **The first search can be early.** Existing documents are indexed automatically on the first search. Until the catch-up is done, that search can return no results, or only some. Wait a short time and search again.

- **You can index ahead of time.** A manual reindex is optional. Use it if you do not want to wait for the first search, for example before a demo:

  ```bash
  curl -u admin:$PASS -X POST http://localhost:6363/api/index/admin/agent_demo
  ```

  The general form is `POST /api/index/{org}/{db}`, with an empty body.

- **Indexing catches up by itself.** The indexer works through commits one at a time. If a commit is not indexed yet, for example because the process stopped, indexing picks it up again at the next search (when auto-indexing is on) or the next time indexing is triggered. Nothing needs to be redone by hand.

A later release may start the catch-up when the indexer starts, so that the first search is ready at once.

## Troubleshooting: search returns nothing

Go through this list in order:

1. **Backend.** Does the server run with `TERMINUSDB_INDEXER_BACKEND=vectorlink`? With Docker Compose, check with `docker compose exec terminusdb env | grep INDEXER`.
2. **Database option.** Does the schema `@context` have `"options": ["store_indices"]` under `@metadata.terminusdb`? Get the schema with `GET /api/document/{org}/{db}?graph_type=schema` and look.
3. **Class metadata.** Does each class you search have an `@metadata.embedding` block with `query` and `template`?
4. **Reindex.** If search still returns nothing after a retry, run `POST /api/index/{org}/{db}`, then check `GET /api/index/{org}/{db}`. `searchable_documents` should not be `0`.
5. **Embeddings model.** On first start, the local model can take a few minutes to load. While it loads, search returns `503`. Wait and try again.

## See also

- [Set it up with an AI agent](/docs/get-started/#set-it-up-with-an-ai-agent) — a prompt that sets up the whole stack and runs a first search.
- [Versioned Search](/docs/versioned-search/) — overview of VectorLink.
- [TerminusDB Push Indexing](/docs/versioned-search-indexing/) — a worked example across three commits.
- [Embedded Engine](/docs/versioned-search-embedded/) — run VectorLink inside TerminusDB.
- [Searching](/docs/versioned-search-querying/) — query modes, filters and the response.

---
tags:
  - how-to
  - vectorlink
title: TerminusDB Push Indexing
nextjs:
  metadata:
    title: TerminusDB Push Indexing
    description: How TerminusDB automatically indexes documents into VectorLink using store_indices, with a worked example across three commits
    keywords: terminusdb, versioned search, indexing, push, store_indices, automatic indexing, vectorlink
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-indexing/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

TerminusDB stores your data and indexes it automatically. When a schema enables `store_indices`, every commit triggers an asynchronous push to VectorLink — no manual curl, no external orchestration. The engine receives an NDJSON delta of inserted, changed, and deleted documents, embeds them, and stores the result as a versioned snapshot tagged with the commit ID.

If search returns nothing, or you want to know what must be in place first, read [How automatic indexing works](/docs/versioned-search-automatic-indexing/).

This page walks through a complete example: create a database, define a schema with embedding metadata, insert documents across three commits, and verify that each commit is indexed and searchable.

{% callout type="note" %}
**Prerequisites**
- The quickstart stack running — see [Versioned Search Quickstart](/docs/versioned-search-quickstart/)
- TerminusDB on `:6365` (the stack maps the container's `:6363` to `:6365` on your host)
{% /callout %}

## 1. Create a database

Create a database with a schema graph:

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/db/admin/star_wars' \
  -H 'Content-Type: application/json' \
  -d '{"label": "Star Wars", "comment": "Characters and species", "schema": true}'
# {"@type":"api:DbCreateResponse","api:status":"api:success"}
```

## 2. Define a schema with `store_indices`

The schema has two parts: the `@context` document enables `store_indices` in `@metadata.terminusdb.options`, and each class declares an `embedding.query` that selects which fields to render as the searchable text string.

```bash
curl -u admin:root -X POST \
  'http://localhost:6365/api/document/admin/star_wars?graph_type=schema&author=admin&message=Add+schema&full_replace=true' \
  -H 'Content-Type: application/json' \
  -d '[
    {
      "@type": "@context",
      "@base": "terminusdb:///data/",
      "@schema": "terminusdb:///schema#",
      "@metadata": {
        "terminusdb": {
          "options": ["store_indices"]
        }
      }
    },
    {
      "@id": "Character",
      "@type": "Class",
      "@key": { "@type": "Lexical", "@fields": ["name"] },
      "name": "xsd:string",
      "bio": "xsd:string",
      "@metadata": {
        "embedding": {
          "query": "query($id: ID){ Character(id: $id) { name bio } }",
          "template": "{{name}}. {{bio}}"
        }
      }
    },
    {
      "@id": "Species",
      "@type": "Class",
      "@key": { "@type": "Lexical", "@fields": ["name"] },
      "name": "xsd:string",
      "description": "xsd:string",
      "@metadata": {
        "embedding": {
          "query": "query($id: ID){ Species(id: $id) { name description } }",
          "template": "{{name}}. {{description}}"
        }
      }
    }
  ]'
# ["terminusdb:///schema#Character","terminusdb:///schema#Species"]
```

The `template` is a Handlebars string that renders each document into the plain text the engine will chunk and embed. For a `Character` named Yoda with a bio field, the rendered string is `"Yoda. A wise old Jedi master, small and green."`.

## 3. Commit 1 — insert two characters

Insert Yoda and Luke Skywalker:

```bash
curl -u admin:root -X POST \
  'http://localhost:6365/api/document/admin/star_wars?author=admin&message=Add+Yoda+and+Luke&compress_ids=true' \
  -H 'Content-Type: application/json' \
  -d '[
    {"@type": "Character", "name": "Yoda", "bio": "A wise old Jedi master, small and green."},
    {"@type": "Character", "name": "Luke Skywalker", "bio": "A farm boy from Tatooine who became a Jedi knight."}
  ]'
# ["Character/Yoda","Character/Luke%20Skywalker"]
```

TerminusDB commits this, then the `post_commit_hook` fires asynchronously. It computes the delta (two inserts), renders each document through the Handlebars template, streams the NDJSON to VectorLink, and polls the task until complete.

Verify the engine received it:

```bash
curl -u admin:root 'http://localhost:6365/api/index/admin/star_wars'
# {
#   "branch": "main",
#   "status": "completed",
#   "last_indexed_commit": "hzrm59olwuyy6n2wt1uuwt1wqekwg2z",
#   "engine": { "searchable_documents": 2, "text_segments_indexed": 2, ... },
#   "branch_processing": { "commits_processed": 2, "total_commits": 2, ... },
#   "error": null
# }
```

The `last_indexed_commit` field shows the latest indexed commit ID and `status: "completed"` means the asynchronous hook has finished.

### What got embedded

Before embedding, TerminusDB renders each document into a plain text string using the GraphQL query and Handlebars template from the schema. For Yoda, the query `query($id: ID){ Character(id: $id) { name bio } }` extracts the `name` and `bio` fields, and the template `{{name}}. {{bio}}` produces:

```
Yoda. A wise old Jedi master, small and green.
```

That string is what the engine chunks, tokenises, and embeds — not the raw JSON document. The GraphQL query is powerful: it can traverse references, so if a `Character` had a `species` field pointing to a `Species` document, the query could follow that edge and include the species description in the rendered text. This means the embedding can capture context from related documents — a taxonomy term, a category label, or a parent record — without denormalising your data.

You can retrieve the exact rendered text that gets embedded by requesting the document with `format=embedding`:

```bash
curl -u admin:root 'http://localhost:6365/api/document/admin/star_wars?format=embedding&id=Character/Yoda'
# Yoda. A wise old Jedi master, small and green.
```

This returns the plain-text output of the Handlebars template — the same string the engine receives for chunking and embedding. You can also fetch all documents of a type by using `type=Character` instead of `id`, which streams one rendered text per line.

You can inspect the actual stored embeddings for any document using the `/api/plugin/search-embeddings` endpoint. Pass a `doc_id` to fetch a specific document's embedding vector:

```bash
curl -u admin:root 'http://localhost:6365/api/plugin/search-embeddings/admin/star_wars?doc_id=Character/Yoda'
```

The response contains the document ID and its embedding — a 768-dimensional float vector (the dimensionality depends on the embedding model). You can also fetch all embeddings for a given type with `doc_type=Character`, which streams one JSON object per line as NDJSON.

Search for "wise old man":

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=wise+old+man&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3717,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 14, "location": 0.0,
#       "snippet": "Yoda. A wise old Jedi master, small and green." }
#   },
#   {
#     "id": "Character/Luke%20Skywalker",
#     "distance": 0.4349,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 13, "location": 0.0,
#       "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#   }
# ]
```

Yoda should be the top result. The `snippet=true` parameter includes the matched chunk text so you can see exactly what was retrieved. TerminusDB resolves the branch from the URL path — no `domain` or `branch` query parameters are needed.

## 4. Commit 2 — add a species

Insert the Mon Calamari species:

```bash
curl -u admin:root -X POST \
  'http://localhost:6365/api/document/admin/star_wars?author=admin&message=Add+Mon+Calamari' \
  -H 'Content-Type: application/json' \
  -d '[
    {"@type": "Species", "name": "Mon Calamari", "description": "An amphibious species resembling squid, known as skilled starship engineers."}
  ]'
# ["terminusdb:///data/Species/Mon%20Calamari"]
```

Again, the hook fires. The delta contains one insert. The engine appends it on top of the previous commit's version — no re-embedding of Yoda or Luke.

Search for "squid people":

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=squid+people&snippet=true'
# [
#   {
#     "id": "Species/Mon%20Calamari",
#     "distance": 0.4717,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 16, "location": 0.0,
#       "snippet": "Mon Calamari. An amphibious species resembling squid, known as skilled starship engineers." }
#   }
# ]
```

Mon Calamari should top the results — even though the word "squid" appears in the description, not the name. The `snippet` field shows the rendered text that was actually embedded, making it easy to see why it matched. That is the semantic payoff.

## 5. Commit 3 — update a document

Change Yoda's bio to add more detail:

```bash
curl -u admin:root -X PUT \
  'http://localhost:6365/api/document/admin/star_wars?author=admin&message=Update+Yoda+bio&create=true' \
  -H 'Content-Type: application/json' \
  -d '{"@type": "Character", "name": "Yoda", "bio": "A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah."}'
# ["terminusdb:///data/Character/Yoda"]
```

The hook fires. The delta contains one `Changed` operation. The engine deletes Yoda's old chunks and re-embeds the new text — no stale vectors remain.

Search for "Jedi teacher on Dagobah":

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=Jedi+teacher+on+Dagobah&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3582,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 20, "location": 0.0,
#       "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   }
# ]
```

Yoda's updated bio now matches this query — the previous version did not contain "Dagobah" or "teacher". The `snippet` field shows the new text, confirming the index reflects the latest commit, not a stale snapshot.

### Search at the first commit

Because the engine stores a versioned snapshot per commit, you can search at any point in history. First, list the commits to get the ID of commit 1. Commit IDs are generated fresh when you create the database, so yours will differ — copy the actual `identifier` value for the "Add Yoda and Luke" commit:

```bash
curl -u admin:root 'http://localhost:6365/api/log/admin/star_wars?count=10'
# [
#   { "@id": "ValidCommit/o4x1v8xu2o0r8sl7dke8mkbdivmu39l", "identifier": "o4x1v8xu2o0r8sl7dke8mkbdivmu39l", "message": "Update Yoda bio", ... },
#   { "@id": "ValidCommit/6qd6132h27ijljrfags2r1mpvbjbmxx", "identifier": "6qd6132h27ijljrfags2r1mpvbjbmxx", "message": "Add Mon Calamari", ... },
#   { "@id": "ValidCommit/xy918u5vxlmz3ocqrs859ocheaaiuqj", "identifier": "xy918u5vxlmz3ocqrs859ocheaaiuqj", "message": "Add Yoda and Luke", ... },
#   { "@id": "ValidCommit/2p0ufdnea9u4vebc4gmsmxgv2187z5y", "identifier": "2p0ufdnea9u4vebc4gmsmxgv2187z5y", "message": "Add schema", ... }
# ]
```

Save it in a shell variable, then search at that commit using the commit path `admin/star_wars/local/commit/<id>`:

```bash
COMMIT1=xy918u5vxlmz3ocqrs859ocheaaiuqj  # replace with your actual ID
curl -u admin:root "http://localhost:6365/api/search/admin/star_wars/local/commit/${COMMIT1}?q=Jedi+teacher+on+Dagobah&snippet=true"
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3186,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 14, "location": 0.0,
#       "snippet": "Yoda. A wise old Jedi master, small and green." }
#   },
#   {
#     "id": "Character/Luke%20Skywalker",
#     "distance": 0.3486,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 19, "location": 0.0,
#       "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#   }
# ]
```

Compare the two results. At commit 1, Yoda's snippet is the original short bio and the distance is 0.3186 — the embedding cannot match "Dagobah" or "teacher" because those words are absent. At HEAD, the updated bio includes both concepts and the distance drops to 0.2185. The same query against two versions of the same document produces different rankings and snippets, proving the index is truly versioned.

## How automatic indexing works

When `store_indices` is enabled, a `post_commit_hook` fires after every commit. The hook:

1. Checks whether the schema has `store_indices` (a fast in-memory option lookup).
2. Computes the document delta between the previous indexed commit and the new head.
3. Renders each changed document through its Handlebars `template` into a plain text string.
4. Streams the NDJSON to VectorLink's `/push` endpoint.
5. Polls `/check` until the task completes.

The hook is optimised for the common case — document-only commits skip all schema checks and notify the indexer directly. Schema-change commits re-read the `store_indices` flag and clear the Rust-side cache if needed.

Without `store_indices`, indexing is skipped entirely — the hook is a cheap no-op (two config checks and a fail).

{% callout type="warning" %}
**A commit can only be indexed once.** The first `/push` for a commit wins: VectorLink reserves it while indexing and marks it indexed when done. A second push for the same commit is rejected with `409` (`commit already pushed (in-flight or indexed)`). With `store_indices` on, the auto-indexer claims every commit as it lands — including schema-only commits, which push with zero documents — so a manual `/push` for a commit the indexer already claimed always fails. Push directly only for databases where `store_indices` is off, and only for commits the indexer has never seen.
{% endcallout %}

## The NDJSON protocol

When TerminusDB pushes to VectorLink, each line in the NDJSON body is one tagged operation:

```json
{"op":"Inserted","id":"<iri>","string":"<rendered text to embed>"}
{"op":"Changed","id":"<iri>","string":"<new text>"}
{"op":"Deleted","id":"<iri>"}
{"op":"Error","message":"<upstream could not render this doc>"}
```

| op | Meaning |
|----|---------|
| `Inserted` | New document — embed and store its chunks. |
| `Changed` | Modified document — **replaces its whole chunk set** (no stale chunks remain). |
| `Deleted` | Removed document — **all its chunks are deleted**. |
| `Error` | The upstream couldn't render one document — that document is **skipped and recorded**, never silently dropped. Indexing continues. |

You normally never see this protocol — TerminusDB handles it internally. It is documented here for debugging and for driving the engine standalone (see the [Quickstart](/docs/versioned-search-quickstart/) for a manual push example).

{% callout type="note" %}
**Asymmetric model prefixes are applied automatically.** The default model (`nomic-embed-text-v2-moe`) expects a `search_document: ` prefix on text being indexed. The engine applies this prefix for you during `/push` — you do not need to prepend it to the `string` field. The same automation applies on the query side (`search_query: ` for `/search`, `clustering: ` for `/duplicates`). See the [API Reference](/docs/versioned-search-api-reference/#asymmetric-model-prefix-handling) for the full prefix mapping.
{% /callout %}

## Triggering a reindex

If the index falls behind (for example, after an engine restart or a schema change that adds new embedding fields), trigger a full reindex from TerminusDB:

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/index/admin/star_wars'
# {"@type":"api:IndexResponse","api:status":"api:success"}
```

TerminusDB recomputes the delta from the last indexed commit to HEAD and pushes it to the engine. Check progress with the same `GET /api/index` endpoint used above.

## Deleting a data product

Deleting the database from TerminusDB automatically purges its search footprint from VectorLink — no separate cleanup is needed:

```bash
curl -u admin:root -X DELETE 'http://localhost:6365/api/db/admin/star_wars'
# {"@type":"api:DbDeleteResponse","api:status":"api:success"}
```

The indexer hook detects the deletion and issues a `DELETE /domain` call to VectorLink on your behalf. The purge is idempotent — a repeat returns success, not `404`.

## How indexing maps to storage

TerminusDB history is linear per branch. The engine mirrors this:

| TerminusDB | Versioned Search (LanceDB) |
|------------|---------------------------|
| domain `org/db` | a Lance dataset |
| commit | a dataset version, bound by a Lance **tag** (`commit:<id>`) |
| index commit `C` from parent `P` | append only the changed documents on top of `P`'s version |
| branch-out at `P` | a Lance branch forked from `P`'s version, **sharing its data blocks** |
| reassign commit pointer | a tag pointing at an existing version (no recompute) |

Vector blocks from the parent commit are reused, never duplicated. A global commit-to-layer index (keyed by commit id, per domain, backed by Lance tags) resolves any commit's layer, so a branch forked at commit `P` finds `P`'s vectors regardless of which branch first indexed it.

---

Next: [Searching](/docs/versioned-search-querying/).

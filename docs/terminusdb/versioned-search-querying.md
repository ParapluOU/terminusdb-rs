---
tags:
  - how-to
  - vectorlink
  - query
title: Searching with Versioned Search
nextjs:
  metadata:
    title: Searching with Versioned Search
    description: How to search — GET vs POST, three modes, filters, pagination, snippets, similar documents, and duplicates
    keywords: terminusdb, versioned search, searching, hybrid, vector, fts, similar, duplicates, suggest, vectorlink
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-querying/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

The examples on this page use the `admin/star_wars` database created in [TerminusDB Push Indexing](/docs/versioned-search-indexing/). If you have not already run those steps, start there first — this page picks up where the indexing walkthrough left off.

All requests go through TerminusDB on port `:6365`. TerminusDB authorises each call and forwards it to the search engine internally — you never talk to the engine directly.

## GET vs POST

- **`GET /api/search/<path>`** — read-only and cacheable. The query text is `q`. All parameters are query parameters. Best for simple, link-safe searches.
- **`POST /api/search/<path>`** — a structured JSON body. Better for long queries or programmatic composition.

Every parameter may be given as a query parameter **or** in the JSON body. **The JSON body wins**: if a field is present in the body, the same-named query parameter is ignored (no merge). This lets you set defaults in the URL and override them in the body.

```bash
# GET
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=wise+old+man&mode=hybrid&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3717,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 26, "location": 0.0,
#       "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   },
#   {
#     "id": "Character/Luke%20Skywalker",
#     "distance": 0.4349,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 19, "location": 0.0,
#       "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#   }
# ]

# POST
curl -u admin:root -X POST 'http://localhost:6365/api/search/admin/star_wars' \
  -H 'Content-Type: application/json' \
  -d '{"q":"wise old man","mode":"hybrid","count":5,"snippet":true}'
```

The path `admin/star_wars` identifies the data product. TerminusDB resolves the branch from the path — no `domain` or `branch` query parameters are needed. To search a specific commit, use the commit path `admin/star_wars/local/commit/<commit_id>` (see [Search at a specific commit](#search-at-a-specific-commit) below).

`q` is required (in the body or the query). Missing it gives `400`.

## Parameters

| Param | Default | Meaning |
|-------|---------|---------|
| `q` | — | The query text. |
| `mode` | `hybrid` | `vector` \| `fts` \| `hybrid`. |
| `start` | `0` | Zero-based offset of the first result (pagination). |
| `count` | `50` | Page size. |
| `doc_type` | — | Restrict to these document types. |
| `doc_id` | — | Restrict to these document IRIs. |
| `snippet` | `false` | Include the matched chunk's text in each hit. |

## Filters

`doc_type` and `doc_id` restrict the result set. They **AND** together (a hit must match both sets) and **OR** within a set.

- As query parameters, repeat them: `?doc_type=People&doc_type=Species`.
- In JSON, use arrays: `"doc_type": ["People", "Species"]`.

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/search/admin/star_wars' \
  -H 'Content-Type: application/json' \
  -d '{"q":"engineer","doc_type":["Species"],"snippet":true}'
# [
#   {
#     "id": "Species/Mon%20Calamari",
#     "distance": 0.4717,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 25, "location": 0.0,
#       "snippet": "Mon Calamari. An amphibious species resembling squid, known as skilled starship engineers." }
#   }
# ]
```

## Pagination

`start` + `count` page the ranked results. To walk a result set: `start=0&count=50`, then `start=50&count=50`, and so on.

## Modes in practice

- **hybrid** (default): combines meaning and keywords. The best general default.
- **vector**: pure semantic similarity. Finds related meaning even with no shared words.
- **fts**: exact keywords, identifiers, rare tokens that embeddings blur.

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=Mon+Calamari&mode=fts&snippet=true'
# [
#   {
#     "id": "Species/Mon%20Calamari",
#     "distance": 0.0,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 25, "location": 0.0,
#       "snippet": "Mon Calamari. An amphibious species resembling squid, known as skilled starship engineers." }
#   }
# ]
```

## Reading the response

```json
[
  { "id": "Character/Yoda", "distance": 0.3717,
    "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 26, "location": 0.0 } },
  { "id": "Character/Luke%20Skywalker", "distance": 0.4349,
    "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 19, "location": 0.0 } }
]
```

- Nearest first. **Distance** in `[0,1]` — `0` is identical, `0.5` is unrelated, `1` is opposite. The distance is that of the document's **best-matching chunk**.
- One hit per document. Chunk fragments are never separate rows.
- **`chunk`** tells you where in the document the match was:
  - `index` — which chunk matched (0-based).
  - `count` — how many chunks the document was split into (`1` if it fit in one).
  - `token_start` / `doc_token_len` — exact token offsets: the chunk's start and the document's total length, in the embedding model's tokens.
  - `location` — convenience fraction `token_start / doc_token_len`, `0.0` (beginning) to `1.0` (end). Multiply by 100 for a percentage — `0.27` is roughly 27% of the way through.
- An **empty array means genuinely no match** — never an error in disguise. Errors are HTTP status codes.

### Getting the matched text

Add `snippet=true` to include the matched chunk's text as `chunk.snippet` (omitted by default to keep responses small):

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=wise+old+man&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3717,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 26, "location": 0.0,
#       "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   }
# ]
```

## Similar documents

Given a known document, find its nearest neighbours in the same snapshot:

```bash
curl -u admin:root 'http://localhost:6365/api/similar/admin/star_wars?id=Character/Yoda&snippet=true'
# [
#   {
#     "id": "Character/Luke%20Skywalker",
#     "distance": 0.2195,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 19, "location": 0.0,
#       "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#   },
#   {
#     "id": "Species/Mon%20Calamari",
#     "distance": 0.3143,
#     "chunk": { "index": 0, "count": 1, "token_start": 0, "doc_token_len": 25, "location": 0.0,
#       "snippet": "Mon Calamari. An amphibious species resembling squid, known as skilled starship engineers." }
#   }
# ]
```

You can also find similar documents by text (the engine embeds the text and runs vector search):

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/similar/admin/star_wars' \
  -H 'Content-Type: application/json' \
  -d '{"text":"bounty hunter in Mandalorian armour"}'
```

Both `/api/similar` variants route through the same catch-up resolution as `/api/search`: an un-indexed commit resolves to the nearest proven ancestor, and the served commit is returned so you can detect staleness.

## Duplicate detection

Surface near-duplicate groups within a population:

```bash
curl -u admin:root 'http://localhost:6365/api/duplicates/admin/star_wars?threshold=0.5&snippet=true'
# [
#   {
#     "distance": 0.2195,
#     "group": [
#       { "id": "Character/Yoda", "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." },
#       { "id": "Character/Luke%20Skywalker", "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#     ]
#   }
# ]
```

Or across two record sets for entity resolution:

```bash
curl -u admin:root 'http://localhost:6365/api/duplicates/admin/er?threshold=0.1&doc_type=Abt&target_doc_type=Buy&snippet=true'
```

Each result is a `{ "group": [ {"id"[, "snippet"]}, … ], "distance": <0..1> }`, sorted nearest-first. `/api/duplicates` is always bounded — it never runs an unbounded all-pairs scan. See [Entity Resolution](/docs/versioned-search-entity-resolution/) for the full workflow.

## Typeahead suggestions

Fast full-text-only autocomplete for UI search boxes. No embedding call — uses the existing inverted index directly, optimised for sub-100ms responses:

```bash
curl -u admin:root 'http://localhost:6365/api/suggest/admin/star_wars?q=wise&count=10'
# {
#   "approximate_match_count": 1,
#   "completions": ["wise old Jedi master", "wise old Jedi", "wise old"],
#   "hits": [
#     { "id": "Character/Yoda", "match_start": 8, "match_end": 12,
#       "next_words": ["old", "Jedi", "master", "small", "and"],
#       "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   ]
# }
```

Returns an approximate match count, completion suggestions extracted from indexed content, and the first N document IDs with match offsets and next-word predictions.

## Search at a specific commit

Because the engine stores a versioned snapshot per commit, you can search at any point in history. Use the commit path `admin/star_wars/local/commit/<commit_id>` — the same format demonstrated in [TerminusDB Push Indexing](/docs/versioned-search-indexing/#search-at-the-first-commit).

Get the commit ID from the log and save it in a shell variable:

```bash
curl -u admin:root 'http://localhost:6365/api/log/admin/star_wars?count=10'
# Copy the "identifier" for the commit you want to search at

COMMIT1=xy918u5vxlmz3ocqrs859ocheaaiuqj  # replace with your actual ID
curl -u admin:root "http://localhost:6365/api/search/admin/star_wars/local/commit/${COMMIT1}?q=Jedi+teacher+on+Dagobah&snippet=true"
```

## Staleness header

Every search response includes a **`TerminusDB-Data-Version`** header that tells you which commit was actually served. When you search at a specific commit that has not yet been indexed, TerminusDB falls back to the nearest indexed ancestor and reports that in the header:

```bash
curl -u admin:root -D - 'http://localhost:6365/api/search/admin/star_wars/local/commit/<unindexed_commit>?q=wise+old+man'
# HTTP/1.1 200 OK
# TerminusDB-Data-Version: commit:<ancestor_commit>
# [ ... results from the ancestor ... ]
```

If the served commit differs from what you asked for, the result is stale. TerminusDB automatically nudges the indexer to catch up — the next search at the same commit should serve the correct version.

You can also send the data-version you expect in the same header on the request. The engine treats it as advisory and always reports what it served.

---

Next: [Clustering Embeddings](/docs/versioned-search-clustering/).

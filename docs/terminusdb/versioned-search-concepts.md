---
tags:
  - explanation
  - vectorlink
title: Versioned Search Concepts
nextjs:
  metadata:
    title: Versioned Search Concepts
    description: Core vocabulary for versioned search — domains, commits, branches, chunks, search modes, and distance
    keywords: terminusdb, versioned search, concepts, domain, commit, branch, chunk, embedding, distance
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-concepts/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

A small vocabulary covers the whole system. This page defines every term you'll encounter in the rest of the Versioned Search documentation.

## Domain — what you address

A **domain** is a data product, addressed as a graphspec. The repository segment is always part of the address and defaults to `local`:

| You write | It means |
|-----------|----------|
| `org/db` | `org/db/local/branch/main` |
| `org/db/<repo>` | `org/db/<repo>/branch/main` |
| `org/db/<repo>/branch/<branch>` | that branch |
| `org/db/<repo>/commit/<commit>` | that commit's snapshot, directly |

A branch names a moving line of history. A **commit** names one fixed point on it. The engine never guesses "latest" — the snapshot you search is always an explicit commit.

## Commit — a searchable snapshot

Each indexed commit is an independent, immutable snapshot. Searching `commit=C1` always returns the same results regardless of later indexing. This reproducibility is the backbone of the system.

Indexing commit `C1` from its parent `C0` only processes what changed. Everything unchanged is reused from `C0`'s snapshot — no re-embedding, no wasted compute.

## Branch — a line of history

History is linear per branch. Branching out at a commit forks a new line that **shares the parent's stored vectors** — no recompute, no copy. The branch only adds what changes on it. The engine tracks branches; TerminusDB owns branch heads and any merge logic.

A branch can fork from any commit, regardless of which branch originally indexed it. The engine resolves a commit's snapshot globally per domain, so `parent_commit=C1` works whether `C1` was indexed on `main` or elsewhere.

## Document and chunk

You index **documents**, each identified by a full IRI (e.g. `terminusdb:///star-wars/People/20`). A long document is split into **chunks** that fit the embedding model's token window, with overlap so nothing is lost at boundaries. Each chunk is embedded separately.

Search runs over chunks but **deduplicates back to documents**: you always get documents back, never chunk fragments. Each result tells you which chunk matched and roughly where in the document it sits (`chunk.index`, `chunk.count`, `chunk.location`), so you can jump to the passage. Add `snippet=true` to include the matched chunk's text in the response.

## Search modes

One search endpoint, three modes:

| Mode | What it does | Use when |
|------|--------------|----------|
| **hybrid** (default) | Vector + full-text, fused via reciprocal-rank fusion | Best general relevance |
| **vector** | Semantic nearest-neighbour only | Pure "find similar meaning" |
| **fts** | Keyword / full-text | Exact terms, identifiers, rare tokens |

In hybrid mode, the same query text is embedded for the vector side and used as full-text terms. Results are deduplicated to documents — you get the best chunk per document, ranked by fused distance.

## Distance

Results are ranked by **distance** in `[0, 1]`:

- `0` — identical content.
- `0.5` — unrelated (orthogonal).
- `1` — opposite.

Smaller is closer. The distance reported for a document is that of its **best-matching chunk**.

When a result's content is literally identical to the query text, the reported distance is `0`. Any difference yields a non-zero distance.

## Embedding

The engine **owns its embedding model** and runs it locally. By default, it uses a CPU model (`multilingual-e5-base`, 768 dimensions, multilingual) served by a local Ollama sidecar. The whole stack works offline after a one-time model pull.

You never pass an embedding key on a search request — the model is the engine's own configuration. The only per-request credential is the admin secret.

The default model uses task prefixes (`passage:` for indexed text, `query:` for queries). The engine injects these automatically from a hard-coded, model-keyed table, so they cannot be misconfigured per deployment.

### Configurable providers

The embedding provider is configurable:

- **Local CPU (default)** — Ollama sidecar serving `multilingual-e5-base` (GGUF, Q8_0). Native, runs on local CPU, no external network after the one-time model pull.
- **Direct OpenAI** — `api.openai.com` for parity with the original VectorLink.
- **Generic / OpenAI-compatible HTTP** — any embeddings endpoint with configurable base URL, model, and dimension (e.g. TEI or vLLM).

## Staleness and catch-up

Indexing is asynchronous, so search can lag behind the write head. If you search a commit that hasn't been indexed yet, the engine **serves the nearest indexed ancestor immediately** — it never blocks, and it never serves a newer-than-requested snapshot (which could leak data the requested commit never had).

The served commit is reported via the **`TerminusDB-Data-Version`** response header. If it differs from what you asked for, the result is stale — the caller can compare the two and push the missing delta to catch up.

If a branch has no indexed ancestor at all, search returns `404`.

---

Next: [Quickstart](/docs/versioned-search-quickstart/).

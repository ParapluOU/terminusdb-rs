---
tags:
  - how-to
  - vectorlink
title: Entity Resolution with Versioned Search
nextjs:
  metadata:
    title: Entity Resolution with Versioned Search
    description: How to use duplicate detection and cross-set matching for entity resolution, with tuning guidance
    keywords: terminusdb, versioned search, entity resolution, duplicates, near-duplicate, record linkage, vectorlink
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-entity-resolution/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

Entity resolution is the process of determining whether records from one or more datasets refer to the same real-world entity. Versioned Search provides two endpoints for this: `/duplicates` for near-duplicate detection within a population, and `/candidates` for raw k-nearest-neighbour gathering across two record sets.

## The mental model: gather wide, then filter tight

Entity resolution runs in two stages:

```mermaid caption="Entity resolution is a two-stage pipeline: gather broadly with k and threshold, then filter tightly with cardinality-aware tau gates. Every tau must be ≤ threshold — you can only filter what you gathered."
flowchart TD
    subgraph GATHER["<b>Stage 1: Gather (broad)</b>"]
        G1["<p style='text-align: left !important; margin: 0'>For each record, fetch its<br/>k nearest neighbours within<br/>threshold distance</p>"] --- G2["<p style='text-align: left !important; margin: 0'>Knobs: k, threshold</p>"]
    end
    subgraph FILTER["<b>Stage 2: Filter (tight)</b>"]
        F1["<p style='text-align: left !important; margin: 0'>Keep only matches that pass<br/>the tau gate for their cardinality</p>"] --- F2["<p style='text-align: left !important; margin: 0'>• reciprocal core (tau_one_to_one)<br/>• set-side extras (tau_one_to_many)<br/>• target-side extras (tau_many_to_one)</p>"]
    end
    GATHER -->|"candidate pairs"| FILTER
```

The golden rule: you can only *filter* what you *gathered*. Every `tau` must be **≤ `threshold`**. The engine rejects `tau > threshold` — you cannot filter looser than you gathered. So gather generously, then tighten.

## Distance scale

All values are on the reference cosine scale **[0, 1]**: `0` = identical, `0.5` = unrelated (orthogonal), `1` = opposite. **Smaller = closer = more similar.**

A `threshold` of `0.3` means "only consider neighbours within 0.3 distance". A `tau` of `0.15` means "only accept as a match if within 0.15".

## Duplicate detection

Surface near-duplicate groups within a single population:

```bash
curl -u admin:root 'http://localhost:7372/duplicates?domain=admin/star_wars&commit=c1&threshold=0.05'
```

Each result is a `{ "group": [ {"id"[, "snippet"]}, … ], "distance": <0..1> }`, sorted nearest-first. `/duplicates` is always bounded — it never runs an unbounded all-pairs scan.

### Cross-set entity resolution

Match records across two sets — for example, matching Abt.com products against Buy.com products:

```bash
curl -u admin:root 'http://localhost:7372/duplicates?domain=admin/er&commit=c1&threshold=0.1&doc_type=Abt&target_doc_type=Buy&snippet=true'
```

- `doc_type` / `doc_id` define the **set** side.
- `target_doc_type` / `target_doc_id` define the **target** side.
- The engine gathers cross-set neighbours and filters by cardinality-aware tau gates.

## The tuning knobs

### `k` — candidate breadth

How many nearest neighbours to gather per record. This is your ceiling on how many matches a single record can have.

- Set `k` to the **maximum plausible number of pairs per record** in your domain. If a set record realistically matches at most 2–3 target records, `k = 3`–`5` is plenty.
- **Keep it low.** `k` does not need to grow with how many chunks a document has — the engine filters to cross-document neighbours, so a small `k` is safe.
- Higher `k` means more candidates considered (more recall headroom) but more noise to filter and slightly more compute. Start low, raise only if you find true matches being cut off at the candidate stage.

**Rule of thumb:** start at `k = 5`. Drop to `2`–`3` for near-1:1 data. Raise toward `10` only if a record legitimately matches many.

### `threshold` — gather ceiling

The maximum distance at which a neighbour is even considered. Anything beyond this is never gathered.

- Set `threshold` wide enough to catch all plausible matches, but not so wide that you drown in noise.
- A good starting point is `0.3` — generous enough for fuzzy matches, tight enough to exclude clearly unrelated records.
- If you see true matches being missed (false negatives), widen `threshold`. If you see too many false positives at the gather stage, tighten it.

### `tau` gates — filter tightness

After gathering, the engine applies cardinality-aware filters:

- **`tau_one_to_one`** — the reciprocal core. Both sides rank each other as nearest. This is the tightest gate and catches high-confidence 1:1 matches.
- **`tau_one_to_many`** — set-side extras. A set record legitimately matches multiple target records.
- **`tau_many_to_one`** — target-side extras. A target record legitimately matches multiple set records.

Each `tau` must be ≤ `threshold`. Start with all `tau` values equal to `threshold` (effectively no extra filtering), then tighten the `tau` gates to improve precision while maintaining recall.

## Candidates: raw KNN gather

For cases where you want the raw k-nearest-neighbour pairs without the tau filtering stage, use `/candidates`:

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

This returns the raw gathered pairs with optional embeddings and content, so you can apply your own filtering logic downstream.

## A worked example

Say you have two product catalogues — Abt and Buy — and want to find which Abt products match which Buy products.

1. **Index both catalogues** in the same data product, with `doc_type` distinguishing them (e.g. `Abt` and `Buy`).
2. **Start with `threshold=0.3`, `k=5`** and call `/duplicates` with `doc_type=Abt&target_doc_type=Buy`.
3. **Inspect the results.** If you see obvious false positives, tighten `threshold` to `0.2` or lower the `tau` gates.
4. **If you see false negatives** (known matches missing), widen `threshold` to `0.35` or raise `k` to `7`.
5. **Iterate** until the F1 score on a known-good sample is acceptable.

The key insight: tuning is a two-stage process. First, set `k` and `threshold` to gather generously. Then, tighten the `tau` gates to filter precisely. You can only filter what you gathered — so get the gather stage right first.

---

Next: [API Reference](/docs/versioned-search-api-reference/).

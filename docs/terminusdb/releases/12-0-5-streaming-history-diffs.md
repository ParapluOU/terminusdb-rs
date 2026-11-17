---
tags:
  - release-notes
  - version-control
  - documents
ai: assisted
title: 'TerminusDB 12.0.5: Streaming History with Diffs'
release:
  version: 12.0.5
  date: 2026-03-26
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.5
nextjs:
  metadata:
    title: 'Streaming History and Document Diffs | TerminusDB 12.0.5'
    description: 'TerminusDB 12.0.5 lets you stream document history with per-commit diffs, so audits show what changed when. Also: group_by on compound values.'
    keywords: terminusdb 12.0.5, document history api, streaming history endpoint, diff over time, group_by, merge_repeats, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-5-streaming-history-diffs/
---

TerminusDB 12.0.5 adds per-commit diffs and streaming to the document history endpoint, so you can audit how a record changed over time. It was released on **26 March 2026**.

- [**History diffs and streaming**](#history-diffs-and-streaming) — see what changed per commit, streamed as it is computed.
- [**`group_by` on more types**](#group_by-on-more-types) — group results on compound values, not just scalars.
- [**`merge_repeats` on update**](#merge_repeats-on-update) — the insert-time merge flag now also works on `PUT`.

## History diffs and streaming

### What it does

`GET /api/history/<path>` gained two options:

- `diff=true` — each history entry includes the structural diff of that commit against the previous one, so you can see exactly which fields changed and when.
- `streaming=true` — history is returned as a chunked NDJSON stream instead of one JSON response, so long histories start arriving immediately.

```bash
curl -u admin:root \
  "http://localhost:6363/api/history/admin/db?diff=true&streaming=true&id=Person/Jane"
```

There is also a `fast` option that works on enterprise-enriched TerminusDB databases that make these lookups over time much faster, but also takes up more storage space.

### Why it matters

The streaming history endpoint is now much more efficient for large histories, and you don't have to wait for the delivery of all results, and the diff in each version is computed on the fly so that it's possible to reconstruct every version on the client side.

The Group By feature enables all sorts of items to be grouped by which were not possible to group on before in WOQL.

**Read more:** [Audit a database's history](/docs/audit-tutorial/) · [Time travel how-to](/docs/time-travel-howto/)

## `group_by` on more types

`group_by` in WOQL now works on compound values — for example dictionaries and other non-scalar terms — because the internal `unliterally` conversion was extended to handle them. Queries that previously failed or had to be restructured now group directly.

**Read more:** [WOQL group_by](/docs/woql-class-reference-guide/#groupby) · [Grouping results](/docs/group-query-results/)

## `merge_repeats` on update

The `merge_repeats` flag ([introduced in 11.1.2](/docs/releases/11-1-2-merge-repeats/)) merges repeated inserts for the same `@id` into one document. It now also applies on document interface *update* (`PUT`), not only on insert.

## Fixes

- Auto-optimizer now only runs on write and when the database has a parent ([#2400](https://github.com/terminusdb/terminusdb/pull/2400))
- Fixed a race condition: operations on a database still being created are now rejected ([#2407](https://github.com/terminusdb/terminusdb/pull/2407))

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.5
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.4...v12.0.5)

---

**Previous:** [TerminusDB 12.0.4: Fast Range Queries and Interval Algebra](/docs/releases/12-0-4-range-queries-time-intervals/) · **Next:** [TerminusDB 12.0.6: Shared Documents and Handlebars Text Rendering](/docs/releases/12-0-6-shared-documents-cascade-delete/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

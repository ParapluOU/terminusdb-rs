---
tags:
  - release-notes
  - documents
  - data-import
ai: assisted
title: 'TerminusDB 11.1.2: Merge Inserts with merge_repeats'
release:
  version: 11.1.2
  date: 2023-09-20
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.2
nextjs:
  metadata:
    title: 'Merge Inserts with merge_repeats | TerminusDB 11.1.2'
    description: 'TerminusDB 11.1.2 lets you merge repeated inserts for one @id with merge_repeats, so bulk imports stay idempotent. Path traversal is faster too.'
    keywords: terminusdb 11.1.2, merge_repeats, upsert documents, merge documents same id, bulk import idempotent, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-2-merge-repeats/
---

TerminusDB 11.1.2 adds `merge_repeats`, an insert flag that merges repeated inserts for the same `@id` into one document instead of failing. It was released on **20 September 2023** and also speeds up path traversal.

- [**`merge_repeats` on insert**](#merge_repeats-on-insert) — upsert-style merges for repeated `@id`s in one import.
- [**Faster queries**](#faster-queries) — better WOQL execution order and faster path traversal.
- [**Schema migrations**](#schema-migrations) — classes can now migrate to abstract or unfoldable.

## `merge_repeats` on insert

### What it does

Pass `merge_repeats=true` on document insert and repeated inserts for the same `@id` merge into a single document instead of erroring. Later inserts of the same ID contribute their properties to the already-stored document — practical when a bulk import contains overlapping or repeated records for the same entity.

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/db?merge_repeats=true" \
  -H "Content-Type: application/json" \
  -d '[{ "@type": "Person", "@id": "Person/1", "name": "Ada" },
       { "@type": "Person", "@id": "Person/1", "city": "London" }]'
```

The flag was restated in the 11.1.3 notes and extended to document update in [12.0.5](/docs/releases/12-0-5-streaming-history-diffs/).

### Why it matters

JSON and JSON-LD documents often have repeated inserts for the same `@id` during bulk imports. This can either be a mistake or genuinely the same document that should be referred from multiple places. TerminusDB is designed with fault intolerance in mind, and makes the merging behaviour opt-in instead of the default.

**Read more:** [Document insertion](/docs/document-insertion/) · [The document model](/docs/documents-explanation/)

## Faster queries

- WOQL query order of execution is improved
- Path traversal is sped up

## Schema migrations

Migrations now support making classes abstract and/or unfoldable — those class-level changes propagate to existing data like other inferred migrations.

## Fixes

- GraphQL enum names map to their proper IRI format
- The container dashboard's server URI is no longer hardcoded

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.2
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.1.1-1...v11.1.2)

---

**Previous:** [TerminusDB 11.1.1: Multiple Inheritance for Properties](/docs/releases/11-1-1-multiple-inheritance-docker-compose/) · **Next:** [TerminusDB 11.1.10: Delete All Documents of a Type](/docs/releases/11-1-10-delete-documents-by-type/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

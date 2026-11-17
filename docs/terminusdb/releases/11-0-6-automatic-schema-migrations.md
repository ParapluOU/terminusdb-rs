---
tags:
  - release-notes
  - schema
  - documents
ai: assisted
title: 'TerminusDB 11.0.6: Automatic Schema Migrations'
release:
  version: 11.0.6
  date: 2023-05-03
  line: '11.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.0.6
nextjs:
  metadata:
    title: 'Automatic Schema Migrations | TerminusDB 11.0.6'
    description: 'TerminusDB 11.0.6 lets you migrate data automatically when the schema changes, so class updates carry existing documents along. Also: paged diffs.'
    keywords: terminusdb 11.0.6, schema migration, allow_destructive_migration, require_migration, evolve database schema, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-0-6-automatic-schema-migrations/
---

TerminusDB 11.0.6 turns schema changes into automatic data migrations: update a class and TerminusDB infers the migration for existing documents. It was released on **3 May 2023**.

- [**Inferred migrations**](#inferred-migrations) — schema changes carry their data transformation with them.
- [**Migration options**](#migration-options) — `allow_destructive_migration` and `require_migration`.
- [**Paged diffs**](#paged-diffs) — `start` and `count` on the diff endpoint.

## Inferred migrations

### What it does

When you modify classes through the document interface, TerminusDB now infers a migration where possible — renames, retypes, weakening constraints — and applies it to the instance data already in the database. No separate migration script.

### Why it matters

Migrations make schema change a lot easier and more reliable, as they automatically transform existing data to match the new schema and the same can be applied when moving data between branches or databases.

**Read more:** [Schema migration reference](/docs/schema-migration-reference-guide/)

## Migration options

Two new options on the document interface:

- `allow_destructive_migration` — permits migrations that modify existing instance data; the schema change transforms objects already stored.
- `require_migration` — fails the request when no migration can be inferred, instead of silently accepting a schema change that leaves data behind.

## Paged diffs

The diff endpoint gains `start` and `count` options for paged results — handy for large diffs between distant commits.

**Read more:** [JSON diff and patch](/docs/json-diff-and-patch/)

## Fixes

- GraphQL: unused optional properties, enums in collections, and `@oneOf` properties now work; stale `@oneOf` objects are cleaned from the schema graph
- Diffing `Set` values now works
- Memory leak fixed in the document API; crash on several compressed requests in one session
- Better error when pushing to a nonexistent remote

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.0.6
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.0.5...v11.0.6)

---

**Previous:** [TerminusDB 11.0.4: The Document History API](/docs/releases/11-0-4-document-history-api/) · **Next:** [TerminusDB 11.1.0: Semantic Indexer and Layer Concat](/docs/releases/11-1-0-semantic-indexer-layer-concat/) · [TerminusDB 11.0 changelog](/docs/releases/11-0-changelog/) · [Release index](/docs/releases/)

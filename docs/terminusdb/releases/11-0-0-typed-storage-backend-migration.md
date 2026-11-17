---
tags:
  - release-notes
  - documents
  - schema
ai: assisted
title: 'TerminusDB 11.0.0: Typed Storage Backend and Migration'
release:
  version: 11.0.0
  date: 2023-01-30
  line: '11.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.0.0
nextjs:
  metadata:
    title: 'Typed Storage Backend and Migration | TerminusDB 11.0.0'
    description: 'TerminusDB 11.0.0 moves storage to typed XSD files and layer archives, so data files are smaller and typed. A one-time migration tool is required.'
    keywords: terminusdb 11.0.0, storage migration 10 to 11, typed xsd storage, layer archive format, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-0-0-typed-storage-backend-migration/
---

TerminusDB 11.0.0 replaces the storage backend with typed XSD storage and layer archives. Released on **30 January 2023**, it changes the on-disk format: **your storage directory must be upgraded before the server will start**.

- [**Typed storage backend**](#typed-storage-backend) — less overhead, faster search, a layer archive format.
- [**GraphQL and frames**](#graphql-and-frames) — `_type`, `@unfoldable`, and `@metadata` in frames.
- [**Required migration**](#required-migration) — run the `terminusdb-10-to-11` tool first.

## Typed storage backend

### What it does

Two changes at the storage layer:

- **Typed storage** for a wide variety of XSD types, reducing storage overhead and improving search performance.
- **A layer archive format** that reduces storage use and latency and simplifies moving stores between machines.

### Why it matters

The performance of the engine improved dramatically and the storage size went down to a smaller fraction of the previous version. Measurements at the time indicated an average of 13 (!!) bytes per triple thanks to the succinct data structures.

## GraphQL and frames

- `_type` on GraphQL objects returns the exact type rather than the subsumed type.
- `@unfoldable` added to document frames — mark a class so its documents are unfolded when referenced.
- `@metadata` added to frames.

## Fixes

- Fixed inverse fields in GraphQL; removed extraneous system objects from the GraphQL schema
- Fixed a GraphQL naming bug that crashed schema generation; improved schema completeness
- Added `x-method-override` to CORS headers

## Required migration

**Backwards-incompatible changes:**

- The storage format changed. After installing 11.0.0, run the [terminusdb-10-to-11](https://github.com/terminusdb/terminusdb-10-to-11) conversion tool (also bundled with [terminusdb-bootstrap](https://github.com/terminusdb/terminusdb-bootstrap)) or the server will not start.
- `xsd:integer`, `xsd:decimal` and their unbounded sub-types are now returned as **strings** through the document interface — arbitrary-precision numbers do not survive most JSON libraries. You can still submit numbers; they come back as strings.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.0.0
```

Then run the storage upgrade tool above before starting the server. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v10.1.11...v11.0.0)

---

**Next:** [TerminusDB 11.0.1: Local Clone and Pre-Commit Hooks](/docs/releases/11-0-1-local-clone-pre-commit-hooks/) · [TerminusDB 11.0 changelog](/docs/releases/11-0-changelog/) · [Release index](/docs/releases/)

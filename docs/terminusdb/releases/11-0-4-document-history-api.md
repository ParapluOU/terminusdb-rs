---
tags:
  - release-notes
  - version-control
  - documents
ai: assisted
title: 'TerminusDB 11.0.4: The Document History API'
release:
  version: 11.0.4
  date: 2023-04-12
  line: '11.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.0.4
nextjs:
  metadata:
    title: 'Document History API | TerminusDB 11.0.4'
    description: 'TerminusDB 11.0.4 lets you ask when a document changed through the history API, so audit trails need no manual bookkeeping. Also: experimental migrations.'
    keywords: terminusdb 11.0.4, document history api, audit document changes, schema migration endpoint, versioned data history, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-0-4-document-history-api/
---

TerminusDB 11.0.4 adds a history API that tells you when a document changed, and an experimental schema migration endpoint. It was released on **12 April 2023**.

- [**History API**](#history-api) — find out when an object changed.
- [**Experimental schema migrations**](#experimental-schema-migrations) — transform existing data when the schema evolves.

## History API

### What it does

`GET /api/history/<path>` answers "when did this document change?" by listing the commits that touched a given document. It is the per-record view of the commit log — the basis for audit trails and time-travel queries. Later releases extended it with [diff output and streaming](/docs/releases/12-0-5-streaming-history-diffs/).

### Why it matters

Understanding specifically when a particular document changed and how it changed is important for audit trails and compliance, and for safe experimentation, always making it easy to get back to a known good version, thanks to the change message, and to branch out to create new versions of the data.

**Read more:** [Audit a database's history](/docs/audit-tutorial/) · [Versioned search history](/docs/versioned-search-history/)

## Experimental schema migrations

A new endpoint migrates a schema **and transforms existing instance data** to match — not only validates it. Schemas can also be updated from a target branch as long as a migration exists. This experiment grew into the [automatic migration inference of 11.0.6](/docs/releases/11-0-6-automatic-schema-migrations/).

**Read more:** [Schema migration reference](/docs/schema-migration-reference-guide/)

## Enhancements

- Loaded layers are kept in an LRU cache for faster repeated queries
- Optimize now also optimizes schema layers; better memory use during commits and squashes

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.0.4
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.0.3...v11.0.4)

---

**Previous:** [TerminusDB 11.0.3: The Patch Endpoint](/docs/releases/11-0-3-patch-endpoint/) · **Next:** [TerminusDB 11.0.6: Automatic Schema Migrations](/docs/releases/11-0-6-automatic-schema-migrations/) · [TerminusDB 11.0 changelog](/docs/releases/11-0-changelog/) · [Release index](/docs/releases/)

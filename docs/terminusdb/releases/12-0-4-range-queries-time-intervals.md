---
tags:
  - release-notes
  - woql
  - schema
ai: assisted
title: 'TerminusDB 12.0.4: Fast Range Queries and Interval Algebra'
release:
  version: 12.0.4
  date: 2026-02-25
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.4
nextjs:
  metadata:
    title: 'Range Queries, Time and Sets | TerminusDB 12.0.4'
    description: 'TerminusDB 12.0.4 lets you run fast triple range queries and Allen interval algebra on ISO 8601 times, so time-aware queries need less code.'
    keywords: terminusdb 12.0.4, triple_slice range query, allen interval algebra, iso 8601 time predicates, set cardinality schema, json-ld context prefixes, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-4-range-queries-time-intervals/
---

TerminusDB 12.0.4 adds fast range queries over triples, ISO 8601 time and Allen interval algebra predicates, Set cardinality constraints, and atomic JSON-LD context management. Released on **25 February 2026**, it is the biggest feature release of the 12.0 line so far.

- [**Triple range queries**](#triple-range-queries) — O(log n) range lookups pushed into the storage engine.
- [**Time and interval predicates**](#time-and-interval-predicates) — ISO 8601 parsing and the 13 Allen relations.
- [**Set cardinality**](#set-cardinality) — cardinality constraints directly on `Set` properties.
- [**Atomic context management**](#atomic-context-management) — versioned prefix changes and JSON `@context`.

## Triple range queries

### What it does

`triple_slice` (and `quad_slice` for named graphs) adds low/high bound parameters to a `triple` pattern. The bounds are pushed into the storage engine, which binary-searches the sorted value dictionary and only iterates the matching range — O(log n) lookups instead of a full scan. When the predicate is ground, the XSD type is inferred from the schema, so untyped bounds are cast for you.

```python
triple_slice(v.doc, "timestamp", v.time, "2025-01-01T00:00:00Z", "2025-02-01T00:00:00Z")
```

**Read more:** [Range queries with triple_slice and quad_slice](/docs/woql-triple-slice/)

## Time and interval predicates

### What it does

Two groups of predicates land together:

- **ISO 8601 time predicates** — parse and compare `xsd:dateTime` values, time points and durations natively, without string comparisons.
- **Allen interval algebra** — the 13 relations between time intervals (before, meets, overlaps, during, starts, finishes, and their inverses) as first-class WOQL predicates, built for temporal knowledge graphs.

### Why it matters

Time processing is crucial in business applications. Intervals are a messy part of that, especially for financial applications where you need to understand how durations really work to get it right. Read more on ISO 8601 for more information. Financial reporting builds on this foundation.

Allen interval algebra is needed to calculate how time relates for building calendars, ensuring all the dates in a year are included in the monthly reporting periods. How do you know if there is a gap between February and March on leap years unless you use Allen interval algebra to verify that all intervals meet?

Additionally, TerminusDB now supports both inclusive and half-open intervals and date ranges, which is an important distinction.

**Read more:** [WOQL time handling](/docs/woql-time-handling/) · [Interval algebra reference](/docs/woql-interval-algebra/) · [Time processing](/docs/time-processing/)

## Set cardinality

`Set` properties now take the same cardinality constraints as the deprecated `Cardinality` type — `@cardinality` for an exact bound, `@min_cardinality` and `@max_cardinality` for ranges. `Cardinality` itself is deprecated; use `Set` instead.

```json
{
  "@id": "Person",
  "@type": "Class",
  "name": "xsd:string",
  "friends": {
    "@type": "Set",
    "@class": "Person",
    "@cardinality": 3
  }
}
```

Documents violating the bounds are rejected at commit time.

**Read more:** [Set with cardinality](/docs/schema-reference-guide/#set-with-cardinality)

## Atomic context management

Schema context elements — `@context` and `@metadata` — are now managed atomically as JSON objects. Prefix changes go through the `/api/prefix/` endpoint and commit with the data that needs them; re-adding a prefix is idempotent. A `@context` value given as an IRI in the schema context is also served as a JSON-LD header value in the document API.

### Why it matters

Prefixes and data now commit together, so a schema change can never leave documents pointing at prefixes that were only half-written. Idempotent re-adds make prefix automation safe to retry.

## Also in 12.0.4

- `collect` keyword, matching `member`, for collecting a variable without grouping
- `comment` predicate for documenting WOQL queries in the AST ([#2337](https://github.com/terminusdb/terminusdb/pull/2337))
- Commit log records the actual user making the change, not only the author string
- `ChangeKey` migration keeps subdocument IDs conformant ([#2373](https://github.com/terminusdb/terminusdb/pull/2373))
- Builds prepared for NixOS; at least 10% faster reasoning-heavy workloads on SWI-Prolog 10
- `@metadata` and `@context` keys starting with `@` handled properly ([#2344](https://github.com/terminusdb/terminusdb/pull/2344))

## Fixes

- `xsd:anyURI` values are now RFC 3987 compliant for URIs and IRIs ([#2338](https://github.com/terminusdb/terminusdb/pull/2338))
- Fixed delegated database access pattern ([#2347](https://github.com/terminusdb/terminusdb/pull/2347))
- Fixed incorrect commit of label during optimization ([#2353](https://github.com/terminusdb/terminusdb/pull/2353))
- Fixed schema migration bug for inherited properties ([#2372](https://github.com/terminusdb/terminusdb/pull/2372))
- Fixed SWI-Prolog 10 stack exhaustion and improved memory management ([#2377](https://github.com/terminusdb/terminusdb/pull/2377))
- Fixed eviction of memory for stale layers ([#2342](https://github.com/terminusdb/terminusdb/pull/2342))

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.4
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.3...v12.0.4)

---

**Previous:** [TerminusDB 12.0.1: GraphQL _count and WOQL Set Operators](/docs/releases/12-0-1-graphql-count-woql-set-operators/) · **Next:** [TerminusDB 12.0.5: Streaming History with Diffs](/docs/releases/12-0-5-streaming-history-diffs/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

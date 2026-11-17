---
tags:
  - release-notes
  - graphql
draft: true
status: draft
ai: assisted
title: 'TerminusDB 11.1.6: Faster GraphQL Lookups on Filtered Values'
release:
  version: 11.1.6
  date: 2023-10-25
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.6
nextjs:
  metadata:
    title: 'Faster GraphQL Filtered Lookups | TerminusDB 11.1.6'
    description: 'TerminusDB 11.1.6 lets GraphQL queries anchor on the value you filter by, so lookups go straight to the record. Also fixes collection filtering and renames.'
    keywords: terminusdb 11.1.6, graphql filter anchor, faster graphql query, graphql collection filtering, set cardinality, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-6-faster-graphql-filtered-lookups/
---

TerminusDB 11.1.6 makes GraphQL queries that filter on a value faster, by anchoring the query on that value. It was released on **25 October 2023**.

- [**Anchor on the filtered value**](#anchor-on-the-filtered-value) — the query starts at the record you asked for.
- [**GraphQL fixes**](#graphql-fixes) — collection filtering, class renames, cardinality schema docs.

## Anchor on the filtered value

### What it does

When a GraphQL filter supplies a concrete value, the query now anchors on that value and looks the record up directly, instead of scanning and then filtering. You don't change anything; the same query just runs faster.

```graphql
query {
  Person(filter: { name: { eq: "Jane" } }) {
    name
  }
}
```

### Why it matters

TerminusDB succinct data structures are extremely fast when used. Here records can be found immediately, no scanning required. TerminusDB has no indices to maintain, by design. Every datastructure is optimized for speed and memory efficiency automatically, in a scalable way.

**Read more:** [Filter with GraphQL](/docs/filter-with-graphql/)

## GraphQL fixes

### What it does

- Filtering over collections works again.
- Renaming bugs on prefixed classes and on subsumption/class hierarchies fixed.
- Schema documents for cardinalities now return `@min_cardinality` and `@max_cardinality`, matching the documented form.
- Replacing a normal document with a JSON document returns a clear error.

**Read more:** [GraphQL naming conventions](/docs/graphql-naming-conventions-reference/)

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.6
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.6)

---

**Previous:** [TerminusDB 11.1.3](/docs/releases/11-1-3-unfold-cyclical-types/) · **Next:** [TerminusDB 11.1.8](/docs/releases/11-1-8-woql-query-ordering/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

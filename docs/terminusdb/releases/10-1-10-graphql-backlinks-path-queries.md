---
tags:
  - release-notes
  - graphql
  - diff-patch
draft: true
status: draft
ai: assisted
title: 'TerminusDB 10.1.10: GraphQL Backlinks, Path Queries and Multi-ID Filters'
release:
  version: 10.1.10
  date: 2022-12-01
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.10
nextjs:
  metadata:
    title: 'GraphQL Backlinks and Path Queries | TerminusDB 10.1.10'
    description: 'TerminusDB 10.1.10 lets you follow links backwards with GraphQL backlinks, run path queries, and filter by multiple IDs, so graph traversal needs no WOQL.'
    keywords: terminusdb 10.1.10, graphql backlinks, graphql path queries, filter multiple ids graphql, collections base types, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-10-graphql-backlinks-path-queries/
---

TerminusDB 10.1.10 makes the new GraphQL endpoint much more capable: backlinks, path queries, and filtering by a list of IDs. It was released on **1 December 2022**.

- [**Follow links backwards with backlinks**](#follow-links-backwards-with-backlinks) — ask which documents point at this one.
- [**Traverse the graph with path queries**](#traverse-the-graph-with-path-queries) — `_path_to_Class` runs a path pattern in GraphQL.
- [**Filter by several IDs, and more**](#filter-by-several-ids-and-more) — multi-ID filters, collections over base types, faster diffs.

## Follow links backwards with backlinks

### What it does

Every link field gets a reverse: if `Starship` has a `pilot` field, a `Person` document answers `_pilot_of_Starship` with the starships pointing at it. You model your data in one direction and query it in both.

```graphql
query {
  People(limit: 1) {
    label
    _pilot_of_Starship { label }
  }
}
```

### Why it matters

This is important to ensure that you can traverse the graph in both directions, which is essential for graph-based queries. Making it easy to do so in GraphQL makes it much easier to build graph-based applications. Concretely, this means that you can now query for all the starships that a person has piloted, and all the people that have piloted a starship from either end of the relationship, not only in the forward direction.

**Read more:** [Backlinks in GraphQL](/docs/back-links-in-graphql/)

## Traverse the graph with path queries

### What it does

GraphQL now runs path queries through `_path_to_CLASS` fields. A path pattern such as `film,<film` follows edges forward (`film`) and backward (`<film`), with repetition and alternation from the full path syntax.

```graphql
query {
  People {
    _path_to_People(path: "(film,<film){1,2}") { label }
  }
}
```

**Read more:** [Path queries in GraphQL](/docs/path-queries-in-graphql/) · [Path query syntax](/docs/path-query-reference-guide/)

## Filter by several IDs, and more

### What it does

- **Multi-ID filters.** GraphQL queries can match documents against a list of IDs, not only one.
- **Collections over base types.** `Set`, `List` and `Array` now work over scalar datatypes, not only classes.
- **Faster diffs.** The diff no longer compares long lists exhaustively.
- **Faster deletes.** The dangling-reference check was reworked, so deleting linked documents costs less.

**Read more:** [Filter with GraphQL](/docs/filter-with-graphql/) · [JSON diff and patch](/docs/json-diff-and-patch/)

## Fixes

- `TERMINUSDB_LOG_PATH` removed; the variable caused crashes.
- GraphiQL no longer hardcodes `localhost:6363`.
- JSON metadata is no longer framed on retrieval.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.10
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.10)

---

**Previous:** [TerminusDB 10.1.9](/docs/releases/10-1-9-unicode-fix-dashboard-path/) · **Next:** [TerminusDB 10.1.11](/docs/releases/10-1-11-wider-graphql-schema-support/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

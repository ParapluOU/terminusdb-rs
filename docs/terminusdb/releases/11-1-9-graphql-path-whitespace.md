---
tags:
  - release-notes
  - graphql
draft: true
status: draft
ai: assisted
title: 'TerminusDB 11.1.9: Whitespace in GraphQL Path Patterns'
release:
  version: 11.1.9
  date: 2023-11-15
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.9
nextjs:
  metadata:
    title: 'Whitespace in GraphQL Path Patterns | TerminusDB 11.1.9'
    description: 'TerminusDB 11.1.9 lets you write GraphQL path patterns with whitespace, so longer patterns stay readable. Also: query-cost reordering fixes.'
    keywords: terminusdb 11.1.9, graphql path pattern whitespace, path query formatting, query reordering fix, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-9-graphql-path-whitespace/
---

TerminusDB 11.1.9 lets you write GraphQL path patterns with whitespace, so a long pattern stays readable. It was released on **15 November 2023**.

## Write readable path patterns

### What it does

The `path` argument of a GraphQL path query now accepts spaces and newlines. A pattern like `(friend, knows){1,3}` can be written with breathing room instead of as one run-on token.

```graphql
query {
  People {
    _path_to_People(path: "( film , <film ){1,2}") { label }
  }
}
```

### Why it matters

Writing codes that humans understand makes them easier to maintain and debug. In an AI age, this will be less of a challenge, but when debugging together with the machine, having humans and machine read the same thing efficiently is important.

**Read more:** [Path queries in GraphQL](/docs/path-queries-in-graphql/) · [Path query syntax](/docs/path-query-reference-guide/)

## Fixes

- Query-cost reordering got fixes, following the modality work in [11.1.8](/docs/releases/11-1-8-woql-query-ordering/).

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.9
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.9)

---

**Previous:** [TerminusDB 11.1.8](/docs/releases/11-1-8-woql-query-ordering/) · **Next:** [TerminusDB 11.1.10](/docs/releases/11-1-10-delete-documents-by-type/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

---
tags:
  - release-notes
  - graphql
  - self-hosted
draft: true
status: draft
ai: assisted
title: 'TerminusDB 10.1.11: GraphQL Covers More Schemas, Safer Upgrades'
release:
  version: 10.1.11
  date: 2023-01-04
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.11
nextjs:
  metadata:
    title: 'Wider GraphQL Schema Support | TerminusDB 10.1.11'
    description: 'TerminusDB 10.1.11 lets GraphQL cover more schemata including subsumption, and adds a store-version check so upgrades fail loudly instead of corrupting data.'
    keywords: terminusdb 10.1.11, graphql schema coverage, store version check, safe upgrade, json logs, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-11-wider-graphql-schema-support/
---

TerminusDB 10.1.11 widens what GraphQL can do with real-world schemas and makes upgrades safer with a startup store-version check. It was released on **4 January 2023**, the last 10.1 release before 11.0.

- [**GraphQL on more schemas**](#graphql-on-more-schemas) — subsumption and class hierarchies now resolve.
- [**Upgrades fail loudly**](#upgrades-fail-loudly) — the server checks the store version before starting.
- [**Structured logs and open clones**](#structured-logs-and-open-clones) — JSON welcome log, public clone access.

## GraphQL on more schemas

### What it does

GraphQL now copes with more TerminusDB schemata, including subsumption: querying a parent class returns its subclasses' documents as well. Queries that previously failed on hierarchies, foreign types or unusual shapes now return results.

### Why it matters

The multiple inheritance in the TerminusDB knowledge graph database schema allows for highly complex controlled relationships between classes. This translate directly into the GraphQL implementation with no additional effort with the fully dynamic GraphQL representation of these relationships.

Queries now resolve all objects not only of the direct type, but also the types inherited from, supporting both direct inheritance and mixin-style composition of types.

**Read more:** [GraphQL naming conventions](/docs/graphql-naming-conventions-reference/) · [How to query with GraphQL](/docs/how-to-query-with-graphql/)

## Upgrades fail loudly

### What it does

On startup the server checks the store version against the version it expects. A mismatched store after an upgrade no longer starts silently and risks corrupting data; the server refuses to boot and tells you why.

## Structured logs and open clones

### What it does

- **JSON welcome log.** The startup message can be emitted as JSON, so log pipelines can parse it.
- **Public clones for the consumer role.** A consumer-role user can clone public data products.

## Fixes

- JWT library build fixed; `fetch` error handling expanded.
- WOQL AST corrections for graph-scoping words.
- Subsumption fixed in both document and GraphQL retrieval.
- Context insert and replace failures now return a clear error.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.11
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.11)

---

**Previous:** [TerminusDB 10.1.10](/docs/releases/10-1-10-graphql-backlinks-path-queries/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

---
tags:
  - release-notes
  - graphql
  - installation
draft: true
status: draft
ai: assisted
title: 'TerminusDB 10.1.8: Query Any Database with GraphQL'
release:
  version: 10.1.8
  date: 2022-11-10
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.8
nextjs:
  metadata:
    title: 'Query a Database with GraphQL | TerminusDB 10.1.8'
    description: 'TerminusDB 10.1.8 lets you query any data product with GraphQL and browse it in GraphiQL, so you can skip writing WOQL. Also: HEAD requests and ARM64 builds.'
    keywords: terminusdb 10.1.8, graphql endpoint, graphiql, graphql database query, arm64 database, head request, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-8-graphql-endpoint/
---

TerminusDB 10.1.8 adds a GraphQL endpoint and a built-in GraphiQL explorer for every data product. It was released on **10 November 2022**.

- [**Query with GraphQL**](#query-with-graphql) — every data product gets a `/api/graphql/` endpoint.
- [**Check existence with HEAD**](#check-existence-with-head) — ask if a document exists without fetching it.
- [**Run on ARM64**](#run-on-arm64) — official builds for Apple Silicon and ARM servers.

## Query with GraphQL

### What it does

Every data product gets a GraphQL endpoint at `/api/graphql/ORG/DB`. TerminusDB generates the GraphQL schema from your TerminusDB schema, so the types and fields match your data. A GraphiQL UI sits at `/graphiql/ORG/DB` for exploring queries in the browser.

```bash
curl -u admin:root -X POST http://localhost:6363/api/graphql/admin/mydb \
  -H 'Content-Type: application/json' \
  -d '{"query": "{ Person { name } }"}'
```

### Why it matters

Data mesh is here, with data products that enables you to scale your data infrastructure. With GraphQL, you can query any data product with a familiar and powerful query language. Every TerminusDB product enables you to encode the data contract directly into the data product knowledge graph schema. 

**Read more:** [GraphQL basics](/docs/graphql-basics/) · [How to query with GraphQL](/docs/how-to-query-with-graphql/)

## Check existence with HEAD

### What it does

`HEAD` on the document endpoint answers "does this document exist?" without returning the body. It's a cheap existence check before you write or fetch.

```bash
curl -I -u admin:root "http://localhost:6363/api/document/admin/mydb?id=Person/Jane"
```

## Run on ARM64

### What it does

TerminusDB now ships ARM64 builds. The server runs natively on Apple Silicon machines and ARM servers, with no emulation.

**Read more:** [Install with Docker](/docs/install-terminusdb-as-a-docker-container/)

## Fixes

- Anonymous users get an explicit URI in the system database.
- Triple-load error reporting is clearer and more reliable.
- `@metadata` ingest and retrieval bugs on classes fixed.
- Diffs no longer return or compare explicit subdocument IDs.
- `sys:Unit` now works inside `Optional` properties.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.8
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.8)

---

**Previous:** [TerminusDB 10.1.7](/docs/releases/10-1-7-dashboard-time-travel/) · **Next:** [TerminusDB 10.1.9](/docs/releases/10-1-9-unicode-fix-dashboard-path/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

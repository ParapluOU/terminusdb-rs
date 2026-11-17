---
tags:
  - release-notes
  - schema
  - self-hosted
ai: assisted
title: 'TerminusDB 11.1.1: Multiple Inheritance for Properties'
release:
  version: 11.1.1
  date: 2023-08-03
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.1
nextjs:
  metadata:
    title: 'Multiple Inheritance, Docker Compose | TerminusDB 11.1.1'
    description: 'TerminusDB 11.1.1 lets classes inherit properties from several parents, so reuse needs no duplication. Also: a docker-compose deployment with VectorLink.'
    keywords: terminusdb 11.1.1, multiple inheritance schema, docker-compose vectorlink, include_children graphql, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-1-multiple-inheritance-docker-compose/
---

TerminusDB 11.1.1 lets classes inherit properties from multiple superclasses, and adds a docker-compose setup that deploys database, VectorLink, dashboard and change-request API in one command. It was released on **3 August 2023**.

- [**Multiple inheritance**](#multiple-inheritance) — properties from several superclasses, strictest wins.
- [**Docker-compose deployment**](#docker-compose-deployment) — TerminusDB + VectorLink + dashboard + change-request API.
- [**GraphQL `include_children`**](#graphql-include_children) — opt out of subsumed types.

## Multiple inheritance

### What it does

A class can now inherit properties from several superclasses, and a subclass may even redefine an inherited property — as long as the redefinition is at least as strict as the strictest super definition. Looser redefinitions fail with a schema error.

### Why it matters

Semantic representation of data requires inheritance and multiple inheritance. The strengthening and weakening of properties is a key part of schema robustness. A good comparison is with Avro SALAD and similar schema languages.

**Read more:** [Multiple inheritance in the schema guide](/docs/schema-reference-guide/#multiple-inheritance)

## Docker-compose deployment

The `docker-compose.yml` now bundles the full stack: the TerminusDB database, VectorLink (experimental vector sidecar), the change-request API and the local dashboard — which experimentally supports AI search and change requests.

**Read more:** [Install as a Docker container](/docs/install-terminusdb-as-a-docker-container/) · [Set up VectorLink](/docs/set-up-vectorlink/)

## GraphQL `include_children`

Generated GraphQL queries gain an `include_children` argument: set it to exclude documents whose type is subsumed by (a subclass of) the queried type.

Also experimental: GraphQL filters support deep matches on IDs, and a new `ids` parameter on `GET /api/document` fetches a list of documents at once.

## Also in 11.1.1

- Schema migration scripts now support changes to class hierarchies and key strategies
- `TERMINUSDB_TRUST_MIGRATIONS` skips the instance-data recheck after a migration (default off while migrations settle in)
- GraphQL keeps a small schema cache for faster repeated queries

## Fixes

- Multi-level GraphQL hierarchies return all objects; prefixed properties, reverse links with sanitized names, and `id` filtering fixed

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.1
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.1.0...v11.1.1)

---

**Previous:** [TerminusDB 11.1.0: Semantic Indexer and Layer Concat](/docs/releases/11-1-0-semantic-indexer-layer-concat/) · **Next:** [TerminusDB 11.1.2: Merge Inserts with `merge_repeats`](/docs/releases/11-1-2-merge-repeats/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

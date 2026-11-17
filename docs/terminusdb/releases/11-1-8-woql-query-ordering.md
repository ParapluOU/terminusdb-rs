---
tags:
  - release-notes
  - query
  - woql
draft: true
status: draft
ai: assisted
title: 'TerminusDB 11.1.8: Smarter WOQL Query Ordering'
release:
  version: 11.1.8
  date: 2023-11-09
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.8
nextjs:
  metadata:
    title: 'Smarter WOQL Query Ordering | TerminusDB 11.1.8'
    description: 'TerminusDB 11.1.8 lets the query compiler reorder WOQL more safely, so your queries run in a cheaper order without changing them. Also: faster database listing.'
    keywords: terminusdb 11.1.8, woql query reordering, query cost estimation, query optimization, list databases by organization, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-8-woql-query-ordering/
---

TerminusDB 11.1.8 makes WOQL query reordering smarter: cost estimates are more reliable and the compiler now enforces modality rules when it reorders. It was released on **9 November 2023**.

- [**Safer query reordering**](#safer-query-reordering) — the compiler moves clauses only when the modes allow it.
- [**Faster database listing**](#faster-database-listing) — list databases by organization.
- [**Optimization becomes opt-in on `ask`**](#optimization-becomes-opt-in-on-ask) — an explicit flag controls it.

## Safer query reordering

### What it does

The query compiler reorders clauses to run cheap ones first. It now uses and enforces *modalities* — which arguments must be bound and which may be unbound — so a reordered query can't read a variable before anything binds it. Cost estimates improved alongside, so the chosen order is more often the right one.

### Why it matters

TODO(human): why it matters

**Read more:** [WOQL explanation](/docs/woql-explanation/) · [WOQL reference](/docs/woql-class-reference-guide/)

## Faster database listing

### What it does

Listing databases is faster and can be scoped to one organization, so servers hosting many orgs answer `db list` quickly.

```bash
terminusdb db list
```

**Read more:** [TerminusDB CLI commands](/docs/terminusdb-cli-commands/)

## Optimization becomes opt-in on `ask`

### What it does

The `ask` query predicate gains an optimization flag, and optimization is now off by default. You opt in per query when you want the optimizer to run.

## Fixes

- GraphQL names are now type-tracked, so the class of renaming bugs fixed in earlier releases should stay fixed.
- An indexing bug that broke GraphQL embedding construction is fixed.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.8
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.8)

---

**Previous:** [TerminusDB 11.1.6](/docs/releases/11-1-6-faster-graphql-filtered-lookups/) · **Next:** [TerminusDB 11.1.9](/docs/releases/11-1-9-graphql-path-whitespace/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

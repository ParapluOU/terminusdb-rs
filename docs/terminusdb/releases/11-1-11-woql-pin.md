---
tags:
  - release-notes
  - woql
  - query
draft: true
status: draft
ai: assisted
title: 'TerminusDB 11.1.11: Pin a WOQL Clause in Place During Optimization'
release:
  version: 11.1.11
  date: 2024-01-11
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.11
nextjs:
  metadata:
    title: 'Pin WOQL Clause Ordering | TerminusDB 11.1.11'
    description: 'TerminusDB 11.1.11 lets you pin a WOQL clause so the optimizer keeps its position, and improves cost estimates. Also: GraphQL lt/le comparison fixes.'
    keywords: terminusdb 11.1.11, woql pin, pin query predicate, woql query optimization, query reordering, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-11-woql-pin/
---

TerminusDB 11.1.11 adds `pin`, a WOQL predicate that tells the query optimizer to leave a clause exactly where you wrote it. It was released on **11 January 2024**.

- [**Pin a clause in place**](#pin-a-clause-in-place) — reordering skips pinned parts of the query.
- [**Better cost estimates**](#better-cost-estimates) — the optimizer picks cheaper plans more often.

## Pin a clause in place

### What it does

The query compiler reorders clauses to run the cheapest first. Wrap a subquery in `pin(...)` and the optimizer keeps it in position. Useful when execution order matters, for example a clause that must run before or after another for reasons the cost model can't see.

### Why it matters

The ability to pin a clause in place gives you more control over query execution order, which can be important for performance optimization and ensuring that certain operations are performed in a specific sequence for accuracy. The query cost optimizations are opt-in and and don't apply unless enabling them manually as they can alter the logical behaviour of the query.

**Read more:** [WOQL explanation](/docs/woql-explanation/) · [WOQL reference](/docs/woql-class-reference-guide/)

## Better cost estimates

### What it does

Query-cost estimation improved again, continuing the reordering work in [11.1.8](/docs/releases/11-1-8-woql-query-ordering/) and [11.1.9](/docs/releases/11-1-9-graphql-path-whitespace/). Test coverage of WOQL optimizations grew a lot.

## Fixes

- `Eval` field order corrected in `woql.json` and internals.
- `Length` mode description fixed; missing `LexicalKey` and `HashKey` definitions added.
- GraphQL `lt` and `le` comparisons fixed.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.11
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.11)

---

**Previous:** [TerminusDB 11.1.10](/docs/releases/11-1-10-delete-documents-by-type/) · **Next:** [TerminusDB 11.1.12](/docs/releases/11-1-12-streaming-woql-queries/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

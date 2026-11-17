---
tags:
  - release-notes
  - woql
  - query
ai: assisted
title: 'TerminusDB 11.1.12: Streaming WOQL Queries'
release:
  version: 11.1.12
  date: 2024-03-13
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.12
nextjs:
  metadata:
    title: 'Streaming WOQL Queries | TerminusDB 11.1.12'
    description: 'TerminusDB 11.1.12 lets you stream WOQL query results and call libraries of reusable parametric queries, so big answers arrive as they are computed.'
    keywords: terminusdb 11.1.12, streaming woql results, parametric query library, collection resource graph path, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-12-streaming-woql-queries/
---

TerminusDB 11.1.12 makes the WOQL endpoint stream query results and adds libraries of reusable parametric queries. Released on **13 March 2024**, it is the last feature release of the 11 line before the 12 series.

- [**Streaming WOQL results**](#streaming-woql-results) — results arrive as they are produced.
- [**Parametric query library**](#parametric-query-library) — reference reusable queries inside complex ones.
- [**Collection resources**](#collection-resources) — graph endpoints take ordinary collection paths.

## Streaming WOQL results

### What it does

The WOQL endpoint can now return streaming data — results are sent as they are computed instead of being buffered into one response. (The release notes flag this as not yet documented; see the endpoint reference for current behaviour.)

### Why it matters

Large result sets no longer need to be buffered in full before the first byte is sent. Clients see results sooner and can process rows incrementally, which keeps memory use flat and latency low for big queries.

**Read more:** [WOQL query streaming](/docs/woql-query-streaming/) · [Run a WOQL query](/docs/run-woql-query/)

## Parametric query library

The WOQL endpoint accepts a **library of parametric queries** in the request — named, reusable query templates that a complex query can invoke, instead of inlining the same logic repeatedly.

**Read more:** [NamedParametricQuery](/docs/woql-class-reference-guide/#namedparametricquery) · [NamedQuery](/docs/woql-class-reference-guide/#namedquery)

## Collection resources

Endpoints that take a graph resource now also accept ordinary collection resources, optionally followed by `/schema` or `/instance` (instance is the default) — so URLs like `admin/db` work where `admin/db/local/branch/main` used to be required.

## Also in 11.1.12

- Initial support for building with SWI-Prolog 9.2
- Query optimization changed to opt-in and is disabled by default

## Fixes

- Internal WOQL graph no longer recreated at every startup
- Looking up a single missing document no longer errors
- WOQL Dictionary templates work when only partially filled
- Fixed delta-rollup dictionary translation panics during deletion

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.12
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.1.11...v11.1.12)

---

**Previous:** [TerminusDB 11.1.10: Delete All Documents of a Type](/docs/releases/11-1-10-delete-documents-by-type/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

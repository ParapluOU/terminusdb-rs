---
tags:
  - release-notes
  - dashboard
draft: true
status: draft
ai: generated
title: 'TerminusDB 10.1.7: Time Travel and New XSD Types in the Dashboard'
release:
  version: 10.1.7
  date: 2022-10-17
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.7
nextjs:
  metadata:
    title: 'Dashboard Time Travel and xsd Types | TerminusDB 10.1.7'
    description: 'TerminusDB 10.1.7 restores dashboard time travel and group_by, adds xsd:Name and xsd:anyURI types, and hardens the server against string crashes.'
    keywords: terminusdb 10.1.7, dashboard time travel, group_by dashboard, xsd:name, xsd:anyuri, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-7-dashboard-time-travel/
---

TerminusDB 10.1.7 was released on **17 October 2022**. It is mostly a dashboard release: `group_by` queries and time travel work again, and the dashboard understands two more XSD property types. Server-side, Prolog string handling was hardened.

- [**Dashboard time travel and group_by**](#dashboard-time-travel-and-group_by) — browse past commits and aggregate in the UI.
- [**More XSD types**](#more-xsd-types) — `xsd:Name` and `xsd:anyURI` as property types.
- [**Hardened string handling**](#hardened-string-handling) — unexpected characters no longer crash the server.

## Dashboard time travel and group_by

### What it does

The bundled dashboard gains a new version with the client updated to match. Two broken flows are repaired: `group_by` queries run again, and time travel works again, so you can inspect a data product as it was at an earlier commit.

**Read more:** [TerminusDB dashboard](/docs/dashboard/) · [Time travel how-to](/docs/time-travel-howto/)

## More XSD types

### What it does

The dashboard now supports `xsd:Name` and `xsd:anyURI` property types, and a `xsd:Name` parsing bug in the server is fixed. Schemas that use these types display and edit cleanly.

**Read more:** [XSD data types](/docs/xsd-data-types/)

## Hardened string handling

### What it does

Serialization and deserialization of Prolog strings was tightened, so unexpected characters no longer crash the server. A related fix repairs data products that were corrupted by wrong prefixes.

## Fixes

All of the above, plus: corrupted data products on wrong prefixes repaired.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.7
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.7)

---

**Previous:** [TerminusDB 10.1.4](/docs/releases/10-1-4-multilingual-schemas-metadata/) · **Next:** [TerminusDB 10.1.8](/docs/releases/10-1-8-graphql-endpoint/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

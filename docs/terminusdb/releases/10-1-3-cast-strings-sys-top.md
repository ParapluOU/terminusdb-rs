---
tags:
  - release-notes
  - cli
  - woql
draft: true
status: draft
ai: assisted
title: 'TerminusDB 10.1.3: Cast Strings to Graph Nodes in WOQL'
release:
  version: 10.1.3
  date: 2022-08-16
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.3
nextjs:
  metadata:
    title: 'Cast Strings to sys:Top in WOQL | TerminusDB 10.1.3'
    description: 'TerminusDB 10.1.3 lets you cast a string to sys:Top in WOQL so a string can act as a graph node. Also: pretty-printed CLI lists, a log count option.'
    keywords: terminusdb 10.1.3, woql typecast, sys:Top, cast string to node, terminusdb log count, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-3-cast-strings-sys-top/
---

TerminusDB 10.1.3 was released on **16 August 2022**. A small release whose headline is a WOQL typecast: casting a string to `sys:Top` lets the string act as a node in the graph.

- [**Cast strings to `sys:Top`**](#cast-strings-to-systop) — use a string value where a node is expected.
- [**CLI polish**](#cli-polish) — pretty-printed lists, a `count` option on `log`, clearer errors.

## Cast strings to sys:Top

### What it does

WOQL can now cast a string to `sys:Top`, the top of the type hierarchy. A string that names a node, such as an ID or a prefixed IRI, can then be used in positions that expect a node, and a query can turn plain data into graph references without reshaping the input first.

### Why it matters

WOQL is the raw and applied datalog language of the TerminusDB databae system. Being able to cast a string to `sys:Top` allows for more flexible and powerful queries, enabling users to work with string values as if they were graph nodes.

**Read more:** [WOQL typecast reference](/docs/woql-class-reference-guide/#Typecast)

## CLI polish

### What it does

- **Pretty-printed lists.** `terminusdb db list` output is easier to scan.
- **`terminusdb log --count`.** Count commits without fetching them.
- **Better error messages** throughout the CLI.

**Read more:** [TerminusDB CLI commands](/docs/terminusdb-cli-commands/)

## Fixes

- Changing a password no longer strips the user's capabilities.
- WOQL typed-list inserts, IANA code recognition, read-lock errors, and inheritance checks for missing classes all fixed.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.3
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.3)

---

**Previous:** [TerminusDB 10.1.2](/docs/releases/10-1-2-dashboard-users-history-api/) · **Next:** [TerminusDB 10.1.4](/docs/releases/10-1-4-multilingual-schemas-metadata/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

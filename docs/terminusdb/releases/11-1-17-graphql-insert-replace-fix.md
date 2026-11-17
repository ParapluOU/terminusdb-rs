---
tags:
  - release-notes
  - graphql
draft: true
status: draft
ai: generated
title: 'TerminusDB 11.1.17: Enable Mutations for GraphQL Insert and Replace'
release:
  version: 11.1.17
  date: 2025-10-08
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.17
nextjs:
  metadata:
    title: 'GraphQL Insert and Replace Fix | TerminusDB 11.1.17'
    description: 'TerminusDB 11.1.17 fixes GraphQL mutations for insert and replace so document writes through GraphQL work again, plus Docker and CI build fixes.'
    keywords: terminusdb 11.1.17, graphql mutations, graphql insert, graphql replace, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-17-graphql-insert-replace-fix/
---

TerminusDB 11.1.17 fixes GraphQL mutations for document insert and replace, so writes through the GraphQL endpoint work again. It was released on **8 October 2025**.

## What shipped

- Fixes mutations for GraphQL insert and replace ([#2232](https://github.com/terminusdb/terminusdb/pull/2232), fixes [#2174](https://github.com/terminusdb/terminusdb/pull/2174))

## Fixes

- Fix publish action to trigger builds ([#2218](https://github.com/terminusdb/terminusdb/pull/2218))
- Update bump-version.yml ([#2217](https://github.com/terminusdb/terminusdb/pull/2217))
- Automatically Bump Version via Pull Request ([#2219](https://github.com/terminusdb/terminusdb/pull/2219))
- Bump form-data from 4.0.0 to 4.0.4 in /tests ([#2223](https://github.com/terminusdb/terminusdb/pull/2223))
- Bump brace-expansion from 1.1.11 to 1.1.12 in /tests ([#2234](https://github.com/terminusdb/terminusdb/pull/2234))
- Bump semver in /tests ([#2233](https://github.com/terminusdb/terminusdb/pull/2233))
- Fix cookies containing a colon crashing requests ([#2235](https://github.com/terminusdb/terminusdb/pull/2235))
- Fix Dockerfile workdir and packs ([#2230](https://github.com/terminusdb/terminusdb/pull/2230))
- Fix Docker file text ([#2236](https://github.com/terminusdb/terminusdb/pull/2236))
- Better signal handling ([#2231](https://github.com/terminusdb/terminusdb/pull/2231))
- Release notes for 11.1.17 ([#2237](https://github.com/terminusdb/terminusdb/pull/2237))

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.17
```

[Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.17)

---

**Next:** [TerminusDB 11.1.15](/docs/releases/11-1-changelog/#v11-1-15) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

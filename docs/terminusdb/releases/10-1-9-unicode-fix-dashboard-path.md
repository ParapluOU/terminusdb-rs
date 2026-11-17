---
tags:
  - release-notes
  - dashboard
draft: true
status: draft
ai: generated
title: 'TerminusDB 10.1.9: Unicode Crash Fix and a Configurable Dashboard Path'
release:
  version: 10.1.9
  date: 2022-11-15
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.9
nextjs:
  metadata:
    title: 'Unicode Crash Fix, Dashboard Path | TerminusDB 10.1.9'
    description: 'TerminusDB 10.1.9 fixes a crash on special unicode characters and a set-property inheritance bug, and adds TERMINUSDB_DASHBOARD_PATH for builds.'
    keywords: terminusdb 10.1.9, unicode crash fix, terminusdb_dashboard_path, set property inheritance, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-9-unicode-fix-dashboard-path/
---

TerminusDB 10.1.9 was released on **15 November 2022**. It fixes a crash on special unicode characters and adds a build-time option for the dashboard path.

- [**Serve the dashboard from a custom path**](#serve-the-dashboard-from-a-custom-path) — `TERMINUSDB_DASHBOARD_PATH` at compile time.
- [**Unicode and inheritance fixes**](#unicode-and-inheritance-fixes) — no more crashes on multibyte characters.

## Serve the dashboard from a custom path

### What it does

The `TERMINUSDB_DASHBOARD_PATH` environment variable sets where the bundled dashboard is served, at compile time. If you embed TerminusDB behind a proxy or under a path prefix, the dashboard can live where your routing expects it.

**Read more:** [TerminusDB dashboard](/docs/dashboard/) · [Docker advanced configuration](/docs/docker-advanced-configuration/)

## Unicode and inheritance fixes

### What it does

- A crash on special unicode characters is fixed. The server calculated string lengths wrongly on multibyte input.
- `Set` property inheritance in document retrieval now resolves correctly.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.9
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.9)

---

**Previous:** [TerminusDB 10.1.8](/docs/releases/10-1-8-graphql-endpoint/) · **Next:** [TerminusDB 10.1.10](/docs/releases/10-1-10-graphql-backlinks-path-queries/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

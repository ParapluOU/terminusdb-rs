---
tags:
  - release-notes
  - version-control
  - cli
ai: assisted
title: 'TerminusDB 11.0.1: Local Clone and Pre-Commit Hooks'
release:
  version: 11.0.1
  date: 2023-02-22
  line: '11.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.0.1
nextjs:
  metadata:
    title: 'Local Clone and Pre-Commit Hooks | TerminusDB 11.0.1'
    description: 'TerminusDB 11.0.1 lets you clone a database locally without a remote and run pre-commit schema checks, so offline work and validation get easier.'
    keywords: terminusdb 11.0.1, local clone, grpc label store, pre-commit hook, cli impersonation, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-0-1-local-clone-pre-commit-hooks/
---

TerminusDB 11.0.1 adds local clone, a gRPC label-store option, CLI user impersonation, and a pre-commit schema validation hook. It was released on **22 February 2023**, with one backwards-incompatible fix to document replacement.

- [**Local clone**](#local-clone) — clone a database without a remote.
- [**CLI impersonation and pre-commit hook**](#cli-impersonation-and-pre-commit-hook) — run CLI commands as another user; validate schemas before commit.

## Local clone

`terminusdb clone` now works locally — you can copy a database on the same server without configuring a remote.

**Read more:** [Clone a project](/docs/clone-a-project/) · [CLI clone command](/docs/clone/)

### Why it matters

Local cloning makes team collaboration and scaling much easier, many writers can write to different data products all on their own, and it enables better change control.

## CLI impersonation and pre-commit hook

- **User impersonation** in the CLI tool — run operations as another user.
- **Pre-commit hook** — implement custom schema validations that run before a commit is accepted.

## Also in 11.0.1

- Faster login (fewer hash roundtrips), faster db list, no auth needed on the `ok` endpoint
- System schema graph is pinned and loaded only once

**Breaking:** document replacement no longer inserts nested top-level documents unless you pass the `create` option. Subdocuments are not affected.

## Fixes

- Fixed SSL errors in fetch and push with newer OpenSSL
- Fixed duplicate insertion of nested documents; stale databases cleaned up when a clone fails
- Fixed `GYear` support and the startup message when no store exists

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.0.1
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.0.0...v11.0.1)

---

**Previous:** [TerminusDB 11.0.0: Typed Storage Backend and Migration](/docs/releases/11-0-0-typed-storage-backend-migration/) · **Next:** [TerminusDB 11.0.3: The Patch Endpoint](/docs/releases/11-0-3-patch-endpoint/) · [TerminusDB 11.0 changelog](/docs/releases/11-0-changelog/) · [Release index](/docs/releases/)

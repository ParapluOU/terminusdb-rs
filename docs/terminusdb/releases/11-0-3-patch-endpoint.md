---
tags:
  - release-notes
  - diff-patch
  - documents
ai: assisted
title: 'TerminusDB 11.0.3: The Patch Endpoint'
release:
  version: 11.0.3
  date: 2023-03-16
  line: '11.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.0.3
nextjs:
  metadata:
    title: 'Apply Diffs with the Patch Endpoint | TerminusDB 11.0.3'
    description: 'TerminusDB 11.0.3 lets you apply a JSON diff to documents with the patch endpoint, so structural edits land in one commit. Also: a memory-pinning option.'
    keywords: terminusdb 11.0.3, patch endpoint, apply json diff, structural diff patch, merge conflict resolution, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-0-3-patch-endpoint/
---

TerminusDB 11.0.3 adds the patch endpoint, the counterpart to diff that applies a structural JSON diff to documents in a branch. It was released on **16 March 2023**.

- [**Patch endpoint**](#patch-endpoint) — send a diff, get a commit.
- [**Pinned organizations**](#pinned-organizations) — keep a whole team's data products in memory.

## Patch endpoint

### What it does

`POST /api/patch/<path>` takes a diff — the same structure produced by the diff endpoint — and applies it to the documents of a branch in a single commit. Together, diff and patch are the primitives behind merge workflows: diff two commits, keep the changes you want, apply them where you need them.

### Why it matters

Traditional diff and patch enabled the software industry to merge changes across branches and resolve conflicts. This worked well for code, TerminusDB enables the same capability for knowledge graphs at scale and for the communication of information across the commercial and business-oriented world.

**Read more:** [Patch endpoint reference](/docs/patch-endpoint/) · [JSON diff and patch](/docs/json-diff-and-patch/) · [Diff and patch operations](/docs/diff-and-patch-operations/)

## Pinned organizations

A new environment variable pins an organization: all data products of that organization or team are kept in memory for faster access.

## Fixes

- Full schema replace no longer fails early when an ID is missing
- String filters now work on all stringy types, not only strings
- Document delete returns `204` and non-GET responses are no longer chunked
- Fixed inversion naming for non-standard GraphQL class names

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.0.3
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.0.2...v11.0.3)

---

**Previous:** [TerminusDB 11.0.1: Local Clone and Pre-Commit Hooks](/docs/releases/11-0-1-local-clone-pre-commit-hooks/) · **Next:** [TerminusDB 11.0.4: The Document History API](/docs/releases/11-0-4-document-history-api/) · [TerminusDB 11.0 changelog](/docs/releases/11-0-changelog/) · [Release index](/docs/releases/)

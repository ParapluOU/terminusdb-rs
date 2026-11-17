---
tags:
  - release-notes
  - schema
  - documents
draft: true
status: draft
ai: assisted
title: 'TerminusDB 11.1.3: Unfold Cyclical Types Safely'
release:
  version: 11.1.3
  date: 2023-09-28
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.3
nextjs:
  metadata:
    title: 'Unfold Cyclical Types Safely | TerminusDB 11.1.3'
    description: 'TerminusDB 11.1.3 lets you mark cyclical types @unfoldable, so linked documents expand inline without recursion errors. Retrieval now always detects cycles.'
    keywords: terminusdb 11.1.3, unfoldable cyclical types, cycle detection, nested document retrieval, merge_repeats, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-3-unfold-cyclical-types/
---

TerminusDB 11.1.3 lets you use `@unfoldable` on cyclical types, such as a `Person` whose `friend` property points at another `Person`. It was released on **28 September 2023**.

- [**Unfold cyclical types**](#unfold-cyclical-types) — a cycle returns an ID reference instead of recursing forever.
- [**Always-on cycle detection**](#always-on-cycle-detection) — document retrieval detects cycles everywhere.

## Unfold cyclical types

### What it does

`@unfoldable` now works on types that refer back to themselves. When retrieval hits a document it has already expanded, it returns the document's ID instead of unfolding again. Before this release, a cyclical `@unfoldable` schema could recurse without end; now it can't.

```json
{
  "@id": "Person",
  "@type": "Class",
  "@unfoldable": [],
  "name": "xsd:string",
  "friend": { "@type": "Optional", "@class": "Person" }
}
```

A `Person` whose `friend` chain comes back around returns the repeated document as `Person/...` rather than expanding again.

### Why it matters

This change allows you to safely unfold cyclical types without causing infinite recursion. It's particularly useful for social networks, recommendation systems, and other domains where entities can reference each other in complex ways.

**Read more:** [@unfoldable in the schema guide](/docs/schema-reference-guide/#unfoldable) · [Document unfolding reference](/docs/document-unfolding-reference/)

## Always-on cycle detection

### What it does

Document retrieval now always runs cycle detection, regardless of schema annotations. Foreign types also interact correctly with collections.

The release notes also restate the `merge_repeats` insert flag, which first shipped in [TerminusDB 11.1.2](/docs/releases/11-1-2-merge-repeats/).

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.3
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v11.1.3)

---

**Previous:** [TerminusDB 11.1.2](/docs/releases/11-1-2-merge-repeats/) · **Next:** [TerminusDB 11.1.6](/docs/releases/11-1-6-faster-graphql-filtered-lookups/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

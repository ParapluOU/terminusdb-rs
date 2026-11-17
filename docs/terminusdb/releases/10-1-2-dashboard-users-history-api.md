---
tags:
  - release-notes
  - dashboard
  - cli
draft: true
status: draft
ai: assisted
title: 'TerminusDB 10.1.2: Manage Users in the Dashboard, Query History over the API'
release:
  version: 10.1.2
  date: 2022-08-10
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.2
nextjs:
  metadata:
    title: 'User Dashboard and History API | TerminusDB 10.1.2'
    description: 'TerminusDB 10.1.2 lets you manage users in the dashboard and read commit history over an API, plus unfoldable classes and new db CLI commands.'
    keywords: terminusdb 10.1.2, user management dashboard, database history api, unfoldable class, terminusdb cli db list, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-2-dashboard-users-history-api/
---

TerminusDB 10.1.2 was released on **10 August 2022**. It is the largest 10.1 update: user management moves into the dashboard, history and database listing become API calls, and `@unfoldable` makes linked documents expand inline.

- [**Manage users in the dashboard**](#manage-users-in-the-dashboard) — create and manage users from the UI or the new endpoints.
- [**Query history over the API**](#query-history-over-the-api) — `api/log` returns the commit history of a data product.
- [**Unfold linked documents by default**](#unfold-linked-documents-by-default) — mark a class `@unfoldable` and it expands inline on retrieval.
- [**New CLI commands and faster raw JSON**](#new-cli-commands-and-faster-raw-json) — `db list`, `db update`, `reset`, branch diffs by name.

## Manage users in the dashboard

### What it does

A new dashboard covers data product and user management, backed by user management endpoints on the server. The OpenAPI specification was expanded and moved into the source tree, so the published spec tracks the code.

### Why it matters

Having a graphical user interface is important for adoption, as it provides a more intuitive and user-friendly experience than using the command line or API directly.

**Read more:** [TerminusDB dashboard](/docs/dashboard/) · [Managing users and invitations](/docs/managing-users-and-invitations/)

## Query history over the API

### What it does

The new `api/log` route returns the history of a data product: which commits landed, when, and by whom.

```bash
curl -u admin:root http://localhost:6363/api/log/admin/mydb
```

The matching CLI commands are `terminusdb log` and `terminusdb db list`, which now shows databases with richer metadata through `api/list`.

### Why it matters

Seeing the history of a data product is important for understanding its evolution and tracking changes over time. It helps you answer compliance questions of who changed what when and understand the context of your data. The historical log is an enabler for git-for-data so that the commits to pick for a 3-way merge can be identified.

**Read more:** [Audit a database's history](/docs/audit-tutorial/) · [TerminusDB CLI commands](/docs/terminusdb-cli-commands/)

## Unfold linked documents by default

### What it does

A class marked `@unfoldable` returns its documents expanded inline wherever they are linked, instead of as bare references.

```json
{
  "@id": "Address",
  "@type": "Class",
  "@unfoldable": [],
  "street": "xsd:string",
  "city": "xsd:string"
}
```

Schemas where unfolding would recurse forever are rejected with an error, so a circular model fails at schema time rather than at read time.

### Why it matters

Circular references in data models can cause infinite loops, and they are not uncommon. By rejecting such schemas at schema time, we prevent runtime errors and ensure that the data model is consistent and performs predictably. This has been improved in later versions so that a reference is added at the tip of the circular reference, preventing the infinite loop, while enabling manual resolution at the same time for actual circular references.

**Read more:** [@unfoldable in the schema guide](/docs/schema-reference-guide/#unfoldable) · [Document unfolding reference](/docs/document-unfolding-reference/)

## New CLI commands and faster raw JSON

### What it does

- **Diff branches by name.** Branch diffs take branch names, not commit IDs, so comparing `main` to a feature branch no longer needs a commit lookup first.
- **`terminusdb db update` and `terminusdb reset`.** Update a database in place or reset it, from the CLI.
- **Faster raw JSON processing.** Large arrays in particular load faster.
- **Configurable JWT claim name.** The name the server reads from a JWT can be configured, easing integration with identity providers that use a non-default claim.

**Read more:** [TerminusDB CLI commands](/docs/terminusdb-cli-commands/) · [JSON diff and patch](/docs/json-diff-and-patch/)

## Fixes

- Referential-integrity checking no longer rejects valid documents.
- Raw JSON works inside `List`, `Set` and `Array` properties.
- `path{n,m}` path queries parse string bounds forgivingly.
- Enum checking in containers and full enum URI inference fixed.
- Assorted CLI error-handling improvements. The man page now explains `DB_SPEC`.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.2
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.2)

---

**Next:** [TerminusDB 10.1.3](/docs/releases/10-1-3-cast-strings-sys-top/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)

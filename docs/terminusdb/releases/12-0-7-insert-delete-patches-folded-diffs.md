---
tags:
  - release-notes
  - version-control
  - documents
ai: assisted
title: 'TerminusDB 12.0.7: Resolve Merge Conflicts with Insert and Delete Patches'
release:
  version: 12.0.7
  date: 2026-08-10
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.7
nextjs:
  metadata:
    title: 'Resolve Merge Conflicts with Patches | TerminusDB 12.0.7'
    description: 'TerminusDB 12.0.7 lets the patch endpoint insert and delete documents, so you can resolve merge conflicts in one commit. Also: folded diffs, Elixir client.'
    keywords: terminusdb 12.0.7, resolve merge conflict database, json patch insert delete, git for data merge, version controlled json, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-7-insert-delete-patches-folded-diffs/
---

TerminusDB 12.0.7 lets you insert and delete documents inside a patch, so you can fix a merge conflict by hand in one commit. It was released on **10 August 2026**.

- [**Insert and delete in patches**](#insert-and-delete-in-patches) — resolve a merge conflict and bring in new documents in one commit.
- [**Folded diffs**](#folded-diffs) — choose if the diff endpoint expands subdocuments.
- [**Elixir client**](#elixir-client) — a community client for Elixir.

## Insert and delete in patches

### What it does

`POST /api/patch/<path>` now accepts three kinds of operations in its `patch` array:

- **Field-level patch** (as before): `SwapValue` on one field of an existing document.
- **Insert**: `{"@op": "Insert", "@insert": { ...document... }}`
- **Delete**: `{"@op": "Delete", "@delete": {"@id": "..."}}`

Insert and Delete use the same format as the output of the diff endpoint. So you can run a diff, keep only the changes you want, and send the rest as a patch. Inserts can also use `@capture` and `@ref` to link new documents to each other in the same commit.

### Why it matters

Say `main` and `feature` both changed `Person/A`, and `feature` also added `Person/B`. Before 12.0.7 there was no clean way to keep main's `Person/A` and still bring in `Person/B`:

- The patch endpoint could change existing fields, but it could not insert `Person/B`.
- The apply endpoint could insert, but keeping main's version gave a merge conflict. You had to add a new commit on the feature branch first.

Now one patch can do it all. This is the same diff → pick → apply flow you know from Git, for JSON documents.

### Example

Keep main's `Person/A`, insert `Person/B` from the feature branch, and remove an old record, in one atomic commit:

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/patch/admin/people/local/branch/main" \
  -H "Content-Type: application/json" \
  -d '{
    "patch": [
      {"@op": "Insert", "@insert": {"@id": "Person/B", "@type": "Person", "name": "Bob"}},
      {"@op": "Delete", "@delete": {"@id": "Person/Old"}}
    ],
    "author": "admin",
    "message": "Merge feature: add Person/B, keep main version of Person/A"
  }'
```

To take the feature branch's value for a field instead, add a field-level patch:

```json
{"@id": "Person/A", "name": {"@op": "SwapValue", "@before": "Monica", "@after": "Laura"}}
```

If a document does not match what the patch expects, the call fails with a conflict and nothing is written.

**Read more:** [Patch endpoint](/docs/patch-endpoint/) · [How to merge branches and handle conflicts](/docs/merge-howto/#3-handle-merge-conflicts) · [JSON diff and patch](/docs/json-diff-and-patch/)

## Folded diffs

`POST /api/diff` has a new `unfold` parameter. The default is `true`, which is the old behaviour: subdocuments are expanded inline before the comparison. Set `"unfold": false` to compare documents as they are stored, without expanding subdocuments. The CLI has a matching `--unfold` flag.

```json
{ "before_data_version": "main", "after_data_version": "feature", "document_id": "Person/A", "unfold": false }
```

**Read more:** [Diff endpoint](/docs/json-diff-and-patch/#diff)

## Elixir client

There is now a community [Elixir client](https://github.com/terminusdb/terminusdb#clients), linked from the TerminusDB README. Thanks @thanos.

## Fixes

- RFC 3986 scheme detection in `uri_has_protocol/1` ([#2458](https://github.com/terminusdb/terminusdb/pull/2458))
- No more false patch conflicts on `xsd:decimal` properties ([#2462](https://github.com/terminusdb/terminusdb/pull/2462))
- `@shared` is part of the Rust change detection ([#2478](https://github.com/terminusdb/terminusdb/pull/2478))
- SWI-Prolog upgraded to 10.0.2; JWT handling is now backed by Rust ([#2507](https://github.com/terminusdb/terminusdb/pull/2507))
- macOS build prerequisites documented ([#2502](https://github.com/terminusdb/terminusdb/pull/2502))

Thanks to our three new contributors: @Kidnix101, @drernie and @thanos.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.7
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.6...v12.0.7)

---

**Previous:** [TerminusDB 12.0.6: Shared Documents and Handlebars Text Rendering](/docs/releases/12-0-6-shared-documents-cascade-delete/) · **Next:** [TerminusDB 12.0.8: UUID v7 Keys and Faster JSON Loading](/docs/releases/12-0-8-uuid-v7-faster-json-loading/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

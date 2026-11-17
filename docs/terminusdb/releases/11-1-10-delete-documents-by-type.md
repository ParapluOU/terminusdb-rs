---
tags:
  - release-notes
  - documents
  - woql
ai: assisted
title: 'TerminusDB 11.1.10: Delete All Documents of a Type'
release:
  version: 11.1.10
  date: 2024-01-11
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.10
nextjs:
  metadata:
    title: 'Delete All Documents of a Type | TerminusDB 11.1.10'
    description: 'TerminusDB 11.1.10 lets you delete every document of a type in one call, so cleanup needs no loop. Also: WOQL AST type inference, faster deletes.'
    keywords: terminusdb 11.1.10, delete all documents of type, bulk delete documents, woql type inference, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-10-delete-documents-by-type/
---

TerminusDB 11.1.10 lets the document endpoint delete every document of a type in one call. It was released on **11 January 2024**, and deletions generally got faster.

- [**Delete by type**](#delete-by-type) — clear a whole class in one request.
- [**WOQL AST type inference**](#woql-ast-type-inference) — values in queries get their types inferred.
- [**Metadata migrations**](#metadata-migrations) — `@metadata`/`@documentation` changes count as weakening.

## Delete by type

### What it does

`DELETE /api/document/<path>?type=<Class>` removes every instance document of the given class in one commit — the fast path for clearing a collection before a re-import, without enumerating IDs. (`nuke=true` goes further and removes all documents in the graph.)

```bash
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/db?type=Person&author=admin&message=Clear+Persons"
```

### Why it matters

Easy handling of documents is important for data management and data governance. With TerminusDB you never have to fear to delete, going back to history and create a branch is so easy!

**Read more:** [Delete a document](/docs/delete-a-document/)

## WOQL AST type inference

The WOQL AST parser now infers types for values, so queries need fewer explicit type annotations.

## Metadata migrations

Changing `@metadata` or `@documentation` on a class is now treated as a *weakening* operation and generates a migration — keeping schema history consistent rather than silently rewriting metadata.

## Enhancements

- Faster deletions across the board

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.10
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.1.9...v11.1.10)

---

**Previous:** [TerminusDB 11.1.2: Merge Inserts with merge_repeats](/docs/releases/11-1-2-merge-repeats/) · **Next:** [TerminusDB 11.1.12: Streaming WOQL Queries](/docs/releases/11-1-12-streaming-woql-queries/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

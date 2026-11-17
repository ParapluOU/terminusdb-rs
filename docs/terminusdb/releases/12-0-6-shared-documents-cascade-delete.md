---
tags:
  - release-notes
  - schema
  - documents
ai: assisted
title: 'TerminusDB 12.0.6: Shared Documents and Handlebars Text Rendering'
release:
  version: 12.0.6
  date: 2026-06-24
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.6
nextjs:
  metadata:
    title: 'Shared Documents with Cascade Delete | TerminusDB 12.0.6'
    description: 'TerminusDB 12.0.6 lets you share documents between parents with @shared and cascade-delete them, so linked records clean themselves up. Also: Handlebars.'
    keywords: terminusdb 12.0.6, shared documents, cascade delete graph database, handlebars embedding render, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-6-shared-documents-cascade-delete/
---

TerminusDB 12.0.6 adds `@shared` documents that are cascade-deleted when their last referrer goes away, and a Handlebars text-rendering format for the document API. It was released on **24 June 2026**.

- [**`@shared` documents**](#shared-documents) — standalone documents that are deleted automatically when their last reference goes away.
- [**Embedding text output**](#embedding-text-output) — render documents as plain text through a schema-defined Handlebars template.
- [**Fixes**](#fixes) — `@ref` resolution for tagged unions, non-ASCII document loss, and `SwapValue` on compound values.

## `@shared` documents

### What it does

`@shared` is a new class-level annotation, a middle ground between a regular document and a `@subdocument`:

- Like a regular document, a `@shared` document has its own IRI and can be referenced from any number of other documents.
- Like a subdocument, it has a managed lifecycle: when the last document that references it is deleted, the `@shared` document is deleted too — in the same transaction.

```json
{
  "@id": "Tag",
  "@type": "Class",
  "@shared": [],
  "label": "xsd:string"
}
```

This is designed for flows where documents are created standalone and linked later — for example loading data in several transactions that are merged into one commit. The cascade delete is atomic: either all deletions succeed or the whole transaction rolls back.

### Why it matters

At shared is important when you build a knowledge graph out of JSON-LD documents and think of the question of lifecycles. How do you lifecycle the triples of a JSON-LD document without annotation of what triples belong to which document? 

With `@shared` you can say that this document exists only as long as it is referenced. But it lives its own life, can be referenced by many and be updated independently. But once the last reference is removed, it is deleted automatically. This is very useful for representing knowledge graphs in TerminusDB.

Together with the XML and Triples features of TerminusDB Enterprise, you can now represent knowledge graphs in a very natural way, including for the blank nodes handling that are not required in a TerminusDB knowledge graph thanks to the schema, where instead they get skolemized and replaced by schema frames.

### Example

```json
{ "@type": "Tag", "@id": "Tag/featured", "label": "Featured" }
```

```json
{ "@type": "Article", "@id": "Article/1", "title": "On graphs", "tags": ["Tag/featured"] }
```

When `Article/1` is the last document holding a reference to `Tag/featured` and it is deleted, `Tag/featured` is removed in the same commit.

**Read more:** [The document model](/docs/documents-explanation/) · [Schema reference](/docs/schema-reference-guide/) · [Subdocuments](/docs/schema-reference-guide/#subdocument)

## Embedding text output

### What it does

The document API can now render documents as `text/plain` using a Handlebars template defined in the schema under `@metadata.embedding`. Ask for it with `format=embedding` or an `Accept: text/plain` header:

```bash
curl -u admin:root "http://localhost:6363/api/document/admin/db?format=embedding&id=Animal/Plato"
```

```json
{
  "@id": "Animal",
  "@type": "Class",
  "@key": { "@type": "Lexical", "@fields": ["name"] },
  "name": "xsd:string",
  "@metadata": {
    "embedding": {
      "query": "query ($id: ID) { Animal(id: $id) { name } }",
      "template": "The animal is named {{name}}."
    }
  }
}
```

For `Animal/Plato` this returns `The animal is named Plato.` — a flat text rendering of the document, made for embedding into vector indexes such as VectorLink. Classes without an embedding definition return a `no_embedding_query` error.

### Why it matters

Templates turn any document into embedding-ready text in a single API call, removing the transformation step from your indexing pipeline. The same template keeps rendered text consistent as documents change, so vector indexes stay in sync with the data.

## Fixes

- `@ref` capture resolution now works with `TaggedUnion` variants ([#2432](https://github.com/terminusdb/terminusdb/pull/2432))
- Fixed silent document loss for non-ASCII characters in the document API ([#2450](https://github.com/terminusdb/terminusdb/pull/2450))
- `SwapValue` patches now handle dicts, lists and numbers correctly ([#2438](https://github.com/terminusdb/terminusdb/pull/2438))

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.6
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.5...v12.0.6)

---

**Previous:** [TerminusDB 12.0.5: Streaming History with Diffs](/docs/releases/12-0-5-streaming-history-diffs/) · **Next:** [TerminusDB 12.0.7: Resolve Merge Conflicts with Insert and Delete Patches](/docs/releases/12-0-7-insert-delete-patches-folded-diffs/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

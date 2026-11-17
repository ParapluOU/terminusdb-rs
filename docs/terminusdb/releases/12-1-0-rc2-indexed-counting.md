---
tags:
  - release-notes
  - woql
  - graphql
  - documents
ai: assisted
title: 'TerminusDB 12.1.0-rc2: Indexed Counting'
release:
  version: 12.1.0-rc2
  line: '12.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/pull/2601
nextjs:
  metadata:
    title: 'Indexed Counting | TerminusDB 12.1.0-rc2'
    description: 'TerminusDB 12.1.0-rc2 counts triples, documents and GraphQL result sets straight from the store indexes: WOQL triple_count patterns and triple_range_count, counted=true document listings, and fast GraphQL _count.'
    keywords: terminusdb 12.1, triple_count, triple_range_count, woql count, graphql _count, counted documents, rdf triple count
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-1-0-rc2-indexed-counting/
---

TerminusDB 12.1.0-rc1 makes counting fast everywhere you used to enumerate to count. These features are staged for the 12.1 release and available in the 12.1 release candidates.

- [**WOQL pattern and range counts**](#woql-pattern-and-range-counts) — `triple_count` accepts partial patterns; `triple_range_count` counts typed values in a half-open range.
- [**`counted` document listings**](#counted-document-listings) — `counted=true` returns just the number of documents a listing would yield.
- [**Fast GraphQL `_count`**](#fast-graphql-_count) — empty-filter `_count` resolves from the store indexes.

All of them are answered from the persistent indexes in terminusdb-store, so no triples or documents are materialized. On one million documents, counting drops from seconds of streaming to about 20 milliseconds.

## WOQL pattern and range counts

`triple_count` keeps its resource form and gains a pattern form `(subject, predicate, object, count, [graph])`. Each component can be bound or a variable acting as a wildcard:

```javascript
WOQL.triple_count("v:s", "rdf:type", "scm:Person", "v:n")
```

`triple_range_count(subject, predicate, low, high, count, [graph])` counts triples whose typed object value falls in the half-open range `[low, high)`, optionally restricted by subject and predicate. `quad_range_count` passes an explicit graph.

```javascript
WOQL.triple_range_count("v:s", "scm:age", { "@type": "xsd:integer", "@value": 18 }, { "@type": "xsd:integer", "@value": 30 }, "v:n")
```

## `counted` document listings

Pass `counted=true` on the document listing endpoint to get the count the listing would yield, without the documents:

```bash
curl "http://localhost:6363/api/document/admin/db?type=Person&counted=true"
```

```json
{ "@type": "api:DocumentCountResponse", "api:count": 1000000 }
```

Type-restricted counts include subclasses, and subdocuments follow the same rules as the listing (`unfold=true` excludes them). `counted` cannot be combined with `id`, `ids` or `query` — the server answers `api:CountedNotSupportedForSelector`.

## Fast GraphQL `_count`

`_count` on a document field resolves through the indexes when the filter is empty:

```graphql
{ _count(Person: {}) }
```

Filtered `_count` traverses as before; the fast path applies to the unfiltered case. The same surface is available in the JavaScript and Python clients.

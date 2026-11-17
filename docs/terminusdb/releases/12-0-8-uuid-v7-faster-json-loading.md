---
tags:
  - release-notes
  - schema
  - documents
ai: assisted
title: 'TerminusDB 12.0.8: UUID v7 Keys and 4.6× Faster Raw JSON Loading'
release:
  version: 12.0.8
  date: 2026-10-12
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.8
nextjs:
  metadata:
    title: 'UUID v7 Keys and Faster JSON Loading | TerminusDB 12.0.8'
    description: 'TerminusDB 12.0.8 lets you use UUID v7 document keys, so you can keep IDs issued by other systems. Raw JSON loads about 4.6× faster than before.'
    keywords: terminusdb 12.0.8, uuid v7 keys, uuidv7 database id, import external IRIs, knowledge graph database release, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-8-uuid-v7-faster-json-loading/
---

TerminusDB 12.0.8 adds `UuidV7` document keys that accept IDs issued by other systems, and loads raw JSON about 4.6× faster. It was released on **12 October 2026**.

- [**UuidV7 keys**](#uuid-v7-keys) — keep the IDs your data already has, or let TerminusDB create time-ordered UUIDs.
- [**Faster inserts**](#faster-inserts) — ID creation is about 6× faster. A 100 MB raw JSON file now loads in 27 s instead of 125 s.
- [**Fixes**](#fixes) — 11 fixes for GraphQL, UTF-8, `@unfold`, bundles and JWT scopes.

## UuidV7 keys

### What it does

`UuidV7` is a new `@key` type for classes. It works in two ways:

- **You supply an `@id`.** TerminusDB uses it as it is. It does not need to start with the class `@base`. UUIDs, GUIDs and full IRIs from other systems are all accepted.
- **You leave out `@id`.** TerminusDB creates one: the class `@base` plus a new RFC 9562 UUID v7, for example `Artwork/01a120f0-47a7-7f20-9295-b3034d7fc5d3`.

UUID v7 values contain a timestamp, so new IDs sort by creation time.

### Why it matters

Enterprise data often arrives with identifiers that were created somewhere else. Ontologies such as Linked Art use instance IRIs like `https://linked.art/example/object/47`. Before 12.0.8, the `Random` key refused these, because a submitted `@id` had to sit under the class `@base`. You had to rewrite every IRI before you could load the data.

With `UuidV7` you can load that data into your knowledge graph as it is, and still get generated IDs for new records.

### Example

```json
{
  "@id": "Artwork",
  "@type": "Class",
  "@key": { "@type": "UuidV7" },
  "title": "xsd:string"
}
```

Insert one document with an external IRI and one without an `@id`:

```json
[
  { "@type": "Artwork", "@id": "https://linked.art/example/object/47", "title": "Night Watch" },
  { "@type": "Artwork", "title": "New acquisition" }
]
```

The second document gets an ID like `Artwork/01a120f0-47a7-7f20-9295-b3034d7fc5d3`.

`UuidV7` works for top-level classes, `@subdocument` classes and `TaggedUnion` members. It is also available in WOQL (`UuidV7`, like `RandomKey`), in the JavaScript client (`WOQL.idgen_uuid_v7(base, uri)`) and in the Python client (`UuidV7Key`).

**Read more:** [UuidV7 in the schema reference](/docs/schema-reference-guide/#uuid-v7) · [All key types](/docs/schema-reference-guide/#key) · [WOQL UuidV7](/docs/woql-class-reference-guide/#UuidV7)

## Faster inserts

ID creation is one of the most common steps when you insert documents. In 12.0.8:

- Creating one ID went from about 1,200 ns to about 200 ns. This also speeds up `Random`, `Lexical` and `Hash` keys.
- Content hashes for raw JSON (`sys:JSON`, `sys:JSONDocument`) are now computed in Rust. A nested document of about 60 triples inserts in 0.47 ms instead of 2.0 ms (about 4.3×).
- A 100 MB raw JSON file loads, converts to triples and commits in **27.2 s instead of 125.5 s** on the same hardware.

The IDs are byte-identical to before, so existing data does not change.

**Read more:** [Raw JSON IDs](/docs/schema-reference-guide/#id-handling-for-sys-json-and-sys-json-document)

## Fixes

- Fixed `unbundle` failing with `unknown_layer_reference` across stores ([#2540](https://github.com/terminusdb/terminusdb/pull/2540))
- Fixed insert performance regressions ([#2529](https://github.com/terminusdb/terminusdb/pull/2529))
- Field-level `@unfold` now works for `List` and multi-dimensional `Array` properties ([#2513](https://github.com/terminusdb/terminusdb/pull/2513))
- GraphQL frames accept the `@shared` class annotation ([#2533](https://github.com/terminusdb/terminusdb/pull/2533))
- GraphQL `__typename` and fragments resolve on generated types ([#2551](https://github.com/terminusdb/terminusdb/pull/2551))
- Removed GraphQL debug logging that could panic on multibyte UTF-8 ([#2532](https://github.com/terminusdb/terminusdb/pull/2532))
- `PUT` with `charset=utf-8` writes documents again ([#2544](https://github.com/terminusdb/terminusdb/pull/2544))
- Streaming history endpoint returns correct UTF-8 ([#2545](https://github.com/terminusdb/terminusdb/pull/2545))
- WOQL JSON inserts booleans and list scalars correctly ([#2546](https://github.com/terminusdb/terminusdb/pull/2546))
- JWT scope upgrade resolves roles by id, not display name ([#2539](https://github.com/terminusdb/terminusdb/pull/2539))
- Removed a leftover debug log ([#2534](https://github.com/terminusdb/terminusdb/pull/2534))

Also: faster CLI and integration tests ([#2543](https://github.com/terminusdb/terminusdb/pull/2543)) and dependency bumps. Thanks to our two new contributors, @hideo67 and @bixycler.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.8
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.7...v12.0.8)

---

**Previous:** [TerminusDB 12.0.7: Resolve Merge Conflicts with Insert and Delete Patches](/docs/releases/12-0-7-insert-delete-patches-folded-diffs/) · **Next:** [TerminusDB 12.0.9: Security Fixes and Patch for Raw JSON Documents](/docs/releases/12-0-9-security-fixes-raw-json-patch/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

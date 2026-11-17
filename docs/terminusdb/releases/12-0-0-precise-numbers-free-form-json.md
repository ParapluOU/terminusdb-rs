---
tags:
  - release-notes
  - woql
  - documents
ai: assisted
title: 'TerminusDB 12.0.0: Precise Numbers and Free-Form JSON'
release:
  version: 12.0.0
  date: 2025-12-02
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.0
nextjs:
  metadata:
    title: 'Precise Numbers and Free-Form JSON | TerminusDB 12.0.0'
    description: 'TerminusDB 12.0.0 lets you store free-form JSON with deduplication and keep precise rational numbers, so numeric data stays exact. Also: sys:Dictionary docs.'
    keywords: terminusdb 12.0.0, high precision json numbers, sys:JSON free form json, sys:Dictionary, xdd:json, breaking changes, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-0-precise-numbers-free-form-json/
---

TerminusDB 12.0.0 adds precise rational numbers, free-form JSON storage with deduplication, and stricter interfaces. Released on **2 December 2025**, it is the first stable release of the 12 line, maintained by [Gettier](https://gettier.ai). The on-disk storage format is unchanged since version 11. **This release has breaking changes** — read the list below before upgrading.

- [**Precise JSON numbers**](#precise-json-numbers) — rationals instead of floats, decimals up to 20 places.
- [**Free-form JSON and `sys:Dictionary`**](#free-form-json-and-sysdictionary) — schemaless JSON fields with deduplication.
- [**WOQL: `slice`, `dot`, `RandomKey`**](#woql-slice-dot-randomkey) — list slicing and dictionary addressing.
- [**Safer errors and a no-root image**](#safer-errors-and-a-no-root-image) — request IDs, tracing, no stack traces.

## Precise JSON numbers

Numbers in all APIs are now real JSON numbers processed as rationals — not strings, and not float/double — with high-precision decimals capped at 20 decimal places. `sys:JSON` stores arbitrary precision decimals and integers up to 256 digits. WOQL calculations use precise rationals for correct results.

**Breaking:** API clients that sent or expected numbers as strings must be updated, and code relying on old wildcard `path()` behaviour may return duplicates — `.*` now traverses `List` correctly, and `group_by()` unwraps single-element templates automatically.

**Read more:** [JSON types `sys:JSON` and `sys:JSONDocument`](/docs/schema-reference-guide/#json-types-sys-json-and-sys-jsondocument)

## Free-form JSON and `sys:Dictionary`

Two new ways to store JSON without a rigid schema:

- **`sys:JSON`** — a property type for arbitrary, schemaless JSON inside a document. Values are deduplicated and stored reference-counted, so repeated JSON costs almost nothing. `xdd:json` is its WOQL literal form, and `dot()` can address fields inside it.
- **`sys:JSONDocument`** — a top-level document type for completely unstructured JSON, addressable by a normal `@id` or a random one.
- **`sys:Dictionary`** — a new datatype for dictionary documents and dictionary templates, typecastable to and from `xdd:json`.

### Why it matters

The TerminusDB 12.0 Knowledge Graph Database release is a huge business-enabling milestone release: With free-form JSON support; arbitrary precision decimals across JSON, GraphQL, RDF and WOQL; and better observability.

First, TerminusDB can now fully compete head to head with noSQL systems that don't have schema, with automatically deduplicated JSON storage that scales. Where other systems hit 16MB JSON limits.

TerminusDB handles JSON of almost any size by design, MongoDB for example has a 16MB limit, but TerminusDB doesn't. JSON documents can also be traversed and queries like any other RDF graph of triples, making every JSON a knowledge graph. This is important for many applications that need immutable databases with version control over JSON documents and are stuck with relational databases.

Diff and patch is possible over JSON documents, which is important for many applications that need to track changes over time.

The rational numbers support means that TerminusDB has practically unlimited precision numbers and is not limited to floats and doubles. The ISO JSON support means that the the wire format for TerminusDB JSON transports is of full precision and not limited to doubles. This matters a lot for those that need 18 digits, 20 digits, 36 digits or some other safe precision for their scientific or financial applications.

**Read more:** [`sys:JSON` and `sys:JSONDocument`](/docs/schema-reference-guide/#json-types-sys-json-and-sys-jsondocument) · [ID handling for raw JSON](/docs/schema-reference-guide/#id-handling-for-sys-json-and-sys-jsondocument)

## WOQL: `slice`, `dot`, `RandomKey`

- `slice()` — new operator for WOQL list item handling.
- `dot()` — now works on `path()` edge variable bindings, so you can address parts of a matched path directly.
- `RandomKey` — new WOQL type replacing `RandomIdgen` for consistency; the JS client gains `idgen_random()` and the Python client replaces `random_idgen()` with `idgen_random()`.

**Read more:** [WOQL class reference](/docs/woql-class-reference-guide/)

## Safer errors and a no-root image

- Every error response carries a unique request ID (UUID v4) for debugging, with W3C Trace Context headers for distributed tracing.
- Stack traces are no longer exposed in HTTP responses; errors share one general format with helpful Prolog error terms.
- The Docker image no longer needs root, and the auto-optimizer for continuous roll-ups is enabled by default.
- The dashboard component is deprecated but can still be enabled manually — see [dashboard docs](https://terminusdb.com/docs/dashboard).

### Why it matters

Production deployments get a smaller attack surface: stack traces no longer leak internals, and every error carries a request ID you can follow through logs and traces. The container drops its root requirement, and the auto-optimizer being on by default means databases stay fast without scheduled maintenance.

## Fixes

- `path()` `times{0,N}` correctly handles zero hops
- Fixed string vs atom comparisons when patching enum values
- Fixed error message duplication between `api:error` and `api:message`
- New error type `api:IncompatibleNumericComparison` for clearer numeric mismatches

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.0
```

No storage migration is needed — the on-disk format is unchanged since version 11 — but **review the breaking changes** above first: numbers-as-strings clients, `path()` `.*` patterns, and `group_by()` single-element templates all changed behaviour. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.1.17...v12.0.0)

---

**Next:** [TerminusDB 12.0.1: GraphQL _count and WOQL Set Operators](/docs/releases/12-0-1-graphql-count-woql-set-operators/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

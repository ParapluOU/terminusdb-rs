---
tags:
  - release-notes
  - graphql
  - woql
ai: assisted
title: 'TerminusDB 12.0.1: GraphQL _count and WOQL Set Operators'
release:
  version: 12.0.1
  date: 2025-12-15
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.1
nextjs:
  metadata:
    title: 'GraphQL _count and WOQL Set Operators | TerminusDB 12.0.1'
    description: 'TerminusDB 12.0.1 lets you count results in GraphQL and run fast WOQL set operators, so aggregation and set logic get simpler. Also: field-level @unfold.'
    keywords: terminusdb 12.0.1, graphql _count, unfold schema annotation, woql union intersection difference, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-1-graphql-count-woql-set-operators/
---

TerminusDB 12.0.1 adds `_count` to GraphQL, fast set operators to WOQL, and field-level unfolding to the schema language. It was released on **15 December 2025**.

- [**GraphQL `_count`**](#graphql-_count) — count filtered results without returning them.
- [**Field-level `@unfold`**](#field-level-unfold) — control which properties expand when a document is read.
- [**WOQL set operators**](#woql-set-operators) — high-performance union, intersection and difference.

## GraphQL `_count`

### What it does

Every class query in GraphQL now has a `_count` field that returns the number of documents matching the filter, without transferring the documents themselves.

```graphql
query { Person(name: "Jane") { _count } }
```

### Why it matters

Counting documents is important. It was missing and now it is here. It is not hyper-performance yet, the indices are there but not in use yet. This will be instant in a future release (post an issue if counting fast is important to you).

**Read more:** [GraphQL query reference](/docs/graphql-query-reference/) · [How to query with GraphQL](/docs/how-to-query-with-graphql/)

## Field-level `@unfold`

### What it does

`@unfold` can now be applied to a single property, not only at class level. Add `"@unfold": true` to an `Optional`, `Set`, `Array`, `List` or `Cardinality` property to expand that linked document inline on read, while leaving other properties folded — even when the target class is not `@unfoldable`.

```json
{
  "@id": "Order",
  "@type": "Class",
  "customer": { "@type": "Optional", "@class": "Customer", "@unfold": true },
  "product": { "@type": "Optional", "@class": "Product" }
}
```

Reading an `Order` returns `customer` unfolded inline and `product` as an ID reference. Cycles are detected at runtime: a recurring document comes back as its ID instead of recursing forever.

### Why it matters

You can shape each read to the client that consumes it — expand the fields a UI needs inline and keep the rest as references. Responses get smaller, extra round trips disappear, and cyclic graphs stay safe because recursion stops at the first repeat.

**Read more:** [`@unfold` in the schema guide](/docs/schema-reference-guide/#unfold-field-level)

## WOQL set operators

New high-performance WOQL predicates — `union`, `intersection` and `difference` — combine or subtract whole result sets in one step instead of nested `or`/`not` constructs.

```python
intersection(query_a, query_b)
```

**Read more:** [WOQL Union](/docs/woql-class-reference-guide/#setunion) · [WOQL Intersection](/docs/woql-class-reference-guide/#setintersection) · [WOQL Difference](/docs/woql-class-reference-guide/#setdifference)

## Fixes

- `sys:JSON` numeric values now cast correctly to other numeric types
- Fixed nested subdocument ID generation
- Fixed stray subdocument residue during deletion (a 12.0.0 regression)
- Fixed Foreign type schema validation and inclusion in the schema graph
- Aligned `@unfold` with `@unfoldable` and prevented possible WOQL unfolding issues
- Reduced server startup delay; re-enabled the in-memory server for testing

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.1
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.0...v12.0.1)

---

**Previous:** [TerminusDB 12.0.0: Precise Numbers and Free-Form JSON](/docs/releases/12-0-0-precise-numbers-free-form-json/) · **Next:** [TerminusDB 12.0.4: Fast Range Queries and Interval Algebra](/docs/releases/12-0-4-range-queries-time-intervals/) · [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)

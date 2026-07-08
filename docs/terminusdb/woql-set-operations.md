---
tags:
  - woql
  - reference
  - advanced
title: WOQL Set Operations Reference
nextjs:
  metadata:
    title: WOQL Set Operations Reference
    description: Reference guide for WOQL set operations including set_difference, set_intersection, set_union, set_member, and list_to_set
    keywords: terminusdb, datalog, diff, query language, terminusdb query, woql, woql set operations reference
    alternates:
      canonical: https://terminusdb.org/docs/woql-set-operations/
media: []
---

WOQL provides native set operations with O(n log n) performance, enabling efficient comparison of large datasets. These operations can handle 100,000+ elements in under a second.

> **See also:** [Integration tests](https://github.com/terminusdb/terminusdb/blob/main/tests/test/woql-set-operations.js) | [CSV comparison guide](/docs/compare-csv-values-with-woql/)

## Operations Overview

| Operation | Description | Complexity |
|-----------|-------------|------------|
| `list_to_set` | Sorts list and removes duplicates | O(n log n) |
| `set_difference(A, B, Result)` | Elements in A but not in B | O(n log n) |
| `set_intersection(A, B, Result)` | Elements in both A and B | O(n log n) |
| `set_union(A, B, Result)` | All unique elements from A and B | O(n log n) |
| `set_member(Element, Set)` | Check if element is in set | O(log n) |

## list_to_set

Converts a list to a sorted set, removing duplicates.

```javascript
WOQL.list_to_set("v:my_list", "v:my_set")
```

## set_difference

Returns elements in the first set that are not in the second set.

```javascript
WOQL.set_difference("v:set_a", "v:set_b", "v:only_in_a")
```

**Example:** Finding items to add and remove during synchronization:

```javascript
WOQL.and(
  WOQL.list_to_set("v:source_list", "v:source_set"),
  WOQL.list_to_set("v:target_list", "v:target_set"),
  WOQL.set_difference("v:source_set", "v:target_set", "v:to_add"),
  WOQL.set_difference("v:target_set", "v:source_set", "v:to_remove"),
)
```

## set_intersection

Returns elements that exist in both sets.

```javascript
WOQL.set_intersection("v:set_a", "v:set_b", "v:in_both")
```

## set_union

Returns all unique elements from both sets combined.

```javascript
WOQL.set_union("v:set_a", "v:set_b", "v:combined")
```

## set_member

Checks if an element exists in a set. Returns true/false via backtracking.

```javascript
WOQL.set_member("v:element", "v:my_set")
```

**Use with `not` for exclusion checks:**

```javascript
WOQL.not(WOQL.set_member("v:element", "v:excluded_set"))
```

## Performance Benchmarks

| Elements | set_difference | set_intersection |
|----------|----------------|------------------|
| 1,000    | 7ms            | 5ms              |
| 5,000    | 30ms           | 17ms             |
| 10,000   | 64ms           | 66ms             |
| 100,000  | 577ms          | 445ms            |

## Complete Example: Data Synchronization

```javascript
WOQL.and(
  // Build source list using group_by
  WOQL.group_by(
    [],
    ["val"],
    "v:source_list",
    WOQL.or(
      WOQL.eq("v:val", WOQL.literal("A", "xsd:string")),
      WOQL.eq("v:val", WOQL.literal("B", "xsd:string")),
      WOQL.eq("v:val", WOQL.literal("C", "xsd:string")),
    )
  ),
  
  // Build target list
  WOQL.group_by(
    [],
    ["val"],
    "v:target_list",
    WOQL.or(
      WOQL.eq("v:val", WOQL.literal("B", "xsd:string")),
      WOQL.eq("v:val", WOQL.literal("C", "xsd:string")),
      WOQL.eq("v:val", WOQL.literal("D", "xsd:string")),
    )
  ),
  
  // Convert to sets
  WOQL.list_to_set("v:source_list", "v:source_set"),
  WOQL.list_to_set("v:target_list", "v:target_set"),
  
  // Compute differences
  WOQL.set_difference("v:source_set", "v:target_set", "v:to_add"),
  WOQL.set_difference("v:target_set", "v:source_set", "v:to_remove"),
  WOQL.set_intersection("v:source_set", "v:target_set", "v:unchanged"),
)
```

**Result:**
- `to_add`: `["A"]` (in source, not in target)
- `to_remove`: `["D"]` (in target, not in source)
- `unchanged`: `["B", "C"]` (in both)

## Streaming with set_member

For large datasets, use `set_member` with backtracking to check individual values:

```javascript
WOQL.and(
  // Load reference set
  WOQL.group_by([], ["val"], "v:ref_list", /* ... */),
  WOQL.list_to_set("v:ref_list", "v:ref_set"),
  
  // Stream through values to check
  WOQL.triple("v:doc", "@schema:id", "v:id"),
  
  // O(log n) membership check
  WOQL.not(WOQL.set_member("v:id", "v:ref_set")),
)
```

This processes one value at a time, using O(log n) lookups against the set.

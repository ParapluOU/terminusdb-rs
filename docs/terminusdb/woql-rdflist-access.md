---
tags:
  - woql
  - reference
  - rdf
  - advanced
title: RDF List Access Operations
nextjs:
  metadata:
    title: RDF List Access Operations
    description: Learn how to read elements from RDF lists using peek, member, length, list, and slice operations in WOQL.
    keywords: terminusdb, datalog, query language, rdf, rdf list access operations, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-access/
media: []
---

This guide covers operations for reading elements from RDF lists. These operations allow you to inspect list contents without modifying the underlying structure.

## What You'll Learn

- How to read the first element of a list
- How to iterate through all list elements
- How to count list elements
- How to extract slices of lists
- How to access elements by index (nth0/nth1)

## Operations Summary

| Operation | Description | Time Complexity |
|-----------|-------------|-----------------|
| `rdflist_peek` | Get first element | O(1) |
| `rdflist_member` | Iterate all elements | O(n) |
| `rdflist_length` | Count elements | O(n) |
| `rdflist_list` | Collect all elements as array | O(n) |
| `rdflist_slice` | Extract a range of elements | O(n) |

## rdflist_peek

Gets the first element (head) of an RDF list without removing it, or match it with the value in the variable when using as a match operator.

### Syntax

```javascript
WOQL.lib().rdflist_peek(consSubject, valueVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `valueVar` | string | Variable to bind the first value |

### Example

```javascript
const query = WOQL.lib().rdflist_peek(listHead, "v:first_value");
const result = await client.query(query);

const firstValue = result.bindings[0]["first_value"];
console.log(firstValue["@value"]); // "Task A"
```

### Use Cases

- **Preview**: Check the first item without consuming it
- **Stack peek**: Implement stack-like peek operations
- **Validation**: Verify the head element before processing

## rdflist_member

Traverses an RDF list and binds each element to a variable through [unification](/docs/what-is-unification). Returns multiple bindings, one for each element. Can be used to both match existing list elements and generate solutions.

### Syntax

```javascript
WOQL.lib().rdflist_member(consSubject, valueVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `valueVar` | string | Variable to bind each element |

### Example

```javascript
const query = WOQL.lib().rdflist_member(listHead, "v:value");
const result = await client.query(query);

// Returns multiple bindings
const values = result.bindings.map(b => {
  const val = b["value"] || b["v:value"];
  return val?.["@value"] || val;
});

console.log(values); // ["Task A", "Task B", "Task C"]
```

### Use Cases

- **Iteration**: Process each element in the list
- **Search**: Find elements matching criteria
- **Aggregation**: Collect values for further processing

### Combining with Other Queries

```javascript
// Find all tasks with high priority
const query = WOQL.and(
  WOQL.lib().rdflist_member(taskListHead, "v:task_id"),
  WOQL.triple("v:task_id", "priority", "v:priority"),
  WOQL.greater("v:priority", 5)
);
```

## rdflist_length

Counts the number of elements in an RDF list, or match that the number of elements match a variable.

### Syntax

```javascript
WOQL.lib().rdflist_length(consSubject, lengthVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `lengthVar` | string | Variable to bind the count |

### Example

```javascript
const query = WOQL.lib().rdflist_length(listHead, "v:count");
const result = await client.query(query);

const length = parseInt(result.bindings[0]["count"]["@value"]);
console.log(`List has ${length} elements`);
```

### Use Cases

- **Validation**: Check list size constraints
- **Pagination**: Calculate total pages
- **Capacity checks**: Verify list doesn't exceed limits

## rdflist_list

Collects all elements from an RDF list into a single array in one binding. Can also be used to verify that an RDF list matches a given array.

### Syntax

```javascript
WOQL.lib().rdflist_list(consSubject, listVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `listVar` | string | Variable to bind the collected array |

### Example

```javascript
const query = WOQL.lib().rdflist_list(listHead, "v:all_items");
const result = await client.query(query);

const items = result.bindings[0]["all_items"];
console.log(items); // [{@value: "A"}, {@value: "B"}, {@value: "C"}]

// Extract values
const values = items.map(item => item?.["@value"] || item);
console.log(values); // ["A", "B", "C"]
```

### Use Cases

- **Export**: Get all list items in one result
- **Comparison**: Compare entire lists
- **Serialization**: Convert list to JSON array

### Difference from rdflist_member

| Operation | Returns | Bindings |
|-----------|---------|----------|
| `rdflist_member` | One element per binding | Multiple bindings |
| `rdflist_list` | All elements as array | Single binding |

## rdflist_slice

Extracts a range of elements from an RDF list. Uses 0-based indexing and an exclusive end. Think of the positions to match to be the positions "between" the elements. 0 is before the first element, 1 is after the first element, etc. Thus, to slice the first element, slice between 0 and 1.

### Syntax

```javascript
WOQL.lib().rdflist_slice(consSubject, start, end, resultVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `start` | number | Start index (0-based, inclusive) |
| `end` | number | End index (exclusive) |
| `resultVar` | string | Variable to bind the slice array |

### Examples

```javascript
// List: [A, B, C, D]

// Get first two elements: slice(0, 2) → [A, B]
const first2 = WOQL.lib().rdflist_slice(listHead, 0, 2, "v:result");

// Get middle elements: slice(1, 3) → [B, C]
const middle = WOQL.lib().rdflist_slice(listHead, 1, 3, "v:result");

// Get last two elements: slice(2, 4) → [C, D]
const last2 = WOQL.lib().rdflist_slice(listHead, 2, 4, "v:result");

// Single element: slice(1, 2) → [B]
const single = WOQL.lib().rdflist_slice(listHead, 1, 2, "v:result");

// Empty slice when start >= end: slice(2, 2) → []
const empty = WOQL.lib().rdflist_slice(listHead, 2, 2, "v:result");
```

### Use Cases

- **Pagination**: Get a page of results
- **Preview**: Show first N items
- **Windowing**: Process data in chunks

## Accessing Elements by Index

You can use slice instead of `nth0` or `nth1` operations for single items.

### Using Slice for Index Access

```javascript
// Get element at index 2 (0-based)
const getAt2 = WOQL.lib().rdflist_slice(listHead, 2, 3, "v:element");
const result = await client.query(getAt2);
const element = result.bindings[0]["element"][0]; // First item of single-element array
```

### Using Path Queries instead, for manual parsing

Notice that `WOQL.path()` uses regex style matching (inclusive), whereas slice uses exclusive matching, like in Javascript.

```javascript
// Access elements by path traversal
// Path {n,n} traverses exactly n rdf:rest links
const pathQuery = WOQL.and(
  WOQL.path(listHead, "rdf:rest{2,2}", "v:cell"),  // Skip 2 cells
  WOQL.triple("v:cell", "rdf:first", "v:value")    // Get value
);
```

### Zero-Based vs One-Based Indexing

- **0-based (nth0)**: First element is at index 0
- **1-based (nth1)**: First element is at index 1

Use slice with appropriate indices:
- nth0(2) = slice(2, 3)[0]
- nth1(2) = slice(1, 2)[0]

## Complete Example: List Analysis

List head in the example below is the object of a triple from a list. A list defined on a document/record in TerminusDB will have a triple such as `triple("v:docId", "list", "v:listHeadId")`. The listHeadId would be what is called a Cons, an rdf:List "construct" that is the start of a list.

```javascript
import { WOQLClient, WOQL } from "terminusdb";

async function analyzeList(client, listHead) {
  // Get list length
  const lengthQuery = WOQL.lib().rdflist_length(listHead, "v:len");
  const lengthResult = await client.query(lengthQuery);
  const length = parseInt(lengthResult.bindings[0]["len"]["@value"]);
  console.log(`List length: ${length}`);
  
  if (length === 0) {
    console.log("List is empty");
    return;
  }
  
  // Get first element
  const peekQuery = WOQL.lib().rdflist_peek(listHead, "v:first");
  const peekResult = await client.query(peekQuery);
  console.log(`First element: ${peekResult.bindings[0]["first"]["@value"]}`);
  
  // Get all elements
  const listQuery = WOQL.lib().rdflist_list(listHead, "v:all");
  const listResult = await client.query(listQuery);
  const allItems = listResult.bindings[0]["all"];
  console.log("All elements:", allItems.map(i => i["@value"]));
  
  // Get first half
  const halfLength = Math.ceil(length / 2);
  const sliceQuery = WOQL.lib().rdflist_slice(listHead, 0, halfLength, "v:half");
  const sliceResult = await client.query(sliceQuery);
  const firstHalf = sliceResult.bindings[0]["half"];
  console.log("First half:", firstHalf.map(i => i["@value"]));
}
```

## Working with Document References in Lists

```javascript
// Create tasks
await client.addDocument([
  { "@type": "Task", "@id": "Task/1", title: "Fix bug", priority: 1 },
  { "@type": "Task", "@id": "Task/2", title: "Update docs", priority: 2 }
]);

// Read task details from list
const detailsQuery = WOQL.and(
  WOQL.lib().rdflist_member(taskListHead, "v:task_id"),
  WOQL.triple("v:task_id", "title", "v:title"),
  WOQL.triple("v:task_id", "priority", "v:priority")
);

const details = await client.query(detailsQuery);
// Returns: [{task_id: "Task/1", title: "Fix bug", priority: 1}, ...]
```

## Read More

- [RDF List Operations Overview](/docs/woql-rdflist-operations/) - Main guide
- [List Creation Operations](/docs/woql-rdflist-creation/) - Creating lists
- [List Modification Operations](/docs/woql-rdflist-modification/) - Adding and removing elements
- [Path Queries](/docs/path-queries-in-woql/) - Advanced traversal patterns
- [Integration Tests](https://github.com/terminusdb/terminusdb-client-js/blob/main/integration_tests/woql_rdflist_operations.test.ts) - Complete test examples

---
tags:
  - woql
  - reference
  - rdf
  - advanced
title: RDF List Transformation Operations
nextjs:
  metadata:
    title: RDF List Transformation Operations
    description: Learn how to reorder RDF lists using swap and reverse operations in WOQL for TerminusDB.
    keywords: terminusdb, datalog, query language, rdf, rdf list transformation operations, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-transformation/
media: []
---

This guide covers operations for transforming and reordering RDF lists. These operations modify the order of elements without adding or removing them.

## What You'll Learn

- How to swap elements at different positions
- How to reverse an entire list
- Common use cases for list reordering

## Operations Summary

| Operation | Description | Time Complexity |
|-----------|-------------|-----------------|
| `rdflist_swap` | Exchange elements at two positions | O(n) |
| `rdflist_reverse` | Reverse the entire list in-place | O(n) |

## rdflist_swap

Exchanges the values at two positions in the list. Both positions use 0-based indexing.

### Syntax

```javascript
WOQL.lib().rdflist_swap(consSubject, positionA, positionB)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `positionA` | number | First position (0-based) |
| `positionB` | number | Second position (0-based) |

### Examples

```javascript
// List: [A, B, C, D]

// Swap first and last: [D, B, C, A]
await client.query(WOQL.lib().rdflist_swap(listHead, 0, 3));

// Swap adjacent elements: [A, C, B, D]
await client.query(WOQL.lib().rdflist_swap(listHead, 1, 2));

// Swap middle with first: [C, B, A, D]
await client.query(WOQL.lib().rdflist_swap(listHead, 2, 0));
```

### No-Op When Positions Are Equal

```javascript
// Swapping position 1 with itself does nothing
await client.query(WOQL.lib().rdflist_swap(listHead, 1, 1));
// List remains unchanged
```

### Complete Swap Example with Documents

This example creates a schema with a document containing an ordered list, then swaps elements.

```javascript
// Step 1: Define schema with a document that has an ordered list
const schema = [
  {
    "@type": "Class",
    "@id": "Playlist",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "songs": { "@type": "List", "@class": "xsd:string" }
  }
];

await client.addDocument(schema, { graph_type: "schema" });

// Step 2: Create a playlist document with songs
const playlist = {
  "@type": "Playlist",
  "name": "Road Trip",
  "songs": ["Highway Star", "Born to Run", "Life is a Highway", "On the Road Again"]
};

await client.addDocument(playlist);

// Step 3: Get the list head from the document
const getListHead = WOQL.triple("Playlist/Road%20Trip", "songs", "v:listHead");
const headResult = await client.query(getListHead);
const listHead = headResult.bindings[0]["listHead"];

// Step 4: Swap songs - move "Life is a Highway" (pos 2) to first position (pos 0)
// Before: ["Highway Star", "Born to Run", "Life is a Highway", "On the Road Again"]
await client.query(WOQL.lib().rdflist_swap(listHead, 2, 0));
// After:  ["Life is a Highway", "Born to Run", "Highway Star", "On the Road Again"]

// Step 5: Verify the result
const readResult = await client.query(
  WOQL.lib().rdflist_list(listHead, "v:songs")
);
const songs = readResult.bindings[0]["songs"].map(s => s["@value"]);
console.log(songs);
// ["Life is a Highway", "Born to Run", "Highway Star", "On the Road Again"]

// The document now reflects the swapped order
const updatedPlaylist = await client.getDocument({ id: "Playlist/Road%20Trip" });
console.log(updatedPlaylist.songs);
// ["Life is a Highway", "Born to Run", "Highway Star", "On the Road Again"]
```

### Use Cases

- **Sorting**: Implement sorting algorithms
- **Reordering**: Move items to different positions
- **User-driven ordering**: Respond to drag-and-drop operations
- **Shuffling**: Randomize list order

## rdflist_reverse

Reverses the order of all elements in a list. This is an **in-place** operation that modifies the list structure.

### Syntax

```javascript
WOQL.lib().rdflist_reverse(consSubject)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |

### Examples

```javascript
// 4-element list: [A, B, C, D] → [D, C, B, A]
await client.query(WOQL.lib().rdflist_reverse(listHead));

// 2-element list: [X, Y] → [Y, X]
await client.query(WOQL.lib().rdflist_reverse(listHead));

// Single-element list: [Only] → [Only] (unchanged)
await client.query(WOQL.lib().rdflist_reverse(listHead));
```

### Complete Reverse Example

> **Note:** The `add_triple` calls below are a low-level RDF operation for constructing cons cells (`rdf:List`, `rdf:first`, `rdf:rest`). This is not the standard document API — use `insert_document` for document-oriented data.

```javascript
// Create list: [A, B, C, D]
const createQuery = WOQL.and(
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell4"),
  WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("A")),
  WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
  WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("B")),
  WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
  WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("C")),
  WOQL.add_triple("v:cell3", "rdf:rest", "v:cell4"),
  WOQL.add_triple("v:cell4", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell4", "rdf:first", WOQL.string("D")),
  WOQL.add_triple("v:cell4", "rdf:rest", "rdf:nil")
);

const result = await client.query(createQuery);
const listHead = result.bindings[0]["cell1"];

// Reverse the list in place
await client.query(WOQL.lib().rdflist_reverse(listHead));

// Read the reversed list using path query with ordering
const readQuery = WOQL.and(
  WOQL.path(listHead, "rdf:rest*", "v:node", "v:path"),
  WOQL.length("v:path", "v:pos"),
  WOQL.triple("v:node", "rdf:first", "v:val")
);
const readResult = await client.query(WOQL.order_by("v:pos", readQuery));

const values = readResult.bindings
  .map(b => b["val"]["@value"])
  .filter(v => v !== undefined);
console.log(values); // ["D", "C", "B", "A"]
```

### Edge Cases

```javascript
// Reversing a 2-element list
// [X, Y] → [Y, X]
await client.query(WOQL.lib().rdflist_reverse(twoElementList));

// Reversing a single-element list (no change)
// [Only] → [Only]
await client.query(WOQL.lib().rdflist_reverse(singleElementList));

// Reversing an empty list (no-op)
// rdf:nil → rdf:nil
```

### Use Cases

- **Display order**: Show newest first vs oldest first
- **Undo stack**: Process items in reverse order
- **Algorithm implementation**: Part of various algorithms
- **Data transformation**: Prepare data for different views

### Reversing After Pushes

```javascript
// Build list by pushing (results in reverse order)
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("C")));
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("B")));
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("A")));
// List is now: [A, B, C] (pushed in reverse order)

// Or push in natural order and reverse
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("A")));
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("B")));
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("C")));
// List is: [C, B, A]

await client.query(WOQL.lib().rdflist_reverse(listHead));
// List is now: [A, B, C]
```

## Complete Example: Priority Reordering

```javascript
import { WOQLClient, WOQL } from "terminusdb";

async function priorityReorder(client) {
  // Create a task list: [Task1, Task2, Task3, Task4]
  const createQuery = WOQL.and(
    WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
    WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
    WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
    WOQL.idgen_random("terminusdb://data/Cons/", "v:cell4"),
    WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("Low Priority")),
    WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
    WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("Medium Priority")),
    WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
    WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("High Priority")),
    WOQL.add_triple("v:cell3", "rdf:rest", "v:cell4"),
    WOQL.add_triple("v:cell4", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:cell4", "rdf:first", WOQL.string("Urgent")),
    WOQL.add_triple("v:cell4", "rdf:rest", "rdf:nil")
  );
  
  const result = await client.query(createQuery);
  const listHead = result.bindings[0]["cell1"];
  
  console.log("Initial order:");
  await printList(client, listHead);
  // [Low Priority, Medium Priority, High Priority, Urgent]
  
  // Move Urgent to the front by swapping with position 0
  await client.query(WOQL.lib().rdflist_swap(listHead, 3, 0));
  
  console.log("After moving Urgent to front:");
  await printList(client, listHead);
  // [Urgent, Medium Priority, High Priority, Low Priority]
  
  // Reverse to show low priority first
  await client.query(WOQL.lib().rdflist_reverse(listHead));
  
  console.log("After reversing (low priority first):");
  await printList(client, listHead);
  // [Low Priority, High Priority, Medium Priority, Urgent]
}

async function printList(client, listHead) {
  const query = WOQL.lib().rdflist_list(listHead, "v:items");
  const result = await client.query(query);
  const items = result.bindings[0]["items"].map(i => i["@value"]);
  console.log(items);
}
```

## Performance Considerations

- **Swap** requires traversing to both positions, so swapping elements near the end is slower
  - If the values to swap are known and unique, it is faster to identify the Cons from from the values and swap (remove and add) the rdf:first triples from each cons.
- **Reverse** traverses the entire list once, making it O(n)
- For frequent reordering of large lists, consider using a different data structure

## Read More

- [RDF List Operations Overview](/docs/woql-rdflist-operations/) - Main guide
- [List Creation Operations](/docs/woql-rdflist-creation/) - Creating lists
- [List Access Operations](/docs/woql-rdflist-access/) - Reading list elements
- [List Modification Operations](/docs/woql-rdflist-modification/) - Adding and removing elements
- [Integration Tests](https://github.com/terminusdb/terminusdb-client-js/blob/main/integration_tests/woql_rdflist_operations.test.ts) - Complete test examples

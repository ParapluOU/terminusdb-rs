---
tags:
  - woql
  - reference
  - rdf
  - advanced
title: RDF List Modification Operations
nextjs:
  metadata:
    title: RDF List Modification Operations
    description: Learn how to add, remove, and modify elements in RDF lists using push, pop, append, insert, drop, and clear operations in WOQL.
    keywords: terminusdb, datalog, query language, rdf, rdf list modification operations, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-modification/
media: []
---

This guide covers operations for modifying RDF lists. These operations allow you to add, remove, and manage list elements.

Use the operators together for various scenarios. To operate a list like a stack, use the push and pop operators. To create a queue, use the append and pop operator (append to the end of the list is O(n) in performance and pop is o(1)).

## What You'll Learn

- How to add elements to the front or back of a list
- How to remove elements from a list
- How to insert elements at specific positions
- How to clear an entire list

## Operations Summary

| Operation | Description | Time Complexity |
|-----------|-------------|-----------------|
| `rdflist_push` | Add element to front (in-place) | O(1) |
| `rdflist_pop` | Remove element from front (in-place) | O(1) |
| `rdflist_append` | Add element to end | O(n) |
| `rdflist_insert` | Insert at specific position | O(n) |
| `rdflist_drop` | Remove element at position | O(n) |
| `rdflist_clear` | Remove all elements | O(n) |

## rdflist_push

Adds a new element to the front of a list. This is an **in-place** operation that modifies the list structure at the same head IRI. Useful for creating stacks or queues.

### Syntax

```javascript
WOQL.lib().rdflist_push(consSubject, value)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `value` | any | Value to add (use WOQL.string() for strings) |

### Example

```javascript
// Before: [A, B, C]
const pushQuery = WOQL.lib().rdflist_push(listHead, WOQL.string("Z"));
const result = await client.query(pushQuery);
// After: [Z, A, B, C]

// Verify the change
const peekQuery = WOQL.lib().rdflist_peek(listHead, "v:first");
const peekResult = await client.query(peekQuery);
console.log(peekResult.bindings[0]["first"]["@value"]); // "Z"
```

### Building Lists with Multiple Pushes

> **Note:** The `add_triple` calls in this guide are low-level RDF operations for constructing and modifying cons cells (`rdf:List`, `rdf:first`, `rdf:rest`). This is not the standard document API — use `insert_document` for document-oriented data.

```javascript
// Create a list by pushing elements (results in reverse order)
const createQuery = WOQL.and(
  WOQL.idgen_random("terminusdb://data/Cons/", "v:list"),
  WOQL.add_triple("v:list", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:list", "rdf:first", WOQL.string("First")),
  WOQL.add_triple("v:list", "rdf:rest", "rdf:nil")
);

const result = await client.query(createQuery);
const listHead = result.bindings[0]["list"];

// Push more elements
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("Second")));
await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("Third")));

// List is now: [Third, Second, First]
```

### Use Cases

- **Stack operations**: Implement LIFO (Last In, First Out) structures
- **Prepending**: Add items to the beginning efficiently
- **Building lists**: Create lists by pushing elements

## rdflist_pop

Removes and returns the first element from a list. This is an **in-place** operation, which means that the list (the Cons) reference from where it is referenced stays the same, such as from a document.

### Syntax

```javascript
WOQL.lib().rdflist_pop(consSubject, valueVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `valueVar` | string | Variable to bind the removed value |

### Example

```javascript
// Before: [X, Y, Z]
const popQuery = WOQL.lib().rdflist_pop(listHead, "v:popped");
const result = await client.query(popQuery);
// After: [Y, Z]

const popped = result.bindings[0]["popped"]["@value"];
console.log(`Removed: ${popped}`); // "X"

// Verify the list changed
const lengthQuery = WOQL.lib().rdflist_length(listHead, "v:len");
const lengthResult = await client.query(lengthQuery);
console.log(`Length: ${lengthResult.bindings[0]["len"]["@value"]}`); // "2"
```

### Use Cases

- **Stack operations**: Implement pop for LIFO structures
- **Queue processing**: Remove items from front
- **Destructuring**: Extract head and work with tail

## rdflist_append

Adds an element to the end of a list. Creates a new cons cell and links it to the end of the list. Useful for appending elements to a FIFO queue. 

### Syntax

```javascript
WOQL.lib().rdflist_append(consSubject, value, newCellVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `value` | any | Value to append |
| `newCellVar` | string | Variable to bind the new cell ID |

### Example

```javascript
// Before: [A, B, C]
const appendQuery = WOQL.lib().rdflist_append(
  listHead,
  WOQL.string("D"),
  "v:new_cell"
);
const result = await client.query(appendQuery);
// After: [A, B, C, D]

// The operation deletes the old nil reference and creates a new cell
console.log(`Inserts: ${result.inserts}`);
console.log(`Deletes: ${result.deletes}`);
```

### Use Cases

- **Queue operations**: Add items to the back (FIFO)
- **Building ordered lists**: Maintain insertion order
- **Extending lists**: Add items without changing existing order

## rdflist_insert

Inserts an element at a specific position in the list. Uses 0-based indexing.

### Syntax

```javascript
WOQL.lib().rdflist_insert(consSubject, position, value)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `position` | number | Index where to insert (0-based) |
| `value` | any | Value to insert |

### Examples

```javascript
// List: [A, B, C]

// Insert at position 0 (head): [X, A, B, C]
await client.query(
  WOQL.lib().rdflist_insert(listHead, 0, WOQL.string("X"))
);

// Insert at position 1 (after head): [A, Y, B, C]
await client.query(
  WOQL.lib().rdflist_insert(listHead, 1, WOQL.string("Y"))
);

// Insert at position 2 (middle): [A, B, Z, C]
await client.query(
  WOQL.lib().rdflist_insert(listHead, 2, WOQL.string("Z"))
);
```

### Verification Example

```javascript
// Create list: [A, B, C]
const createQuery = WOQL.and(
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
  WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("A")),
  WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
  WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("B")),
  WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
  WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("C")),
  WOQL.add_triple("v:cell3", "rdf:rest", "rdf:nil")
);

const result = await client.query(createQuery);
const listHead = result.bindings[0]["cell1"];

// Insert "X" at position 0
await client.query(
  WOQL.lib().rdflist_insert(listHead, 0, WOQL.string("X"))
);

// Verify: should be [X, A, B, C]
const readQuery = WOQL.lib().rdflist_member(listHead, "v:val");
const readResult = await client.query(readQuery);
const values = readResult.bindings.map(b => b["val"]["@value"]);
console.log(values); // ["X", "A", "B", "C"]
```

## rdflist_drop

Removes an element at a specific position from the list. Uses 0-based indexing. Remember to also delete subdocuments using `delete_document()` if needed by using `rdflist_slice()` to query them.

### Syntax

```javascript
WOQL.lib().rdflist_drop(consSubject, position)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `position` | number | Index of element to remove (0-based) |

### Examples

```javascript
// List: [A, B, C]

// Drop at position 0 (head): [B, C]
await client.query(WOQL.lib().rdflist_drop(listHead, 0));

// Drop at position 1 (middle): [A, C]
await client.query(WOQL.lib().rdflist_drop(listHead, 1));

// Drop at position 2 (last): [A, B]
await client.query(WOQL.lib().rdflist_drop(listHead, 2));
```

### Use Cases

- **Removal by index**: Delete specific items
- **Filtering**: Remove unwanted elements
- **Trimming**: Remove elements from specific positions

## rdflist_clear

Removes all elements from a list, deleting all cons cells and returning `rdf:nil`.

### Syntax

```javascript
WOQL.lib().rdflist_clear(consSubject, emptyListVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head identifier |
| `emptyListVar` | string | Variable to bind the empty list (rdf:nil) |

### Example

```javascript
// Clear a list
const clearQuery = WOQL.lib().rdflist_clear(listHead, "v:empty");
const result = await client.query(clearQuery);

const emptyList = result.bindings[0]["empty"];
console.log(emptyList); // "rdf:nil"
console.log(`Deleted ${result.deletes} triples`);

// Verify the old cons cells are gone
const verifyQuery = WOQL.triple(listHead, "v:pred", "v:obj");
const verifyResult = await client.query(verifyQuery);
console.log(verifyResult.bindings.length); // 0
```

### Important: Document References

When clearing a list that contains document references, **only the list structure is deleted**, not the referenced documents:

```javascript
// Create tasks
await client.addDocument([
  { "@type": "Task", "@id": "Task/1", title: "Task 1" },
  { "@type": "Task", "@id": "Task/2", title: "Task 2" }
]);

// Build list of task references
// ... (list contains Task/1 and Task/2)

// Clear the list
await client.query(WOQL.lib().rdflist_clear(listHead, "v:empty"));

// Tasks still exist!
const task1 = await client.getDocument({ id: "Task/1" });
console.log(task1.title); // "Task 1" - still there
```

### Pattern: Delete All Referenced Documents and Clear List

```javascript
const deleteAndClear = WOQL.and(
  // Get the list head
  WOQL.triple("Project/1", "tasks", "v:listHead"),
  
  // Delete all documents in the list
  WOQL.group_by(
    [],
    "v:task",
    "v:tasks",
    WOQL.and(
      WOQL.lib().rdflist_member("v:listHead", "v:task"),
      WOQL.delete_document("v:task")
    )
  ),
  
  // Clear the list structure
  WOQL.lib().rdflist_clear("v:listHead", "v:empty"),
  
  // Update the parent reference to point to empty list
  WOQL.delete_triple("Project/1", "tasks", "v:listHead"),
  WOQL.add_triple("Project/1", "tasks", "v:empty")
);

await client.query(deleteAndClear);
```

## Complete Example: Task Queue

```javascript
import { WOQLClient, WOQL } from "terminusdb";

async function taskQueueDemo(client) {
  // Create initial queue with one task
  const createQueue = WOQL.and(
    WOQL.idgen_random("terminusdb://data/Cons/", "v:queue"),
    WOQL.add_triple("v:queue", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:queue", "rdf:first", WOQL.string("Initial Task")),
    WOQL.add_triple("v:queue", "rdf:rest", "rdf:nil")
  );
  
  const result = await client.query(createQueue);
  const queue = result.bindings[0]["queue"];
  
  // Add tasks to the back (queue behavior)
  await client.query(
    WOQL.lib().rdflist_append(queue, WOQL.string("Task 2"), "v:cell")
  );
  await client.query(
    WOQL.lib().rdflist_append(queue, WOQL.string("Task 3"), "v:cell")
  );
  
  console.log("Queue after appends:");
  const allTasks = await client.query(
    WOQL.lib().rdflist_list(queue, "v:tasks")
  );
  console.log(allTasks.bindings[0]["tasks"].map(t => t["@value"]));
  // ["Initial Task", "Task 2", "Task 3"]
  
  // Process (pop) from the front
  const popResult = await client.query(
    WOQL.lib().rdflist_pop(queue, "v:task")
  );
  console.log(`Processing: ${popResult.bindings[0]["task"]["@value"]}`);
  // "Initial Task"
  
  // Insert high-priority task at position 0
  await client.query(
    WOQL.lib().rdflist_insert(queue, 0, WOQL.string("URGENT"))
  );
  
  console.log("Queue after urgent insert:");
  const finalTasks = await client.query(
    WOQL.lib().rdflist_list(queue, "v:tasks")
  );
  console.log(finalTasks.bindings[0]["tasks"].map(t => t["@value"]));
  // ["URGENT", "Task 2", "Task 3"]
  
  // Clear the queue when done
  await client.query(WOQL.lib().rdflist_clear(queue, "v:empty"));
  console.log("Queue cleared");
}
```

## Read More

- [RDF List Operations Overview](/docs/woql-rdflist-operations/) - Main guide
- [List Creation Operations](/docs/woql-rdflist-creation/) - Creating lists
- [List Access Operations](/docs/woql-rdflist-access/) - Reading list elements
- [List Transformation Operations](/docs/woql-rdflist-transformation/) - Reordering lists
- [Integration Tests](https://github.com/terminusdb/terminusdb-client-js/blob/main/integration_tests/woql_rdflist_operations.test.ts) - Complete test examples

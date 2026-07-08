---
tags:
  - woql
  - reference
  - rdf
  - intermediate
title: WOQL RDF List Operations
nextjs:
  metadata:
    title: WOQL RDF List Operations
    description: Comprehensive guide to working with rdf:List structures in TerminusDB using WOQL library functions for list manipulation.
    keywords: terminusdb, datalog, query language, rdf, terminusdb query, woql, woql rdf list operations
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-operations/
media: []
---

RDF Lists (`rdf:List`) are linked list data structures in RDF that use `rdf:first`, `rdf:rest`, and `rdf:nil` predicates. TerminusDB provides a comprehensive WOQL library for manipulating these structures efficiently.

## What You'll Learn

By the end of this guide, you'll understand:
- How RDF lists are structured internally
- How to create, read, and modify RDF lists using WOQL
- Best practices for list operations in TerminusDB

## Understanding RDF List Structure

An RDF list is a chain of "cons cells" where each cell contains:
- `rdf:first` → the value at this position
- `rdf:rest` → pointer to the next cell (or `rdf:nil` for the end)

```text
List: ["First", "Second", "Third"]

Structure:
Cell1 → rdf:type = rdf:List
     → rdf:first = "First"
     → rdf:rest = Cell2

Cell2 → rdf:type = rdf:List
     → rdf:first = "Second"
     → rdf:rest = Cell3

Cell3 → rdf:type = rdf:List
     → rdf:first = "Third"
     → rdf:rest = rdf:nil
```

## Operations Overview

| Category | Operations | Description |
|----------|------------|-------------|
| **[Creation](/docs/woql-rdflist-creation/)** | [`rdflist_empty`](/docs/woql-rdflist-creation/#rdflist_empty), [`rdflist_is_empty`](/docs/woql-rdflist-creation/#rdflist_is_empty) | Create and check empty lists |
| **[Access](/docs/woql-rdflist-access/)** | [`rdflist_peek`](/docs/woql-rdflist-access/#rdflist_peek), [`rdflist_member`](/docs/woql-rdflist-access/#rdflist_member), [`rdflist_length`](/docs/woql-rdflist-access/#rdflist_length), [`rdflist_list`](/docs/woql-rdflist-access/#rdflist_list), [`rdflist_slice`](/docs/woql-rdflist-access/#rdflist_slice) | Read list elements |
| **[Modification](/docs/woql-rdflist-modification/)** | [`rdflist_push`](/docs/woql-rdflist-modification/#rdflist_push), [`rdflist_pop`](/docs/woql-rdflist-modification/#rdflist_pop), [`rdflist_append`](/docs/woql-rdflist-modification/#rdflist_append), [`rdflist_insert`](/docs/woql-rdflist-modification/#rdflist_insert), [`rdflist_drop`](/docs/woql-rdflist-modification/#rdflist_drop), [`rdflist_clear`](/docs/woql-rdflist-modification/#rdflist_clear) | Add, remove, and modify elements |
| **[Transformation](/docs/woql-rdflist-transformation/)** | [`rdflist_swap`](/docs/woql-rdflist-transformation/#rdflist_swap), [`rdflist_reverse`](/docs/woql-rdflist-transformation/#rdflist_reverse) | Reorder list elements |

## Quick Start Example

Here's a complete example demonstrating common list operations:

```javascript
import { WOQLClient, WOQL } from "terminusdb";

const client = new WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root"
});

// Create a list manually, independent of documents
// Note: add_triple is a low-level RDF operation for constructing cons cells.
// Use insert_document for document-oriented data.
const createList = WOQL.and(
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
  
  WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("Task A")),
  WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
  
  WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("Task B")),
  WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
  
  WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("Task C")),
  WOQL.add_triple("v:cell3", "rdf:rest", "rdf:nil")
);

const result = await client.query(createList);
const listHead = result.bindings[0]["cell1"];

// Read all values
const readAll = WOQL.lib().rdflist_member(listHead, "v:value");
const values = await client.query(readAll);
// Returns: ["Task A", "Task B", "Task C"]

// Get list length
const getLength = WOQL.lib().rdflist_length(listHead, "v:count");
const lengthResult = await client.query(getLength);
// Returns: 3
```

## Detailed Guides

Explore each category of operations in detail:

### [List Creation](/docs/woql-rdflist-creation/)
Learn how to create empty lists and check if lists are empty.

### [List Access](/docs/woql-rdflist-access/)
Learn how to read elements from lists using peek, member, length, list, and slice operations.

### [List Modification](/docs/woql-rdflist-modification/)
Learn how to add, remove, and modify list elements with push, pop, append, insert, drop, and clear.

### [List Transformation](/docs/woql-rdflist-transformation/)
Learn how to reorder lists with swap and reverse operations.

## Performance Characteristics

| Operation | Time Complexity | Description |
|-----------|-----------------|-------------|
| `rdflist_peek` | O(1) | Direct access to first element |
| `rdflist_push` | O(1) | Add to front (in-place modification) |
| `rdflist_pop` | O(1) | Remove from front (in-place modification) |
| `rdflist_member` | O(n) | Single traversal |
| `rdflist_length` | O(n) | Single traversal with count |
| `rdflist_append` | O(n) | Must traverse to find end |
| `rdflist_insert` | O(n) | Traverse to position |
| `rdflist_drop` | O(n) | Traverse to position |
| `rdflist_clear` | O(n) | Delete all cells |
| `rdflist_reverse` | O(n) | In-place reversal |
| `rdflist_swap` | O(n) | Traverse to both positions |

## Best Practices

### Use Library Functions
Prefer library functions over manual triple manipulation:

```javascript
// Good - use library functions to ensure accurate operations
WOQL.lib().rdflist_push(listHead, WOQL.string("New Item"))

// Avoid manual manipulation as ensuring accuracy is hard
WOQL.and(
  WOQL.idgen_random(...),
  WOQL.add_triple(...),
  // ... many lines
)
```

### Descriptive Variable Names
Use descriptive variable names with the `v:` prefix, or use the `WOQL.vars()` function:

```javascript
// Good
WOQL.lib().rdflist_member("List/tasks", "v:task_title")

// Less clear
WOQL.lib().rdflist_member("List/tasks", "v:x")
```

### Error Handling
Always check results for empty bindings:

```javascript
const result = await client.query(query);
if (!result?.bindings || result.bindings.length === 0) {
  // Handle empty result
}
```

### Atomic Operations
Group related list operations in a single query for atomicity for ACID transactions:

```javascript
// Atomic operation - both succeed or both fail
WOQL.and(
  WOQL.lib().rdflist_pop("List/queue", "v:item"),
  WOQL.lib().rdflist_append("List/processed", "v:item", "v:new_cell")
)
```

## Working with Document References

Lists can contain references to documents:

```javascript
// Schema with list of document references
const schema = {
  "@type": "Class",
  "@id": "Playlist",
  "@key": { "@type": "Random" },
  name: "xsd:string",
  songs: { "@type": "List", "@class": "Song" }
};

// Read songs with their details
const songsQuery = WOQL.and(
  WOQL.lib().rdflist_member(playlistHead, "v:song_id"),
  WOQL.triple("v:song_id", "title", "v:title"),
  WOQL.triple("v:song_id", "artist", "v:artist")
);
```

## Read More

- [Integration Tests](https://github.com/terminusdb/terminusdb-client-js/blob/main/integration_tests/woql_rdflist_operations.test.ts) - Complete test examples
- [RDF List Vocabulary](https://www.w3.org/TR/rdf-schema/#ch_list) - W3C RDF Schema specification
- [WOQL Basics](/docs/woql-basics/) - WOQL fundamentals
- [Query Arrays and Sets](/docs/query-arrays-and-sets-in-woql/) - Working with other collection types
- [Path Queries](/docs/path-queries-in-woql/) - Advanced traversal patterns

---
tags:
  - woql
  - reference
  - rdf
  - advanced
title: RDF List Creation Operations
nextjs:
  metadata:
    title: RDF List Creation Operations
    description: Learn how to create empty RDF lists and check if lists are empty using WOQL library functions in TerminusDB.
    keywords: terminusdb, datalog, query language, rdf, rdf list creation operations, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-creation/
media: []
---

This guide covers operations for creating RDF lists and checking their state. These are fundamental operations you'll use when initializing lists or validating list contents.

## What You'll Learn

- How to create an empty RDF list
- How to check if a list is empty
- How to build lists manually using triples

## Operations Summary

| Operation | Description | Returns |
|-----------|-------------|---------|
| `rdflist_empty(listVar)` | Create or bind to an empty list | `rdf:nil` |
| `rdflist_is_empty(consSubject)` | Check if a list is empty | Succeeds if empty |

## rdflist_empty

Creates or binds a variable to an empty list (`rdf:nil`).

### Syntax

```javascript
WOQL.lib().rdflist_empty(listVar)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `listVar` | string | Variable to bind to the empty list |

### Example

```javascript
// Create an empty list
const query = WOQL.lib().rdflist_empty("v:empty_list");
const result = await client.query(query);

const emptyList = result.bindings[0]["empty_list"];
console.log(emptyList); // "rdf:nil"
```

### Use Cases

- **Initialization**: Set up a default empty list value
- **Default values**: Provide empty lists when no data exists
- **Conditional logic**: Use as a base case for list building

## rdflist_is_empty

Checks if a list is empty. The query succeeds (returns bindings) if the list is empty, and fails (returns no bindings) if the list has elements.

### Syntax

```javascript
WOQL.lib().rdflist_is_empty(consSubject)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `consSubject` | string | The list head to check |

### Example

```javascript
// Check if a list is empty
const checkQuery = WOQL.lib().rdflist_is_empty("rdf:nil");
const checkResult = await client.query(checkQuery);

if (checkResult.bindings && checkResult.bindings.length > 0) {
  console.log("List is empty");
} else {
  console.log("List has elements");
}
```

### Use Cases

- **Validation**: Verify list state before operations
- **Conditional logic**: Branch based on list contents
- **Guard clauses**: Prevent operations on empty lists

## Building Lists Manually

For complete control, you can build RDF lists using triples directly. This is useful for understanding the underlying structure.

> **Note:** The `add_triple` calls below are a low-level RDF operation for constructing cons cells (`rdf:List`, `rdf:first`, `rdf:rest`). This is not the standard document API — use `insert_document` for document-oriented data.

### Creating a Three-Element List

```javascript
// List: ["First", "Second", "Third"]
const createList = WOQL.and(
  // Generate random IDs for cons cells
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
  
  // Cell 1: first element
  WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("First")),
  WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
  
  // Cell 2: second element
  WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("Second")),
  WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
  
  // Cell 3: third element (points to nil)
  WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("Third")),
  WOQL.add_triple("v:cell3", "rdf:rest", "rdf:nil")
);

const result = await client.query(createList);
const listHead = result.bindings[0]["cell1"];
```

### Creating a Single-Element List

```javascript
const createSingleElement = WOQL.and(
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell"),
  WOQL.add_triple("v:cell", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell", "rdf:first", WOQL.string("Only Item")),
  WOQL.add_triple("v:cell", "rdf:rest", "rdf:nil")
);
```

## Complete Example: Initialize and Validate

```javascript
import { WOQLClient, WOQL } from "terminusdb";

const client = new WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root"
});

async function initializeAndValidate() {
  // Create an empty list
  const emptyQuery = WOQL.lib().rdflist_empty("v:my_list");
  const emptyResult = await client.query(emptyQuery);
  const myList = emptyResult.bindings[0]["my_list"];
  
  console.log("Created list:", myList); // "rdf:nil"
  
  // Verify it's empty
  const checkQuery = WOQL.lib().rdflist_is_empty(myList);
  const checkResult = await client.query(checkQuery);
  
  if (checkResult.bindings?.length > 0) {
    console.log("✓ List is correctly empty");
  }
  
  // Now create a list with elements
  const createQuery = WOQL.and(
    WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
    WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
    WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("Item")),
    WOQL.add_triple("v:cell1", "rdf:rest", "rdf:nil")
  );
  
  const createResult = await client.query(createQuery);
  const nonEmptyList = createResult.bindings[0]["cell1"];
  
  // Verify it's NOT empty
  const checkNonEmpty = WOQL.lib().rdflist_is_empty(nonEmptyList);
  const nonEmptyResult = await client.query(checkNonEmpty);
  
  if (!nonEmptyResult.bindings?.length) {
    console.log("✓ List correctly has elements");
  }
}

initializeAndValidate();
```

## Integration with Document Schema

Lists can be defined in your schema and populated automatically:

```javascript
// Schema definition
const schema = [
  {
    "@type": "Class",
    "@id": "TodoList",
    "@key": { "@type": "Random" },
    name: "xsd:string",
    items: { "@type": "List", "@class": "xsd:string" }
  }
];

await client.addDocument(schema, { graph_type: "schema" });

// Create a document with an empty list
const emptyTodoList = {
  "@type": "TodoList",
  name: "My Tasks",
  items: []  // Empty list in JSON becomes rdf:nil
};

await client.addDocument(emptyTodoList);

// Create a document with items
const populatedTodoList = {
  "@type": "TodoList",
  name: "Shopping",
  items: ["Milk", "Bread", "Eggs"]
};

await client.addDocument(populatedTodoList);
```

## Read More

- [RDF List Operations Overview](/docs/woql-rdflist-operations/) - Main guide
- [List Access Operations](/docs/woql-rdflist-access/) - Reading list elements
- [List Modification Operations](/docs/woql-rdflist-modification/) - Adding and removing elements
- [Integration Tests](https://github.com/terminusdb/terminusdb-client-js/blob/main/integration_tests/woql_rdflist_operations.test.ts) - Complete test examples

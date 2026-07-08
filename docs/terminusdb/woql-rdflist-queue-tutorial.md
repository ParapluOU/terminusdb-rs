---
tags:
  - woql
  - tutorial
  - rdf
  - advanced
title: RDF List Queue Tutorial
nextjs:
  metadata:
    title: RDF List Queue Tutorial
    description: Build a task queue using RDF list operations in TerminusDB with step-by-step examples.
    keywords: terminusdb, datalog, query language, rdf, rdf list queue tutorial, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/woql-rdflist-queue-tutorial/
---

This tutorial demonstrates how to build a task queue using WOQL RDF list operations. You'll learn how to create, modify, and query ordered lists stored as document properties.

With the built in plain JSON support (sys:JSONDocument and sys:JSON data structure) and these list operators, queue operations can be performed directly on JSON lists created using the JSON documents interface using advanced declarative logic on the server side with ACID transactions.

## Prerequisites

- Node.js 16+
- TerminusDB running on `localhost:6363`
- `terminusdb` installed

## Document-Based Task Queue

This example creates a `TaskQueue` document with an ordered list of tasks, then demonstrates all the list operations.

### 1. Install TerminusDB (or use cloud with token)
Get TerminusDB running on `localhost:6363` ([see Quickstart Guide](/docs/get-started/))

To just start it:

```bash
docker run --pull always -d -p 127.0.0.1:6363:6363 -v terminusdb_storage:/app/terminusdb/storage --name terminusdb terminusdb/terminusdb-server:v12

```

### 2. Install javascript TerminusDB client

```bash
npm install terminusdb
```

### 3. Create the script, including Setup and Schema

Combine all the sections together, or run it step by step.

```javascript
import { WOQLClient, WOQL } from "terminusdb";

const client = new WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root",
});

// Create database
await client.createDatabase("task_queue_demo", {
  label: "Task Queue Demo",
  comment: "Demonstrates rdflist operations with documents",
});

client.db("task_queue_demo");

// Define schema with a TaskQueue document containing a List
const schema = [
  {
    "@type": "Class",
    "@id": "TaskQueue",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "tasks": { "@type": "List", "@class": "xsd:string" }
  }
];

await client.addDocument(schema, { graph_type: "schema" });
```

### Create Initial Queue

```javascript
// Create a task queue document with initial tasks
const taskQueue = {
  "@type": "TaskQueue",
  "name": "MyQueue",
  "tasks": ["Setup environment", "Write tests", "Deploy"]
};

await client.addDocument(taskQueue);

// Get the list head from the document for list operations
const getListHead = WOQL.triple("TaskQueue/MyQueue", "tasks", "v:listHead");
const result = await client.query(getListHead);
const listHead = result.bindings[0]["listHead"];
console.log("List head:", listHead);
```

### Read Operations

```javascript
// Peek at the first task (without removing)
const peekResult = await client.query(
  WOQL.lib().rdflist_peek(listHead, "v:first")
);
console.log("First task:", peekResult.bindings[0]["first"]["@value"]);
// Output: "Setup environment"

// Get the queue length
const lengthResult = await client.query(
  WOQL.lib().rdflist_length(listHead, "v:len")
);
console.log("Queue length:", lengthResult.bindings[0]["len"]["@value"]);
// Output: 3

// Get all tasks as an array
const listResult = await client.query(
  WOQL.lib().rdflist_list(listHead, "v:tasks")
);
const tasks = listResult.bindings[0]["tasks"].map(t => t["@value"]);
console.log("All tasks:", tasks);
// Output: ["Setup environment", "Write tests", "Deploy"]

// Iterate through tasks with member
const memberResult = await client.query(
  WOQL.lib().rdflist_member(listHead, "v:task")
);
memberResult.bindings.forEach((b, i) => {
  console.log(`Task ${i}: ${b["task"]["@value"]}`);
});

// Get a slice (positions 0-2, exclusive end)
const sliceResult = await client.query(
  WOQL.lib().rdflist_slice(listHead, 0, 2, "v:slice")
);
const slice = sliceResult.bindings[0]["slice"].map(t => t["@value"]);
console.log("First two tasks:", slice);
// Output: ["Setup environment", "Write tests"]
```

### Add Tasks

```javascript
// Push a task to the front (high priority)
await client.query(
  WOQL.lib().rdflist_push(listHead, WOQL.string("URGENT: Fix bug"))
);

// Append a task to the end
await client.query(
  WOQL.lib().rdflist_append(listHead, WOQL.string("Cleanup"), "v:newCell")
);

// Insert at a specific position (index 2)
await client.query(
  WOQL.lib().rdflist_insert(listHead, 2, WOQL.string("Code review"))
);

// Verify the queue
const updatedResult = await client.query(
  WOQL.lib().rdflist_list(listHead, "v:tasks")
);
console.log(updatedResult.bindings[0]["tasks"].map(t => t["@value"]));
// Output: ["URGENT: Fix bug", "Setup environment", "Code review", "Write tests", "Deploy", "Cleanup"]
```

### Process Tasks

```javascript
// Pop the first task (removes and returns it)
const popResult = await client.query(
  WOQL.lib().rdflist_pop(listHead, "v:task")
);
console.log("Processing:", popResult.bindings[0]["task"]["@value"]);
// Output: "URGENT: Fix bug"

// Drop task at position 1 (removes without returning)
await client.query(
  WOQL.lib().rdflist_drop(listHead, 1)
);

// Check remaining tasks
const afterOps = await client.query(
  WOQL.lib().rdflist_list(listHead, "v:tasks")
);
console.log(afterOps.bindings[0]["tasks"].map(t => t["@value"]));
// Output: ["Setup environment", "Write tests", "Deploy", "Cleanup"]
```

### Reorder Tasks

```javascript
// Swap positions: move "Deploy" (index 2) to front (index 0)
await client.query(
  WOQL.lib().rdflist_swap(listHead, 2, 0)
);

// Reverse the entire queue
await client.query(
  WOQL.lib().rdflist_reverse(listHead)
);

// Verify order
const reorderedResult = await client.query(
  WOQL.lib().rdflist_list(listHead, "v:tasks")
);
console.log(reorderedResult.bindings[0]["tasks"].map(t => t["@value"]));
```

### Clear the Queue

```javascript
// Clear all tasks from the queue
await client.query(
  WOQL.lib().rdflist_clear(listHead, "v:empty")
);

// Verify the queue is empty
const isEmptyResult = await client.query(
  WOQL.lib().rdflist_is_empty(listHead)
);
const isEmpty = isEmptyResult.bindings && isEmptyResult.bindings.length > 0;
console.log("Queue is empty:", isEmpty);
// Output: true
```

### Verify Changes in Document

The list operations modify the underlying RDF structure. The document API reflects these changes:

```javascript
const updatedDoc = await client.getDocument({ id: "TaskQueue/MyQueue" });
console.log(updatedDoc.tasks);
// Shows the current state of the tasks list
```

---

## Understanding RDF List Internals with Cons Cells

RDF lists are implemented as linked cons cells. Each cell has:
- `rdf:first` - the value at this position
- `rdf:rest` - pointer to the next cell (or `rdf:nil` for end)

This example shows how to work directly with the cons cell structure.

> **Note:** The `add_triple` calls below are a low-level RDF operation for constructing cons cells (`rdf:List`, `rdf:first`, `rdf:rest`). This is not the standard document API — use `insert_document` for document-oriented data.

### Create a List Manually

```javascript
// Create a list [A, B, C] using cons cells directly
const createList = WOQL.and(
  // Generate unique IDs for each cell
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell1"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell2"),
  WOQL.idgen_random("terminusdb://data/Cons/", "v:cell3"),
  
  // Cell 1: first="A", rest=cell2
  WOQL.add_triple("v:cell1", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell1", "rdf:first", WOQL.string("A")),
  WOQL.add_triple("v:cell1", "rdf:rest", "v:cell2"),
  
  // Cell 2: first="B", rest=cell3
  WOQL.add_triple("v:cell2", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell2", "rdf:first", WOQL.string("B")),
  WOQL.add_triple("v:cell2", "rdf:rest", "v:cell3"),
  
  // Cell 3: first="C", rest=nil (end of list)
  WOQL.add_triple("v:cell3", "rdf:type", "rdf:List"),
  WOQL.add_triple("v:cell3", "rdf:first", WOQL.string("C")),
  WOQL.add_triple("v:cell3", "rdf:rest", "rdf:nil")
);

const result = await client.query(createList);
const listHead = result.bindings[0]["cell1"];
```

### Visualizing the Structure

```
listHead (cell1)
    ├── rdf:type  → rdf:List
    ├── rdf:first → "A"
    └── rdf:rest  → cell2
                      ├── rdf:type  → rdf:List
                      ├── rdf:first → "B"
                      └── rdf:rest  → cell3
                                        ├── rdf:type  → rdf:List
                                        ├── rdf:first → "C"
                                        └── rdf:rest  → rdf:nil
```

### Query the Structure Directly

```javascript
// Traverse the list manually using path queries
const traverseQuery = WOQL.and(
  WOQL.triple(listHead, "rdf:first", "v:first"),
  WOQL.triple(listHead, "rdf:rest", "v:second_cell"),
  WOQL.triple("v:second_cell", "rdf:first", "v:second"),
  WOQL.triple("v:second_cell", "rdf:rest", "v:third_cell"),
  WOQL.triple("v:third_cell", "rdf:first", "v:third")
);

const traverseResult = await client.query(traverseQuery);
const { first, second, third } = traverseResult.bindings[0];
console.log([first["@value"], second["@value"], third["@value"]]);
// Output: ["A", "B", "C"]
```

### Create an Empty List

```javascript
// An empty list is just a reference to rdf:nil
const createEmpty = WOQL.lib().rdflist_empty("v:emptyList");
const emptyResult = await client.query(createEmpty);
console.log(emptyResult.bindings[0]["emptyList"]);
// Output: "rdf:nil"

// Check if a list is empty
const checkEmpty = WOQL.lib().rdflist_is_empty("rdf:nil");
const isEmptyCheck = await client.query(checkEmpty);
console.log("Is empty:", isEmptyCheck.bindings.length > 0);
// Output: true
```

### Why Use Library Functions?

The `WOQL.lib().rdflist_*` functions handle:
- Automatic cons cell creation with unique IDs
- Proper linking of `rdf:rest` pointers
- Cleanup of removed cells
- Edge cases (empty lists, single elements)

Manual cons cell manipulation is error-prone. Use the library functions for reliability.

---

## Complete Runnable Example

Save this as `task-queue-demo.js`:

```javascript
const TerminusClient = require("terminusdb");
const WOQL = TerminusClient.WOQL;

async function main() {
  const client = new TerminusClient.WOQLClient("http://localhost:6363", {
    user: "admin",
    organization: "admin",
    key: "root",
  });

  // Setup database
  try { await client.deleteDatabase("task_queue_demo"); } catch (e) {}
  await client.createDatabase("task_queue_demo", {
    label: "Task Queue Demo",
    comment: "RDF list operations demo",
  });
  client.db("task_queue_demo");

  // Create schema
  await client.addDocument([{
    "@type": "Class",
    "@id": "TaskQueue",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "tasks": { "@type": "List", "@class": "xsd:string" }
  }], { graph_type: "schema" });

  // Create document with initial tasks
  await client.addDocument({
    "@type": "TaskQueue",
    "name": "MyQueue",
    "tasks": ["Task 1", "Task 2", "Task 3"]
  });

  // Get list head
  const headResult = await client.query(
    WOQL.triple("TaskQueue/MyQueue", "tasks", "v:listHead")
  );
  const listHead = headResult.bindings[0]["listHead"];
  console.log("List head:", listHead);

  // Demonstrate operations
  console.log("\n--- Initial State ---");
  let tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Push to front ---");
  await client.query(WOQL.lib().rdflist_push(listHead, WOQL.string("Urgent")));
  tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Append to end ---");
  await client.query(WOQL.lib().rdflist_append(listHead, WOQL.string("Final"), "v:c"));
  tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Pop from front ---");
  const pop = await client.query(WOQL.lib().rdflist_pop(listHead, "v:t"));
  console.log("Popped:", pop.bindings[0]["t"]["@value"]);
  tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Swap positions 0 and 2 ---");
  await client.query(WOQL.lib().rdflist_swap(listHead, 0, 2));
  tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Reverse ---");
  await client.query(WOQL.lib().rdflist_reverse(listHead));
  tasks = await getTaskList(client, listHead);
  console.log("Tasks:", tasks);

  console.log("\n--- Document reflects changes ---");
  const doc = await client.getDocument({ id: "TaskQueue/MyQueue" });
  console.log("Document tasks:", doc.tasks);
}

async function getTaskList(client, listHead) {
  const result = await client.query(WOQL.lib().rdflist_list(listHead, "v:t"));
  return result.bindings[0]["t"].map(t => t["@value"]);
}

main().catch(console.error);
```

Run with:
```bash
node task-queue-demo.js
```

## Read More

- [RDF List Operations Overview](/docs/woql-rdflist-operations/) - All operations reference
- [List Creation](/docs/woql-rdflist-creation/) - Creating and checking empty lists
- [List Access](/docs/woql-rdflist-access/) - Reading list elements
- [List Modification](/docs/woql-rdflist-modification/) - Adding and removing elements
- [List Transformation](/docs/woql-rdflist-transformation/) - Reordering lists
- [W3C RDF Schema - rdf:List](https://www.w3.org/TR/rdf-schema/#ch_list) - RDF list specification

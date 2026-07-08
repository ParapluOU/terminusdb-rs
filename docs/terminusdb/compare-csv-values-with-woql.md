---
tags:
  - woql
  - how-to
  - data-import
  - advanced
title: How to compare values with a CSV file using WOQL
nextjs:
  metadata:
    title: How to compare values with a CSV file using WOQL
    description: Learn how to efficiently compare a list of values against a CSV file to find additions, deletions, and unchanged items using WOQL
    keywords: terminusdb, compare values with a csv file using woql, datalog, query language, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/compare-csv-values-with-woql/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with a schema
- A CSV file to compare against
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to compare CSV data with database values using WOQL.
{% /callout %}

{% prerequisites-clone /%}

This guide shows you how to compare a list of values against a CSV file using WOQL. This is useful for synchronization tasks where you need to determine what has been added, deleted, or remains unchanged between two data sources.

## The Problem

When synchronizing data, you often need to compare:
- A list of values you have (e.g., current employee IDs)
- A CSV file with reference data (e.g., an HR export)

You want to categorize each value as:
- **to_add**: Values in your list that are not in the CSV
- **to_delete**: Values in the CSV that are not in your list  
- **no_change**: Values that exist in both

## The Solution: Using Set Operations

The most efficient approach uses WOQL's native set operations (`set_difference`, `set_intersection`, `set_union`) which provide O(n log n) performance. These operations can handle 100,000+ elements in under a second.

> **See also:** [Integration tests for these patterns](https://github.com/terminusdb/terminusdb/blob/main/tests/test/woql-csv-comparison.js)

### Example CSV File

Create a file called `employee-list.csv`:

```csv
EmployeeId
E001
E002
E003
E005
E007
```

### Complete WOQL Query with Set Operations

```javascript
WOQL.and(
  // Step 1: Define your source list using group_by
  WOQL.group_by(
    [],
    ["source_val"],
    "v:source_list",
    WOQL.or(
      WOQL.eq("v:source_val", WOQL.literal("E001", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E002", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E004", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E006", "xsd:string")),
    )
  ),
  
  // Step 2: Read CSV once into a list variable
  WOQL.group_by(
    [],
    ["csv_val"],
    "v:csv_list",
    WOQL.get(WOQL.as("EmployeeId", "v:csv_val"))
        .post("employee-list.csv", { type: "csv" })
  ),
  
  // Step 3: Convert lists to sets (sorts and removes duplicates)
  WOQL.list_to_set("v:source_list", "v:source_set"),
  WOQL.list_to_set("v:csv_list", "v:csv_set"),
  
  // Step 4: Compute set differences and intersection
  WOQL.set_difference("v:source_set", "v:csv_set", "v:to_add"),      // In source but not CSV
  WOQL.set_difference("v:csv_set", "v:source_set", "v:to_delete"),   // In CSV but not source
  WOQL.set_intersection("v:source_set", "v:csv_set", "v:no_change"), // In both
)
```

### Expected Results

With the source list `[E001, E002, E004, E006]` and CSV containing `[E001, E002, E003, E005, E007]`:

```json
{
  "to_add": ["E004", "E006"],
  "to_delete": ["E003", "E005", "E007"],
  "no_change": ["E001", "E002"]
}
```

### How Set Operations Work

| Operation | Description | Complexity |
|-----------|-------------|------------|
| `list_to_set` | Sorts list and removes duplicates | O(n log n) |
| `set_difference(A, B, Result)` | Elements in A but not in B | O(n log n) |
| `set_intersection(A, B, Result)` | Elements in both A and B | O(n log n) |
| `set_union(A, B, Result)` | All unique elements from A and B | O(n log n) |
| `set_member(Element, Set)` | Check if element is in set | O(log n) |

### Performance Benchmarks (for only the set operation itself)

| Elements | set_difference | set_intersection |
|----------|----------------|------------------|
| 1,000    | 7ms            | 5ms              |
| 5,000    | 30ms           | 17ms             |
| 10,000   | 64ms           | 66ms             |
| 100,000  | 577ms          | 445ms            |

---

## Comparing Database Content with CSV (Large Datasets)

When comparing large databases against CSV files, the key is to **stream** through one dataset while using efficient lookups against the other. Never load both datasets entirely into memory.

### Strategy: Stream the Larger Dataset, Index the Smaller

| Scenario | Stream through | Index/Set |
|----------|---------------|-----------|
| Large DB, small CSV | Database triples | CSV as set |
| Small DB, large CSV | CSV rows | Database index |

### Finding Database Records Not in CSV

When you have a large database and a smaller CSV file, load the CSV into a set and stream through database triples:

```javascript
WOQL.and(
  // Step 1: Load CSV into a set (small, fits in memory)
  WOQL.group_by(
    [],
    ["csv_val"],
    "v:csv_list",
    WOQL.get(WOQL.as("EmployeeId", "v:csv_val"))
        .post("employee-list.csv", { type: "csv" })
  ),
  WOQL.list_to_set("v:csv_list", "v:csv_set"),
  
  // Step 2: Stream through database records via backtracking
  WOQL.triple("v:Employee", "rdf:type", "@schema:Employee"),
  WOQL.triple("v:Employee", "@schema:employee_id", "v:db_id"),
  
  // Step 3: For each DB record, check if NOT in CSV set - O(log n)
  WOQL.not(WOQL.set_member("v:db_id", "v:csv_set")),
)
```

This streams through database triples one at a time via backtracking. Each `set_member` check is O(log n), making this efficient even with millions of database records.

### Finding CSV Values Not in Database

When checking CSV values against a large database, stream through CSV rows and use indexed triple lookups:

```javascript
WOQL.and(
  // Step 1: Stream CSV rows via backtracking
  WOQL.get(WOQL.as("EmployeeId", "v:csv_id"))
      .post("employee-list.csv", { type: "csv" }),
  
  // Step 2: For each CSV row, check against database index
  WOQL.not(
    WOQL.triple("v:AnyEmployee", "@schema:employee_id", "v:csv_id")
  ),
)
```

The triple lookup uses database indexes, so each check is O(log n). This processes one CSV row at a time without loading the entire database.

### Complete Streaming Comparison

To get both directions (what's in DB but not CSV, and what's in CSV but not DB) efficiently:

```javascript
// Query 1: Database records not in CSV
WOQL.and(
  WOQL.group_by([], ["csv_val"], "v:csv_list",
    WOQL.get(WOQL.as("EmployeeId", "v:csv_val"))
        .post("employee-list.csv", { type: "csv" })
  ),
  WOQL.list_to_set("v:csv_list", "v:csv_set"),
  WOQL.triple("v:Employee", "rdf:type", "@schema:Employee"),
  WOQL.triple("v:Employee", "@schema:employee_id", "v:db_id"),
  WOQL.not(WOQL.set_member("v:db_id", "v:csv_set")),
  WOQL.eq("v:status", WOQL.literal("to_remove", "xsd:string")),
)

// Query 2: CSV values not in database
WOQL.and(
  WOQL.get(WOQL.as("EmployeeId", "v:csv_id"))
      .post("employee-list.csv", { type: "csv" }),
  WOQL.not(WOQL.triple("v:AnyEmployee", "@schema:employee_id", "v:csv_id")),
  WOQL.eq("v:status", WOQL.literal("to_add", "xsd:string")),
)
```

For very large datasets, running these as two separate queries is more memory-efficient than trying to compute both in a single query.

---

## Alternative Approach: Using Member and Not

For smaller datasets or when you need row-by-row categorization in the results, the member-based approach provides clear per-value categorization:

### Complete WOQL Query

```javascript
WOQL.and(
  // Step 1: Define your source list using group_by
  WOQL.group_by(
    [],
    ["source_val"],
    "v:source_list",
    WOQL.or(
      WOQL.eq("v:source_val", WOQL.literal("E001", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E002", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E004", "xsd:string")),
      WOQL.eq("v:source_val", WOQL.literal("E006", "xsd:string")),
    )
  ),
  
  // Step 2: Read CSV once into a list variable
  WOQL.group_by(
    [],
    ["csv_val"],
    "v:csv_list",
    WOQL.get(WOQL.as("EmployeeId", "v:csv_val"))
        .post("employee-list.csv", { type: "csv" })
  ),
  
  // Step 3: Get all unique values from both sources
  WOQL.distinct(["v:value"],
    WOQL.or(
      WOQL.member("v:value", "v:source_list"),
      WOQL.member("v:value", "v:csv_list"),
    )
  ),
  
  // Step 4: Categorize each value
  WOQL.or(
    // to_add: in source_list but NOT in csv_list
    WOQL.and(
      WOQL.member("v:value", "v:source_list"),
      WOQL.not(WOQL.member("v:value", "v:csv_list")),
      WOQL.eq("v:category", WOQL.literal("to_add", "xsd:string")),
    ),
    // to_delete: in csv_list but NOT in source_list
    WOQL.and(
      WOQL.member("v:value", "v:csv_list"),
      WOQL.not(WOQL.member("v:value", "v:source_list")),
      WOQL.eq("v:category", WOQL.literal("to_delete", "xsd:string")),
    ),
    // no_change: in BOTH lists
    WOQL.and(
      WOQL.member("v:value", "v:source_list"),
      WOQL.member("v:value", "v:csv_list"),
      WOQL.eq("v:category", WOQL.literal("no_change", "xsd:string")),
    ),
  ),
)
```

### Expected Results

With the source list `[E001, E002, E004, E006]` and CSV containing `[E001, E002, E003, E005, E007]`:

```json
[
  { "value": "E004", "category": "to_add" },
  { "value": "E006", "category": "to_add" },
  { "value": "E003", "category": "to_delete" },
  { "value": "E005", "category": "to_delete" },
  { "value": "E007", "category": "to_delete" },
  { "value": "E001", "category": "no_change" },
  { "value": "E002", "category": "no_change" }
]
```

## Using Set Operations with the JavaScript Client

```javascript
const TerminusClient = require('terminusdb')

const client = new TerminusClient.WOQLClient('http://localhost:6363', {
  user: 'admin',
  key: 'root',
})

async function compareWithCSV() {
  await client.connect()
  await client.db('tdb-example-mydb')
  
  const csvContent = `EmployeeId
E001
E002
E003
E005
E007`
  
  const query = WOQL.and(
    WOQL.group_by(
      [],
      ["source_val"],
      "v:source_list",
      WOQL.or(
        WOQL.eq("v:source_val", WOQL.literal("E001", "xsd:string")),
        WOQL.eq("v:source_val", WOQL.literal("E002", "xsd:string")),
        WOQL.eq("v:source_val", WOQL.literal("E004", "xsd:string")),
        WOQL.eq("v:source_val", WOQL.literal("E006", "xsd:string")),
      )
    ),
    WOQL.group_by(
      [],
      ["csv_val"],
      "v:csv_list",
      WOQL.get(WOQL.as("EmployeeId", "v:csv_val"))
          .post("employee-list.csv", { type: "csv" })
    ),
    WOQL.list_to_set("v:source_list", "v:source_set"),
    WOQL.list_to_set("v:csv_list", "v:csv_set"),
    WOQL.set_difference("v:source_set", "v:csv_set", "v:to_add"),
    WOQL.set_difference("v:csv_set", "v:source_set", "v:to_delete"),
    WOQL.set_intersection("v:source_set", "v:csv_set", "v:no_change"),
  )
  
  const result = await client.query(query, null, null, {
    files: { 'employee-list.csv': csvContent }
  })
  
  console.log(result.bindings)
}
```

## Comparing Two CSV Files

Compare two CSV files using set operations:

```javascript
WOQL.and(
  // Read first CSV into set A
  WOQL.group_by(
    [],
    ["val_a"],
    "v:list_a",
    WOQL.get(WOQL.as("Id", "v:val_a"))
        .post("file-a.csv", { type: "csv" })
  ),
  WOQL.list_to_set("v:list_a", "v:set_a"),
  
  // Read second CSV into set B
  WOQL.group_by(
    [],
    ["val_b"],
    "v:list_b",
    WOQL.get(WOQL.as("Id", "v:val_b"))
        .post("file-b.csv", { type: "csv" })
  ),
  WOQL.list_to_set("v:list_b", "v:set_b"),
  
  // Compute differences and intersection
  WOQL.set_difference("v:set_a", "v:set_b", "v:only_in_a"),
  WOQL.set_difference("v:set_b", "v:set_a", "v:only_in_b"),
  WOQL.set_intersection("v:set_a", "v:set_b", "v:in_both"),
)
```

---

## Performance Considerations

### Set Operations (Recommended)

The set operations provide excellent performance for large datasets:

| Dataset Size | Performance | Recommendation |
|--------------|-------------|----------------|
| Up to 10,000 | < 100ms | Use set operations directly |
| 10,000-100,000 | 100-600ms | Use set operations directly |
| 100,000+ | ~1 second | Consider chunked processing |

### Member-Based Approach

The member-based approach has O(n²) complexity and works well for smaller datasets:

| Dataset Size | Performance | Recommendation |
|--------------|-------------|----------------|
| Up to 1,000 | Fast | Works well |
| 1,000-5,000 | Several seconds | Consider set operations |
| 5,000+ | May timeout | Use set operations instead |

### Very Large Datasets (Millions of Records)

For very large databases or CSV files:

1. **Use streaming patterns** - Process CSV rows one at a time with backtracking instead of collecting into lists
2. **Use indexed triple lookups** - Let the database use indexes for individual lookups
3. **Collect CSV into a set, iterate database** - Keep the smaller dataset in memory as a set, iterate the larger one
4. **Consider chunked processing** - Process data in batches of 50,000-100,000 records

## Summary

The recommended techniques for CSV comparison in WOQL are:

1. **Use `group_by`** to read CSV data once into a list variable
2. **Use `list_to_set`** to convert lists to sorted sets
3. **Use `set_difference` and `set_intersection`** for O(n log n) comparison
4. **Use `set_member`** for O(log n) membership checks against large sets

For comparing database content with CSV:

1. **Use triple patterns** to query database values efficiently
2. **Use backtracking** for memory-efficient processing of large datasets
3. **Collect the smaller dataset** into a set, iterate the larger one

The set operations can handle 100,000+ elements in under a second, making them suitable for production synchronization tasks.

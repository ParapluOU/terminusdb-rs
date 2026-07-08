---
tags:
  - woql
  - how-to
  - data-import
  - advanced
title: How to extract tabular data from JSON with WOQL
nextjs:
  metadata:
    title: How to extract tabular data from JSON with WOQL
    description: Learn how to extract rows and columns from JSON arrays using the sys:Dictionary pattern and dot operator in WOQL
    keywords: terminusdb, datalog, extract tabular data from json with woql, query language, terminusdb query, woql
    alternates:
      canonical: https://terminusdb.org/docs/extract-table-from-json-with-woql/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with JSON documents
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to extract tabular data from JSON documents using WOQL.
{% /callout %}

{% prerequisites-clone /%}

This guide shows you how to extract tabular data from JSON structures using WOQL. This is useful when you have JSON data with arrays of objects and want to query it like a table with rows and columns.

## The Pattern

Use the `dot` operator to navigate JSON structures and `member` to iterate over arrays:

```javascript
select("a", "b").and(
  eq("v:val", new Doc({ val: [{ a: "1", b: "2" }, { a: "2", b: "3" }] })),
  dot("v:val", "val", "v:list"),
  member("v:member", "v:list"),
  dot("v:member", "a", "v:a"),
  dot("v:member", "b", "v:b")
)
```

This returns:

| a | b |
|---|---|
| 1 | 2 |
| 2 | 3 |

## How It Works

### Step 1: Define the JSON Data

```javascript
eq("v:val", new Doc({
  val: [
    { a: "1", b: "2" },
    { a: "2", b: "3" }
  ]
}))
```

The `new Doc()` wrapper creates a `sys:Dictionary` structure that WOQL can traverse. This is the key to working with arbitrary JSON in queries.

### Step 2: Navigate to the Array

```javascript
dot("v:val", "val", "v:list")
```

The `dot` operator accesses the `val` property, binding the array to `v:list`.

### Step 3: Iterate Over Rows

```javascript
member("v:member", "v:list")
```

The `member` predicate iterates over each element in the array, binding each object to `v:member` in turn.

### Step 4: Extract Columns

```javascript
dot("v:member", "a", "v:a"),
dot("v:member", "b", "v:b")
```

Use `dot` again to extract specific fields from each row object.

### Step 5: Select Output Columns

```javascript
select("a", "b")
```

The `select` at the start specifies which variables to return in the result.

## Complete Example: Employee Data

```javascript
let v = Vars("data", "employees", "emp", "id", "name", "department", "salary")

select(v.id, v.name, v.department, v.salary)
.and(
  eq(v.data, new Doc({
    employees: [
      { id: "E001", name: "Alice", department: "Engineering", salary: 75000 },
      { id: "E002", name: "Bob", department: "Sales", salary: 65000 },
      { id: "E003", name: "Carol", department: "Engineering", salary: 80000 },
      { id: "E004", name: "Dave", department: "Marketing", salary: 70000 },
    ]
  })),
  dot(v.data, "employees", v.employees),
  member(v.emp, v.employees),
  dot(v.emp, "id", v.id),
  dot(v.emp, "name", v.name),
  dot(v.emp, "department", v.department),
  dot(v.emp, "salary", v.salary)
)
```

Result:

```json
[
  { "id": "E001", "name": "Alice", "department": "Engineering", "salary": 75000 },
  { "id": "E002", "name": "Bob", "department": "Sales", "salary": 65000 },
  { "id": "E003", "name": "Carol", "department": "Engineering", "salary": 80000 },
  { "id": "E004", "name": "Dave", "department": "Marketing", "salary": 70000 }
]
```

## Filtering Rows

Add constraints to filter the data:

```javascript
let v = Vars("data", "employees", "emp", "id", "name", "department", "salary")

select(v.id, v.name, v.salary)
.and(
  eq(v.data, new Doc({
    employees: [
      { id: "E001", name: "Alice", department: "Engineering", salary: 75000 },
      { id: "E002", name: "Bob", department: "Sales", salary: 65000 },
      { id: "E003", name: "Carol", department: "Engineering", salary: 80000 },
    ]
  })),
  dot(v.data, "employees", v.employees),
  member(v.emp, v.employees),
  dot(v.emp, "id", v.id),
  dot(v.emp, "name", v.name),
  dot(v.emp, "department", v.department),
  dot(v.emp, "salary", v.salary),
  // Filter: only Engineering department
  eq(v.department, string("Engineering"))
)
```

Result:

```json
[
  { "id": "E001", "name": "Alice", "salary": 75000 },
  { "id": "E003", "name": "Carol", "salary": 80000 }
]
```

## Nested JSON Structures

The `dot` operator works with nested objects too:

```javascript
let v = Vars("data", "users", "user", "name", "address", "city", "country")

select(v.name, v.city, v.country)
.and(
  eq(v.data, new Doc({
    users: [
      { 
        name: "Alice", 
        address: { city: "London", country: "UK" } 
      },
      { 
        name: "Bob", 
        address: { city: "Paris", country: "France" } 
      },
    ]
  })),
  dot(v.data, "users", v.users),
  member(v.user, v.users),
  dot(v.user, "name", v.name),
  dot(v.user, "address", v.address),
  dot(v.address, "city", v.city),
  dot(v.address, "country", v.country)
)
```

Result:

```json
[
  { "name": "Alice", "city": "London", "country": "UK" },
  { "name": "Bob", "city": "Paris", "country": "France" }
]
```

## Comparing JSON Tables

Combine with the [CSV comparison pattern](/docs/compare-csv-values-with-woql/) to compare two JSON datasets:

```javascript
let v = Vars("source", "target", "source_list", "target_list", 
             "s_item", "t_item", "s_id", "t_id", "value", "category")

and(
  // Source JSON table
  eq(v.source, new Doc({
    items: [{ id: "1" }, { id: "2" }, { id: "3" }]
  })),
  dot(v.source, "items", v.source_list),
  
  // Target JSON table  
  eq(v.target, new Doc({
    items: [{ id: "2" }, { id: "3" }, { id: "4" }]
  })),
  dot(v.target, "items", v.target_list),
  
  // Get all unique IDs
  distinct(["value"],
    or(
      and(member(v.s_item, v.source_list), dot(v.s_item, "id", v.value)),
      and(member(v.t_item, v.target_list), dot(v.t_item, "id", v.value))
    )
  ),
  
  // Categorize
  or(
    // In source only
    and(
      once(and(member(v.s_item, v.source_list), dot(v.s_item, "id", v.value))),
      not(once(and(member(v.t_item, v.target_list), dot(v.t_item, "id", v.value)))),
      eq(v.category, string("to_add"))
    ),
    // In target only
    and(
      not(once(and(member(v.s_item, v.source_list), dot(v.s_item, "id", v.value)))),
      once(and(member(v.t_item, v.target_list), dot(v.t_item, "id", v.value))),
      eq(v.category, string("to_delete"))
    ),
    // In both
    and(
      once(and(member(v.s_item, v.source_list), dot(v.s_item, "id", v.value))),
      once(and(member(v.t_item, v.target_list), dot(v.t_item, "id", v.value))),
      eq(v.category, string("no_change"))
    )
  )
)
```

## Using with the JavaScript Client

```javascript
const TerminusClient = require('terminusdb')
const { WOQL, Vars, Doc } = TerminusClient

const client = new TerminusClient.WOQLClient('http://localhost:6363', {
  user: 'admin',
  key: 'root',
})

async function queryJsonTable() {
  await client.connect()
  await client.db('tdb-example-mydb')
  
  let v = Vars("data", "items", "item", "id", "name")
  
  const query = WOQL.select(v.id, v.name)
    .and(
      WOQL.eq(v.data, new Doc({
        items: [
          { id: "1", name: "First" },
          { id: "2", name: "Second" },
        ]
      })),
      WOQL.dot(v.data, "items", v.items),
      WOQL.member(v.item, v.items),
      WOQL.dot(v.item, "id", v.id),
      WOQL.dot(v.item, "name", v.name)
    )
  
  const result = await client.query(query)
  console.log(result.bindings)
}
```

## JSON-LD Equivalent

The raw JSON-LD structure for the `dot` operator:

```json
{
  "@type": "Dot",
  "dictionary": { "@type": "Value", "variable": "data" },
  "key": { "@type": "DataValue", "data": "employees" },
  "value": { "@type": "Value", "variable": "employees" }
}
```

## Performance Considerations

- **Small datasets (< 1000 rows)**: This pattern works efficiently
- **Medium datasets (1000-5000 rows)**: May take a few seconds
- **Large datasets (5000+ rows)**: Consider importing as documents for indexed queries

The `dot` + `member` pattern is ideal for:
- API response data
- Configuration files
- Test fixtures
- Small lookup tables

## Summary

The key techniques for extracting tabular data from JSON in WOQL are:

1. **Use `new Doc()`** to wrap JSON data as a `sys:Dictionary`
2. **Use `dot`** to navigate to arrays and extract fields
3. **Use `member`** to iterate over array elements
4. **Use `select`** to specify output columns
5. **Add constraints** with `eq`, `greater`, etc. to filter rows

This pattern provides a powerful way to query JSON data directly in WOQL without needing to import it into the database first.

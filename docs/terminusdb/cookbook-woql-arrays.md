---
tags:
  - woql
  - cookbook
  - schema
  - intermediate
title: "Cookbook: Leveraging Arrays with WOQL"
nextjs:
  metadata:
    title: "Cookbook: Leveraging Arrays with WOQL"
    description: Learn to query and manipulate TerminusDB multidimensional arrays using WOQL patterns for efficient data access and processing
    keywords: arrays, WOQL, multidimensional arrays, data querying, array manipulation, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
media: []
---

## Overview

TerminusDB's multidimensional arrays are powerful structures for storing ordered collections with random access capabilities. This guide teaches you how to effectively query and manipulate arrays using WOQL (Web Object Query Language).

Arrays in TerminusDB are implemented using intermediate indexed objects with specific triple patterns that allow for efficient multidimensional data access. Understanding these internal structures is key to writing effective WOQL queries.

## Understanding Array Storage

### Internal Triple Structure

TerminusDB stores arrays using a specific triple pattern with intermediate objects:

- **`sys:value`**: Contains the actual array element value
- **`sys:index`**: First dimension index (0-based)
- **`sys:index2`**: Second dimension index (for 2D arrays)
- **`sys:indexN`**: Nth dimension index (for N-dimensional arrays)

### Schema Definition

Here's how to define an array in your schema:

```json
{
  "@type": "@context",
  "@base": "http://i/",
  "@schema": "http://s/"
}

{
  "@id": "DataMatrix",
  "@type": "Class",
  "@key": {"@type": "Random"},
  "name": "xsd:string",
  "measurements": {
    "@type": "Array",
    "@dimensions": 2,
    "@class": "xsd:decimal"
  }
}
```

## Basic Array Querying Patterns

### Pattern 1: Finding Array Elements by Value

To find array elements with specific values, use the internal storage pattern:

```javascript
let v = Vars("element", "index1", "index2", "value")

triple(v.element, "sys:value", v.value)
  .triple(v.element, "sys:index", v.index1)
  .triple(v.element, "sys:index2", v.index2)
  .eq(v.value, 42)
```

**What this does**: Finds all array elements where the value equals 42, returning their coordinates.

### Pattern 2: Accessing Elements by Index

Array dimensions use a non-negative integer as the data type and needs to be queried explicitly. To retrieve a specific array element by its coordinates, use below snippet.

Variables are expressed using the implicit style, with the `v:` prefix in this example.

```javascript
triple("v:doc", "measurements", "v:element")
  .triple("v:element", "sys:index", literal(0, "xsd:nonNegativeInteger"))
  .triple("v:element", "sys:index2", literal(1, "xsd:nonNegativeInteger"))
  .triple("v:element", "sys:value", "v:value")
```

**What this does**: Gets the value at position [0,1] in the measurements array.

### Pattern 3: Range Queries on Array Indices

For multidimensional range queries:

```javascript
let v = Vars("element", "index1", "index2", "value")

triple(v.element, "sys:value", v.value)
  .triple(v.element, "sys:index", v.index1)
  .triple(v.element, "sys:index2", v.index2)
  .greater(v.index1, 2)
  .less(v.index2, 5)
  .greater(v.value, 10)
```

**What this does**: Finds elements where first index > 2, second index < 5, and value > 10.

## Advanced Array Operations

### Sparse Array Handling

Arrays can have gaps (null values). To handle sparse arrays without failing the query or subquery, use the opt(ional) pattern:

```javascript
let v = Vars("row", "col", "hasValue")

// Check if position [row, col] has a value, if left unbound, will return
// null on matches with missing values (given optional pattern)
and(
  triple("v:doc", "measurement", "v:element"),
  opt().
    and(
      triple(v.element, "sys:index", v.row),
      triple(v.element, "sys:index2", v.col),
      triple(v.element, "sys:value", v.hasValue)
    )
)
```

## Practical Example

### Example: 3D Array Navigation

Working with 3-dimensional arrays (e.g., time series data):

```javascript
let v = Vars("element", "x", "y", "time", "value")

triple(v.element, "sys:index", v.x)      // X coordinate
  .triple(v.element, "sys:index2", v.y)    // Y coordinate  
  .triple(v.element, "sys:index3", v.time) // Time dimension
  .triple(v.element, "sys:value", v.value)
  .eq(v.time, 5)                           // Specific time slice
```

## Performance Tips & Best Practices

TerminusDB uses auto-indexed values using succinct datastructures and ordered storage which makes lookups very fast. This applies for both arrays and their dimensions and all other data.

Because of this, it is not necessary to create specific indexes in TerminusDB, instead in-memory storage techniques are used by the storage engine to quickly find linked values.

That said, there are still performance optimizations that are possible to limit the cardinality of the unification by the engine.

1. **Use specific index constraints early** in your query to limit the search space.

### Query Optimization Patterns

#### Pattern A: Index-First Querying
```javascript
// Good: Start with index constraints
triple(v.element, "sys:index", v.targetRow)
  .triple(v.element, "sys:index2", v.col)
  .triple(v.element, "sys:value", v.value)
```

#### Pattern B: Value-Based Filtering
```javascript
// When searching by value, avoid searches and instead use the exact value search through the succinct auto-indexing, by placing the value search first and constraints later if any.
triple(v.element, "sys:value", v.targetValue)
  .triple(v.element, "sys:index", v.row)
  .triple(v.element, "sys:index2", v.col)
  .greater(v.row, 0)  // Add meaningful constraints
```

### Memory Considerations

- **Large arrays**: Consider pagination using `limit()` and `start()`
- **Sparse arrays**: Use `opt()` patterns to handle missing values gracefully


## Debugging Array Queries

### Inspecting Array Structure

To understand how your array is stored:

```javascript
let v = Vars("element", "prop", "val")

// View all array element properties
triple(v.element, v.prop, v.val)
  .re("sys:(index|value)", v.prop)  // Only sys properties
```


## Error Handling & Edge Cases

### Multiple variables for multiple matches with all of

When making an all_of match against values, it may be necessary to use multiple variables, one for each match. The reason for this is that the engine will only match against the first variable which will bind to that subject. The consequence is that it will not match another value.

The aim is to have solutions on a single row, which means that every variable need to be bound independently. More avanced solutions are left as an exercise to the reader.

```javascript
and(
  triple("v:doc_subject", "measurements","v:arr_subject1"),
  triple("v:doc_subject", "measurements","v:arr_subject2"),
  select("").
  and(
      eq("v:pos_1_1", literal("1,1", "xsd:string")),
      eq("v:pos_1_2", literal("1,2", "xsd:string")),
      and(
        triple("v:arr_subject1", "sys:index", literal(0, "xsd:nonNegativeInteger")),
        triple("v:arr_subject1", "sys:index2", literal(0, "xsd:nonNegativeInteger")),
        triple("v:arr_subject1", "sys:index3", literal(0, "xsd:nonNegativeInteger")),
        triple("v:arr_subject1", "sys:value", "v:pos_1_1"),
        type_of("v:pos_1_1", "v:v1_type")
      ),
      and(
        triple("v:arr_subject2", "sys:index", literal(1, "xsd:nonNegativeInteger")),
        triple("v:arr_subject2", "sys:index2", literal(0, "xsd:nonNegativeInteger")),
        triple("v:arr_subject2", "sys:index3", literal(0, "xsd:nonNegativeInteger")),
        triple("v:arr_subject2", "sys:value", "v:pos_1_2"),
      )
  )
)
```

### Handling Missing Elements

Always use optional patterns when element existence is uncertain:

```javascript
let v = Vars("doc", "element", "value")

triple(v.doc, "@id", "MyDocument")
  .opt(
    and(
      triple(v.doc, "measurements", v.element),
      triple(v.element, "sys:index", 0),
      triple(v.element, "sys:index2", 0),
      triple(v.element, "sys:value", v.value)
    )
  )
```

## Summary

Working with arrays in WOQL requires understanding the underlying triple storage pattern. Key takeaways:

- **Arrays use `sys:value`, `sys:index`, `sys:index2`, etc. for storage**
- **Start queries with index constraints for better performance** 
- **Use optional patterns for sparse arrays**
- **Consider memory usage with large multidimensional arrays**
- **Debug by examining the raw triple structure**

Master these patterns and you'll be able to efficiently query multidimensional array structures in TerminusDB using WOQL.
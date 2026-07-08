---
tags:
  - woql
  - how-to
  - intermediate
title: "Working with Data in WOQL"
nextjs:
  metadata:
    title: "Working with Data in WOQL"
    description: A practical guide to WOQL's data types, variable binding, solutions, typecasting, dicts, lists, group_by, CSV and JSON conversion, and how backtracking drives result generation.
    keywords: woql data types variables solutions backtracking typecasting dict list group_by csv json member read_document
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-data-handling/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know techniques for handling and transforming data within WOQL queries.
{% /callout %}

This guide covers the practical side of working with data in WOQL: what types exist, how values flow through queries, and how to reshape results for real-world use. It assumes you have completed the [Interactive Tutorial](/docs/woql-tutorial/) or are comfortable with `triple`, `and`, `or`, and `eq`.

---

## How WOQL Produces Results

Every WOQL query returns a **table of solutions**. Each row is one valid assignment of all variables; each column is one variable. Understanding how those rows are generated is the key to writing effective queries.

### Solutions and Backtracking

WOQL is built on a Datalog/Prolog engine. When the engine encounters a predicate that can match multiple values, it produces one solution row per match. If a later constraint fails for a particular match, the engine **backtracks** — it discards that partial solution and tries the next match.

This means:
- **`and`** narrows the solution set. Every constraint must hold simultaneously, so incompatible rows are eliminated.
- **`or`** expands the solution set. Each branch that succeeds contributes its own rows.
- **`opt`** preserves rows. If the inner pattern fails, the row survives with unbound variables instead of being eliminated.

```javascript
// Two triples share v.person — natural join via backtracking.
// The engine iterates all name triples, then for each match
// backtracks into age triples looking for the same v.person.
let v = Vars("person", "name", "age")
and(
  triple(v.person, "name", v.name),
  triple(v.person, "age", v.age)
)
```

The result is not a loop — it is the set of all simultaneously valid bindings. Each row where both triples hold for the same `v.person` is one solution.

### Solutions Are Not Lists

A common source of confusion: the result table *looks* like a list, but inside the query it behaves differently. Each row is produced independently by backtracking. You cannot index into "row 3" or iterate the result set within the same query.

To work with results as a collection inside a query, you need `group_by` to collect rows into an actual list variable. More on that [below](#collecting-solutions-with-group_by).

---

## Variable Types and Values

### Typed Literals

All values stored in TerminusDB carry an XSD type. When you read a property with `triple`, the object position holds a **typed literal** — not a bare JavaScript string or number.

| Schema type | XSD type | Example value |
|-------------|----------|---------------|
| `xsd:string` | `xsd:string` | `"Alice"` |
| `xsd:integer` | `xsd:integer` | `42` |
| `xsd:decimal` | `xsd:decimal` | `3.14` |
| `xsd:boolean` | `xsd:boolean` | `true` |
| `xsd:dateTime` | `xsd:dateTime` | `"2025-01-15T10:30:00Z"` |
| `xsd:date` | `xsd:date` | `"2025-01-15"` |

When matching a specific value in a triple, wrap it with `literal()`:

```javascript
// Match the exact typed string "New York"
triple("v:person", "city", literal("New York", "xsd:string"))
```

Without `literal()`, WOQL may interpret a bare string as a variable name or IRI rather than a data value.

### Checking and Matching Types with `type_of`

The `type_of` predicate works in multiple directions thanks to unification:

```javascript
// Direction 1: Discover the type of a value
type_of("v:value", "v:its_type")

// Direction 2: Filter to only strings
type_of("v:value", "xsd:string")

// Direction 3: Check a specific value's type
type_of(literal(42, "xsd:integer"), "xsd:integer")
```

See the [Datatypes Cookbook](/docs/cookbook-woql-type-of-datatype/) for detailed examples.

### Typecasting with `typecast`

Convert between types with `typecast`:

```javascript
// Cast a string to an integer
typecast("v:string_val", "xsd:integer", "v:int_val")

// Cast an integer to a string
typecast(literal(42, "xsd:integer"), "xsd:string", "v:as_string")

// Cast a dateTime string to xsd:dateTime
typecast(
  literal("2025-01-15T10:30:00Z", "xsd:string"),
  "xsd:dateTime",
  "v:timestamp"
)
```

`typecast` is essential when comparing values from different sources. CSV columns arrive as strings, so casting them to numeric types before comparison prevents type mismatches:

```javascript
and(
  get(as("Age", "v:age_str")).post("people.csv", { type: "csv" }),
  typecast("v:age_str", "xsd:integer", "v:age"),
  greater("v:age", 30)
)
```

---

## DocumentTemplate and the `Doc` Wrapper

### What `Doc` Does

`Doc()` (or `new Doc()`) converts a plain JavaScript object into a `sys:Dictionary` structure that WOQL can traverse. This is how you bring raw JSON data into a query without storing it in the database first.

```javascript
eq("v:data", new Doc({
  name: "Alice",
  scores: [85, 92, 78]
}))
```

The resulting `v:data` is not a plain object — it is a dictionary with a specific internal structure that the `dot` operator can navigate.

### Navigating with `dot`

The `dot` operator accesses fields within a dictionary:

```javascript
and(
  eq("v:data", new Doc({ name: "Alice", city: "London" })),
  dot("v:data", "name", "v:name"),   // v:name = "Alice"
  dot("v:data", "city", "v:city")    // v:city = "London"
)
```

For nested structures, chain `dot` calls:

```javascript
and(
  eq("v:data", new Doc({
    user: { address: { city: "Paris" } }
  })),
  dot("v:data", "user", "v:user"),
  dot("v:user", "address", "v:addr"),
  dot("v:addr", "city", "v:city")    // v:city = "Paris"
)
```

### Arrays in Dicts

When a dict field contains an array, `dot` binds the array as a list. Use `member` to iterate:

```javascript
and(
  eq("v:data", new Doc({
    tags: ["urgent", "billing", "customer"]
  })),
  dot("v:data", "tags", "v:tag_list"),
  member("v:tag", "v:tag_list")      // One row per tag
)
```

See [Extract Table from JSON](/docs/extract-table-from-json-with-woql/) for a complete walkthrough.

---

## Reading Documents into Dicts

### `read_document` Returns a Dict

`read_document` assembles all triples for a document into a single dictionary variable:

```javascript
let v = Vars("id", "doc")
and(
  isa(v.id, "Person"),
  read_document(v.id, v.doc)
)
```

The `v.doc` variable now holds the full document as a dict. You can extract fields with `dot`:

```javascript
let v = Vars("id", "doc", "name", "age")
and(
  isa(v.id, "Person"),
  read_document(v.id, v.doc),
  dot(v.doc, "name", v.name),
  dot(v.doc, "age", v.age)
)
```

### When to Use `triple` vs. `read_document`

| Use case | Approach |
|----------|----------|
| Filter by specific properties | `triple` — only reads what you need |
| Join across documents | `triple` — shared variables create the join |
| Get the complete document | `read_document` — one call, all fields |
| Pass document data downstream | `read_document` + `dot` — structured access |

A common pattern is to filter with `triple`, then read the full document for matching results:

```javascript
let v = Vars("id", "doc", "name")
and(
  triple(v.id, "age", "v:age"),
  greater("v:age", 30),
  read_document(v.id, v.doc),
  dot(v.doc, "name", v.name)
)
```

---

## Lists and the `member` Predicate

### Lists in WOQL

WOQL lists are ordered collections that appear in two contexts:
1. **Schema-defined lists/arrays** — stored as triples with `sys:index` and `sys:value` patterns
2. **Query-constructed lists** — created by `group_by` or literal notation

### Iterating with `member`

`member` generates one solution row per element in a list:

```javascript
and(
  eq("v:colors", ["red", "green", "blue"]),
  member("v:color", "v:colors")
)
// Result: 3 rows — one for each color
```

This is the standard way to "loop" in WOQL. There are no for-loops — instead, `member` drives backtracking over the list elements.

### Combining `member` with Other Predicates

Use `member` to test containment, filter lists, or cross-reference:

```javascript
// Check if a value is in a list
and(
  eq("v:allowed", ["admin", "editor", "viewer"]),
  member("v:role", "v:allowed"),
  eq("v:role", literal("editor", "xsd:string"))
)

// Cross-reference: for each person, check if their city is in a target list
and(
  triple("v:person", "name", "v:name"),
  triple("v:person", "city", "v:city"),
  member("v:city", [
    literal("London", "xsd:string"),
    literal("Paris", "xsd:string")
  ])
)
```

---

## Collecting Solutions with `group_by`

### How `group_by` Works

`group_by` is the bridge between the streaming world of backtracking and the collection world of lists. It runs an inner query, groups the results by specified variables, and collects the grouped values into a list.

```javascript
group_by(
  [grouping_variables],   // Variables to group by (the "key")
  [collected_variables],   // Variables to collect into lists (the "value")
  "v:result_list",         // The output list variable
  inner_query              // The query that produces rows
)
```

### Counting Per Group

```javascript
and(
  group_by(
    ["city"],              // Group by city
    ["person"],            // Collect person IDs
    "v:people_in_city",
    and(
      isa("v:person", "Person"),
      triple("v:person", "city", "v:city")
    )
  ),
  length("v:people_in_city", "v:count")
)
```

Result: one row per city, with `v:count` holding the number of people.

### Collecting All Values into a Single List

Use an empty grouping key `[]` to collect everything into one list:

```javascript
group_by(
  [],                      // No grouping — one big group
  ["name"],                // Collect all names
  "v:all_names",
  and(
    isa("v:person", "Person"),
    triple("v:person", "name", "v:name")
  )
)
// v:all_names is a single list of all person names
```

### Processing Grouped Results

After `group_by`, use `member` to iterate over the collected list, or `length` to count, or set operations to compare:

```javascript
and(
  // Collect names per city
  group_by(
    ["city"],
    ["name"],
    "v:names",
    and(
      triple("v:person", "city", "v:city"),
      triple("v:person", "name", "v:name")
    )
  ),
  // Iterate over each name in the group
  member("v:one_name", "v:names")
)
```

---

## Converting to and from CSV

### Reading CSV Files

Use `get` and `as` to read CSV columns into variables. Each row produces one solution via backtracking:

```javascript
get(
  as("Name", "v:name")
    .as("Age", "v:age")
    .as("City", "v:city")
).post("people.csv", { type: "csv" })
```

CSV values arrive as `xsd:string`. Cast them when needed:

```javascript
and(
  get(as("Name", "v:name").as("Age", "v:age_str"))
    .post("people.csv", { type: "csv" }),
  typecast("v:age_str", "xsd:integer", "v:age"),
  greater("v:age", 25)
)
```

### Collecting CSV into a List

To process CSV data as a whole (for set operations, comparisons, etc.), wrap it in `group_by`:

```javascript
group_by(
  [],
  ["name"],
  "v:all_csv_names",
  get(as("Name", "v:name")).post("people.csv", { type: "csv" })
)
```

### Importing CSV into Documents

Combine CSV reading with `insert_document`:

```javascript
and(
  get(
    as("Name", "v:name")
      .as("Age", "v:age_str")
      .as("City", "v:city")
  ).post("people.csv", { type: "csv" }),
  typecast("v:age_str", "xsd:integer", "v:age"),
  insert_document(
    Doc({
      "@type": "Person",
      "name": "v:name",
      "age": "v:age",
      "city": "v:city"
    }),
    "v:id"
  )
)
```

### Comparing Database with CSV

See the [CSV Comparison Guide](/docs/compare-csv-values-with-woql/) for detailed patterns using `set_difference`, `set_intersection`, and streaming comparisons for large datasets.

---

## Converting to and from JSON

### Bringing JSON into a Query

Use `Doc()` to bring arbitrary JSON into WOQL's dict format, then navigate with `dot` and `member`:

```javascript
let v = Vars("data", "items", "item", "id", "name")
and(
  eq(v.data, new Doc({
    items: [
      { id: "1", name: "Widget" },
      { id: "2", name: "Gadget" }
    ]
  })),
  dot(v.data, "items", v.items),
  member(v.item, v.items),
  dot(v.item, "id", v.id),
  dot(v.item, "name", v.name)
)
// Result: 2 rows — one per item
```

See [Extract Table from JSON](/docs/extract-table-from-json-with-woql/) for filtering, nesting, and comparison patterns.

### Getting Documents as JSON

`read_document` returns the document in TerminusDB's JSON format. In client code, the `bindings` response contains the full document as a JSON object:

```javascript
// JavaScript client
const result = await client.query(
  WOQL.and(
    WOQL.isa("v:id", "Person"),
    WOQL.read_document("v:id", "v:doc")
  )
)

// Each binding has the complete document as a JS object
for (const row of result.bindings) {
  console.log(row.doc)
  // { "@type": "Person", "name": "Alice", "age": 28, ... }
}
```

```python
# Python client
result = client.query(
    wq().woql_and(
        wq().isa("v:id", "@schema:Person"),
        wq().read_document("v:id", "v:doc")
    )
)

for row in result["bindings"]:
    print(row["doc"])
    # {'@type': 'Person', 'name': 'Alice', 'age': 28, ...}
```

### Reshaping JSON Output

Combine `read_document` with `dot` and `select` to extract exactly the fields you need:

```javascript
let v = Vars("id", "doc", "name", "city")
select(v.name, v.city,
  and(
    isa(v.id, "Person"),
    read_document(v.id, v.doc),
    dot(v.doc, "name", v.name),
    dot(v.doc, "city", v.city)
  )
)
// Result: only name and city columns, no document ID or full doc
```

---

## Putting It All Together

Here is a realistic example that combines several techniques: reading CSV data, querying the database, grouping results, and using `member` and `dot` to reshape output.

### Scenario: Reconcile Employee Records

You have a CSV of current employee IDs and want to find which ones are in the database, what department they belong to, and group them by department.

```javascript
let v = Vars(
  "csv_id", "csv_list", "csv_set",
  "emp", "doc", "name", "dept",
  "dept_group", "dept_count"
)

and(
  // Step 1: Load CSV into a set
  group_by([], ["csv_id"], v.csv_list,
    get(as("EmployeeId", "v:csv_id"))
      .post("current-employees.csv", { type: "csv" })
  ),
  list_to_set(v.csv_list, v.csv_set),

  // Step 2: For each employee in CSV that exists in DB,
  //         read their document and extract fields
  member("v:lookup_id", v.csv_set),
  isa(v.emp, "Employee"),
  triple(v.emp, "employee_id", "v:lookup_id"),
  read_document(v.emp, v.doc),
  dot(v.doc, "name", v.name),
  dot(v.doc, "department", v.dept),
)
```

To count per department, wrap the above in a `group_by`:

```javascript
and(
  group_by(
    ["dept"],
    ["name"],
    "v:names_in_dept",
    and(
      // ... the query from above ...
    )
  ),
  length("v:names_in_dept", "v:dept_count")
)
```

---

## Quick Reference

| Concept | Predicate | What it does |
|---------|-----------|--------------|
| Bind a value | `eq(var, value)` | Assigns a value to a variable |
| Typed literal | `literal(value, type)` | Creates a typed value for matching |
| Check/discover type | `type_of(value, type)` | Matches or discovers a value's XSD type |
| Convert type | `typecast(input, target_type, output)` | Casts between XSD types |
| Navigate dict | `dot(dict, key, value)` | Accesses a field in a dictionary |
| Read full document | `read_document(id, doc)` | Loads a complete document as a dict |
| Create dict in query | `Doc({...})` / `new Doc({...})` | Wraps JSON as a sys:Dictionary |
| Iterate a list | `member(element, list)` | One row per list element |
| Group results | `group_by(keys, vals, list, query)` | Collects backtracked rows into lists |
| Count a list | `length(list, count)` | Returns the number of elements |
| Read CSV | `get(as(...)).post(file, {type:"csv"})` | One row per CSV row |
| Set from list | `list_to_set(list, set)` | Sorted, deduplicated collection |
| Set operations | `set_difference`, `set_intersection` | Compare two sets efficiently |

## Further Reading

| Topic | Page |
|-------|------|
| Interactive tutorial (start here) | [Learn WOQL](/docs/woql-tutorial/) |
| Type matching examples | [Datatypes Cookbook](/docs/cookbook-woql-type-of-datatype/) |
| Extract tables from JSON | [JSON Table Extraction](/docs/extract-table-from-json-with-woql/) |
| CSV comparison patterns | [CSV Comparison](/docs/compare-csv-values-with-woql/) |
| Array and set queries | [Arrays & Sets](/docs/query-arrays-and-sets-in-woql/) |
| Set operations reference | [Set Operations](/docs/woql-set-operations/) |
| Subdocument handling | [Subdocuments](/docs/woql-subdocument-handling/) |
| Variables and unification | [Unification](/docs/what-is-datalog/#unification-the-key-mechanism) |
| Complete reference | [WOQL Class Reference](/docs/woql-class-reference-guide/) |

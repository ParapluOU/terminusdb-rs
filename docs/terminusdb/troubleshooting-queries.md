---
tags:
  - woql
  - troubleshooting
  - intermediate
title: Query Errors — TerminusDB Troubleshooting
nextjs:
  metadata:
    title: Query Errors — TerminusDB Troubleshooting
    description: Fix common WOQL and GraphQL query errors in TerminusDB including the anyuri vs string type mismatch, unbound variables, empty results, and path query syntax issues.
    keywords: terminusdb, datalog, debug, error, graphql, path, query, query errors — terminusdb troubleshooting
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/troubleshooting-queries/
media: []
---

# Query Errors

This page covers errors that occur when executing WOQL or GraphQL queries — type mismatches between URI and string types, unbound variables, queries that return empty results unexpectedly, and path query syntax problems.

{% prerequisites-clone /%}

## Symptoms

- Query returns 0 bindings when you expect results
- `"api:ErrorMessage": "Unbound variable"` in WOQL response
- Type mismatch errors when comparing values in WOQL
- GraphQL returns `null` for fields you expect to have data
- `"api:ErrorMessage": "Syntax error"` for path queries

## Common causes

### WOQL: anyuri vs xsd:string type mismatch (critical footgun)

**Error message:** Query returns 0 bindings silently — no error message, just empty results.

**Cause:** TerminusDB stores foreign key references (document links) as `xsd:anyURI` type internally. When you compare a URI-typed value with a string literal using `eq/2`, the types do not match and unification fails silently. This is the single most common WOQL debugging issue.

**Fix:**

Do **not** compare a foreign key field directly to a string literal:

```javascript
// WRONG — comparing anyuri to xsd:string gives 0 bindings
WOQL.and(
  WOQL.triple("v:Doc", "linkedField", "v:Ref"),
  WOQL.eq("v:Ref", WOQL.string("TargetId"))  // Type mismatch!
)
```

Instead, join through the linked document to match on a string-typed field:

```javascript
// CORRECT — join through the document to unify on string fields
WOQL.and(
  WOQL.triple("v:Doc", "linkedField", "v:Ref"),
  WOQL.triple("v:Ref", "name", "v:Name"),
  WOQL.eq("v:Name", WOQL.string("Alice"))  // Both xsd:string — works
)
```

For cases where you need to match a full IRI pattern, use `concat` to build a string and compare:

```javascript
// CORRECT — build IRI string via concat, then unify
WOQL.and(
  WOQL.triple("v:Doc", "linkedField", "v:Ref"),
  WOQL.triple("v:Target", "prefix", "v:Prefix"),
  WOQL.triple("v:Target", "name", "v:Name"),
  WOQL.concat(["v:Prefix", ":", "v:Name"], "v:FullId"),
  WOQL.eq("v:FullId", WOQL.string("myprefix:Alice")),
  WOQL.triple("v:Target", "termId", "v:Ref")  // Unify URI with URI
)
```

**Why this happens:** TerminusDB is type-strict. `xsd:anyURI` and `xsd:string` are different types. Even if the string content looks identical, `eq("v:uriVar", string("same-text"))` will not unify because the types differ.

### WOQL: unbound variable

**Error message:** `"api:ErrorMessage": "Variables are unbound: v:X"` or the query errors with an unbound variable warning

**Cause:** A variable appears in the query output (select) or in a comparison but is never bound by a triple pattern or other binding operation.

**Fix:**

1. Ensure every variable you select is bound somewhere in the query body:
   ```javascript
   // WRONG — v:Age is never bound
   WOQL.select("v:Name", "v:Age").and(
     WOQL.triple("v:Person", "name", "v:Name")
   )

   // CORRECT — v:Age is bound by a triple pattern
   WOQL.select("v:Name", "v:Age").and(
     WOQL.triple("v:Person", "name", "v:Name"),
     WOQL.triple("v:Person", "age", "v:Age")
   )
   ```

2. Check for typos in variable names — `"v:Naem"` vs `"v:Name"` will create two separate unbound variables.

3. When using `optional`, remember that unbound variables in the optional branch may remain unbound if the pattern does not match.

### WOQL: empty results (query logic issue)

**Error message:** No error — query returns `{ "bindings": [] }`

**Cause:** The conjunction of triple patterns has no solution. This often happens when patterns inadvertently conflict or when the data does not match expectations.

**Fix:**

1. **Decompose the query** — run each triple pattern individually to find which one fails:
   ```javascript
   // Test each pattern separately
   WOQL.triple("v:Doc", "@type", "v:Type")  // Does this return results?
   WOQL.triple("v:Doc", "name", "v:Name")   // What about this?
   ```

2. **Check the `@type` value** — remember that types are full IRIs unless a prefix is set:
   ```javascript
   // May need the full type IRI
   WOQL.triple("v:Doc", "rdf:type", "@schema:Person")
   ```

3. **Verify data exists** — check that documents are in the instance graph:
   ```bash
   curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb?type=Person&count=1"
   ```

4. **Check the graph** — ensure you are querying the correct graph (instance vs schema):
   ```javascript
   // Querying schema graph for class definitions
   WOQL.quad("v:Class", "rdf:type", "sys:Class", "schema")
   ```

### Path query syntax errors

**Error message:** `"api:ErrorMessage": "Syntax error in path pattern"` or unexpected results from path queries

**Cause:** Path query syntax is specific and differs from regular triple patterns. Common mistakes include wrong separator characters or missing type annotations.

**Fix:**

1. Use the correct path syntax operators:
   - `,` — sequence (A then B)
   - `|` — alternative (A or B)
   - `+` — one or more
   - `*` — zero or more
   - `(...)` — grouping

2. Example of correct path query:
   ```javascript
   // Find all ancestors via "parent" property
   WOQL.path("v:Person", "parent+", "v:Ancestor", "v:Path")
   ```

3. Field names in paths must match the schema exactly (case-sensitive):
   ```javascript
   // WRONG — wrong case
   WOQL.path("v:X", "Parent+", "v:Y", "v:Path")

   // CORRECT — matches schema field name
   WOQL.path("v:X", "parent+", "v:Y", "v:Path")
   ```

### GraphQL: field returns null unexpectedly

**Error message:** No error, but a field in the GraphQL response is `null` when you expect data.

**Cause:** GraphQL in TerminusDB auto-generates the schema from your document schema. A `null` return usually means one of:
- The field is `Optional` in the schema and has no value set on that document
- The field name in your query has a case mismatch (GraphQL is case-sensitive)
- The linked document referenced by the field has been deleted

**Fix:**

1. **Check the document directly** via the HTTP API to confirm the field exists:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/MyDatabase?id=Person/alice"
   ```

2. **Check field naming** — GraphQL field names match schema property names exactly. `email` ≠ `Email`:
   ```graphql
   # WRONG — wrong case
   query { Person { Email } }

   # CORRECT
   query { Person { email } }
   ```

3. **For linked documents**, ensure the referenced document still exists. A deleted target returns `null` for the entire link field.

4. **Introspect the schema** to see the correct field names and types:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/graphql/admin/MyDatabase" \
     -H "Content-Type: application/json" \
     -d '{"query": "{ __type(name: \"Person\") { fields { name type { name } } } }"}'
   ```

### WOQL: incorrect use of literal() with typed values

**Error message:** Query returns 0 bindings or `"vio:message": "Type error"` — occurs when building typed literals incorrectly.

**Cause:** The `literal()` / `WOQL.string()` / `WOQL.iri()` helpers produce typed RDF values. Using the wrong helper or the wrong datatype URI causes silent unification failure.

**Fix:**

1. **String values** — use `WOQL.string()` or `{"@type": "xsd:string", "@value": "..."}`:
   ```javascript
   // CORRECT for string comparison
   WOQL.eq("v:Name", WOQL.string("Alice"))
   ```

2. **Integer values** — use the correct type annotation:
   ```javascript
   // WRONG — "30" is a string
   WOQL.eq("v:Age", WOQL.string("30"))

   // CORRECT — 30 is typed as integer
   WOQL.eq("v:Age", {"@type": "xsd:integer", "@value": 30})
   ```

3. **IRIs / document references** — use `WOQL.iri()` or the full document path:
   ```javascript
   // CORRECT for matching a specific document
   WOQL.triple(WOQL.iri("Person/alice"), "name", "v:Name")
   ```

4. **Dates** — must be ISO 8601 with the correct XSD type:
   ```javascript
   WOQL.eq("v:Date", {"@type": "xsd:dateTime", "@value": "2024-01-15T00:00:00Z"})
   ```

See [Data Types reference](/docs/data-types) for the full list of supported XSD types and their expected formats.

## Still stuck?

- [Open an issue](https://github.com/terminusdb/terminusdb/issues) with your full query and expected vs actual results
- Check the [WOQL Basics guide](/docs/woql-basics) for query fundamentals
- Check the [Path Query reference](/docs/path-query-reference-guide) for path syntax
- See the [WOQL Interactive Tutorial](/docs/woql-tutorial) for hands-on examples
- See [Troubleshooting Data Model](/docs/troubleshooting-data-model/) for schema-related query issues

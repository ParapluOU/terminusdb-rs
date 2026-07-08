---
tags:
  - woql
  - how-to
  - rdf
  - advanced
title: Execute WOQL JSON-LD Queries Directly
nextjs:
  metadata:
    title: Execute WOQL JSON-LD Queries Directly
    description: How to run raw WOQL JSON-LD queries with the JavaScript client, enabling access to new server features before client library support
    keywords: WOQL, JSON-LD, AST, raw queries, client, JavaScript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-json-ld-queries/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with a schema and data
- Familiarity with WOQL basics and JSON-LD format
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to write WOQL queries using the JSON-LD wire format.
{% /callout %}

This guide shows how to execute WOQL queries using raw JSON-LD format with the JavaScript client, enabling you to use new server features even before they have corresponding helper methods in the client library.

## Why Use JSON-LD Queries?

While the WOQL JavaScript API provides convenient builder methods like `WOQL.triple()` and `WOQL.and()`, there are scenarios where using raw JSON-LD queries is useful:

- **Early Adoption**: Use new server features before the client library adds helper methods
- **Debugging**: Inspect the exact query structure being sent to the server
- **Testing**: Validate server behavior with precise JSON-LD queries
- **Migration**: Port queries from other clients or documentation examples
- **Advanced Features**: Access experimental or specialized functionality

## The WOQL JSON-LD Format

Internally, all WOQL queries are represented as JSON-LD (JSON for Linked Data) before being sent to the TerminusDB server. The client's builder methods simply provide a convenient way to construct this JSON-LD, such as for the Javascript and Python clients.

### Example: Simple Triple Query

**WOQL Builder Syntax:**
```javascript
WOQL.triple('v:Person', 'rdf:type', '@schema:Person')
```

**Equivalent JSON-LD:**
```json
{
  "@type": "Triple",
  "subject": {
    "@type": "NodeValue",
    "variable": "Person"
  },
  "predicate": {
    "@type": "NodeValue",
    "node": "rdf:type"
  },
  "object": {
    "@type": "NodeValue",
    "node": "@schema:Person"
  }
}
```

## Executing JSON-LD Queries

The JavaScript client's `query()` method accepts both `WOQLQuery` objects and raw JSON-LD objects.

### Method 1: Direct JSON-LD with client.query()

Pass the JSON-LD object directly to `client.query()`:

```javascript
const { WOQLClient } = require('terminusdb');

const client = new WOQLClient('http://localhost:6363', {
  user: 'admin',
  organization: 'admin',
  key: 'root'
});

// Raw JSON-LD query
const jsonQuery = {
  "@type": "And",
  "and": [
    {
      "@type": "Triple",
      "subject": { "@type": "NodeValue", "variable": "Person" },
      "predicate": { "@type": "NodeValue", "node": "rdf:type" },
      "object": { "@type": "NodeValue", "node": "@schema:Person" }
    },
    {
      "@type": "Triple",
      "subject": { "@type": "NodeValue", "variable": "Person" },
      "predicate": { "@type": "NodeValue", "node": "@schema:name" },
      "object": { "@type": "NodeValue", "variable": "Name" }
    }
  ]
};

const result = await client.query(jsonQuery);
console.log(result.bindings);
```

### Method 2: Using WOQLQuery().json()

Convert between WOQL builder syntax and JSON-LD using the `json()` method:

```javascript
const { WOQL } = require('terminusdb');

// Convert WOQL to JSON-LD
const woqlQuery = WOQL.triple('v:Person', 'rdf:type', '@schema:Person');
const jsonLD = woqlQuery.json();
console.log(JSON.stringify(jsonLD, null, 2));

// Convert JSON-LD to WOQL
const fromJSON = new WOQLQuery().json(jsonLD);
const result = await client.query(fromJSON);
```

## Real-World Example: RandomKey Without Client Helper

You can use the `RandomKey` feature without the `random_idgen()` client method by passing the JSON-LD directly:

```javascript
// Using RandomKey with raw JSON-LD (works even without client helper)
const randomKeyQuery = {
  "@type": "RandomKey",
  "base": {
    "@type": "DataValue",
    "data": { "@type": "xsd:string", "@value": "Person/" }
  },
  "uri": {
    "@type": "NodeValue",
    "variable": "person_id"
  }
};

const result = await client.query(randomKeyQuery);
console.log(result.bindings[0].person_id);
// Output: "Person/aB3dEf9GhI2jK4lM" (random ID generated)
```

This is equivalent to using the client helper (once it's available):

```javascript
const result = await client.query(
  WOQL.random_idgen('Person/', 'v:person_id')
);
```

## Mixing JSON-LD with WOQL Builder

You can embed JSON-LD within WOQL builder queries for hybrid approaches:

```javascript
const query = WOQL.and(
  // Use builder method
  WOQL.triple('v:Person', 'rdf:type', '@schema:Person'),
  
  // Embed raw JSON-LD for new feature
  {
    "@type": "RandomKey",
    "base": {
      "@type": "DataValue",
      "data": { "@type": "xsd:string", "@value": "Person/" }
    },
    "uri": { "@type": "NodeValue", "variable": "new_id" }
  }
);

const result = await client.query(query);
```

## Common WOQL JSON-LD Patterns

### Data Values

String, number, and other literal values use `DataValue`:

```json
{
  "@type": "DataValue",
  "data": {
    "@type": "xsd:string",
    "@value": "Alice"
  }
}
```

For numbers:
```json
{
  "@type": "DataValue",
  "data": {
    "@type": "xsd:integer",
    "@value": 42
  }
}
```

### Node Values (Variables and IRIs)

Variables:
```json
{
  "@type": "NodeValue",
  "variable": "Person"
}
```

IRIs/Nodes:
```json
{
  "@type": "NodeValue",
  "node": "@schema:Person"
}
```

### Compound Queries

And:
```json
{
  "@type": "And",
  "and": [
    { /* query 1 */ },
    { /* query 2 */ }
  ]
}
```

Or:
```json
{
  "@type": "Or",
  "or": [
    { /* query 1 */ },
    { /* query 2 */ }
  ]
}
```

Select (variable projection):
```json
{
  "@type": "Select",
  "variables": ["Name", "Age"],
  "query": { /* subquery */ }
}
```

## Converting Existing Queries to JSON-LD

To see the JSON-LD for any WOQL query, use the `json()` method:

```javascript
const woqlQuery = WOQL.select('v:Name', 'v:Age').and(
  WOQL.triple('v:Person', 'rdf:type', '@schema:Person'),
  WOQL.triple('v:Person', '@schema:name', 'v:Name'),
  WOQL.triple('v:Person', '@schema:age', 'v:Age')
);

// View the JSON-LD
console.log(JSON.stringify(woqlQuery.json(), null, 2));
```

This outputs the complete JSON-LD structure you can use directly with `client.query()`.

## Tips and Best Practices

### 1. Start with Builder, Convert to JSON-LD

When learning the JSON-LD format, start with the builder syntax and use `json()` to see the structure:

```javascript
const builderQuery = WOQL.limit(10).triple('v:X', 'v:Y', 'v:Z');
console.log(JSON.stringify(builderQuery.json(), null, 2));
```

### 2. Validate JSON-LD Structure

Use the WOQL schema definition to validate your JSON-LD:
- Server schema: `/path/to/terminusdb/src/terminus-schema/woql.json`
- Ensures correct `@type` values and required fields

### 3. Handle Variable Names

Variables in JSON-LD don't use the `v:` prefix - just the name:

```json
{
  "@type": "NodeValue",
  "variable": "Person"  // Not "v:Person"
}
```

But in the builder syntax, you use `'v:Person'` or the `vars()` helper.

### 4. Use for Integration Tests

JSON-LD queries are excellent for integration tests as they're explicit and version-independent:

```javascript
describe('RandomKey functionality', () => {
  it('generates unique IDs', async () => {
    const query = {
      "@type": "RandomKey",
      "base": {
        "@type": "DataValue",
        "data": { "@type": "xsd:string", "@value": "Test/" }
      },
      "uri": { "@type": "NodeValue", "variable": "id" }
    };
    
    const result = await client.query(query);
    expect(result.bindings[0].id).to.include('Test/');
  });
});
```

## Error Handling

When using raw JSON-LD, be aware of common errors:

### Missing Required Fields

```json
{
  "@type": "Triple"
  // Error: Missing subject, predicate, object
}
```

### Incorrect Types

```json
{
  "@type": "TriplePattern",  // Error: Should be "Triple"
  // ...
}
```

### Invalid Variable/Node Specification

```json
{
  "@type": "NodeValue",
  "var": "Person"  // Error: Should be "variable" not "var"
}
```

The server will return detailed error messages indicating which field is problematic.

## Finding JSON-LD Examples

Several sources provide JSON-LD query examples:

1. **Client Test Suites**: Look at test files in `terminusdb-client-js/test/woqlJson/`
2. **Server Tests**: Check `terminusdb/tests/test/` for integration test examples
3. **WOQL Schema**: Review `terminusdb/src/terminus-schema/woql.json` for all query types
4. **Use json() Method**: Convert any builder query to see its JSON-LD structure

## Related Documentation

- [WOQL Basics](/docs/woql-basics/) - Learn the WOQL builder API
- [WOQL Explanation](/docs/woql-explanation/) - Understanding WOQL and JSON-LD
- [JavaScript Client Reference](/docs/javascript/) - Complete API reference
- [WOQL Schema Reference](/docs/woql-class-reference-guide/) - WOQL JSON-LD specification

## Summary

Raw JSON-LD queries provide a powerful way to:
- Access new server features immediately
- Debug and understand query structures
- Create integration tests
- Port queries between different clients

While the WOQL builder API is more convenient for everyday use, understanding and using JSON-LD directly gives you full control and enables early adoption of new TerminusDB features.

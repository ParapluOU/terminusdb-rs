---
tags:
  - graphql
  - reference
  - documents
title: GraphQL Mutations Reference
nextjs:
  metadata:
    title: GraphQL Mutations Reference | TerminusDB
    description: Learn how to insert, replace, and delete documents in TerminusDB using GraphQL mutations including _insertDocuments, _replaceDocuments, and _deleteDocuments.
    keywords: GraphQL mutations, insert documents, replace documents, delete documents, TerminusDB API, data mutations
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/graphql-filter.png?raw=true
    alternates:
      canonical: https://terminusdb.com/docs/graphql-mutations/
media: []
---

GraphQL mutations in TerminusDB allow you to modify data by inserting, replacing, and deleting documents. Mutations can be wrapped with commit information to create a new version in your database's commit history.

## Overview

TerminusDB provides three main mutation operations:

- **`_insertDocuments`** - Insert new documents into your database
- **`_replaceDocuments`** - Replace existing documents or create new ones
- **`_deleteDocuments`** - Delete documents by their IDs

All mutations may include commit metadata (`_commitInfo`) to track changes and must be executed together in a single GraphQL mutation operation.

## Quick Start: curl Example

GraphQL mutations are sent as plain HTTP POST requests to the `/api/graphql` endpoint. The query and variables are JSON fields in the request body — no special client library required:

```bash
curl -X POST http://localhost:6363/api/graphql/MyOrg/MyDatabase \
  -H "Content-Type: application/json" \
  -d '{
    "query": "mutation($input: JSON!, $author: String!, $message: String!) { _commitInfo(author: $author, message: $message) _insertDocuments(json: $input) }",
    "variables": {
      "author": "alice",
      "message": "Add person",
      "input": { "@type": "Person", "name": "Alice", "age": 30 }
    }
  }'
```

The response is standard JSON:

```json
{
  "data": {
    "_insertDocuments": ["Person/abc123"]
  }
}
```

## Commit Information

Mutation accept `_commitInfo` to record the author and message for the commit:

```graphql
mutation {
  _commitInfo(author: "your-name", message: "Describe your changes")
  # Your mutation operations here
}
```

**Parameters:**
- `author` (required): The name or identifier of the person making the change
- `message` (required): A descriptive message explaining what was changed and why

## Insert Documents

The `_insertDocuments` mutation creates new documents in your database.

### Syntax

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "alice", message: "Add new person")
  _insertDocuments(json: $input)
}
```

With variables:

```json
{
  "input": {
    "@type": "Person",
    "name": "Alice",
    "age": 30
  }
}
```

### Parameters

- **`json`** (required): A JSON document or array of documents passed as a GraphQL variable of type `JSON`. Native JSON objects are recommended; stringified JSON strings are supported for backward compatibility.
- **`graph_type`** (optional): Either `InstanceGraph` (default) for data or `SchemaGraph` for schema definitions.
- **`raw_json`** (optional, unsupported): When `true`, inserts raw JSON without schema validation (as a JSON doc). Default is `false`.

### Example: Insert a Single Document

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "alice", message: "Add new person Alice")
  _insertDocuments(json: $input)
}
```

With variables:

```json
{
  "input": {
    "@type": "Person",
    "name": "Alice",
    "age": 30
  }
}
```

### Example: Insert Multiple Documents

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "bob", message: "Bulk import people")
  _insertDocuments(json: $input)
}
```

With variables:

```json
{
  "input": [
    {"@type": "Person", "name": "Bob", "age": 25},
    {"@type": "Person", "name": "Charlie", "age": 35}
  ]
}
```

### Precision Preservation

Native JSON variables preserve full decimal precision end-to-end. When you pass a decimal as a string (e.g. `"0.98765432109876543219"`), the entire precision is retained without truncation. The JSON protocol in TerminusDB also supports arbitrary precision numbers in the wire format, but most language clients will truncate to 64-bit IEEE 754 double precision, beware!

### Backward Compatibility

Stringified JSON can be used inline in the mutation or as a variable. Inline usage requires escaping quotes with backslashes:

```graphql
mutation {
  _commitInfo(author: "alice", message: "Add person")
  _insertDocuments(
    json: "{\"@type\": \"Person\", \"name\": \"Alice\", \"age\": 30}"
  )
}
```

Or as a stringified JSON variable:

```json
{
  "input": "{\"@type\": \"Person\", \"name\": \"Alice\", \"age\": 30}"
}
```

Native JSON objects are recommended for new code since they avoid escaping and preserve numeric precision.

### Response

The mutation returns an array of document IDs for the inserted documents:

```json
{
  "data": {
    "_insertDocuments": [
      "Person/abc123",
      "Person/def456"
    ]
  }
}
```

## Replace Documents

The `_replaceDocuments` mutation updates existing documents or creates new ones if they don't exist (when `create: true`).

### Syntax

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "alice", message: "Update person details")
  _replaceDocuments(
    json: $input
    graph_type: InstanceGraph
    create: false
  )
}
```

With variables:

```json
{
  "input": {
    "@type": "Person",
    "@id": "Person/alice",
    "name": "Alice",
    "age": 31
  }
}
```

### Parameters

- **`json`** (required): A JSON document or array of documents passed as a GraphQL variable of type `JSON`. Must include `@id` field for each document.
- **`graph_type`** (optional): Either `InstanceGraph` (default) for data or `SchemaGraph` for schema.
- **`create`** (optional): When `true`, creates the document if it doesn't exist. Default is `false`.

### Example: Replace an Existing Document

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "alice", message: "Update Alice's age")
  _replaceDocuments(json: $input)
}
```

With variables:

```json
{
  "input": {
    "@type": "Person",
    "@id": "Person/alice",
    "name": "Alice",
    "age": 31
  }
}
```

### Example: Replace or Create Document

```graphql
mutation($input: JSON!) {
  _commitInfo(author: "bob", message: "Upsert person record")
  _replaceDocuments(
    json: $input
    create: true
  )
}
```

With variables:

```json
{
  "input": {
    "@type": "Person",
    "@id": "Person/david",
    "name": "David",
    "age": 28
  }
}
```

### Response

Returns an array of IDs for the replaced/created documents:

```json
{
  "data": {
    "_replaceDocuments": [
      "Person/alice"
    ]
  }
}
```

## Delete Documents

The `_deleteDocuments` mutation removes documents from your database by their IDs.

### Syntax

```graphql
mutation {
  _commitInfo(author: "alice", message: "Remove old records")
  _deleteDocuments(
    ids: ["ID1", "ID2"]
    graph_type: InstanceGraph
  )
}
```

### Parameters

- **`ids`** (required): An array of document IDs to delete.
- **`graph_type`** (optional): Either `InstanceGraph` (default) for data or `SchemaGraph` for schema.

### Example: Delete Single Document

```graphql
mutation {
  _commitInfo(author: "alice", message: "Remove person Alice")
  _deleteDocuments(
    ids: ["Person/alice"]
  )
}
```

### Example: Delete Multiple Documents

```graphql
mutation {
  _commitInfo(author: "admin", message: "Clean up test data")
  _deleteDocuments(
    ids: ["Person/test1", "Person/test2", "Person/test3"]
  )
}
```

### Response

Returns the array of deleted document IDs:

```json
{
  "data": {
    "_deleteDocuments": [
      "Person/alice",
      "Person/bob"
    ]
  }
}
```

## Combining Multiple Mutations

You can combine multiple mutation operations in a single GraphQL mutation:

```graphql
mutation($new: JSON!, $update: JSON!) {
  _commitInfo(author: "alice", message: "Update database with multiple changes")

  # Insert new documents
  _insertDocuments(json: $new)

  # Update existing documents
  _replaceDocuments(json: $update)

  # Delete unwanted documents
  _deleteDocuments(
    ids: ["Person/old_record"]
  )
}
```

With variables:

```json
{
  "new": {"@type": "Person", "name": "Eve", "age": 29},
  "update": {"@type": "Person", "@id": "Person/alice", "name": "Alice Smith", "age": 31}
}
```

## Working with Schema Mutations

You can also modify your database schema using mutations with `graph_type: SchemaGraph`:

```graphql
mutation($schema: JSON!) {
  _commitInfo(author: "admin", message: "Add new class to schema")
  _insertDocuments(
    json: $schema
    graph_type: SchemaGraph
  )
}
```

With variables:

```json
{
  "schema": {
    "@type": "Class",
    "@id": "Company",
    "name": "xsd:string",
    "employees": {"@type": "Set", "@class": "Person"}
  }
}
```

## Best Practices

1. **Always use meaningful commit messages** - Explain what changed and why for better version history.

2. **Include the `@type` field** - Always specify the document type when inserting or replacing.

3. **Use `@id` for replace operations** - Ensure your documents have explicit IDs when replacing to avoid ambiguity.

4. **Batch operations when possible** - Insert or delete multiple documents in a single mutation for better performance.

5. **Use GraphQL variables for JSON documents** - Pass documents as `JSON` type variables instead of inline stringified JSON to avoid escaping errors and preserve numeric precision.

6. **Handle errors gracefully** - Check the response for errors and handle them appropriately in your application.

7. **Use `create: true` carefully** - Only use this option when you explicitly want upsert behavior.

## Error Handling

If a mutation fails, you'll receive an error response:

```json
{
  "data": null,
  "errors": [
    {
      "message": "Document with id 'Person/unknown' not found",
      "path": ["_replaceDocuments"]
    }
  ]
}
```

Common errors include:
- **Document not found**: Attempting to replace a document that doesn't exist (without `create: true`)
- **Schema validation errors**: Inserting documents that don't match the schema
- **Missing required fields**: Not providing required fields like `@type` or `@id`
- **Invalid JSON**: Malformed JSON in the `json` parameter

## Versions

 * v12.0 series and before only support `json` documents in string format
 * v12.1 onward support `json` documents as a JSON object too

## See Also

- [GraphQL Query Reference](/docs/graphql-query-reference/) - Learn how to query your data
- [GraphQL Basics](/docs/graphql-basics/) - Get started with GraphQL queries
- [Connecting to GraphQL](/docs/connecting-to-graphql-reference/) - Set up your GraphQL endpoint
- [Document API Reference](/docs/document-insertion/) - Alternative HTTP API for document operations
- [Schema Reference](/docs/schema-reference-guide/) - Learn about TerminusDB schema design
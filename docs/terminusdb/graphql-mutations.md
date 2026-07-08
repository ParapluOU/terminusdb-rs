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
      canonical: https://terminusdb.org/docs/graphql-mutations/
media: []
---

GraphQL mutations in TerminusDB allow you to modify data by inserting, replacing, and deleting documents. Mutations can be wrapped with commit information to create a new version in your database's commit history.

## Overview

TerminusDB provides three main mutation operations:

- **`_insertDocuments`** - Insert new documents into your database
- **`_replaceDocuments`** - Replace existing documents or create new ones
- **`_deleteDocuments`** - Delete documents by their IDs

All mutations may include commit metadata (`_commitInfo`) to track changes and must be executed together in a single GraphQL mutation operation.

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
mutation {
  _commitInfo(author: "alice", message: "Add new person")
  _insertDocuments(
    json: "JSON string"
  )
}
```

### Parameters

- **`json`** (required): A JSON string containing the document(s) to insert. Can be a single document or an array of documents.
- **`graph_type`** (optional): Either `InstanceGraph` (default) for data or `SchemaGraph` for schema definitions.
- **`raw_json`** (optional, unsupported): When `true`, inserts raw JSON without schema validation (as a JSON doc). Default is `false`.

### Example: Insert a Single Document

```graphql
mutation {
  _commitInfo(author: "alice", message: "Add new person Alice")
  _insertDocuments(
    json: "{\"@type\": \"Person\", \"name\": \"Alice\", \"age\": 30}"
  )
}
```

### Example: Insert Multiple Documents

```graphql
mutation {
  _commitInfo(author: "bob", message: "Bulk import people")
  _insertDocuments(
    json: "[{\"@type\": \"Person\", \"name\": \"Bob\", \"age\": 25}, {\"@type\": \"Person\", \"name\": \"Charlie\", \"age\": 35}]"
  )
}
```

### Response

The mutation returns an array of document IDs for the inserted documents:

```json
{
  "data": {
    "_insertDocuments": [
      "terminusdb:///data/Person/abc123",
      "terminusdb:///data/Person/def456"
    ]
  }
}
```

## Replace Documents

The `_replaceDocuments` mutation updates existing documents or creates new ones if they don't exist (when `create: true`).

### Syntax

```graphql
mutation {
  _commitInfo(author: "alice", message: "Update person details")
  _replaceDocuments(
    json: "JSON string"
    graph_type: InstanceGraph
    create: false
  )
}
```

### Parameters

- **`json`** (required): A JSON string containing the document(s) to replace. Must include `@id` field for each document.
- **`graph_type`** (optional): Either `InstanceGraph` (default) for data or `SchemaGraph` for schema.
- **`create`** (optional): When `true`, creates the document if it doesn't exist. Default is `false`.

### Example: Replace an Existing Document

```graphql
mutation {
  _commitInfo(author: "alice", message: "Update Alice's age")
  _replaceDocuments(
    json: "{\"@type\": \"Person\", \"@id\": \"Person/alice\", \"name\": \"Alice\", \"age\": 31}"
  )
}
```

### Example: Replace or Create Document

```graphql
mutation {
  _commitInfo(author: "bob", message: "Upsert person record")
  _replaceDocuments(
    json: "{\"@type\": \"Person\", \"@id\": \"Person/david\", \"name\": \"David\", \"age\": 28}"
    create: true
  )
}
```

### Response

Returns an array of IDs for the replaced/created documents:

```json
{
  "data": {
    "_replaceDocuments": [
      "terminusdb:///data/Person/alice"
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
mutation {
  _commitInfo(author: "alice", message: "Update database with multiple changes")
  
  # Insert new documents
  _insertDocuments(
    json: "{\"@type\": \"Person\", \"name\": \"Eve\", \"age\": 29}"
  )
  
  # Update existing documents
  _replaceDocuments(
    json: "{\"@type\": \"Person\", \"@id\": \"Person/alice\", \"name\": \"Alice Smith\", \"age\": 31}"
  )
  
  # Delete unwanted documents
  _deleteDocuments(
    ids: ["Person/old_record"]
  )
}
```

## Working with Schema Mutations

You can also modify your database schema using mutations with `graph_type: SchemaGraph`:

```graphql
mutation {
  _commitInfo(author: "admin", message: "Add new class to schema")
  _insertDocuments(
    json: "{\"@type\": \"Class\", \"@id\": \"Company\", \"name\": \"xsd:string\", \"employees\": {\"@type\": \"Set\", \"@class\": \"Person\"}}"
    graph_type: SchemaGraph
  )
}
```

## Best Practices

1. **Always use meaningful commit messages** - Explain what changed and why for better version history.

2. **Include the `@type` field** - Always specify the document type when inserting or replacing.

3. **Use `@id` for replace operations** - Ensure your documents have explicit IDs when replacing to avoid ambiguity.

4. **Batch operations when possible** - Insert or delete multiple documents in a single mutation for better performance.

5. **Handle errors gracefully** - Check the response for errors and handle them appropriately in your application.

6. **Use `create: true` carefully** - Only use this option when you explicitly want upsert behavior.

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

## See Also

- [GraphQL Query Reference](/docs/graphql-query-reference/) - Learn how to query your data
- [GraphQL Basics](/docs/graphql-basics/) - Get started with GraphQL queries
- [Connecting to GraphQL](/docs/connecting-to-graphql-reference/) - Set up your GraphQL endpoint
- [Document API Reference](/docs/document-insertion/) - Alternative HTTP API for document operations
- [Schema Reference](/docs/schema-reference-guide/) - Learn about TerminusDB schema design
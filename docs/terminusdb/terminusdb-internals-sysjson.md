---
title: sys:JSON Internals - How It Works
nextjs:
  metadata:
    title: sys:JSON Internals - How It Works
    description: Technical deep dive into sys:JSON implementation, storage, and behavior in TerminusDB
    keywords: terminusdb, document database, documents, javascript, json-ld, sys:json internals - how it works, terminusdb javascript client, typescript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/terminusdb-internals-sysjson/
media: []
tags:
  - typescript
  - documents
  - explanation
  - advanced
---

The `sys:JSON` type in TerminusDB enables storage of arbitrary JSON data with automatic deduplication and content-addressed storage. This guide explains how `sys:JSON` works internally, its behavior characteristics, and best practices for technical users.

Additionally, the fully git-for-data semantic versioning is used to track changes to the JSON data, making for a full git-for-data experience for JSON with full version history lineage, branching, push, pull and clone.

## Overview

`sys:JSON` stores JSON values (objects, arrays, primitives) using content-addressed storage based on SHA-1 hashing. Multiple documents can safely share identical JSON structures without data duplication or consistency issues.

**Key Features:**
- **Content-addressed storage** - JSON values identified by SHA-1 hash
- **Automatic deduplication** - Identical JSON shared across documents
- **Safe deletion** - Reference counting prevents premature deletion
- **Copy-on-write semantics** - Modifications create new JSON nodes
- **All JSON types supported** - Objects, arrays, strings, numbers, booleans, null

## Storage Architecture

### Content Addressing

When you insert JSON data, TerminusDB:

1. **Computes SHA-1 hash** of the JSON value's canonical representation
2. **Checks if hash exists** in the store
3. **Reuses existing node** if found, or **creates new node** if unique
4. **References by hash** in the parent document

**Example:**

```javascript
// Document A
{
  "@type": "Person",
  "name": "Alice",
  "metadata": {
    "source": "import",
    "version": 1
  }
}

// Document B - shares same metadata
{
  "@type": "Person", 
  "name": "Bob",
  "metadata": {
    "source": "import",  // Same JSON -> same hash
    "version": 1
  }
}
```

Both documents reference the **same physical JSON node** via its hash. This saves storage and ensures consistency.

### Internal Representation

Internally, `sys:JSON` values are stored with metadata:

```javascript
{
  "@type": "sys:JSONDocument",
  "@id": "<sha1-hash>",
  "@value": <actual-json-data>
}
```

**For primitives and arrays:**
```javascript
// String
{ "@type": "sys:JSONDocument", "@id": "abc123...", "@value": "hello" }

// Array
{ "@type": "sys:JSONDocument", "@id": "def456...", "@value": [1, 2, 3] }

// Number
{ "@type": "sys:JSONDocument", "@id": "ghi789...", "@value": 42 }
```

**For objects:**
```javascript
// Object keys become fields directly (no @value wrapper)
{
  "@type": "sys:JSONDocument",
  "@id": "jkl012...",
  "name": "Alice",
  "age": 30
}
```

### Reference Counting

TerminusDB tracks how many documents reference each JSON node:

- **Insert document** → increment reference count
- **Update document** (changing JSON) → decrement old, increment new
- **Delete document** → decrement reference count
- **Count reaches 0** → Relevant JSON node(s) deleted from store

This prevents orphaned data and ensures safe concurrent operations.

## Document API Behavior

The Document API returns `sys:JSON` as **native JSON**, not strings.

```javascript
// Insert
await client.insertDocument({
  "@type": "Person",
  "metadata": {
    "source": "import",
    "tags": ["user", "verified"]
  }
});

// Get - returns native JSON
const doc = await client.getDocument("Person/123");
console.log(doc.metadata.source); // "import" (direct access)
console.log(doc.metadata.tags[0]); // "user" (array access)
```

No parsing required - it's already JSON.

## Supported JSON Types

All JSON types are supported as of **TerminusDB v11.1.x**:

### Objects
```javascript
{
  "@type": "Config",
  "settings": {
    "theme": "dark",
    "language": "en"
  }
}
```
✅ Fully supported

### Arrays
```javascript
{
  "@type": "DataSet",
  "values": [1, 2, 3, 4, 5]
}
```
✅ Fully supported

### Nested Structures
```javascript
{
  "@type": "Complex",
  "data": {
    "items": [
      {"id": 1, "tags": ["a", "b"]},
      {"id": 2, "tags": ["c"]}
    ]
  }
}
```
✅ Fully supported

### Strings
```javascript
{
  "@type": "Message",
  "content": "Hello, world!"
}
```
✅ Fully supported

### Numbers
```javascript
{
  "@type": "Metric",
  "value": 42.5
}
```
✅ Fully supported

### Booleans
```javascript
{
  "@type": "Flag",
  "enabled": true
}
```
✅ Fully supported

### Null (with caveat)
```javascript
// ✅ Null within objects/arrays
{
  "@type": "Record",
  "data": {
    "field": null,
    "items": [1, null, 3]
  }
}

// ⚠️ Top-level null has limitations
{
  "@type": "Record",
  "value": null  // Works but with special handling
}
```

**Top-level null limitation:** Due to internal storage representation, top-level null values work but may require additional handling in some query contexts. **Recommendation:** Wrap null in an object for maximum compatibility:

```javascript
// Preferred
{"status": null}  

// Instead of
null
```

## Update Behavior

### Copy-on-Write

Updating `sys:JSON` fields creates **new JSON nodes**:

```javascript
// Initial document
{
  "@type": "Config",
  "settings": {"version": 1}
}

// Update creates NEW JSON node
{
  "@type": "Config",
  "settings": {"version": 2}  // New hash, new storage
}
```

Old JSON node remains if other documents reference it.

### Multiple Updates

```javascript
// Update 1
await client.updateDocument("Config/abc", {
  "settings": {"version": 2}
});

// Update 2 - safe, creates another new node
await client.updateDocument("Config/abc", {
  "settings": {"version": 3}
});
```

Each update:
1. Creates new JSON node
2. Updates document reference
3. Decrements old node's reference count

## Deletion Behavior

### Safe Deletion

Deleting a document decrements JSON reference counts:

```javascript
// Document A and B share JSON {"type": "user"}
await client.deleteDocument("Person/A");
// JSON node still exists (B still references it)

await client.deleteDocument("Person/B");
// NOW JSON node is deleted (reference count = 0)
```

### Independent Deletion

Documents with shared JSON can be deleted independently:

```javascript
// Both documents share metadata
const doc1 = {
  "@type": "Record",
  "metadata": {"source": "import"}
};

const doc2 = {
  "@type": "Record",
  "metadata": {"source": "import"}  // Same JSON
};

await client.insertDocument(doc1);
await client.insertDocument(doc2);

// Delete doc1 - doc2 unaffected
await client.deleteDocument("Record/doc1");

// doc2 still has intact metadata
const result = await client.getDocument("Record/doc2");
console.log(result.metadata.source); // "import"
```

## Schema Definition

Define `sys:JSON` fields in your schema:

### Required Field
```javascript
{
  "@type": "Class",
  "@id": "Config",
  "settings": "sys:JSON"
}
```

### Optional Field
```javascript
{
  "@type": "Class",
  "@id": "Profile",
  "metadata": {
    "@type": "Optional",
    "@class": "sys:JSON"
  }
}
```

### Set of JSON
```javascript
{
  "@type": "Class",
  "@id": "Batch",
  "items": {
    "@type": "Set",
    "@class": "sys:JSON"
  }
}
```

### Array of JSON
```javascript
{
  "@type": "Class",
  "@id": "Collection",
  "records": {
    "@type": "Array",
    "@class": "sys:JSON"
  }
}
```


## GraphQL Behavior

GraphQL queries return `sys:JSON` fields as **JSON strings**, not objects.

### Expected Behavior

```graphql
query {
  Person {
    name
    metadata  # Returns as string
  }
}
```

**Response:**
```json
{
  "data": {
    "Person": [{
      "name": "Alice",
      "metadata": "{\"source\":\"import\",\"version\":1}"
    }]
  }
}
```

You must **parse the string** to use it as JSON:

```javascript
const parsed = JSON.parse(person.metadata);
console.log(parsed.source); // "import"
```

### Why Strings?

GraphQL has no native JSON type. Returning as string:
- ✅ **Preserves structure** - No loss of arrays, null, etc.
- ✅ **Type safe** - Client knows to parse
- ✅ **Consistent** - Works for objects, arrays, primitives

**All types serialized uniformly:**
```json
{
  "object_field": "{\"key\":\"value\"}",      // Object
  "array_field": "[1,2,3]",                   // Array
  "string_field": "\"hello\"",                // String (with quotes)
  "number_field": "42",                       // Number
  "boolean_field": "true",                    // Boolean
  "null_field": "null"                        // Null
}
```

## Performance Characteristics

### Deduplication Benefits

**Storage savings:**
```javascript
// 1000 documents with same metadata
for (let i = 0; i < 1000; i++) {
  await client.insertDocument({
    "@type": "Record",
    "id": i,
    "metadata": {"version": 1, "source": "import"}
  });
}
// Metadata stored ONCE, referenced 1000 times
```

**Update efficiency:**
```javascript
// Changing one field in large JSON
// Old: Copy entire 10MB JSON
// New: Only changed portions stored, rest referenced
```

### Query Performance

- **Read**: Hash lookup (O(1))
- **Write**: Hash computation + store (O(n) in JSON size)
- **Delete**: Reference count decrement (O(1))

**Best practice:** For frequently updated fields, use separate schema properties instead of embedding in `sys:JSON`.

## Common Patterns

### API Payload Storage

```javascript
{
  "@type": "APIRequest",
  "@id": "APIRequest/123",
  "endpoint": "/users",
  "timestamp": "2025-11-01T04:00:00Z",
  "payload": {
    "user_id": "abc123",
    "action": "login",
    "metadata": {
      "ip": "192.168.1.1",
      "user_agent": "Mozilla/5.0..."
    }
  }  // Stored as sys:JSON
}
```

### Configuration Management

```javascript
{
  "@type": "AppConfig",
  "@id": "AppConfig/production",
  "environment": "production",
  "settings": {
    "features": {
      "new_ui": true,
      "beta_api": false
    },
    "limits": {
      "max_requests": 1000,
      "timeout": 30
    }
  }
}
```

### GeoJSON Storage

```javascript
{
  "@type": "Location",
  "@id": "Location/landmark_123",
  "name": "Central Park",
  "geometry": {
    "type": "Point",
    "coordinates": [-73.965355, 40.782865]
  }  // GeoJSON as sys:JSON
}
```

### Event Logging

```javascript
{
  "@type": "Event",
  "timestamp": "2025-11-01T04:00:00Z",
  "event_type": "user_action",
  "details": {
    "action": "file_upload",
    "file_name": "document.pdf",
    "file_size": 1024000,
    "metadata": {
      "user_agent": "...",
      "ip_address": "..."
    }
  }
}
```

## Troubleshooting

### GraphQL Returns Object Instead of String

**Problem:**
```javascript
// Expected: string
// Actual: object with @value wrapper
{"@value": ["item1", "item2"]}
```

**Solution:** Ensure you're using **TerminusDB v11.1.x or later**. Earlier versions had a bug with primitive/array serialization.

### Cannot Delete Document with JSON

**Problem:**
```
Error: Cannot delete document - referenced by other documents
```

**Cause:** JSON node shared with other documents still referencing it.

**Solution:** Delete all documents referencing the JSON, or update them first.

### Top-Level Null Issues

**Problem:**
```javascript
// This may cause issues in some contexts
{"field": null}  // Top-level null
```

**Solution:** Wrap in object:
```javascript
{"data": {"field": null}}  // Nested null - always works
```

### Performance Degradation with Large JSON

**Problem:** Slow inserts/updates with multi-MB JSON values.

**Cause:** SHA-1 computation and storage overhead.

**Solutions:**
- Split large JSON into separate documents
- Use schema properties for frequently accessed fields
- Consider external blob storage for very large data

## Best Practices

### ✅ Do

- **Use for unstructured data** - API payloads, configs, arbitrary JSON
- **Leverage deduplication** - Let TerminusDB share common structures
- **Parse GraphQL strings** - Always `JSON.parse()` when using GraphQL
- **Nest null values** - Wrap null in objects for maximum compatibility
- **Version your JSON schemas** - Include version fields for evolution

### ❌ Don't

- **Don't use for frequently queried fields** - Use proper schema properties
- **Don't store very large blobs** - Consider external storage for >10MB
- **Don't assume GraphQL returns objects** - It returns strings
- **Don't rely on field ordering** - JSON object key order is not guaranteed
- **Don't use top-level null** unnecessarily - Prefer nested null

## Migration Guide

### From Previous Versions

If migrating from TerminusDB versions before v11.1.x:

1. **Test primitive types** - Arrays, strings, numbers now fully supported
2. **Update GraphQL clients** - Expect strings, not objects
3. **Re-run tests** - Verify deletion/update behavior

### Example Migration

```javascript
// Before (workaround for arrays)
{
  "@type": "Record",
  "data": {
    "items": [1, 2, 3]  // Wrapped in object
  }
}

// After (direct array support)
{
  "@type": "Record",
  "data": [1, 2, 3]  // Direct array
}
```

## Related Documentation

- [sys:JSON in Document UI](/docs/sysjson) - Using sys:JSON with the Document UI
- [Document API](/docs/document-graph-api) - Document insertion and querying
- [GraphQL Reference](/docs/graphql-query-reference) - GraphQL query syntax
- [Schema Reference](/docs/schema-reference-guide) - Complete schema definition guide

## Technical Support

For issues or questions about `sys:JSON`:

- [GitHub Issues](https://github.com/terminusdb/terminusdb/issues)
- [Discord Community](https://discord.gg/terminusdb)
- [Stack Overflow](https://stackoverflow.com/questions/tagged/terminusdb) - Tag: `terminusdb`

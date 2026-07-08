---
title: Data Model Mismatches — TerminusDB Troubleshooting
nextjs:
  metadata:
    title: Data Model Mismatches — TerminusDB Troubleshooting
    description: Fix data model issues in TerminusDB including wrong @id format, Set vs Array confusion, Optional field handling, and @key strategy problems.
    keywords: terminusdb, document database, documents, javascript, json-ld, terminusdb javascript client, typescript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/troubleshooting-data-model/
media: []
tags:
  - typescript
  - documents
  - troubleshooting
---

# Data Model Mismatches

This page covers errors that arise from misunderstanding how TerminusDB generates document identifiers, handles collection types (Set, Array, List), treats optional fields, or applies keying strategies — issues where the data structure does not behave as expected.

{% prerequisites-clone /%}

## Symptoms

- `"Document not found"` when retrieving by `@id`
- Duplicate entries appearing in a Set field
- Array elements returned in unexpected order
- Optional field present as `null` vs field entirely absent
- `@id` value does not match what you expected after insertion

## Common causes

### Document not found (wrong @id format)

**Error message:** `HTTP 404` or `"api:ErrorMessage": "Document not found"` when fetching by `@id`

**Cause:** TerminusDB generates `@id` values based on the `@key` strategy defined in the schema. If you construct an `@id` manually, it may not match the format TerminusDB actually used.

**Fix:**

1. **Understand the keying strategies:**

   | Strategy | `@id` format | Example |
   |----------|-------------|---------|
   | `Lexical` | `ClassName/key-field-value` | `Person/alice` |
   | `Hash` | `ClassName/sha256hash` | `Person/a3f2b8c...` |
   | `ValueHash` | `ClassName/sha256(all-fields)` | `Event/b7d1e4f...` |
   | `Random` | `ClassName/random-id` | `Person/lkj23sdf...` |

2. **For Lexical keys**, the `@id` is deterministic — it is built from the `@key` fields:
   ```json
   {
     "@type": "Class",
     "@id": "Person",
     "@key": { "@type": "Lexical", "@fields": ["name"] },
     "name": "xsd:string"
   }
   ```
   Inserting `{"@type": "Person", "name": "Alice"}` produces `@id = "Person/Alice"`.

3. **For Hash or Random keys**, you cannot predict the `@id`. Retrieve it from the insert response:
   ```bash
   # The response includes the generated @id
   curl -u admin:root -X POST \
     -H "Content-Type: application/json" \
     -d '{"@type": "Event", "title": "Meeting"}' \
     "http://localhost:6363/api/document/admin/tdb-example-mydb"
   ```

4. **Query by field values** instead of guessing the `@id`:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?type=Person&query=%7B%22name%22%3A%22Alice%22%7D"
   ```

### Set vs Array confusion

**Error message:** No error, but unexpected behaviour — duplicates appear or order is lost.

**Cause:** TerminusDB distinguishes between `Set`, `Array`, and `List` types, each with different semantics:

| Type | Ordered | Duplicates | Use case |
|------|---------|-----------|----------|
| `Set` | No | No (deduplicated) | Tags, categories, unique references |
| `Array` | Yes (indexed) | Yes | Ordered sequences, time series |
| `List` | Yes (linked) | Yes | Queues, ordered collections needing insertion |

**Fix:**

1. If you need **order preserved**, use `Array` or `List`, not `Set`:
   ```json
   {
     "@type": "Class",
     "@id": "Playlist",
     "tracks": { "@type": "Array", "@class": "Track" }
   }
   ```

2. If you are seeing **duplicates in a Set**, that means the items are actually different documents (different `@id` values). Check whether you are accidentally creating new documents instead of referencing existing ones.

3. When inserting into an `Array`, include the index explicitly in WOQL or let the Document API manage ordering:
   ```json
   {
     "@type": "Playlist",
     "tracks": [
       { "@type": "Track", "title": "Song A" },
       { "@type": "Track", "title": "Song B" }
     ]
   }
   ```

### Optional field handling

**Error message:** No error, but field appears as `null` in some clients or is entirely absent from the response.

**Cause:** TerminusDB represents Optional fields that have no value by omitting the field entirely from the document — it does not use `null`. Some client libraries may normalise this to `null`.

**Fix:**

1. When reading documents, check for field presence rather than checking for `null`:
   ```javascript
   // WRONG — assumes null means "not set"
   if (doc.nickname === null) { ... }

   // CORRECT — check for field existence
   if (!("nickname" in doc)) { ... }
   // or
   if (doc.nickname === undefined) { ... }
   ```

2. When inserting, simply omit optional fields you do not want to set:
   ```json
   {
     "@type": "Person",
     "name": "Alice"
   }
   ```
   Do **not** set them to `null` explicitly — this may cause a validation error.

3. Schema definition for optional fields:
   ```json
   {
     "@type": "Class",
     "@id": "Person",
     "name": "xsd:string",
     "nickname": { "@type": "Optional", "@class": "xsd:string" }
   }
   ```

### @key strategy issues (unexpected duplicates or conflicts)

**Error message:** `"api:ErrorMessage": "Document already exists"` on insert, or unintended overwrites on update

**Cause:** The `@key` strategy determines when two documents are considered "the same". With `Lexical`, documents with the same key field values are the same document. With `Hash`, every insertion with different non-key field values creates a new document.

**Fix:**

1. **If you get "already exists"** with a `Lexical` key — you are trying to insert a document with key field values that already exist. Use `replace_document` instead:
   ```bash
   curl -u admin:root -X PUT \
     -H "Content-Type: application/json" \
     -d '{"@type": "Person", "name": "Alice", "age": 31}' \
     "http://localhost:6363/api/document/admin/tdb-example-mydb"
   ```

2. **If duplicates appear** with `Hash` or `Random` keys — every insert creates a new document because the `@id` is always unique. If you want deduplication, switch to `Lexical` or `ValueHash`:
   - `Lexical` — deduplicates on specific fields you choose
   - `ValueHash` — deduplicates on all field values combined

3. **Choose your strategy based on semantics:**
   - Use `Lexical` when documents have a natural identifier (username, ISBN, SKU)
   - Use `ValueHash` when the document is defined entirely by its content (events, facts)
   - Use `Hash` or `Random` when every insertion should always create a new record

### Subdocument reference violation

**Error message:** `"vio:message": "Subdocument cannot be referenced from multiple parents"` or unexpected behaviour where editing a subdocument affects the wrong parent

**Cause:** A subdocument (`@subdocument: []`) is owned by exactly one parent document. Unlike regular documents, subdocuments cannot be shared or referenced from multiple places. If you attempt to insert the same subdocument object into two different parent documents, TerminusDB rejects the second insertion.

**Fix:**

1. If you need the same data referenced from multiple places, make it a regular document (remove `@subdocument`):
   ```json
   {
     "@type": "Class",
     "@id": "Address",
     "street": "xsd:string",
     "city": "xsd:string"
   }
   ```
   Then reference it by `@id` from multiple parents.

2. If you need embedded copies (not shared references), create separate subdocument instances in each parent:
   ```json
   {
     "@type": "Person",
     "name": "Alice",
     "home_address": { "@type": "Address", "street": "123 Main St", "city": "London" }
   }
   ```

3. Rule of thumb: use subdocuments for structures that have no identity outside their parent (line items, address blocks, metadata). Use regular documents for entities that exist independently or are referenced from multiple places.

### ValueHash key — document appears to be lost after update

**Error message:** No error, but after updating a field on a ValueHash-keyed document, the old `@id` returns 404 and you cannot find the document.

**Cause:** `ValueHash` computes the `@id` from ALL field values. Changing any field changes the `@id`. The old document effectively disappears (masked) and a new one is created with a different `@id`. This is by design — ValueHash documents are content-addressed and immutable in identity.

**Fix:**

1. **Accept the immutable semantics** — if you use ValueHash, treat documents as immutable facts. Modifications create new facts, old ones are superseded.

2. **Switch to Lexical or Random** if you need to update documents while preserving their `@id`:
   ```json
   {
     "@type": "Class",
     "@id": "Measurement",
     "@key": { "@type": "Lexical", "@fields": ["sensor_id", "timestamp"] },
     "sensor_id": "xsd:string",
     "timestamp": "xsd:dateTime",
     "value": "xsd:decimal"
   }
   ```
   Now updating `value` does not change the document's `@id`.

3. **If you must use ValueHash**, query by field values rather than storing `@id` references:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/MyDatabase?type=Measurement&query=%7B%22sensor_id%22%3A%22sensor-1%22%7D"
   ```

See [Schema Reference — Key strategies](/docs/schema-reference-guide/) for detailed documentation on when to use each keying strategy.

## Still stuck?

- [Open an issue](https://github.com/terminusdb/terminusdb/issues) with your schema definition and the operation that fails
- Check the [Schema Reference](/docs/schema-reference-guide) for `@key` strategy documentation
- Check the [Documents explanation](/docs/documents-explanation) for how TerminusDB models data
- See the [Document Types Compared](/docs/document-types-comparison) page for Set vs Array vs List details

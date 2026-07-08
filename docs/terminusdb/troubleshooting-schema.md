---
title: Schema Validation Errors — TerminusDB Troubleshooting
nextjs:
  metadata:
    title: Schema Validation Errors — TerminusDB Troubleshooting
    description: Diagnose and fix schema validation errors in TerminusDB including type mismatches on insert, missing required fields, class not found, and enum validation failures.
    keywords: terminusdb, data model, document type, javascript, schema, terminusdb javascript client, typescript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/troubleshooting-schema/
media: []
tags:
  - typescript
  - schema
  - troubleshooting
---

# Schema Validation Errors

This page covers errors that occur when inserting or updating documents that violate the database schema — type mismatches, missing required fields, references to non-existent classes, or invalid enum values.

{% prerequisites-clone /%}

## Symptoms

- `"api:ErrorMessage": "Schema check failure"` in the response
- `"vio:message": "Expected value of type ..."` violation report
- HTTP `400 Bad Request` with a witness (violation explanation) in the body
- `"@type": "vio:ViolationWithDatatypeObject"` in the error response
- Document insertion silently rejected (no error but document not persisted)

## Common causes

### Type mismatch on insert

**Error message:** `"vio:message": "Expected value of type 'xsd:integer' but got 'xsd:string'"`

**Cause:** A field value does not match the type declared in the schema. For example, passing `"age": "thirty"` when the schema expects `"age": xsd:integer`.

**Fix:**

1. Check your schema definition for the field's expected type:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&type=MyClass"
   ```
2. Correct the value to match the declared type:
   ```json
   {
     "@type": "Person",
     "name": "Alice",
     "age": 30
   }
   ```
3. Common type pitfalls:
   - Integers must be numbers, not strings: `30` not `"30"`
   - Booleans must be `true`/`false`, not `"true"`/`"false"`
   - Dates must be ISO 8601 format: `"2024-01-15"` not `"15/01/2024"`

### Missing required fields

**Error message:** `"vio:message": "Required field 'name' is missing"`

**Cause:** The schema defines a field as mandatory (not `Optional`) but the document being inserted omits it.

**Fix:**

1. Include all mandatory fields in your document. Check which fields are required:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&type=MyClass"
   ```
2. Fields declared as `"Optional"` can be omitted; all other fields are required.
3. If you want a field to be optional, update the schema:
   ```json
   {
     "@type": "Class",
     "@id": "Person",
     "name": "xsd:string",
     "nickname": { "@type": "Optional", "@class": "xsd:string" }
   }
   ```

### Class not found

**Error message:** `"vio:message": "Class 'Persn' not found in schema"` or `"@type not found"`

**Cause:** The `@type` value in your document does not match any class defined in the schema. This is usually a typo or case mismatch.

**Fix:**

1. List all classes in the schema:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema"
   ```
2. Ensure the `@type` matches exactly (case-sensitive):
   ```json
   {
     "@type": "Person",
     "name": "Alice"
   }
   ```
3. If you have not yet added the schema, insert it first:
   ```bash
   curl -u admin:root -X POST \
     -H "Content-Type: application/json" \
     -d '{"@type": "Class", "@id": "Person", "name": "xsd:string"}' \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema"
   ```

### Invalid enum value

**Error message:** `"vio:message": "Value 'active' is not a valid member of enum 'Status'"`

**Cause:** The value supplied for an enum field is not one of the declared enum members. Enum values in TerminusDB are URIs, not plain strings.

**Fix:**

1. Check the enum definition:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&id=Status"
   ```
2. Use the full enum value (class name + `/` + value):
   ```json
   {
     "@type": "Order",
     "status": "Status/Active"
   }
   ```
3. Not just the bare value:
   ```json
   {
     "@type": "Order",
     "status": "Active"
   }
   ```

### Schema check failure on replacement (incompatible migration)

**Error message:** `"api:ErrorMessage": "Schema check failure"` when replacing a schema definition via `PUT`, or `"vio:message": "Existing data violates new schema constraint"`

**Cause:** TerminusDB validates all existing data against the new schema. If you strengthen a constraint (e.g., making a previously Optional field mandatory, removing an enum value, or narrowing a type), existing documents that violate the new constraint cause the schema update to be rejected.

**Fix:**

1. **Check what data would violate the new schema** — query for documents missing the new mandatory field:
   ```bash
   curl -u admin:root \
     "http://localhost:6363/api/document/admin/MyDatabase?type=Person&query=%7B%7D" \
     | jq '.[] | select(.email == null or .email == "")'
   ```

2. **Migrate data first, then tighten the schema:**
   - Add the field as `Optional` first
   - Backfill existing documents with valid values
   - Then change the field to mandatory

3. **Use schema weakening** — changes that make the schema less restrictive always succeed:
   - Adding new Optional fields
   - Adding new classes
   - Widening a type (e.g., `xsd:integer` → `xsd:decimal`)

   See [Schema Weakening](/docs/what-is-schema-weakening/) for the full rules.

4. **For schema migrations** that require both schema and data changes in one step, see [Schema Migration Reference](/docs/schema-migration-reference-guide/).

### Circular inheritance or self-reference

**Error message:** `"api:ErrorMessage": "Cycle detected in class hierarchy"` or the schema insertion hangs/fails

**Cause:** A class definition uses `@inherits` in a way that creates a cycle — for example, Class A inherits from Class B, which inherits from Class A. TerminusDB detects inheritance cycles and rejects the schema.

**Fix:**

1. Review your class hierarchy for circular dependencies:
   ```json
   // WRONG — circular inheritance
   { "@type": "Class", "@id": "A", "@inherits": "B", "x": "xsd:string" }
   { "@type": "Class", "@id": "B", "@inherits": "A", "y": "xsd:string" }
   ```

2. Break the cycle by extracting shared fields into an abstract base class:
   ```json
   { "@type": "Class", "@id": "Base", "@abstract": [], "x": "xsd:string", "y": "xsd:string" }
   { "@type": "Class", "@id": "A", "@inherits": "Base" }
   { "@type": "Class", "@id": "B", "@inherits": "Base" }
   ```

3. For property-level self-reference (a class with a field that references itself), this is NOT a cycle error — it is valid:
   ```json
   {
     "@type": "Class",
     "@id": "TreeNode",
     "children": { "@type": "Set", "@class": "TreeNode" }
   }
   ```
   Self-referencing properties are fine; only inheritance cycles are prohibited.

## Still stuck?

- [Open an issue](https://github.com/terminusdb/terminusdb/issues) with your schema definition and the document you are trying to insert
- Check the [Schema Reference](/docs/schema-reference-guide) for complete type documentation
- Check the [Data Types reference](/docs/data-types) for supported XSD types
- See [Troubleshooting Data Model](/docs/troubleshooting-data-model/) for issues with document structure and keying
- See [Schema Migration Reference](/docs/schema-migration-reference-guide/) for safe migration strategies

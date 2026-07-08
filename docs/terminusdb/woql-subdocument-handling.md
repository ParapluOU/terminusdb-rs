---
tags:
  - woql
  - how-to
  - schema
  - documents
  - intermediate
title: Subdocument Handling with WOQL
nextjs:
  metadata:
    title: Subdocument Handling with WOQL
    description: A comprehensive guide to creating, reading, updating, and deleting subdocuments using WOQL queries in TerminusDB.
    keywords: terminusdb, data model, datalog, document, document database, document type, documents, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-subdocuments.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-subdocument-handling/
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with subdocument types in its schema
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query and manipulate subdocuments using WOQL.
{% /callout %}

Subdocuments in TerminusDB are documents that exist only as part of a parent document. Unlike regular documents, subdocuments cannot exist independently and must always be linked to a parent document through a property. This guide consolidates all subdocument operations using WOQL.

## Understanding the @linked-by Annotation

When working with subdocuments through the document interface (WOQL's `insert_document`, `update_document`, etc.), you must include the `@linked-by` annotation. This annotation serves a critical purpose for schema validation.

### Why @linked-by is Required

The `@linked-by` annotation tells the schema checker which parent document the subdocument belongs to and through which property it is linked. This is essential because:

1. **Schema validation context**: The schema checker needs to understand the document graph to validate that the subdocument is correctly typed for the property it's assigned to
2. **Graph traversal**: During document operations, TerminusDB builds an understanding of the document structure to ensure referential integrity
3. **Type checking**: The annotation allows verification that the subdocument type matches the expected type defined in the parent's schema

### Important: @linked-by is Not Stored

The `@linked-by` annotation is **not persisted in the triple store**. It is active only during document handling operations to provide the schema checker with the necessary context about the current graph structure. Once the document operation completes, only the actual data and the linking triple are stored.

### The @linked-by Structure

```javascript
{
  "@linked-by": {
    "@id": "ParentDocument/id",    // The parent document's ID
    "@property": "propertyName"     // The property linking to this subdocument
  }
}
```

## Create a Subdocument

Creating a subdocument requires two operations:
1. Insert the subdocument with `@linked-by` annotation
2. Add the triple linking the parent to the subdocument

> **Note:** `add_triple` is a low-level triple manipulation operation; schema validation still applies. Both `add_triple` and `insert_document` are valid approaches for working with data, and each has its use. Not all documents can be created by manipulating triples alone — subdocuments require `insert_document` with the `@linked-by` annotation so the schema checker can accept them. Here, `add_triple` establishes the linking triple from parent to subdocument and must be paired with `insert_document` for subdocument creation.

```javascript
let v = Vars("subdocId");
and(
  insert_document(
    doc({
      "@type": "PersonRole",
      "@linked-by": {
        "@id": "Person/John",
        "@property": "role"
      },
      title: "Manager",
      department: "Engineering"
    }),
    v.subdocId
  ),
  add_triple("Person/John", "role", v.subdocId)
)
```

The `insert_document` creates the subdocument with its properties, and `add_triple` establishes the linking triple from the parent document to the subdocument.

## Read a Subdocument

Reading a subdocument works the same as reading any document once you have its ID:

```javascript
let v = Vars("subdoc", "subdocId");
and(
  triple("Person/John", "role", v.subdocId),
  read_document(v.subdocId, v.subdoc)
)
```

This query:
1. Finds the subdocument ID linked from `Person/John` via the `role` property
2. Reads the complete subdocument into the `subdoc` variable

## Update a Subdocument

Subdocuments cannot be updated in place using `update_document` (which only works for top-level documents). Instead, you must delete the old subdocument and create a new one with the updated content.

This approach works best with random-keyed identifiers on subdocuments:

```javascript
let v = Vars("parentDoc", "oldSubdoc", "newSubdoc");
select(v.oldSubdoc, v.newSubdoc).and(
  eq(v.oldSubdoc, "Person/John/role/PersonRole/cxW1Egirxm8-QYrq"),
  triple(v.parentDoc, "role", v.oldSubdoc),
  
  delete_document(v.oldSubdoc),
  
  insert_document(
    doc({
      "@type": "PersonRole",
      "@linked-by": {
        "@id": v.parentDoc,
        "@property": "role"
      },
      title: "Senior Manager",
      department: "Engineering"
    }),
    v.newSubdoc
  ),
  
  update_triple(v.parentDoc, "role", v.newSubdoc, v.oldSubdoc)
)
```

This query:
1. Finds the parent document that links to the subdocument
2. Deletes the old subdocument
3. Creates a new subdocument with updated values (including the required `@linked-by`)
4. Updates the linking triple to point to the new subdocument

The `select` at the beginning ensures the old and new subdocument IDs are returned in the bindings.

## Delete a Subdocument

Deleting a subdocument requires removing both the subdocument and the triple that links it from the parent:

```javascript
let v = Vars("parentDoc", "subdocId");
and(
  eq(v.subdocId, "Person/John/role/PersonRole/cxW1Egirxm8-QYrq"),
  triple(v.parentDoc, "role", v.subdocId),
  delete_document(v.subdocId),
  delete_triple(v.parentDoc, "role", v.subdocId)
)
```

If you only delete the subdocument without removing the linking triple, you will have a dangling reference in your data.

## Working with Multiple Subdocuments

When a property can hold multiple subdocuments (Set or List types), you can iterate over them:

```javascript
let v = Vars("person", "roleId", "roleDoc");
and(
  isa(v.person, "Person"),
  triple(v.person, "roles", v.roleId),
  read_document(v.roleId, v.roleDoc)
)
```

To add an additional subdocument to a Set or List property:

```javascript
let v = Vars("newRoleId");
and(
  insert_document(
    doc({
      "@type": "PersonRole",
      "@linked-by": {
        "@id": "Person/John",
        "@property": "roles"
      },
      title: "Consultant"
    }),
    v.newRoleId
  ),
  add_triple("Person/John", "roles", v.newRoleId)
)
```

## Best Practices

1. **Always include @linked-by**: When using document interface operations (`insert_document`, etc.) with subdocuments, always include the `@linked-by` annotation to ensure proper schema validation

2. **Use random keys for updatable subdocuments**: If you need to update subdocuments frequently, use `@key: { "@type": "Random" }` in your schema to generate unique IDs that don't depend on content

3. **Maintain referential integrity**: Always delete the linking triple when deleting a subdocument, and always create the linking triple when creating a subdocument

4. **Consider using top-level document updates**: For complex subdocument updates, it may be simpler to read the entire parent document, modify it in your application, and use `update_document` on the parent

## See Also

- [Add Documents with WOQL](/docs/add-documents-with-woql/) - General document insertion
- [Edit Documents with WOQL](/docs/edit-documents-with-woql/) - Document updates
- [Delete Documents with WOQL](/docs/delete-documents-with-woql/) - Document deletion
- [Schema Reference](/docs/schema-reference-guide/) - Subdocument schema definitions

---
tags:
  - troubleshooting
  - schema
  - curl
  - advanced
title: Troubleshooting Document ID Migration
nextjs:
  metadata:
    title: Troubleshooting Document ID Migration
    description: How to fix subdocument ID prefix mismatches after schema changes using the ChangeKey migration operation.
    keywords: terminusdb, data model, debug, document, document type, error, schema, troubleshooting
---

# Troubleshooting Document ID Migration

When working with subdocuments in TerminusDB, you may encounter an error like:

{% prerequisites-clone /%}

```
submitted_document_id_does_not_have_expected_prefix
```

This typically happens when subdocument IDs no longer match the prefix expected by the current schema, for example after a database restore, manual data manipulation, or changes to the `@base` prefix.

## Understanding the Problem

TerminusDB generates subdocument IDs by combining the parent document's ID with the property path and the subdocument's key. For a `Parent` document with ID `Parent/abc123` containing a `Child` subdocument on the `child` property, the expected subdocument ID prefix would be:

```
terminusdb:///data/Parent/abc123/child/Child/
```

If the subdocument's actual ID does not start with this prefix, operations like `diff`, `patch`, and `replace_document` will fail with the prefix mismatch error.

## Solution: Use ChangeKey Migration

The `ChangeKey` migration operation can fix non-conforming subdocument IDs while preserving document data and top-level IDs.

### Same-Strategy ChangeKey (Recommended for Fixing Prefixes)

When you apply a `ChangeKey` with the same key strategy that the class already uses (e.g., Random to Random), the migration:

1. **Preserves** the top-level document ID
2. **Preserves** subdocument IDs that already conform to the expected prefix
3. **Regenerates** only the subdocument IDs that have non-conforming prefixes
4. Cascades regeneration to descendants when a parent subdocument ID is regenerated

This is the safest approach because it minimizes the number of ID changes.

### Example: Fix Subdocument IDs via the API

```bash
curl -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -u admin:root \
  -d '{
    "author": "admin",
    "message": "Fix non-conforming subdocument IDs",
    "operations": [{
      "@type": "ChangeKey",
      "class": "MyParentClass",
      "key": "Random"
    }]
  }'
```

Replace `MyParentClass` with the class that contains the subdocuments with non-conforming IDs. Use the same key type that the class currently uses.

### Targeting a Subdocument Class Directly

You can also target the subdocument class itself. The migration will automatically find all root parent documents that contain instances of the targeted subdocument class and reprocess them:

```bash
curl -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -u admin:root \
  -d '{
    "author": "admin",
    "message": "Fix Child subdocument IDs",
    "operations": [{
      "@type": "ChangeKey",
      "class": "MySubdocumentClass",
      "key": "Random"
    }]
  }'
```

### Changing Key Strategy

If you want to change the key strategy entirely (e.g., from Random to Lexical), all IDs including the top-level document ID will be regenerated. References from other documents are updated automatically:

```bash
curl -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -u admin:root \
  -d '{
    "author": "admin",
    "message": "Change to Lexical keys",
    "operations": [{
      "@type": "ChangeKey",
      "class": "MyClass",
      "key": "Lexical",
      "fields": ["name"]
    }]
  }'
```

## Verifying the Fix

After running the migration, verify that your documents can be retrieved and modified without errors:

```bash
# Get documents of the migrated class
curl "http://localhost:6363/api/document/admin/tdb-example-mydb?type=MyParentClass&as_list=true" \
  -u admin:root

# Try a replace operation to confirm IDs are valid
curl -X PUT "http://localhost:6363/api/document/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -u admin:root \
  -d '{ ... your document ... }'
```

## Related

- [Schema Migration Reference Guide](/docs/schema-migration-reference-guide) for all available migration operations

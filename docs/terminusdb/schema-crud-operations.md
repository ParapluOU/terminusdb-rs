---
title: Schema CRUD Operations
nextjs:
  metadata:
    title: Schema CRUD Operations — TerminusDB
    description: Create, read, update, and delete schema classes and properties in TerminusDB using the TypeScript client, Python client, or HTTP API.
    keywords: terminusdb, schema, crud, create schema, read schema, update schema, delete schema, graph_type, schema operations, typescript client, python client, http api
    alternates:
      canonical: https://terminusdb.org/docs/schema-crud-operations/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
tags:
  - typescript
  - python
  - schema
  - how-to
  - intermediate
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A database created ([create database guide](/docs/create-a-database/))
- Familiarity with schema concepts ([schema reference](/docs/schema-reference-guide/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will be able to create, inspect, modify, and delete schema classes in TerminusDB using the HTTP API, TypeScript client, or Python client.
{% /callout %}

Schema defines the structure of your documents — their types, properties, keys, and relationships. All schema operations use the **Document API** with `graph_type=schema`. This guide covers the full lifecycle: creating classes, inspecting existing schema, updating fields, and removing types.

---

{% prerequisites-clone /%}

## Create schema classes

Add new document types to your database. Each class is a JSON object with `@type: "Class"`.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const createSchema = async () => {
  const schema = [
    {
      "@type": "Class",
      "@id": "Country",
      "@key": { "@type": "Lexical", "@fields": ["name"] },
      "name": "xsd:string",
      "population": { "@type": "Optional", "@class": "xsd:integer" },
    },
    {
      "@type": "Class",
      "@id": "Person",
      "@key": { "@type": "Lexical", "@fields": ["name"] },
      "name": "xsd:string",
      "age": "xsd:integer",
      "nationality": "Country",
    },
  ]

  const result = await client.addDocument(schema, { graph_type: "schema" })
  console.log("Created:", result)
  // ["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
schema = [
    {
        "@type": "Class",
        "@id": "Country",
        "@key": {"@type": "Lexical", "@fields": ["name"]},
        "name": "xsd:string",
        "population": {"@type": "Optional", "@class": "xsd:integer"},
    },
    {
        "@type": "Class",
        "@id": "Person",
        "@key": {"@type": "Lexical", "@fields": ["name"]},
        "name": "xsd:string",
        "age": "xsd:integer",
        "nationality": "Country",
    },
]

results = client.insert_document(schema, graph_type="schema")
print("Created:", results)
# ["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Add+classes" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "@type": "Class",
      "@id": "Country",
      "@key": {"@type": "Lexical", "@fields": ["name"]},
      "name": "xsd:string",
      "population": {"@type": "Optional", "@class": "xsd:integer"}
    },
    {
      "@type": "Class",
      "@id": "Person",
      "@key": {"@type": "Lexical", "@fields": ["name"]},
      "name": "xsd:string",
      "age": "xsd:integer",
      "nationality": "Country"
    }
  ]'
```

**Expected response:**

```json
["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" %}
Schema changes create a new commit. If instance documents already exist that violate the new schema, the operation will fail. Use [schema migration](/docs/schema-migration-reference-guide/) to transform existing data during schema changes.
{% /callout %}

---

## Read schema (inspect classes)

Retrieve your schema to inspect existing classes, their fields, and relationships. Use the same Document API with `graph_type=schema` for GET operations.

### List all schema classes

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const listSchema = async () => {
  const classes = await client.getDocument({
    graph_type: "schema",
    as_list: true,
  })
  console.log("Schema classes:", classes)
}
```

Returns all schema objects including the `@context`:

```json
[
  {
    "@type": "@context",
    "@schema": "terminusdb:///schema#",
    "@base": "terminusdb:///data/"
  },
  {
    "@type": "Class",
    "@id": "Country",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "population": {"@type": "Optional", "@class": "xsd:integer"}
  },
  {
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": "Country"
  }
]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
classes = list(client.get_all_documents(graph_type="schema"))
print("Schema classes:", classes)
```

Returns:

```python
[
    {"@type": "@context", "@schema": "terminusdb:///schema#", "@base": "terminusdb:///data/"},
    {"@type": "Class", "@id": "Country", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "population": {"@type": "Optional", "@class": "xsd:integer"}},
    {"@type": "Class", "@id": "Person", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "age": "xsd:integer", "nationality": "Country"},
]
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&as_list=true"
```

**Expected response:**

```json
[
  {"@type": "@context", "@schema": "terminusdb:///schema#", "@base": "terminusdb:///data/"},
  {"@type": "Class", "@id": "Country", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "population": {"@type": "Optional", "@class": "xsd:integer"}},
  {"@type": "Class", "@id": "Person", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "age": "xsd:integer", "nationality": "Country"}
]
```
{% /code-tab %}
{% /code-tabs %}

### Get a single class definition

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const getClass = async () => {
  const personClass = await client.getDocument({
    graph_type: "schema",
    id: "Person",
  })
  console.log("Person class:", personClass)
}
```

Returns:

```json
{
  "@type": "Class",
  "@id": "Person",
  "@key": {"@type": "Lexical", "@fields": ["name"]},
  "name": "xsd:string",
  "age": "xsd:integer",
  "nationality": "Country"
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
person_class = client.get_document("Person", graph_type="schema")
print("Person class:", person_class)
```

Returns:

```json
{"@type": "Class", "@id": "Person", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "age": "xsd:integer", "nationality": "Country"}
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&id=Person"
```

**Expected response:**

```json
{
  "@type": "Class",
  "@id": "Person",
  "@key": {"@type": "Lexical", "@fields": ["name"]},
  "name": "xsd:string",
  "age": "xsd:integer",
  "nationality": "Country"
}
```
{% /code-tab %}
{% /code-tabs %}

### Filter by type

Retrieve only classes (excluding the `@context` object):

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const getClassesOnly = async () => {
  const classes = await client.getDocument({
    graph_type: "schema",
    type: "Class",
    as_list: true,
  })
  console.log("Document classes:", classes)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
classes = list(client.get_all_documents(graph_type="schema", doc_type="Class"))
print("Document classes:", classes)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&type=Class&as_list=true"
```
{% /code-tab %}
{% /code-tabs %}

---

## Update schema (modify classes)

Update an existing class definition using PUT (full replacement of the class document). The class `@id` identifies which class to update.

### Add a field to an existing class

To add an **optional** field, simply PUT the class with the new field included. Optional fields are backward-compatible — existing documents remain valid.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const addOptionalField = async () => {
  const updatedPerson = {
    "@type": "Class",
    "@id": "Person",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": "Country",
    "email": { "@type": "Optional", "@class": "xsd:string" },
  }

  await client.updateDocument(updatedPerson, { graph_type: "schema" })
  console.log("Added email field to Person")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
updated_person = {
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": "Country",
    "email": {"@type": "Optional", "@class": "xsd:string"},
}

client.update_document(updated_person, graph_type="schema")
print("Added email field to Person")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Add+email+field" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": "Country",
    "email": {"@type": "Optional", "@class": "xsd:string"}
  }'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" %}
**Adding a required field** (not wrapped in `Optional`) is a **strengthening** operation. It will fail if documents of that type already exist — they would lack the new required field. Use [schema migration](/docs/schema-migration-reference-guide/) with a `CreateClassProperty` operation and a default value instead.
{% /callout %}

### Change a field type (schema weakening)

Widening a type (e.g., from a specific class to a more general one) is a **weakening** operation and succeeds directly:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const widenField = async () => {
  // Change nationality from required "Country" to optional
  const updatedPerson = {
    "@type": "Class",
    "@id": "Person",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": { "@type": "Optional", "@class": "Country" },
    "email": { "@type": "Optional", "@class": "xsd:string" },
  }

  await client.updateDocument(updatedPerson, { graph_type: "schema" })
  console.log("Made nationality optional")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
updated_person = {
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": {"@type": "Optional", "@class": "Country"},
    "email": {"@type": "Optional", "@class": "xsd:string"},
}

client.update_document(updated_person, graph_type="schema")
print("Made nationality optional")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Make+nationality+optional" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "nationality": {"@type": "Optional", "@class": "Country"},
    "email": {"@type": "Optional", "@class": "xsd:string"}
  }'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="note" %}
**Schema weakening** (making fields optional, adding optional fields, adding new classes) always succeeds because existing data still conforms. **Schema strengthening** (making fields required, narrowing types, removing fields) requires migration. See [What is Schema Weakening?](/docs/what-is-schema-weakening/) for details.
{% /callout %}

---

## Delete schema classes

Remove a class from your schema. You must first remove all instance documents of that type and any properties in other classes that reference it.

### Delete a class

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const deleteClass = async () => {
  await client.deleteDocument({
    graph_type: "schema",
    id: "Country",
  })
  console.log("Deleted Country class")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.delete_document("Country", graph_type="schema")
print("Deleted Country class")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Delete+Country+class" \
  -H "Content-Type: application/json" \
  -d '["Country"]'
```

**Expected response:**

```json
["terminusdb:///schema#Country"]
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" %}
**Deletion order matters.** You cannot delete a class that is referenced by other classes. First remove or update properties that reference the class, then delete it. If instance documents of the class exist, delete those first.
{% /callout %}

### Safe deletion sequence

When removing a class that is referenced elsewhere:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const safeDeleteCountry = async () => {
  // 1. Remove field referencing Country from Person
  const updatedPerson = {
    "@type": "Class",
    "@id": "Person",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "age": "xsd:integer",
    "email": { "@type": "Optional", "@class": "xsd:string" },
    // nationality removed
  }
  await client.updateDocument(updatedPerson, { graph_type: "schema" })

  // 2. Delete all Country instance documents
  const countries = await client.getDocument({ type: "Country", as_list: true })
  if (countries.length > 0) {
    const ids = countries.map((c: { "@id": string }) => c["@id"])
    await client.deleteDocument({ id: ids })
  }

  // 3. Now safe to delete the class
  await client.deleteDocument({ graph_type: "schema", id: "Country" })
  console.log("Country class safely deleted")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
# 1. Remove field referencing Country from Person
updated_person = {
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "email": {"@type": "Optional", "@class": "xsd:string"},
    # nationality removed
}
client.update_document(updated_person, graph_type="schema")

# 2. Delete all Country instance documents
countries = list(client.get_all_documents(doc_type="Country"))
if countries:
    ids = [c["@id"] for c in countries]
    client.delete_document(ids)

# 3. Now safe to delete the class
client.delete_document("Country", graph_type="schema")
print("Country class safely deleted")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
# 1. Update Person to remove nationality field
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Remove+nationality" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Class",
    "@id": "Person",
    "@key": {"@type": "Lexical", "@fields": ["name"]},
    "name": "xsd:string",
    "age": "xsd:integer",
    "email": {"@type": "Optional", "@class": "xsd:string"}
  }'

# 2. Delete Country instance documents (if any)
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?author=admin&message=Delete+countries&type=Country"

# 3. Delete the Country class
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Delete+Country+class" \
  -H "Content-Type: application/json" \
  -d '["Country"]'
```
{% /code-tab %}
{% /code-tabs %}

---

## Full schema replacement

Replace the entire schema in one operation. This is useful for CI/CD pipelines or programmatic schema management where you maintain the schema as code.

{% callout type="warning" %}
Full replacement overwrites **all** schema documents. If your new schema is incompatible with existing instance data, the operation will fail. Use this for new databases or with [schema migration](/docs/schema-migration-reference-guide/) for existing data.
{% /callout %}

### Replace all schema classes

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const replaceFullSchema = async () => {
  const fullSchema = [
    {
      "@type": "@context",
      "@schema": "terminusdb:///schema#",
      "@base": "terminusdb:///data/",
    },
    {
      "@type": "Class",
      "@id": "Product",
      "@key": { "@type": "Lexical", "@fields": ["sku"] },
      "sku": "xsd:string",
      "name": "xsd:string",
      "price": "xsd:decimal",
      "category": { "@type": "Optional", "@class": "xsd:string" },
    },
    {
      "@type": "Class",
      "@id": "Order",
      "@key": { "@type": "Random" },
      "product": "Product",
      "quantity": "xsd:integer",
      "placed_at": "xsd:dateTime",
    },
  ]

  await client.updateDocument(fullSchema, {
    graph_type: "schema",
    create: true,
  })
  console.log("Full schema replaced")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
full_schema = [
    {
        "@type": "@context",
        "@schema": "terminusdb:///schema#",
        "@base": "terminusdb:///data/",
    },
    {
        "@type": "Class",
        "@id": "Product",
        "@key": {"@type": "Lexical", "@fields": ["sku"]},
        "sku": "xsd:string",
        "name": "xsd:string",
        "price": "xsd:decimal",
        "category": {"@type": "Optional", "@class": "xsd:string"},
    },
    {
        "@type": "Class",
        "@id": "Order",
        "@key": {"@type": "Random"},
        "product": "Product",
        "quantity": "xsd:integer",
        "placed_at": "xsd:dateTime",
    },
]

client.update_document(full_schema, graph_type="schema", create=True)
print("Full schema replaced")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Replace+schema&create=true" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "@type": "@context",
      "@schema": "terminusdb:///schema#",
      "@base": "terminusdb:///data/"
    },
    {
      "@type": "Class",
      "@id": "Product",
      "@key": {"@type": "Lexical", "@fields": ["sku"]},
      "sku": "xsd:string",
      "name": "xsd:string",
      "price": "xsd:decimal",
      "category": {"@type": "Optional", "@class": "xsd:string"}
    },
    {
      "@type": "Class",
      "@id": "Order",
      "@key": {"@type": "Random"},
      "product": "Product",
      "quantity": "xsd:integer",
      "placed_at": "xsd:dateTime"
    }
  ]'
```
{% /code-tab %}
{% /code-tabs %}

### The @context object

When replacing the full schema, always include the `@context` object first. It defines the URI prefixes for your schema and instance data:

```json
{
  "@type": "@context",
  "@schema": "terminusdb:///schema#",
  "@base": "terminusdb:///data/",
  "xsd": "http://www.w3.org/2001/XMLSchema#"
}
```

| Field | Purpose | Default |
|-------|---------|---------|
| `@schema` | URI prefix for class names | `terminusdb:///schema#` |
| `@base` | URI prefix for document IDs | `terminusdb:///data/` |
| Custom prefixes | Additional URI namespaces | (none) |

If you omit `@context` from a full replacement, TerminusDB uses the defaults. Custom prefixes you previously defined will be lost.

---

## Schema migration (breaking changes)

When schema changes conflict with existing instance data, use the **Migration API** to transform data automatically. Migrations handle operations that direct PUT cannot:

- Adding **required** fields (with default values for existing documents)
- **Renaming** properties (preserving data)
- **Casting** field types (with type conversion)
- **Deleting** classes (removing instance data)

### Example: add a required field with a default

```bash
curl -u admin:root -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -d '{
    "author": "admin",
    "message": "Add required sku field with default",
    "operations": [
      {
        "@type": "CreateClassProperty",
        "class": "Product",
        "property": "sku",
        "type": "xsd:string",
        "default": {"@type": "Default", "value": "UNKNOWN"}
      }
    ]
  }'
```

All existing `Product` documents receive `"sku": "UNKNOWN"` automatically.

### Preview with dry-run

Test migrations without applying them:

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/migration/admin/tdb-example-mydb?dry_run=true" \
  -H "Content-Type: application/json" \
  -d '{
    "author": "admin",
    "message": "Preview migration",
    "operations": [
      {"@type": "CreateClassProperty", "class": "Product", "property": "sku", "type": "xsd:string", "default": {"@type": "Default", "value": "UNKNOWN"}}
    ]
  }'
```

For the full migration operation reference, see [Schema Migration Reference](/docs/schema-migration-reference-guide/).

---

## Common patterns

### Schema as code (CI/CD)

Maintain your schema in version control and apply it on deployment:

```typescript
import { readFileSync } from "fs"

const deploySchema = async () => {
  const schema = JSON.parse(readFileSync("./schema.json", "utf-8"))
  await client.updateDocument(schema, {
    graph_type: "schema",
    create: true,
  })
  console.log("Schema deployed from schema.json")
}
```

### Inspect schema changes (diff)

Compare schema between branches to review changes before merging:

```bash
curl -u admin:root \
  "http://localhost:6363/api/diff/admin/tdb-example-mydb/local/branch/main/local/branch/feature?document_id=Person&graph_type=schema"
```

### Branch-based schema development

Develop schema changes on a branch, test them, then merge:

```typescript
const schemaOnBranch = async () => {
  // Create a feature branch
  await client.branch("schema-update")
  client.checkout("schema-update")

  // Make schema changes on the branch
  const newClass = {
    "@type": "Class",
    "@id": "Address",
    "@key": { "@type": "Random" },
    "street": "xsd:string",
    "city": "xsd:string",
    "postcode": "xsd:string",
  }
  await client.addDocument(newClass, { graph_type: "schema" })

  // Test, then merge back to main
  client.checkout("main")
  await client.rebase({ rebase_from: "admin/tdb-example-mydb/local/branch/schema-update" })
}
```

---

## Next steps

- [Schema Reference Guide](/docs/schema-reference-guide/) — complete schema language specification
- [Schema Migration](/docs/schema-migration-reference-guide/) — transform data during breaking changes
- [Schema Weakening](/docs/what-is-schema-weakening/) — understand backward-compatible changes
- [Add Documents](/docs/add-a-document/) — insert data conforming to your schema
- [Schema Queries with WOQL](/docs/schema-queries-with-woql/) — query schema programmatically with WOQL

---
tags:
  - woql
  - how-to
  - data-import
  - rdf
  - intermediate
title: Import JSON-LD Documents with WOQL
nextjs:
  metadata:
    title: Import JSON-LD Documents into TerminusDB with WOQL
    description: TerminusDB stores data as RDF triples and presents it as JSON-LD. This guide shows how to load existing JSON-LD from files, APIs, or other sources using the Document API or WOQL.
    keywords: JSON-LD, import, WOQL, insert_document, migration
    alternates:
      canonical: https://terminusdb.org/docs/import-jsonld-woql/
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with a schema
- JSON-LD data to import
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to import JSON-LD data into TerminusDB using WOQL.
{% /callout %}

TerminusDB stores all data natively as RDF triples and presents it as JSON-LD — inserting a JSON-LD document is the standard write operation, not a special import step. This guide covers how to bring in existing JSON-LD data from external sources: files, APIs, or other TerminusDB instances.

{% callout type="note" title="Enterprise edition" %}
Native `@context` processing for JSON-LD is available in the TerminusDB Enterprise Edition, together with Turtle and XML document interface support. The standard edition supports JSON-LD documents without custom `@context` declarations.
{% /callout %}

## Method 1: HTTP Document API (recommended)

The simplest way to import JSON-LD documents is the Document API. Each document must have `@type` and `@id` fields matching your schema:

```bash
# Insert a single JSON-LD document
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=admin&message=Import+documents" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Person",
    "@id": "Person/jane",
    "name": "Jane Smith",
    "email": "jane@example.com"
  }'
```

For multiple documents, send a JSON array:

```bash
# Import multiple documents at once
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=admin&message=Bulk+import" \
  -H "Content-Type: application/json" \
  -d '[
    {"@type": "Person", "@id": "Person/jane", "name": "Jane Smith", "email": "jane@example.com"},
    {"@type": "Person", "@id": "Person/joe", "name": "Joe Bloggs", "email": "joe@example.com"}
  ]'
```

## Method 2: Schemaless import with raw_json

If your JSON-LD does not match an existing schema — or you want to import it without defining a schema first — use `raw_json=true`:

```bash
# Import arbitrary JSON-LD without schema validation
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=admin&message=Import+raw&raw_json=true" \
  -H "Content-Type: application/json" \
  -d '{
    "@id": "terminusdb:///data/org/acme",
    "@type": "http://schema.org/Organization",
    "http://schema.org/name": "Acme Corp",
    "http://schema.org/url": "https://acme.example.com"
  }'
```

This preserves the full JSON-LD structure including external vocabulary IRIs.

## Method 3: WOQL insert_document

Use `insert_document` in WOQL when you need to programmatically generate documents or transform data during import:

```typescript
import TerminusClient from "terminusdb"

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root",
})
await client.connect()
client.db("MyDatabase")

// Import documents using WOQL insert_document
const WOQL = TerminusClient.WOQL

const people = [
  { "@type": "Person", "name": "Jane Smith", "email": "jane@example.com" },
  { "@type": "Person", "name": "Joe Bloggs", "email": "joe@example.com" },
]

for (const person of people) {
  await client.query(
    WOQL.insert_document(person, "v:Id"),
    `Import ${person.name}`
  )
}
```

## Method 4: Import from a file (Python)

Read a JSON-LD file and insert all documents:

```python
import json
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="MyDatabase")

# Read JSON-LD file (array of documents)
with open("people.jsonld", "r") as f:
    documents = json.load(f)

# Insert all documents in one commit
result = client.insert_document(
    documents,
    commit_msg="Import from people.jsonld"
)
print(f"Imported {len(result)} documents")
```

## Tips

- **Schema first:** Define your schema before importing typed documents. The database rejects documents that do not match the schema unless you use `raw_json=true`.
- **Batch size:** For large imports (thousands of documents), send batches of 100–500 documents per request to avoid timeouts.
- **Idempotency:** If a document with the same `@id` already exists, use `PUT` (replace) instead of `POST` (insert) to update it.
- **Context handling:** TerminusDB uses its own `@context` for prefix mappings. External JSON-LD `@context` fields are not preserved — map external IRIs to your schema types before import.

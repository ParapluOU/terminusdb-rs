---
title: Add a Schema
nextjs:
  metadata:
    title: Add a Schema — TerminusDB
    description: Define and add a schema to your TerminusDB database using the HTTP API, TypeScript client, or Python client.
    keywords: terminusdb, schema, add schema, class, document type, data model, javascript client, python client, http api
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/add-a-schema/
media: []
tags:
  - typescript
  - python
  - schema
  - how-to
  - beginner
---

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363` — see [installation guide](/docs/install-terminusdb-as-a-docker-container/)
- A database created: [Create a Database guide](/docs/create-a-database/)
- A connected client: [TypeScript](/docs/connect-with-the-javascript-client/) or [Python](/docs/connect-with-python-client/)
{% /callout %}

{% prerequisites-clone /%}


{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will have defined a schema (document types) in your TerminusDB database using the HTTP API, TypeScript, or Python.
{% /callout %}

## Define a schema

A schema in TerminusDB defines your document types — their fields, data types, keys, and relationships. Schema documents are JSON objects with `@type: "Class"`.

### HTTP API

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=admin&message=Add+schema" \
  -H "Content-Type: application/json" \
  -d '[
    {"@type": "Class", "@id": "Country", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string"},
    {"@type": "Class", "@id": "Person", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "nationality": "Country"}
  ]'
```

**Expected response:**

```json
["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const schema = [
  {
    "@type": "Class",
    "@id": "Country",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
  },
  {
    "@type": "Class",
    "@id": "Person",
    "@key": { "@type": "Lexical", "@fields": ["name"] },
    "name": "xsd:string",
    "nationality": "Country",
  },
];

const result = await client.addDocument(schema, { graph_type: "schema" });
console.log("Schema created:", result);
// ["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
schema = [
    {"@type": "Class", "@id": "Country", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string"},
    {"@type": "Class", "@id": "Person", "@key": {"@type": "Lexical", "@fields": ["name"]}, "name": "xsd:string", "nationality": "Country"},
]

results = client.insert_document(schema, graph_type="schema")
print("Schema created:", results)
# ["terminusdb:///schema#Country", "terminusdb:///schema#Person"]
```
{% /code-tab %}
{% /code-tabs %}

---

## Schema concepts

| Concept | Description | Example |
|---------|-------------|---------|
| `@type: "Class"` | Declares a document type | `{"@type": "Class", "@id": "Person"}` |
| `@key` | Determines how document IDs are generated | `{"@type": "Lexical", "@fields": ["name"]}` |
| Field types | Properties use XSD types or references to other classes | `"name": "xsd:string"`, `"nationality": "Country"` |
| `@subdocument` | Embedded objects without independent identity | `"@subdocument": []` |

### Key strategies

- **Lexical** — deterministic IDs from field values (e.g., `Person/Alice`)
- **Hash** — content-addressed IDs from a hash of specified fields
- **ValueHash** — IDs derived from all field values (immutable documents)
- **Random** — auto-generated unique IDs (default)

---

## Next steps

- [Add Documents](/docs/add-a-document/) — insert data conforming to your schema
- [Schema Reference](/docs/schema-reference-guide/) — complete schema specification
- [Edit Documents](/docs/edit-a-document/) — update existing documents

---
title: Add Documents
nextjs:
  metadata:
    title: Add Documents
    description: How to insert documents into TerminusDB using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, add document, insert document, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/add-a-document
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-use-add-documents.png?raw=true
tags:
  - typescript
  - python
  - documents
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A connected client instance with a schema defined
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have inserted documents into your TerminusDB database using either the JavaScript client, the Python client, or the HTTP API directly.
{% /callout %}

After you have connected to a database and [added a schema](/docs/add-a-schema/), you can insert documents that conform to the schema.

{% prerequisites-clone /%}

## Insert a single document

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const doc = {
    "@type" : "Player",
    name    : "George",
    position: "Center Back",
}

const addDoc = async () => {
  const result = await client.addDocument(doc)
  console.log("Document added:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
document = {"@type": "Player", "name": "George", "position": "Center Back"}
result = client.insert_document(document)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Add+document" \
  -H "Content-Type: application/json" \
  -d '{"@type":"Player","name":"George","position":"Center Back"}'
```
{% /code-tab %}
{% /code-tabs %}

## Insert multiple documents

You can insert several documents in a single operation:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const players = [
    {
        "@type" : "Player",
        name    : "George",
        position: "Center Back",
    },
    {
        "@type" : "Player",
        name    : "Doug",
        position: "Full Back",
    },
    {
        "@type" : "Player",
        name    : "Karen",
        position: "Center Forward",
    },
]

const addDocs = async () => {
  const result = await client.addDocument(players)
  console.log("Documents added:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
documents = [
    {"@type": "Player", "name": "George", "position": "Center Back"},
    {"@type": "Player", "name": "Doug", "position": "Full Back"},
    {"@type": "Player", "name": "Karen", "position": "Center Forward"},
]
results = client.insert_document(documents)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Add+documents" \
  -H "Content-Type: application/json" \
  -d '[
    {"@type":"Player","name":"George","position":"Center Back"},
    {"@type":"Player","name":"Doug","position":"Full Back"},
    {"@type":"Player","name":"Karen","position":"Center Forward"}
  ]'
```
{% /code-tab %}
{% /code-tabs %}

## Insert schema documents

You can also add schema documents programmatically:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const schema = { "@type": "Class", "@id": "Player", name: "xsd:string", position: "xsd:string" }

const addSchema = async () => {
  const result = await client.addDocument(schema, { graph_type: "schema" })
  console.log("Schema added:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
schema = {"@type": "Class", "@id": "Player", "name": "xsd:string", "position": "xsd:string"}
result = client.insert_document(schema, graph_type="schema")
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Add+schema&graph_type=schema" \
  -H "Content-Type: application/json" \
  -d '{"@type":"Class","@id":"Player","name":"xsd:string","position":"xsd:string"}'
```
{% /code-tab %}
{% /code-tabs %}

## Next steps

You've inserted documents into TerminusDB. From here you can:

- [**Get documents**](/docs/get-documents/) — retrieve and filter the documents you just added
- [**Edit documents**](/docs/edit-a-document/) — update existing documents
- [**Add a schema**](/docs/add-a-schema/) — add type validation and constraints to your data

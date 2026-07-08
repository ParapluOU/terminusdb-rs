---
title: Get Documents
nextjs:
  metadata:
    title: Get Documents
    description: How to retrieve documents from TerminusDB using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, get document, fetch document, retrieve, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/get-documents/
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-use-get-documents.png?raw=true
tags:
  - typescript
  - python
  - documents
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A database with existing documents
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have retrieved documents from your TerminusDB database using the JavaScript client, the Python client, or the HTTP API.
{% /callout %}

{% prerequisites-clone /%}

## Get a single document

Retrieve a document by its ID:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const getDoc = async () => {
  const doc = await client.getDocument({ id: "Player/Doug" })
  console.log("Player/Doug:", doc)
}
```

Returns:

```json
{
  "@id"   : "Player/Doug",
  "@type" : "Player",
  "name"    : "Doug",
  "position": "Full Back"
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
document = client.get_document("Player/Doug")
```

Returns:

```json
{
  "@id"   : "Player/Doug",
  "@type" : "Player",
  "name"    : "Doug",
  "position": "Full Back"
}
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?id=Player/Doug"
```

Returns:

```json
{
  "@id"   : "Player/Doug",
  "@type" : "Player",
  "name"    : "Doug",
  "position": "Full Back"
}
```
{% /code-tab %}
{% /code-tabs %}

## Get all documents

Retrieve every document in the database:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const getDocs = async () => {
  const documents = await client.getDocument({ as_list: "true" })
  console.log("All documents:", documents)
}
```

Returns:

```json
[
  {
    "@id"   : "Player/Doug",
    "@type" : "Player",
    "name"    : "Doug",
    "position": "Full Back"
  },
  {
    "@id"   : "Player/George",
    "@type" : "Player",
    "name"    : "George",
    "position": "Center Back"
  },
  {
    "@id"   : "Player/Karen",
    "@type" : "Player",
    "name"    : "Karen",
    "position": "Center Forward"
  }
]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
documents = list(client.get_all_documents())
```

`get_all_documents` returns a generator — wrap it with `list()` if you need to iterate more than once.

```json
[
  {
    "@id"   : "Player/Doug",
    "@type" : "Player",
    "name"    : "Doug",
    "position": "Full Back"
  },
  {
    "@id"   : "Player/George",
    "@type" : "Player",
    "name"    : "George",
    "position": "Center Back"
  },
  {
    "@id"   : "Player/Karen",
    "@type" : "Player",
    "name"    : "Karen",
    "position": "Center Forward"
  }
]
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?as_list=true"
```

Returns:

```json
[
  {
    "@id"   : "Player/Doug",
    "@type" : "Player",
    "name"    : "Doug",
    "position": "Full Back"
  },
  {
    "@id"   : "Player/George",
    "@type" : "Player",
    "name"    : "George",
    "position": "Center Back"
  },
  {
    "@id"   : "Player/Karen",
    "@type" : "Player",
    "name"    : "Karen",
    "position": "Center Forward"
  }
]
```
{% /code-tab %}
{% /code-tabs %}

## Next steps

You've retrieved documents from TerminusDB. Next, you might want to:

- [**Edit documents**](/docs/edit-a-document/) — update the documents you just fetched
- [**Query with WOQL**](/docs/run-woql-query/) — use pattern-matching queries for complex filtering
- [**Delete documents**](/docs/delete-a-document/) — remove documents you no longer need

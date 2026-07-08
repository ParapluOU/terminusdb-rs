---
title: Edit Documents
nextjs:
  metadata:
    title: Edit Documents
    description: How to update documents in TerminusDB using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, edit document, update document, modify, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/edit-a-document/
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-use-edit-a-document.png?raw=true
tags:
  - typescript
  - python
  - documents
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A connected client instance with existing documents
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have updated existing documents in your TerminusDB database using the JavaScript client, the Python client, or the HTTP API.
{% /callout %}

To update a document, first [retrieve it](/docs/get-documents/), make your changes, then submit the update. The document must include its `@id` and `@type` so TerminusDB knows which record to replace.

{% prerequisites-clone /%}

## Update a document

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const doc = {
    "@id"   : "Player/George",
    "@type" : "Player",
    name    : "George",
    position: "Center Back",
}

doc.position = "Full Back"

const updateDoc = async () => {
  const result = await client.updateDocument(doc)
  console.log("Updated document:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
doc = {
    "@id"      : "Player/George",
    "@type"    : "Player",
    "name"     : "George",
    "position" : "Center Back",
}
doc["position"] = "Full Back"
client.update_document(doc)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Update+document" \
  -H "Content-Type: application/json" \
  -d '{"@id":"Player/George","@type":"Player","name":"George","position":"Full Back"}'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="note" %}
The update operation replaces the entire document. Include all fields — any field omitted from the update payload will be removed from the stored document.
{% /callout %}

## Next steps

You've updated documents in TerminusDB. From here you can:

- [**JSON Diff and Patch**](/docs/json-diff-and-patch/) — see exactly what changed between document versions
- [**Branch your database**](/docs/branch-howto/) — make changes on an isolated branch before merging
- [**Delete documents**](/docs/delete-a-document/) — remove documents you no longer need

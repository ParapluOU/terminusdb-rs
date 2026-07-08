---
title: Delete Documents
nextjs:
  metadata:
    title: Delete Documents
    description: How to delete documents from TerminusDB using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, delete document, remove document, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/delete-a-document/
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-use-delete-a-document.png?raw=true
tags:
  - typescript
  - python
  - documents
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A connected client instance with documents to delete
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have deleted documents from your TerminusDB database using the JavaScript client, the Python client, or the HTTP API.
{% /callout %}

To delete a document you need its document ID (e.g. `Player/George`).

{% prerequisites-clone /%}

## Delete a document

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const deleteDoc = async () => {
  const docId = "Player/George"
  await client.deleteDocument({ id: docId })
  console.log(`Deleted ${docId}`)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
doc_id = "Player/George"
client.delete_document(doc_id)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Delete+document" \
  -H "Content-Type: application/json" \
  -d '["Player/George"]'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="note" %}
Deleting a document is permanent within the current branch. If you need to recover a deleted document, use [time travel](/docs/time-travel-howto/) to access a previous commit.
{% /callout %}

## Next steps

You've learnt the full CRUD lifecycle. To go further:

- [**Time travel**](/docs/time-travel-howto/) — recover deleted documents by accessing previous commits
- [**Branch and merge**](/docs/branch-howto/) — test destructive changes safely on a branch
- [**Query with WOQL**](/docs/run-woql-query/) — find documents using pattern-matching queries

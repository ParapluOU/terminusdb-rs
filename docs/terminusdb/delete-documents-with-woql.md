---
tags:
  - woql
  - how-to
  - documents
  - intermediate
title: How to delete documents using WOQL
nextjs:
  metadata:
    title: How to delete documents using WOQL
    description: A how-to guide showing how to construct a WOQL query to delete documents.
    keywords: terminusdb, datalog, delete, delete documents using woql, document, document database, documents, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-delete-documents.png
    alternates:
      canonical: https://terminusdb.org/docs/delete-documents-with-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to delete documents using WOQL queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Delete a document in WOQL

Deleting a document in WOQL is possible using the `delete_document` keyword.

First, let's insert a document.

```javascript
let v = Vars("id");
insert_document(doc({'@type' : 'Planet', label: 'Planet-X'}), v.id)
```

Supposing we get back the following:

```json
"Planet/01dd97a75800f01f43ab7ab55b6dd08f198dd34d2bdbbeeb7bf4edee45111863"
```

Now we can delete it with the following:

```javascript
delete_document("Planet/01dd97a75800f01f43ab7ab55b6dd08f198dd34d2bdbbeeb7bf4edee45111863")
```

## Delete a subdocument in WOQL

Subdocuments can be deleted using the `delete_document` keyword, but it's important to also delete the triple that links the subdocument from the parent document. Here we resolve the parent document in the variable `v:parentdoc`.

```javascript
and(
  eq("v:subdoc", "Person/John/role/PersonRole/cxW1Egirxm8-QYrq"),
  triple("v:parentdoc", "role", "v:subdoc"),
  delete_document("v:subdoc"),
  delete_triple("v:parentdoc", "role", "v:subdoc"),
)
```
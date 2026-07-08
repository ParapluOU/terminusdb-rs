---
tags:
  - woql
  - how-to
  - documents
  - intermediate
title: How to add and delete documents and subdocuments using WOQL
nextjs:
  metadata:
    title: How to add and delete documents and subdocuments using WOQL
    description: A how-to guide  with an example showing how to add documents and subdocuments using a WOQL query.
    keywords: terminusdb, add, create, datalog, document, document database, documents, insert
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-add-documents.png
    alternates:
      canonical: https://terminusdb.org/docs/add-documents-with-woql/
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to insert documents using WOQL queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Add a document in WOQL

You can add a document in WOQL using the `insert_document` keyword.

```javascript
let v = Vars("id");
insert_document(doc({'@type' : 'Planet', label: 'Planet-X'}), v.id)
```

We can also add documents by using a variable. For instance, we can create a new planet for each individual in the star wars universe as follows:

```javascript
let v = Vars("person", "name");
and(isa(v.person, "People"),
    triple(v.person,"label",v.name),
    insert_document(doc({'@type' : 'Planet', label: v.name})))
```

## Create and link a subdocument in WOQL

Subdocuments are linked to their parent document using the `@linked-by` annotation in `insert_document`. This tells TerminusDB which parent document owns the subdocument and which property links them.

Simple example for adding a subdocument:

```javascript
insert_document(
  new doc({
    "@type": "PersonRole",
    "@linked-by": { "@id": "Person/John", "@property": "role" }
  }),
  "v:SubdocumentId"
)
```

The `@linked-by` annotation specifies that this subdocument is owned by `Person/John` via its `role` property. TerminusDB automatically creates the linking triple when the document is inserted.

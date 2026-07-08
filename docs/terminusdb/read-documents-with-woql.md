---
tags:
  - woql
  - how-to
  - documents
  - beginner
title: Read documents with WOQL
nextjs:
  metadata:
    title: Read documents with WOQL
    description: A guide to show how to read documents with WOQL in your TerminusDB projects.
    keywords: terminusdb, datalog, document, document database, documents, json-ld, query language, read documents with woql
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-read-documents.png
    alternates:
      canonical: https://terminusdb.org/docs/read-documents-with-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to read and retrieve documents using WOQL queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

You can read a document after finding the document id as follows:

```javascript
let v = Vars("doc", "id");
and(isa(v.id, "People"),
    triple(v.id, "label", string("Bossk")),
    read_document(v.id, v.doc))
```

This finds a `People` document, makes sure it has the label `"Bossk"` and then reads the document into the variable `doc`.

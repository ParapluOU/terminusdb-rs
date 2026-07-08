---
tags:
  - woql
  - how-to
  - schema
  - intermediate
title: Schema Queries with WOQL
nextjs:
  metadata:
    title: Schema Queries with WOQL
    description: A guide to show how to query schema with WOQL in your TerminusDB projects.
    keywords: terminusdb, data model, datalog, document type, query, query language, schema, schema queries with woql
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-query-schema.png
    alternates:
      canonical: https://terminusdb.org/docs/schema-queries-with-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query and inspect your schema using WOQL.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Finding elements from the schema.

In order to query the schema, you can use _graph_ arguments to WOQL. TerminusDB stores each branch as a pair of graphs, an instance graph and a schema graph.

We can specify the graph by passing it as an argument to the `quad` word.

To find all classes in the schema we can write:

```javascript
let v = Vars("cls");
quad(v.cls, "rdf:type", "sys:Class", "schema")
```

This results in:

{%table%}

- cls
---
- @schema:Film
---
- @schema:People
---
- @schema:Planet
---
- @schema:Species
---
- @schema:Starship
---
- @schema:Vehicle

{%/table%}

The `@schema` denotes the default schema prefix, and makes it clear that this identifier lives in the schema name space rather than the data name space.

We can also use the typical `triple` word if we use `from` to set our default graph to `schema` instead of `instance`.

```javascript
let v = Vars("cls");
from("schema")
  .triple(v.cls, "rdf:type", "sys:Class")
```
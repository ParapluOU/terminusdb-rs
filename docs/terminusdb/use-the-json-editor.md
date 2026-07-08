---
tags:
  - how-to
  - documents
  - dashboard
title: Use the JSON View for building a Schema
nextjs:
  metadata:
    title: Use the JSON View for building a Schema
    description: "Use the JSON editor to build your TerminusDB schema. Define documents, properties, links, and enums. "
    keywords: terminusdb, dashboard, dfrnt hub, document, document database, documents, edit, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/schema-as-code.png
    alternates:
      canonical: https://terminusdb.org/docs/use-the-json-editor/
media:
  - alt: Create a new product with the TerminusDB dashboard
    caption: ""
    media_type: Image
    title: Create a new product with the TerminusDB dashboard
    value: https://assets.terminusdb.com/docs/new-data-product.png
  - alt: TerminusDB schema editor JSON view
    caption: ""
    media_type: Image
    title: TerminusDB schema editor JSON view
    value: https://assets.terminusdb.com/docs/schema-as-code.png
---

{% callout type="note" %}
**Prerequisites**
- A DFRNT Hub account or TerminusDB with the dashboard
- A data product with documents
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to use the JSON editor to view and edit documents.
{% /callout %}

## Make a New Data Product

First, log in to DFRNT TerminusDB cloud, choose (or create) a team, and then click on "New Data Product".

![Create a new product with the DFRNT TerminusDB cloud or TerminusDB dashboard](https://assets.terminusdb.com/docs/new-data-product.png)

## Create a schema as JSON

Now click on the pink bubbles on the left panel. This takes you to the schema builder page. Select JSON view from the tab and you'll see your entire schema as JSON.

![TerminusDB schema editor JSON view](https://assets.terminusdb.com/docs/schema-as-code.png)

If you click on the Edit button in the upper right-hand corner, you'll be able to directly edit the schema.

If you have no data in your database, it should be possible to freely edit the schema. However, if you have data, then you may not be able to make arbitrary edits. The schema editor will warn you upon submission if some restrictions are violated.

Essentially it should _always_ be possible to [weaken](/docs/what-is-schema-weakening/) the schema safely through the interface. However, other changes will require [schema migration](/docs/schema-migration-reference-guide/).
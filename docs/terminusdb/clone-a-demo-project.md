---
tags:
  - tutorial
  - collaboration
  - curl
  - beginner
title: Clone a Demo Database
nextjs:
  metadata:
    title: Clone a Demo Database
    description: Clone a demo database from the public templates server to your local TerminusDB instance — no account required.
    keywords: terminusdb, clone, demo database, getting started, star wars, ecommerce
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/clone-a-demo-project/
media: []
---

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


Clone a pre-populated demo database from the public templates server to your local TerminusDB instance. No account required — just a running TerminusDB on `localhost:6363`.

## Available templates

The public templates server at `data.terminusdb.org` hosts curated datasets you can clone in one command:

| Template | Documents | Use case |
|----------|-----------|----------|
| **Star Wars** | ~85 (People, Films, Planets, Species, Starships, Vehicles) | Learning WOQL queries and GraphQL, following tutorials |
| **Ecommerce** | ~155 (Customers, Orders, OrderLines, Products, Categories) | Business scenario tutorials (branch, diff, merge) |

## Clone Star Wars

```bash
curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
  -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars demo dataset"}'
```

Then explore it: [Explore a Real Dataset](/docs/explore-a-real-dataset/)

## Clone Ecommerce

```bash
curl -u admin:root -X POST http://localhost:6363/api/clone/admin/ecommerce \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
  -d '{"remote_url": "https://data.terminusdb.org/public/ecommerce", "label": "Ecommerce", "comment": "Ecommerce tutorial dataset"}'
```

Then explore it: [Explore an Ecommerce Dataset](/docs/explore-ecommerce-dataset/)

## How it works

The clone operation uses TerminusDB's git-for-data architecture:

1. Your local instance contacts `data.terminusdb.org` (authenticated with `public:public` read-only credentials)
2. The entire database — schema, documents, and commit history — is pulled to your local instance
3. The clone is now yours: query it, branch it, modify it. Changes stay local.

Cloning is efficient even for large datasets because TerminusDB uses content-addressed storage — only unique data layers are transferred.

## Next steps

- [Get Started](/docs/get-started/) — the complete quickstart tutorial
- [Explore a Real Dataset](/docs/explore-a-real-dataset/) — Star Wars dataset walkthrough
- [Explore an Ecommerce Dataset](/docs/explore-ecommerce-dataset/) — business scenario walkthrough
- [WOQL Basics](/docs/woql-basics/) — learn the query language
- [GraphQL Basics](/docs/graphql-basics/) — query with GraphQL

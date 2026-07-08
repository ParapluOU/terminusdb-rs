---
tags:
  - how-to
  - vectorlink
  - dfrnt-cloud
title: Index Your Data
nextjs:
  metadata:
    title: Index Your Data
    description: How to index your content and data with VectorLink
    keywords: terminusdb, index, index your data, vector
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.org/docs/index-your-data/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with a schema and data
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have configured indexes on your TerminusDB data for faster queries.
{% /callout %}

Once you have configured OpenAI, you can index your data. Indexing happens on a commit level so to start indexing you need a new commit.

To do this, create and approve a change request. The indexing process will begin.

You can see the commit index history by clicking on the cog symbol on the left. Here you can also restart indexing processes.

Once you have indexed your data, you can ask the semantic index server questions about your data and content.

To submit prompts about your data, select the magnifying glass icon from the left and fill in the form with your prompts.
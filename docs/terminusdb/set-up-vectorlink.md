---
tags:
  - how-to
  - vectorlink
  - dfrnt-cloud
title: Set up VectorLink
nextjs:
  metadata:
    title: Set up VectorLink
    description: Steps to set up VectorLink to work with OpenAI
    keywords: terminusdb, set up vectorlink, vector
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.org/docs/set-up-vectorlink/
media:
  - alt: Add your OpenAI key in the relevant section within your profile
    caption: ""
    media_type: Image
    title: Add your OpenAI key in the relevant section within your profile
    value: https://assets.terminusdb.com/docs/vectorlink-openai-key.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- An OpenAI API key (for embeddings)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have VectorLink configured for vector similarity search on your TerminusDB data.
{% /callout %}

VectorLink is a semantic indexer for TerminusDB. It is a vector database that uses OpenAI, vector embeddings and GraphQL to provide AI-assisted semantic search, similar search, clustering and entity resolution.

To use VectorLink you need an OpenAI API key. The OpenAI key applies to the team and all data products within that team.

## Set OpenAI API Key

To set your OpenAI Key -

1.  Log in to the user interface dashboard - dfrnt.com
2.  Select a Team
3.  Select your Profile by clicking on ▼ at the top-right corner of the screen.
4.  Paste your OpenAI API key and press save.
5.  Check that the checkbox is ticked to ensure the automatic document indexing will run after every change request approval process

![Add your OpenAI key in the relevant section within your profile](https://assets.terminusdb.com/docs/vectorlink-openai-key.png)
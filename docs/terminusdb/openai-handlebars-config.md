---
tags:
  - how-to
  - vectorlink
  - dfrnt-cloud
title: OpenAI and Handlebars Configuration
nextjs:
  metadata:
    title: OpenAI and Handlebars Configuration
    description: How to configure OpenAI embeddings and Handlebars templates for VectorLink semantic indexing in TerminusDB.
    keywords: terminusdb, openai and handlebars configuration
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.org/docs/openai-handlebars-config/
media:
  - alt: High quality text embeddings for OpenAI using GraphQL query with Handlebars templates
    caption: ""
    media_type: Image
    title: High quality text embeddings for OpenAI using GraphQL query with Handlebars templates
    value: https://assets.terminusdb.com/docs/vectorlink-text-embeddings.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- VectorLink set up ([setup guide](/docs/set-up-vectorlink/))
- An OpenAI API key
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have configured Handlebars templates for OpenAI-powered document generation.
{% /callout %}

To use VectorLink’s semantic search you first need to configure the Handlebars semantic templates to generate the vector embeddings in a word-based query format for OpenAI.

![High quality text embeddings for OpenAI using GraphQL query with Handlebars templates](https://assets.terminusdb.com/docs/vectorlink-text-embeddings.png)

## Quick Guide

You need to create a Handlebars embedding for each document class that you want to index.

Assuming you have logged into TerminusDB, selected a team and data product, do the following -

1.  Choose the OpenAI icon from the menu on the left
2.  Using the dropdown menu, select the document class you want to generate an embedding for.
3.  Check the GraphQL query - ensure that all properties that you want to include in your embedding are there.
4.  Write your Handlebars template and press preview.
5.  Check the preview pane to ensure the embedding is as it should be.
6.  When done, press save.
7.  Repeat the process for all the document classes you want indexing.

## Choose your Document Class

From the dropdown menu, choose the document class you want to index.

This generates the GraphQL query to include in the template. Check that all of the document properties you want to index are there. In some cases, it is more useful to choose a property label rather than an ID.

## Write Handlebars Semantic Templates

> If you clone the Star Wars database from `data.terminusdb.org` (see [Explore a Real Dataset](/docs/explore-a-real-dataset/)), it comes with a working Handlebars template for you to copy.

This is currently a manual process. The templates are written in an easy-to-understand way.

Using the Star Wars dataset as an example, this shows the structure of a semantic template for the People class in Star Wars:

```json
{
    "embedding": {
        "query": "query($id: ID){ People(id : $id) { birth_year, created, desc, edited, eye_color, gender, hair_colors, height, homeworld { label }, label, mass, skin_colors, species { label }, url } }",
        "template": "The person's name is {{label}}.{{#if desc}} They are described with the following synopsis: {{#each desc}} *{{this}} {{/each}}.{{/if}}{{#if gender}} Their gender is {{gender}}.{{/if}}{{#if hair_colors}} They have the following hair colours: {{hair_colors}}.{{/if}}{{#if mass}} They have a mass of {{mass}}.{{/if}}{{#if skin_colors}} Their skin colours are {{skin_colors}}.{{/if}}{{#if species}} Their species is {{species.label}}.{{/if}}{{#if homeworld}} Their homeworld is {{homeworld.label}}.{{/if}}"
    }
}
```

The query parameters use the document properties. This information is used in the template. Additional information is then added to give each property some context.

Notice that a link document property can reference the linked document and its properties, for example -

```handlebars
{{homeworld.label}}
```

More information about Handlebars can be found here - [Handlebars documentation](https://handlebarsjs.com/guide/).

## Preview & Save Config

Preview your template by pressing the Preview button on the right.

Adjust your template if it needs it.

Press save. The template will be saved for that document class.

Repeat these steps for each class you want to be indexed.
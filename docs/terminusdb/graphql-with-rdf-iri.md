---
tags:
  - graphql
  - how-to
  - rdf
title: GraphQL with RDF URI/IRI
nextjs:
  metadata:
    title: GraphQL with RDF URI/IRI
    keywords: GraphQL, RDF, URI, IRI, semantic web
    description: Using GraphQL with RDF and URI/IRI fields with TerminusDB
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/graphql-query-reference/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with RDF-style data (custom IRIs)
- Familiarity with GraphQL basics ([getting started](/docs/graphql-basics/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query documents with custom RDF IRIs using GraphQL.
{% /callout %}

GraphQL is a data-oriented language with queries composed of queries, arguments and fields.

TerminusDB on the other hand is an RDF Knowledge Graph database with classes and properties, and with IRIs (URIs) as identifiers to connect JSON documents, JSON subdocuments and managing database IDs.

The TerminusDB GraphQL endpoint enables a data-informed ability to process typed RDF knowledge graphs with IRIs as identifiers, using GraphQL. This enables a fantastic interoperability between the worlds of graphs and traditional databases.

This means that id:s will not have the prefixed shorthand IRIs that are seen in the regular document interface of TerminusDB, but the fully qualified IRIs that uses the `@base` prefix.

For instance, a document might be retrieved by supplying the id as a variable in the following way, expecting the full IRI:

```graphql
query getPerson($id: ID) {
  Person(id: $id) {
    name
  }
}
```

With variables:

```json
{
  "id": "terminusdb:///data/Person/Luke%20Skywalker"
}
```

Note that the `id` value must use the full IRI (starting with the `@base` prefix, typically `terminusdb:///data/`), not the short-form document identifier used in the document API.

## IRI handling in results

When querying documents via GraphQL, the `_id` field returns the full IRI of each document:

```graphql
query {
  Person(limit: 2) {
    _id
    name
  }
}
```

This returns fully qualified IRIs such as `terminusdb:///data/Person/Luke%20Skywalker` rather than the shortened `Person/Luke%20Skywalker` form seen in the document interface.

## Relationship with WOQL

The same IRI behaviour is present in WOQL queries, where identifiers are full IRIs. Read more about [prefixed IRIs and RDF contexts](/docs/graphs-explanation/) to understand how TerminusDB manages document identifiers across its interfaces.
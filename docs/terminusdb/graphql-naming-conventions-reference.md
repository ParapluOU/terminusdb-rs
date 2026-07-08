---
tags:
  - graphql
  - reference
title: GraphQL Naming Conventions Reference Guide
nextjs:
  metadata:
    title: GraphQL Naming Conventions Reference Guide
    description: A reference guide detail the GraphQL naming conventions in TerminusDB and TerminusDB.
    keywords: terminusdb, api, graphql, graphql naming conventions reference guide, query, terminusdb graphql
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/graphql-naming-conventions-reference/
media: []
---

This page documents how TerminusDB translates its schema class and property names into valid GraphQL identifiers. Because TerminusDB's schema language supports the full IRI character set (including colons, slashes, and accented characters) while GraphQL restricts names to `[A-Z][a-z][0-9][_]`, an automatic translation layer maps between the two. Understanding these conventions helps you predict what your GraphQL types and fields will be called.

TerminusDB auto-generates a complete GraphQL schema from your document classes. For each class, it creates query types, filter types, and mutation types — all following the naming rules described below.

## Underscore as reserved

When names are likely to create conflicts with user-defined names, TerminusDB will typically use an `_` at the beginning to avoid naming conflicts. This is done on filter fields that share the same object level with user-defined properties for instance: `_and`, `_or` and `_not`.

## Translation

All names of GraphQL classes in TerminusDB and all properties of TerminusDB classes, as well as all enums, are translated to viable GraphQL names. This is done by replacing each non-representable character with an `_`. In addition, underscores at the beginning of a class name or property are disallowed. This is to ensure there are no collisions with TerminusDB's own auto-generated properties and classes.

Should a collision arise, TerminusDB should give a GraphQL error on retrieval of the schema. In future, we will allow this check to occur at schema submission time, and will also allow explicit renaming in TerminusDB classes.

For instance, the TerminusDB class is defined as:

```json
{ "@type" : "Class",
  "@id" : "Galactic-Civilisation",
  "name" : "xsd:string",
  "kardashev-scale" : "xsd:integer" }
```

will be translated to:

```graphql
type Query {
  Galactic_Civilisation(
    id: ID
    """skip N elements"""
    offset: Int
    """limit results to N elements"""
    limit: Int
    filter: Galactic_Civilisation_Filter
    """order by the given fields"""
    orderBy: Galactic_Civilisation_Ordering
    ): [Galactic_Civilisation!]!
}
type Galactic_Civilisation {
    name: String!
    kardashev_scale: BigInt!
}
```
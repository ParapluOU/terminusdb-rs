---
tags:
  - graphql
  - reference
  - advanced
title: System Graph Interface to GraphQL
nextjs:
  metadata:
    title: System Graph Interface to GraphQL
    description: TerminusDB technical documentation - System Graph Interface to GraphQL
    keywords: GraphQL, system graph, control graph, Commit Graph, meta Graph
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/system-graph-graphql-interface-reference/
media: []
---

TerminusDB also exposes its internal working graphs, the system graph, the meta graph, and the commit-graph. These three graphs can be queried with a self-documenting GraphQL interface by going to the appropriate API for a data product.

## System Graph

For instance, to get _only_ system graph access, you can use the following endpoint:

```url
http://localhost:6363/api/graphql/_system
```

## Meta Graph

To get the System Graph and Meta graph which belongs to a specific data product you can use the following endpoint:

```url
http://localhost:6363/api/graphql/ORG/DATA_PRODUCT/_meta
```

## Commit Graph

To get access to branches, commits, commit logs, as well as the meta and system graph, you can use the following endpoint:

```url
http://localhost:6363/api/graphql/ORG/DATA_PRODUCT/local/_commits
```

## See also

* [Querying Layers with GraphQL](/docs/querying-layers-with-graphql)
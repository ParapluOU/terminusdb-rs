---
tags:
  - graphql
  - how-to
  - path-queries
title: Path Queries in GraphQL
nextjs:
  metadata:
    title: Path Queries in GraphQL
    description: How to do path queries on TerminusDB data products using GraphQL
    keywords: terminusdb, api, graph traversal, graphql, path, path queries in graphql, path query, query
    openGraph:
      images: https://assets.terminusdb.com/docs/graphql-path-query.png
    alternates:
      canonical: https://terminusdb.org/docs/path-queries-in-graphql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to traverse graph paths using TerminusDB's GraphQL path queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Using a Path Query

Sometimes we want to search for links that are not immediate, but need to follow a chain of links to get the object of interest. TerminusDB gives us [path queries](/docs/path-query-reference-guide/) which allow us to succinctly express this.

We can find a path in GraphQL by using the `_path_to_CLASS` query, where CLASS is the name of one of our classes. One path should be populated for each of the available classes.

To find everyone who was in a film with Chewbacca, we can write:

```graphql
query{
   People(filter:{label:{eq:"Chewbacca"}}){
     label
     _path_to_People(path:"film,<film"){
       label
    }
  }
}
```

The `film` is the current film which the Chewbacca object points at. Then `<film` means follow _backwards_ to people on the film field.

This process can be repeated to find second order connections, as follows:

```graphql
query{
   People(filter:{label:{eq:"Chewbacca"}}){
     label
     _path_to_People(path:"(film,<film){1,2}"){
       label
    }
  }
}
```

This says that we should repeat the process one or two times before terminating.

More complex patterns can be built using the full [path query syntax](/docs/path-query-reference-guide/) described in our documentation.
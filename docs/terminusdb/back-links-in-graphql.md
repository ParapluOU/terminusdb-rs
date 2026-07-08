---
tags:
  - graphql
  - how-to
  - intermediate
title: Use GraphQL Back Links for easy queries
nextjs:
  metadata:
    title: Use GraphQL Back Links for easy queries
    description: How to traverse relationships in reverse using GraphQL back links (inverseOf) in TerminusDB to query parent documents from children.
    keywords: backlink, inverseOf, inverse links, GraphQL, TerminusDB
    openGraph:
      images: https://assets.terminusdb.com/docs/graphql-backlink.png
    alternates:
      canonical: https://terminusdb.org/docs/back-links-in-graphql/
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query back-links (reverse relationships) using GraphQL.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Using a Back Link

Many times when we are looking at an object, we are interested in which objects are pointing to it. In the TerminusDB, each object gets a number of extended queries which allows one to discover any objects which point at that object.

Once you have cloned the Star Wars demo, go to the [GraphQL query panel](/docs/graphql-basics/) and type:

```graphql
query{
  People(limit:1) {
      █
  }
}
```

We would like to find the first person in the database, and then find out which starships they are the pilot of. A `Starship` has a `pilot` field, and the backlink is automatically constructed as the `pilot_of_Starship` by TerminusDB.

```graphql
query{
   People(limit:1){
     label
     _pilot_of_Starship{
      label
    }
  }
}
```

This _back link_ will give us back the following:

```json
{
  "data": {
    "People": [
      {
        "label": "Luke Skywalker",
        "_pilot_of_Starship": [
          {
            "label": "X-wing"
          },
          {
            "label": "Imperial shuttle"
          }
        ]
      }
    ]
  }
}
```

Backlinking allows us to focus on modeling our data in a natural way, while still allowing us to follow the graph in either direction of a field or its opposite without bias. Sometimes these backlinks are called inverseOf or inverse links. 
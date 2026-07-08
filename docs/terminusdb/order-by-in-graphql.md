---
tags:
  - graphql
  - how-to
  - beginner
title: Order By in GraphQL
nextjs:
  metadata:
    title: Order By in GraphQL
    description: How to use the order by argument in GraphQL queries with TerminusDB
    keywords: terminusdb, api, graphql, order by, order by in graphql, ordering, query, sort
    openGraph:
      images: https://assets.terminusdb.com/docs/graphql-order-by.png
    alternates:
      canonical: https://terminusdb.org/docs/order-by-in-graphql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to sort and order results in GraphQL queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Ordering results of a GraphQL query

By default, results in GraphQL will choose an implementation specific order which may not even be stable between invocations. If you need results in a _specific order_ then you need to supply an `orderBy` argument.

We can search for the names of people in reverse alphabetical order such that we only recover the first 5 results in the following way:

```graphql
query{
   People(limit:5, orderBy:{label:DESC}){
    label
  }
}
```

This will give us the following people:

```json
{
  "data": {
    "People": [
      {
        "label": "Zam Wesell"
      },
      {
        "label": "Yoda"
      },
      {
        "label": "Yarael Poof"
      },
      {
        "label": "Wilhuff Tarkin"
      },
      {
        "label": "Wicket Systri Warrick"
      }
    ]
  }
}
```

Order by can also take more than one argument, allowing us to order on more than one value using the remaining arguments when there is a tie in the preceding (lexicographic ordering).

We can see this by searching for species, and which language they speak and their name. Since many will share the same language, we can see the ordering of the fields independently.

```graphql
query{
   Species(offset:8, limit:5, orderBy:{language:ASC, label:ASC}){
    label
    language
  }
}
```

And here we have a number of Galactic Basic speakers who nevertheless are ordered by species name.

```json
{
  "data": {
    "Species": [
      {
        "label": "Ewok",
        "language": "Ewokese"
      },
      {
        "label": "Human",
        "language": "Galactic Basic"
      },
      {
        "label": "Rodian",
        "language": "Galactic Basic"
      },
      {
        "label": "Yoda's species",
        "language": "Galactic basic"
      },
      {
        "label": "Geonosian",
        "language": "Geonosian"
      }
    ]
  }
}
```
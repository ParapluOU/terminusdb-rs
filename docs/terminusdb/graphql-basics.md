---
tags:
  - graphql
  - how-to
  - beginner
title: Learn the GraphQL Basics for TerminusDB
nextjs:
  metadata:
    title: Learn the GraphQL Basics for TerminusDB
    keywords: IRI, RDF, GraphQL, CMS
    description: Learn to query TerminusDB using GraphQL with a Star Wars dataset cloned from the public templates server.
    openGraph:
      images: https://assets.terminusdb.com/docs/graphqll-basics.png
    alternates:
      canonical: https://terminusdb.org/docs/graphql-basics/
media: []
---

This guide teaches you the basics of querying TerminusDB with GraphQL. You will learn how to write queries, select fields, and traverse document relationships — all using a Star Wars dataset as a worked example.

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

You can run GraphQL queries against your local instance at `http://localhost:6363/api/graphql/admin/star-wars`.

Now you have two panels, one on the left for query, and one on the right for results.

## Entering a query

First type `query{` into the query panel. It should look like this:

```graphql
query{
   █
}
```

If at the cursor point you type: `Ctrl-c` you'll get a list of options you can choose from. These options are legal GraphQL syntax according to your provided schema. Let's search for people from the Star Wars universe.

```graphql
query{
   People{
      label
   }
}
```

The `label` property in this schema, supplies the name of the person we are interested in. Of course this query might give us a bit too much, so let us also limit it.

```graphql
query{
   People(limit: 5){
      label
   }
}
```

This should result in:

```json
{
  "data": {
    "People": [
      {
        "label": "Luke Skywalker"
      },
      {
        "label": "Obi-Wan Kenobi"
      },
      {
        "label": "Anakin Skywalker"
      },
      {
        "label": "Wilhuff Tarkin"
      },
      {
        "label": "Chewbacca"
      }
    ]
  }
}
```

To get more fields in our query, we can just add words, using `Ctrl-c` if we are stuck for names of fields.

```graphql
query{
   People(limit: 5){
      label
   }
}
```

When following links to other objects, we have to embed a query inside our query. So, for instance, if we want to know the homeworld that each of these people come from we can write:

```graphql
query{
   People(limit: 2){
      label
      homeworld{
        label
      }
   }
}
```

This will get us:

```json
{
  "data": {
    "People": [
      {
        "label": "Luke Skywalker",
        "homeworld": {
          "label": "Tatooine"
        }
      },
      {
        "label": "Obi-Wan Kenobi",
        "homeworld": {
          "label": "Stewjon"
        }
      }
    ]
  }
}
```

## Paging

If we want to page the results, we can also add an offset to our query, and we'll get _the next_ results.

```graphql
query{
   People(limit: 2, offset:2){
      label
      homeworld{
        label
      }
   }
}
```

And now we get two more:

```json
{
  "data": {
    "People": [
      {
        "label": "Anakin Skywalker",
        "homeworld": {
          "label": "Tatooine"
        }
      },
      {
        "label": "Wilhuff Tarkin",
        "homeworld": {
          "label": "Eriadu"
        }
      }
    ]
  }
}
```

## Next steps

- [**Filter results**](/docs/filter-with-graphql/) — use `_filter` arguments to narrow queries
- [**GraphQL mutations**](/docs/graphql-mutations/) — insert, replace, and delete documents via GraphQL
- [**Path queries in GraphQL**](/docs/path-queries-in-graphql/) — traverse relationships across document types
- [**Connect with Apollo Client**](/docs/connect-with-apollo-client/) — integrate TerminusDB GraphQL into a JavaScript application
- [**GraphQL query reference**](/docs/graphql-query-reference/) — full reference for types, arguments, and fields


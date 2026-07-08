---
tags:
  - woql
  - tutorial
  - beginner
title: WOQL Basics — Pattern Matching and Graph Traversal
nextjs:
  metadata:
    title: WOQL Basics — Pattern Matching and Graph Traversal
    keywords: woql, terminusdb query language, datalog, graph traversal, pattern matching, triple query, document graph query, woql tutorial
    description: WOQL is TerminusDB's Datalog-based query language for pattern matching, graph traversal, and data transformation. Learn the basics with worked examples.
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-basics.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-basics/
media:
  - alt: WOQL query playground in the TerminusDB dashboard
    caption: ""
    media_type: Image
    title: WOQL query playground in the TerminusDB dashboard
    value: https://assets.terminusdb.com/docs/how-to-query-woql.png
---

WOQL (Web Object Query Language) is TerminusDB's Datalog-based query language. It finds patterns across documents and their relationships by declaring what you want — not how to get it. Unlike SQL, WOQL traverses document links natively without JOINs. Unlike GraphQL (which TerminusDB also supports), WOQL handles complex pattern matching, recursive traversal, aggregation, and data transformation in a single composable query.

Not sure which query interface to use? See [Choosing a Query Interface](/docs/querying-terminusdb) for a comparison of WOQL, GraphQL, and the HTTP Document API.

{% callout type="note" %}
**Prerequisites**
This page uses the Star Wars dataset for examples. [Clone it from the public templates server](/docs/explore-a-real-dataset/#step-1) or follow the [main quickstart](/docs/get-started/) first.
{% /callout %}

![WOQL query playground in the TerminusDB dashboard](https://assets.terminusdb.com/docs/how-to-query-woql.png)

## Your first WOQL query

WOQL queries are composed of variables, triple patterns, and combinators. Let's start with a simple query that looks at one field.

We need to describe which variables we want to use, and we do that with `Vars`.

Next we add the `limit` word, to limit to 10 entries.

Then we complete the query with a `triple` word, using the `source` variable, the `label` field, and the `destination` variable.

```javascript
let v = Vars("source", "destination");
limit(10).triple(v.source, 'label', v.destination)
```

The results will come back in a table below in the UI. In the client it will return as a list of JSON objects, having each of the variables described in `Vars` bound.

The `destination` variable is filled with elements of type _string_, because `label` always terminates in a string. However we can also add other fields to our object, to search for more information by chaining `triple` together.

```javascript
let v = Vars("person", "eyes", "name");
limit(5)
  .triple(v.person, 'label', v.name)
  .triple(v.person, 'eye_color', v.eyes)
```

This query results in the following:

{% table %}

- Name
- Eyes
- Person

---

- Luke Skywalker
- blue
- People/1

--- 

- Obi-Wan Kenobi
- blue-gray
- People/10

---

- Anakin Skywalker
- blue
- People/11

---

- Wilhuff Tarkin
- blue
- People/12

---

- Chewbacca
- blue
- People/13

{% /table %}

## and

The `.` syntax is actually introducing an implicit `and` between `triple` words. We can rewrite our query above as:

```javascript
let v = Vars("person", "eyes", "name");
limit(5)
  .and(triple(v.person, 'label', v.name),
       triple(v.person, 'eye_color', v.eyes))
```

## select

Since we probably do not really need the `person` variable, as it is an id, and we are just using it to make sure we are talking about the _same_ person in both triples, we can use `select` to remove it.

```javascript
let v = Vars("person", "eyes", "name");
limit(5)
  .select(v.name, v.eyes)
  .and(triple(v.person, 'label', v.name),
       triple(v.person, 'eye_color', v.eyes))
```

Now we get back the table with the `person` column removed.

## Next steps

- [Full WOQL Tutorial](/docs/how-to-query-with-woql/) — 12-step hands-on guide covering all WOQL operations
- [WOQL Explained](/docs/woql-explanation/) — understand Datalog, unification, and composable logic
- [Path Queries in WOQL](/docs/path-queries-in-woql/) — recursive graph traversal with regex-like path expressions
- [Filter with WOQL](/docs/filter-with-woql/) — conditional filtering and pattern matching
- [WOQL Class Reference](/docs/woql-class-reference-guide/) — complete API reference for all WOQL words
- [GraphQL Basics](/docs/graphql-basics/) — the alternative query interface for simpler read patterns
- [TerminusDB Query Cookbook](/docs/terminusdb-query-cookbook/) — real-world query recipes
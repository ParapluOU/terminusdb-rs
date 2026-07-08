---
tags:
  - woql
  - how-to
  - path-queries
  - intermediate
title: Path Queries in WOQL
nextjs:
  metadata:
    title: Path Queries in WOQL
    description: A guide to show how to do path queries in WOQL for your TerminusDB projects.
    keywords: terminusdb, datalog, graph traversal, path, path queries in woql, path query, query language, relationships
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-path-query.png
    alternates:
      canonical: https://terminusdb.org/docs/path-queries-in-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to traverse graph paths using WOQL path queries.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## How to use `path`

TerminusDB gives us [path queries](/docs/path-query-reference-guide/) which allow us to succinctly express chains of relationships.

The `path` keyword allows you to find a path through the graph traversing intermediate edges. An example would be finding a group of individuals who have at some point shared a vehicle as a pilot or piloted another vehicle that in turn was shared with someone. This is a _transitive_ relationship and will explore the entire graph.

For instance

```javascript
let v = Vars("person1", "person2");
path(v.person1, "(<pilot,pilot>)+", v.person2)
```

This `path` means we follow the `pilot` field _backwards_ (because of the `<` arrow), to the vehicle of which the person is a pilot and then follow it forwards `pilot>` any number of times _but at least once_ which is what the `+` means.

The path itself can also be returned by adding another field, as so:

```javascript
let v = Vars("person1", "person2", "path");
path(v.person1, "(<pilot,pilot>)+", v.person2, v.path)
```

This can be inspected to understand the manner in which we got from `person1` to `person2`.
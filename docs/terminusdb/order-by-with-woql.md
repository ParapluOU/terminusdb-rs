---
tags:
  - woql
  - how-to
  - intermediate
title: How to Order Results in WOQL
nextjs:
  metadata:
    title: How to Order Results in WOQL
    description: A guide to show you how to order results using order_by in WOQL.
    keywords: terminusdb, datalog, order by, order results in woql, ordering, query language, sort, terminusdb query
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-order-by.png
    alternates:
      canonical: https://terminusdb.org/docs/order-by-with-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to sort and order query results using WOQL.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## Ordering results using `order_by`

The `order_by` keyword will allow you to sort results.

```javascript
let v = Vars("person", "label", "eyes", "group");
limit(2)
.order_by(["eyes", "desc"])
.select(v.eyes, v.group)
.group_by(
  "eyes",
  ["label"],
  v.group,
  and(triple(v.person, "rdf:type", "@schema:People"),
      triple(v.person, "label", v.label),
      triple(v.person, "eye_color", v.eyes)))
```

This returns the first two results of people, who have a given eye color, sorted by eye color, in reverse order.

To get the alternative order, you can write:

```javascript
let v = Vars("person", "label", "eyes", "group");
limit(2)
.order_by(["eyes", "asc"])
.select(v.eyes, v.group)
.group_by(
  "eyes",
  ["label"],
  v.group,
  and(triple(v.person, "rdf:type", "@schema:People"),
      triple(v.person, "label", v.label),
      triple(v.person, "eye_color", v.eyes)))
```

Or simply:

```javascript
let v = Vars("person", "label", "eyes", "group");
limit(2)
.order_by("eyes")
.select(v.eyes, v.group)
.group_by(
  "eyes",
  ["label"],
  v.group,
  and(triple(v.person, "rdf:type", "@schema:People"),
      triple(v.person, "label", v.label),
      triple(v.person, "eye_color", v.eyes)))
```
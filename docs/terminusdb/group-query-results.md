---
tags:
  - woql
  - how-to
  - intermediate
title: How to Group Results in WOQL
nextjs:
  metadata:
    title: How to Group Results in WOQL
    description: A guide to show how to group results of data in your TerminusDB projects using WOQL.
    keywords: terminusdb, aggregate, datalog, find, group by, group results in woql, grouping, query
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-group-query-results.png
    alternates:
      canonical: https://terminusdb.org/docs/group-query-results/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to group and aggregate query results using WOQL.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

## How to use Group By

If we need to group variables according to some criteria, we can create an aggregate of solutions using `group_by`.

A group by is composed of a _focus_, a _template_, and a _group_ together with a query.

### Single Variable Template (Flat Array)

When grouping a single variable, the template produces a **flat array** of values:

```javascript
let v = Vars("person", "label", "eyes", "group", "member");
limit(1)
.group_by(
  "eyes",
  ["label"],
  v.group,
  and(triple(v.person, "rdf:type", "@schema:People"),
      triple(v.person, "label", v.label),
      triple(v.person, "eye_color", v.eyes)))
```

The first argument, here `"eyes"` refers to the eyes variable, and is the variable around which to form the group, the _focus_.

The second `["label"]` is the _template_, which refers to the variable `"label"`. Since the template contains only one variable, the results will be collected as a **flat array**.

The third variable `v.group`, is the _group_ variable, which will include groups of templates for each set of solutions which shares a _focus_.

This raw query output will be:

```json
{
    "eyes": {
        "@type": "xsd:string",
        "@value": "black"
    },
    "group": [
        {
            "@type": "xsd:string",
            "@value": "Greedo"
        },
        {
            "@type": "xsd:string",
            "@value": "Nien Nunb"
        },
        {
            "@type": "xsd:string",
            "@value": "Gasgano"
        },
        {
            "@type": "xsd:string",
            "@value": "Kit Fisto"
        },
        {
            "@type": "xsd:string",
            "@value": "Plo Koon"
        },
        {
            "@type": "xsd:string",
            "@value": "Lama Su"
        },
        {
            "@type": "xsd:string",
            "@value": "Taun We"
        },
        {
            "@type": "xsd:string",
            "@value": "Shaak Ti"
        },
        {
            "@type": "xsd:string",
            "@value": "Tion Medon"
        },
        {
            "@type": "xsd:string",
            "@value": "BB8"
        }
    ],
    "label": null,
    "person": null
}
```

Notice how each result is a flat array of values, not nested arrays. To make this into individual solutions, use the `member` operator: `member(v.member, v.group)`, which will expand the `group` array into individual elements in the v.member variable.

### Multiple Variable Template (Array of Tuples)

When grouping multiple variables, the template produces an **array of tuples**:

```javascript
let v = Vars("person", "label", "eyes", "group");
limit(1)
.group_by(
  "eyes",
  ["label", "eyes"],
  v.group,
  and(triple(v.person, "rdf:type", "@schema:People"),
      triple(v.person, "label", v.label),
      triple(v.person, "eye_color", v.eyes)))
```

Now the template `["label", "eyes"]` includes two variables, so each result will be a tuple (array) containing both values:

```json
{
    "eyes": {
        "@type": "xsd:string",
        "@value": "black"
    },
    "group": [
        [
            {
                "@type": "xsd:string",
                "@value": "Greedo"
            },
            {
                "@type": "xsd:string",
                "@value": "black"
            }
        ],
        [
            {
                "@type": "xsd:string",
                "@value": "Nien Nunb"
            },
            {
                "@type": "xsd:string",
                "@value": "black"
            }
        ],
        [
            {
                "@type": "xsd:string",
                "@value": "Gasgano"
            },
            {
                "@type": "xsd:string",
                "@value": "black"
            }
        ],
        [
            {
                "@type": "xsd:string",
                "@value": "Kit Fisto"
            },
            {
                "@type": "xsd:string",
                "@value": "black"
            }
        ],
        [
            {
                "@type": "xsd:string",
                "@value": "Plo Koon"
            },
            {
                "@type": "xsd:string",
                "@value": "black"
            }
        ]
    ],
    "label": null,
    "person": null
}
```

Each element in the `group` array is now a tuple `[label, eyes]` containing both values.
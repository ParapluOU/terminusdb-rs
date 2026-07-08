---
tags:
  - woql
  - how-to
  - intermediate
title: Mathematical Operations in WOQL
nextjs:
  metadata:
    title: Mathematical Operations in WOQL
    description: How to perform arithmetic, aggregation, and mathematical comparisons in WOQL queries with TerminusDB.
    keywords: terminusdb, datalog, mathematical operations in woql, query language, terminusdb query, woql
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-maths-query.png
    alternates:
      canonical: https://terminusdb.org/docs/maths-based-queries-in-woql/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with numeric data
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to perform mathematical operations in WOQL queries.
{% /callout %}

WOQL has a number of mathematical operations that can be performed. These include, `plus`, `minus`, `divide`, `times`, `div` (for integer division), `exp` and `floor`.

To use these operations you need to `evaluate` an arithmetic expression, and then you will be able to bind the result to a variable.

For instance:

```javascript
let v = Vars("result");
evaluate(times(2,3), v.result)
```

This will store the value of 2 times 3 in the variable `result`. The bindings which result from this query are:

```json
[ {"result": {"@type":"xsd:decimal", "@value":6}} ]
```

You can also chain these together, to build up more complicated computations, or use the results obtained by queries to derive new values.

```javascript
let v = Vars("result1", "result2");
and(evaluate(times(2,3), v.result1),
    evaluate(times(v.result1,3), v.result2))
```

Which results in:

```json
[ {"result1": {"@type":"xsd:decimal", "@value":6},
   "result2": {"@type":"xsd:decimal", "@value":18}} ]
```
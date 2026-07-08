---
title: Run a WOQL Query
nextjs:
  metadata:
    title: Run a WOQL Query — TerminusDB
    description: Execute WOQL queries against your TerminusDB database using the HTTP API, TypeScript client, or Python client.
    keywords: terminusdb, woql, query, datalog, javascript client, python client, http api, search, find, triple
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/run-woql-query/
media: []
tags:
  - woql
  - typescript
  - python
  - how-to
  - beginner
---

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363` — see [installation guide](/docs/install-terminusdb-as-a-docker-container/)
- A database with a schema and data
- A connected client: [TypeScript](/docs/connect-with-the-javascript-client/) or [Python](/docs/connect-with-python-client/)
{% /callout %}

{% prerequisites-clone /%}


{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will have executed a WOQL query against your TerminusDB database using the HTTP API, TypeScript, or Python.
{% /callout %}

## Run a query

WOQL (Web Object Query Language) is TerminusDB's query language. It operates on triples — the fundamental unit of data in the graph. A simple query retrieves all triples in the database:

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/woql/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"query": {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Subject"}, "predicate": {"@type": "NodeValue", "variable": "Predicate"}, "object": {"@type": "Value", "variable": "Object"}}}'
```

**Expected response:**

```json
{
  "bindings": [
    {"Subject": "terminusdb:///data/Person/Alice", "Predicate": "name", "Object": {"@value": "Alice", "@type": "xsd:string"}},
    {"Subject": "terminusdb:///data/Person/Alice", "Predicate": "rdf:type", "Object": "Person"}
  ],
  "deletes": 0,
  "inserts": 0,
  "transaction_retry_count": 0
}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const WOQL = TerminusClient.WOQL;
const v = WOQL.Vars("Subject", "Predicate", "Object");
const query = WOQL.triple(v.Subject, v.Predicate, v.Object);
const result = await client.query(query);

console.log(JSON.stringify(result, null, 2));
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import WOQLQuery, Client

query = WOQLQuery().woql_and(
    WOQLQuery().triple("v:Subject", "v:Predicate", "v:Object")
)
result = client.query(query)

for binding in result["bindings"]:
    print(binding)
```
{% /code-tab %}
{% /code-tabs %}

---

## Filter by type and property

A more practical query: find all documents of a specific type and read a property value.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/woql/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"query": {"@type": "And", "and": [{"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Person"}, "predicate": {"@type": "NodeValue", "node": "rdf:type"}, "object": {"@type": "NodeValue", "node": "@schema:Person"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Person"}, "predicate": {"@type": "NodeValue", "node": "name"}, "object": {"@type": "DataValue", "variable": "Name"}}]}}'
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const v = WOQL.Vars("Person", "Name");
const query = WOQL.and(
  WOQL.triple(v.Person, "rdf:type", "@schema:Person"),
  WOQL.triple(v.Person, "name", v.Name),
);
const result = await client.query(query);

for (const binding of result.bindings) {
  console.log(binding.Name);
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
query = WOQLQuery().woql_and(
    WOQLQuery().triple("v:Person", "rdf:type", "@schema:Person"),
    WOQLQuery().triple("v:Person", "@schema:name", "v:Name"),
)
result = client.query(query)

for binding in result["bindings"]:
    print(binding["Name"])
```
{% /code-tab %}
{% /code-tabs %}

---

## Next steps

- [WOQL Basics](/docs/woql-basics/) — in-depth guide to WOQL concepts and syntax
- [WOQL Common Patterns](/docs/woql-common-patterns/) — practical query recipes (filter, join, aggregate)
- [WOQL Reference](/docs/woql-class-reference-guide/) — complete language reference

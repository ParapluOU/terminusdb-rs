---
tags:
  - woql
  - cookbook
  - beginner
title: WOQL Common Patterns
nextjs:
  metadata:
    title: WOQL Common Patterns — 10 Copy-Paste Recipes
    description: Ten practical WOQL query patterns you can copy and adapt — filter, join, aggregate, path query, insert, update, delete, schema query, time-travel, and subgraph queries.
    keywords: woql examples, woql query patterns, terminusdb query tutorial, woql filter, woql join, woql aggregate
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-common-patterns/
media: []
lastUpdated: "2026-05-01"
---

Ten practical WOQL query patterns you can copy and adapt. Each recipe shows the TypeScript SDK form and the equivalent WOQL AST (JSON) that the API accepts.

{% callout type="note" %}
**Prerequisites**
Examples use the public Star Wars database. Clone it first:
```bash
curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star_wars \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
  -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars"}'
```
{% /callout %}

---

## 1. Filter by property value

**When to use:** Find documents matching a specific field value.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Find all people with blue eyes
const query = WOQL.and(
  WOQL.triple("v:Person", "rdf:type", "@schema:People"),
  WOQL.triple("v:Person", "eye_color", WOQL.string("blue")),
  WOQL.triple("v:Person", "name", "v:Name")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "rdf:type"},
        "object": {"@type": "NodeValue", "node": "@schema:People"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "eye_color"},
        "object": {"@type": "DataValue", "data": "blue"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "variable": "Name"}
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"Name": "Luke Skywalker"}, {"Name": "Owen Lars"}, {"Name": "Beru Whitesun lars"}, {"Name": "Anakin Skywalker"}]
```

---

## 2. Join two document types

**When to use:** Traverse a relationship between documents — no JOIN syntax needed, just follow the link.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Find people and their homeworld name
const query = WOQL.and(
  WOQL.triple("v:Person", "rdf:type", "@schema:People"),
  WOQL.triple("v:Person", "name", "v:PersonName"),
  WOQL.triple("v:Person", "homeworld", "v:Planet"),
  WOQL.triple("v:Planet", "name", "v:PlanetName")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "rdf:type"},
        "object": {"@type": "NodeValue", "node": "@schema:People"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "variable": "PersonName"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "homeworld"},
        "object": {"@type": "NodeValue", "variable": "Planet"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Planet"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "variable": "PlanetName"}
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"PersonName": "Luke Skywalker", "PlanetName": "Tatooine"}, {"PersonName": "Darth Vader", "PlanetName": "Tatooine"}, {"PersonName": "Leia Organa", "PlanetName": "Alderaan"}]
```

---

## 3. Aggregate (count)

**When to use:** Count documents matching a condition.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Count all people
const query = WOQL.count("v:Count",
  WOQL.triple("v:Person", "rdf:type", "@schema:People")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "Count",
    "count": {"@type": "DataValue", "variable": "Count"},
    "query": {
      "@type": "Triple",
      "subject": {"@type": "NodeValue", "variable": "Person"},
      "predicate": {"@type": "NodeValue", "node": "rdf:type"},
      "object": {"@type": "NodeValue", "node": "@schema:People"}
    }
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"Count": 82}]
```

---

## 4. Path query (follow relationships)

**When to use:** Traverse a chain of relationships — like a recursive JOIN.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Find all planets reachable from a person via homeworld links
const query = WOQL.and(
  WOQL.triple("v:Person", "name", WOQL.string("Luke Skywalker")),
  WOQL.path("v:Person", "homeworld", "v:Planet", "v:Path"),
  WOQL.triple("v:Planet", "name", "v:PlanetName")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "data": "Luke Skywalker"}
      },
      {
        "@type": "Path",
        "subject": {"@type": "Value", "variable": "Person"},
        "pattern": {"@type": "PathPredicate", "predicate": "homeworld"},
        "object": {"@type": "Value", "variable": "Planet"},
        "path": {"@type": "Value", "variable": "Path"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Planet"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "variable": "PlanetName"}
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"PlanetName": "Tatooine"}]
```

---

## 5. Insert a document

**When to use:** Add a new document to the database.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

const query = WOQL.insert_document({
  "@type": "Planets",
  "name": "Mandalore",
  "climate": "temperate",
  "terrain": "forests, jungles",
  "population": "4200000000"
});
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "InsertDocument",
    "document": {
      "@type": "Value",
      "dictionary": {
        "@type": "DictionaryTemplate",
        "data": [
          {"@type": "FieldValuePair", "field": "@type", "value": {"@type": "Value", "data": "Planets"}},
          {"@type": "FieldValuePair", "field": "name", "value": {"@type": "Value", "data": "Mandalore"}},
          {"@type": "FieldValuePair", "field": "climate", "value": {"@type": "Value", "data": "temperate"}},
          {"@type": "FieldValuePair", "field": "terrain", "value": {"@type": "Value", "data": "forests, jungles"}},
          {"@type": "FieldValuePair", "field": "population", "value": {"@type": "Value", "data": "4200000000"}}
        ]
      }
    }
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
{"bindings": [{}], "inserts": 1, "deletes": 0}
```

{% callout type="note" %}
**Simpler alternative**
For simple inserts, the [Document API](/docs/http-documents-api/) is easier:
`POST /api/document/admin/star_wars/local/branch/main` with the JSON document as body.
{% /callout %}

---

## 6. Update a property

**When to use:** Change a field value on an existing document.

The simplest approach is the Document API with PUT:

```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/star_wars/local/branch/main?author=admin&message=Update+Tatooine+climate" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Planets/1", "@type": "Planets", "name": "Tatooine", "climate": "hot and arid", "terrain": "desert", "population": "200000"}'
```

For conditional updates, use WOQL's `update_document`:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

const query = WOQL.and(
  WOQL.read_document("terminusdb:///data/Planets/1", "v:Doc"),
  WOQL.update_document({
    "@id": "Planets/1",
    "@type": "Planets",
    "name": "Tatooine",
    "climate": "hot and arid",
    "terrain": "desert",
    "population": "200000"
  })
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "ReadDocument",
        "identifier": {"@type": "NodeValue", "node": "terminusdb:///data/Planets/1"},
        "document": {"@type": "Value", "variable": "Doc"}
      },
      {
        "@type": "UpdateDocument",
        "document": {
          "@type": "Value",
          "dictionary": {
            "@type": "DictionaryTemplate",
            "data": [
              {"@type": "FieldValuePair", "field": "@id", "value": {"@type": "Value", "data": "Planets/1"}},
              {"@type": "FieldValuePair", "field": "@type", "value": {"@type": "Value", "data": "Planets"}},
              {"@type": "FieldValuePair", "field": "name", "value": {"@type": "Value", "data": "Tatooine"}},
              {"@type": "FieldValuePair", "field": "climate", "value": {"@type": "Value", "data": "hot and arid"}},
              {"@type": "FieldValuePair", "field": "terrain", "value": {"@type": "Value", "data": "desert"}},
              {"@type": "FieldValuePair", "field": "population", "value": {"@type": "Value", "data": "200000"}}
            ]
          }
        }
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
{"bindings": [{}], "inserts": 1, "deletes": 1}
```

---

## 7. Delete a document

**When to use:** Remove a document from the database.

The simplest approach is the Document API with DELETE:

```bash
curl -u admin:root -X DELETE \
  "http://localhost:6363/api/document/admin/star_wars/local/branch/main?id=terminusdb:///data/Planets/1&author=admin&message=Remove+Tatooine"
```

For conditional deletion, use WOQL's `delete_document`:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

const query = WOQL.delete_document("terminusdb:///data/Planets/1");
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "DeleteDocument",
    "identifier": {"@type": "NodeValue", "node": "terminusdb:///data/Planets/1"}
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
{"bindings": [{}], "inserts": 0, "deletes": 1}
```

---

## 8. Query schema classes

**When to use:** List all document types defined in the schema.

The simplest approach is the Document API:

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/star_wars/local/branch/main?graph_type=schema&as_list=true"
```

For specific class properties, use WOQL with the `schema` graph:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Find all classes and their fields
const query = WOQL.and(
  WOQL.quad("v:Class", "rdf:type", "sys:Class", "schema"),
  WOQL.triple("v:Class", "name", "v:ClassName")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Quad",
        "subject": {"@type": "NodeValue", "variable": "Class"},
        "predicate": {"@type": "NodeValue", "node": "rdf:type"},
        "object": {"@type": "NodeValue", "node": "sys:Class"},
        "graph": "schema"
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Class"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "variable": "ClassName"}
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"Class": "People", "ClassName": "People"}, {"Class": "Planets", "ClassName": "Planets"}, {"Class": "Films", "ClassName": "Films"}]
```

---

## 9. Time-travel (query at a previous commit)

**When to use:** See the database state at any point in history — without modifying anything.

First, get the commit history:

```bash
curl -u admin:root "http://localhost:6363/api/log/admin/star_wars/local/branch/main?count=3"
```

Then query at a specific commit by using the commit path:

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/star_wars/local/commit/{commit_id}?type=People&as_list=true"
```

Replace `{commit_id}` with the commit identifier from the log. You see the exact state at that moment — a snapshot frozen in time.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
  db: "star_wars",
});

// Get commit history
const log = await client.getCommitHistory();
const previousCommit = log[1].identifier;

// Query at that commit
const docs = await client.getDocument({
  type: "People",
  as_list: true,
  commit: previousCommit,
});
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
// Time-travel uses the Document API with a commit path, not WOQL.
// GET /api/document/{org}/{db}/local/commit/{commit_id}?type=People&as_list=true
//
// The commit_id comes from the log:
// GET /api/log/{org}/{db}/local/branch/main?count=3
```
{% /code-tab %}
{% /code-tabs %}

---

## 10. Find documents within a subgraph

**When to use:** Query only documents linked from a specific root — useful for extracting connected subsets.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";
const WOQL = TerminusClient.WOQL;

// Find all films that a specific person appears in
const query = WOQL.and(
  WOQL.triple("v:Person", "name", WOQL.string("Luke Skywalker")),
  WOQL.triple("v:Film", "characters", "v:Person"),
  WOQL.triple("v:Film", "title", "v:Title")
);
```
{% /code-tab %}
{% code-tab label="WOQL AST" %}
```json
{
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "DataValue", "data": "Luke Skywalker"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Film"},
        "predicate": {"@type": "NodeValue", "node": "characters"},
        "object": {"@type": "NodeValue", "variable": "Person"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Film"},
        "predicate": {"@type": "NodeValue", "node": "title"},
        "object": {"@type": "DataValue", "variable": "Title"}
      }
    ]
  }
}
```
{% /code-tab %}
{% /code-tabs %}

**Expected result:**

```json
[{"Title": "A New Hope"}, {"Title": "The Empire Strikes Back"}, {"Title": "Return of the Jedi"}, {"Title": "Revenge of the Sith"}]
```

---

## Next steps

- [WOQL Getting Started](/docs/woql-getting-started/) — learn the query language fundamentals
- [WOQL Class Reference](/docs/woql-class-reference-guide/) — complete AST class documentation
- [Version Control Operations](/docs/version-control-operations/) — branch, merge, diff, time-travel
- [Explore the Ecommerce Dataset](/docs/explore-ecommerce-dataset/) — hands-on tutorial with a business dataset

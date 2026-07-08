---
title: Create a Database
nextjs:
  metadata:
    title: Create a Database — TerminusDB
    description: Create a new TerminusDB database using the HTTP API, TypeScript client, or Python client.
    keywords: terminusdb, create database, javascript client, python client, http api, document database, new database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/create-a-database/
media: []
tags:
  - typescript
  - python
  - documents
  - how-to
  - beginner
---

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363` — see [installation guide](/docs/install-terminusdb-as-a-docker-container/)
- A client SDK installed: [TypeScript](/docs/install-terminusdb-js-client/) or [Python](/docs/install-terminusdb-js-client/)
- A connected client instance: [TypeScript](/docs/connect-with-the-javascript-client/) or [Python](/docs/connect-with-python-client/)
{% /callout %}

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will have created a new database in TerminusDB using the HTTP API, TypeScript, or Python.
{% /callout %}

## Create a database

A database is the top-level container for your data. It holds a schema, instance data, and a full commit history. Create one with a single call.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/db/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"label": "My Database", "comment": "A new database for my project", "schema": true}'
```

**Expected response:**

```json
{"@type":"api:DbCreateResponse","api:status":"api:success"}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
});

await client.createDatabase("tdb-example-mydb", {
  label: "My Database",
  comment: "A new database for my project",
  schema: true,
});

// The client is now connected to "tdb-example-mydb"
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root")

client.create_database(
    "tdb-example-mydb",
    "admin",
    label="My Database",
    description="A new database for my project",
    prefixes={
        "@base": "terminusdb:///data/",
        "@schema": "terminusdb:///schema#",
    },
)
```
{% /code-tab %}
{% /code-tabs %}

---

## Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `label` | Human-readable name for the database | Required |
| `comment` / `description` | Description of the database | Optional |
| `schema` | Whether to create with a schema graph | `true` |
| `prefixes` | Custom `@base` and `@schema` IRI prefixes | TerminusDB defaults |

---

## Next steps

- [Add a Schema](/docs/add-a-schema/) — define document types in your new database
- [Add Documents](/docs/add-a-document/) — insert data once you have a schema
- [Connect Guide](/docs/connect-with-the-javascript-client/) — connection options and authentication

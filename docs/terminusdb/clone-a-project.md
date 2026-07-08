---
title: Clone a Database
nextjs:
  metadata:
    title: Clone a Database
    description: How to clone a TerminusDB database from a remote server using the TypeScript client, Python client, or HTTP API.
    keywords: terminusdb, clone, clone database, replicate, distributed, typescript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/clone-a-project/
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-collaboration-clone.png?raw=true
tags:
  - typescript
  - python
  - collaboration
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- Access to the remote database you want to clone
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have cloned a remote database to your local TerminusDB instance — like `git clone` for databases.
{% /callout %}

Cloning creates a full local copy of a remote database, including all documents, schema, and commit history. You can then modify your clone independently and synchronise changes with [pull](/docs/pull-from-project/) or [push](/docs/push-to-project/).

## Clone a public database

Clone the Star Wars database from the public data server:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const clonePublicDb = async () => {
  client.remoteAuth({ type: "basic", user: "public", key: "public" })
  const cloneDetails = {
    remote_url: "https://data.terminusdb.org/public/star-wars",
    label: "Star Wars",
    comment: "Cloned from public data server",
  }
  await client.clonedb(cloneDetails, "star-wars")
  console.log("Database cloned successfully")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", team="admin")

client.clone(
    "https://data.terminusdb.org/public/star-wars",
    "star-wars",
    remote_auth={"type": "basic", "user": "public", "key": "public"},
)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/clone/admin/star-wars" \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
  -d '{
    "remote_url": "https://data.terminusdb.org/public/star-wars",
    "label": "Star Wars",
    "comment": "Cloned from public data server"
  }'
```
{% /code-tab %}
{% /code-tabs %}

You now have a local copy of the Star Wars database with its full commit history.

## Clone a private database

If the database is not public, provide an API token. For example, to clone from a DFRNT Hub team:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const clonePrivateDb = async () => {
  client.remoteAuth({ type: "token", key: "YOUR_API_TOKEN_HERE" })
  const cloneDetails = {
    remote_url: "https://cloud.dfrnt.com/MyTeam/MyTeam/mydb",
    label: "My Database",
    comment: "Cloned from DFRNT Hub",
  }
  await client.clonedb(cloneDetails, "my_local_copy")
  console.log("Private database cloned successfully")
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", team="admin")

client.clone(
    "https://cloud.dfrnt.com/MyTeam/MyTeam/mydb",
    "my_local_copy",
    remote_auth={"type": "token", "key": "YOUR_API_TOKEN_HERE"},
)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/clone/admin/my_local_copy" \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Token YOUR_API_TOKEN_HERE" \
  -d '{
    "remote_url": "https://cloud.dfrnt.com/MyTeam/MyTeam/mydb",
    "label": "My Database",
    "comment": "Cloned from DFRNT Hub"
  }'
```
{% /code-tab %}
{% /code-tabs %}

## Next steps

- [Pull updates](/docs/pull-from-project/) — fetch new commits from the remote
- [Push changes](/docs/push-to-project/) — send your local changes to the remote
- [Branch](/docs/branch-howto/) — create a feature branch on your clone

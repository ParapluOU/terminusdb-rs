---
title: Pull Updates from Remote
nextjs:
  metadata:
    title: Pull Updates from Remote — TerminusDB
    description: How to pull new commits from a remote TerminusDB database into your local clone using the TypeScript client, Python client, or HTTP API.
    keywords: terminusdb, pull, sync, fetch, remote, distributed, typescript client, python client, http api
    alternates:
      canonical: https://terminusdb.org/docs/pull-from-project/
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
- A cloned database ([clone guide](/docs/clone-a-project/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have pulled new commits from a remote branch into your local clone — like `git pull` for databases.
{% /callout %}

After cloning a database, the remote may receive new commits. Pull fetches those changes and merges them into your local branch.

## Pull from a remote branch

Pull the `enriched` branch from the Star Wars public data server. This branch adds a `Starship` class and starship data:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const pullEnrichedBranch = async () => {
  client.remoteAuth({ type: "basic", user: "public", key: "public" })

  const result = await client.pull({
    remote: "origin",
    remote_branch: "enriched",
    message: "Pull enriched branch with starship data",
  })
  console.log("Pull result:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.connect(user="admin", key="root", team="admin", db="star-wars")

result = client.pull(
    remote="origin",
    remote_branch="enriched",
    message="Pull enriched branch with starship data",
    remote_auth={"type": "basic", "user": "public", "key": "public"},
)
print("Pull result:", result)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/pull/admin/star-wars/local/branch/main" \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
  -d '{
    "remote": "origin",
    "remote_branch": "enriched"
  }'
```
{% /code-tab %}
{% /code-tabs %}

After pulling, your local `main` branch now includes the Starship schema and data from the remote `enriched` branch.

## Inspect what changed

After pulling, inspect the new data that arrived:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const inspectChanges = async () => {
  // Get recent commits to see what was pulled
  const log = await client.getCommitHistory()
  console.log("Recent commits:", log.slice(0, 3))

  // Get all documents to see new starship data
  const docs = await client.getDocument({ as_list: "true", type: "Starship" })
  console.log("Starships:", docs)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
# View recent commits
commits = client.log(count=3)
print("Recent commits:", commits)

# Query the new Starship documents
starships = list(client.get_all_documents(graph_type="instance", type="Starship"))
print("Starships:", starships)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
# View recent commit log
curl -u admin:root \
  "http://localhost:6363/api/log/admin/star-wars/local/branch/main?count=3"

# Get Starship documents
curl -u admin:root \
  "http://localhost:6363/api/document/admin/star-wars/local/branch/main?type=Starship&as_list=true"
```
{% /code-tab %}
{% /code-tabs %}

## Next steps

- [Push changes](/docs/push-to-project/) — send your local modifications to a remote
- [Time travel](/docs/time-travel-howto/) — inspect the state before the pull
- [Reset](/docs/reset-a-project/) — undo the pull by resetting to a previous commit

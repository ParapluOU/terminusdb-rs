---
title: Push Changes to Remote
nextjs:
  metadata:
    title: Push Changes to Remote — TerminusDB
    description: How to push local commits to a remote TerminusDB database using the TypeScript client, Python client, or HTTP API.
    keywords: terminusdb, push, sync, publish, remote, distributed, typescript client, python client, http api
    alternates:
      canonical: https://terminusdb.org/docs/push-to-project/
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
- A cloned database with local changes ([clone guide](/docs/clone-a-project/))
- Write access to the remote database
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have pushed local commits to a remote TerminusDB server — like `git push` for databases.
{% /callout %}

After making changes to your local clone, push sends those commits to the remote so others can access them.

## Push to a remote

Push your local `main` branch to the remote `origin`:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const pushToRemote = async () => {
  client.remoteAuth({ type: "token", key: "YOUR_API_TOKEN_HERE" })

  const result = await client.push({
    remote: "origin",
    remote_branch: "main",
    message: "Push local changes to remote",
  })
  console.log("Push result:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.connect(user="admin", key="root", team="admin", db="my_local_copy")

result = client.push(
    remote="origin",
    remote_branch="main",
    message="Push local changes to remote",
    remote_auth={"type": "token", "key": "YOUR_API_TOKEN_HERE"},
)
print("Push result:", result)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/push/admin/my_local_copy/local/branch/main" \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Token YOUR_API_TOKEN_HERE" \
  -d '{
    "remote": "origin",
    "remote_branch": "main"
  }'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" %}
Push requires **write access** to the remote database. For public data servers (like `data.terminusdb.org`), you can only pull — not push. Push works with your own DFRNT Hub databases or self-hosted remotes where you have credentials.
{% /callout %}

## Push a specific branch

Push a feature branch to a new remote branch:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const pushFeatureBranch = async () => {
  client.remoteAuth({ type: "token", key: "YOUR_API_TOKEN_HERE" })
  client.checkout("feature")

  const result = await client.push({
    remote: "origin",
    remote_branch: "feature",
    message: "Push feature branch for review",
  })
  console.log("Feature branch pushed:", result)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.checkout("feature")

result = client.push(
    remote="origin",
    remote_branch="feature",
    message="Push feature branch for review",
    remote_auth={"type": "token", "key": "YOUR_API_TOKEN_HERE"},
)
print("Feature branch pushed:", result)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/push/admin/my_local_copy/local/branch/feature" \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Token YOUR_API_TOKEN_HERE" \
  -d '{
    "remote": "origin",
    "remote_branch": "feature"
  }'
```
{% /code-tab %}
{% /code-tabs %}

## Next steps

- [Pull updates](/docs/pull-from-project/) — fetch changes others have pushed
- [Branch](/docs/branch-howto/) — create a feature branch before pushing
- [Clone](/docs/clone-a-project/) — start from a fresh clone of a remote database

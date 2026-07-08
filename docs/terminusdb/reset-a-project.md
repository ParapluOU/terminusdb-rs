---
title: Reset a Branch to a Previous Commit
nextjs:
  metadata:
    title: Reset a Branch to a Previous Commit
    description: How to reset a TerminusDB branch to a specific commit using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, reset, rollback, branch, commit, undo, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/reset-a-project/
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-collaboration-reset.png?raw=true
tags:
  - typescript
  - python
  - version-control
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A database with commit history
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have reset a branch to a previous commit — effectively undoing all changes made after that point.
{% /callout %}

Resetting a branch moves the HEAD pointer back to a specific commit, discarding all commits that came after it. This is like `git reset --hard` — use it when you want to undo recent changes.

{% prerequisites-clone /%}

## Get the commit history

First, find the commit ID you want to reset to:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const getHistory = async () => {
  const commits = await client.getCommitHistory()
  console.log("Commits:", commits.slice(0, 5))
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
commits = client.log(count=5)
for commit in commits:
    print(f"{commit['identifier']} — {commit['message']} ({commit['author']})")
```

Example output:

```json
{
  "@id": "InitialCommit/hpl18q42dbnab4vzq8me4bg1xn8p2a0",
  "@type": "InitialCommit",
  "author": "system",
  "identifier": "hpl18q42dbnab4vzq8me4bg1xn8p2a0",
  "message": "create initial schema",
  "timestamp": 1660919664.9129035
}
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root \
  "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=5"
```
{% /code-tab %}
{% /code-tabs %}

## Reset to a specific commit

Once you have a commit identifier, reset the branch HEAD:

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const resetBranch = async () => {
  const commitId = "hpl18q42dbnab4vzq8me4bg1xn8p2a0"
  await client.resetBranch("main", commitId)
  console.log("Branch reset to", commitId)
}
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
commit_id = "hpl18q42dbnab4vzq8me4bg1xn8p2a0"
client.reset(commit_id)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/reset/admin/tdb-example-mydb/local/branch/main" \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/tdb-example-mydb/local/commit/hpl18q42dbnab4vzq8me4bg1xn8p2a0"}'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" %}
Reset is destructive — all commits after the target commit are discarded from the branch. If you need to preserve history while undoing changes, consider using [time travel](/docs/time-travel-howto/) to read the old state and then insert a new corrective commit.
{% /callout %}

## Next steps

- [Time travel](/docs/time-travel-howto/) to inspect historical states without resetting
- [Branch](/docs/branch-howto/) to experiment safely before resetting
- [Squash](/docs/squash-projects/) to clean up commit history without losing data

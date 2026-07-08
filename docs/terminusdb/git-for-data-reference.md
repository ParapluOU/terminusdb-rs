---
tags:
  - reference
  - version-control
  - intermediate
title: Git-for-Data Reference
nextjs:
  metadata:
    title: Git-for-Data Reference — Push, Pull, Fetch, Clone in TerminusDB
    description: Complete reference for Git-for-Data operations in TerminusDB — push, pull, fetch, clone, rebase, and remote management with HTTP API, TypeScript, and Python examples.
    keywords: git for data, terminusdb push, terminusdb pull, terminusdb fetch, terminusdb clone, data replication, database sync, remote database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/git-for-data-reference/
media: []
lastUpdated: "2026-05-01"
---

{% callout title="What you'll achieve" %}
By the end of this guide, you will know how to push, pull, fetch, and clone data between TerminusDB instances — and how to manage remotes, rebase branches, and resolve diverged histories.
{% /callout %}

**Git-for-Data** lets you transport data between TerminusDB instances using operations that mirror git: `clone`, `push`, `pull`, and `fetch`. You can collaborate on structured data by synchronising content repositories across cloud-hosted and local TerminusDB instances.

{% callout title="Prerequisites" %}
- Two TerminusDB instances running (examples use `localhost:6363` as local and `localhost:6364` as remote)
- A database on the remote instance to clone from, or a local database with a remote configured
- Examples use `admin/tdb-example-mydb` with basic authentication
{% /callout %}

{% prerequisites-clone /%}


---

## Remotes

Git-for-data operations use **remotes** — stored references to remote databases including branch information and layer state. One or more remotes can be added to a database.

### Add a remote

Register a remote TerminusDB instance for push/pull operations:

```bash
curl -u admin:root -X POST http://localhost:6363/api/remote/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"remote_name": "origin", "remote_location": "http://localhost:6364/admin/tdb-example-mydb"}'
```

**Expected response:**

```json
{"@type": "api:RemoteResponse", "api:status": "api:success"}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
  db: "tdb-example-mydb",
});

await client.addRemote("origin", "http://localhost:6364/admin/tdb-example-mydb");
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="tdb-example-mydb")

client.add_remote("origin", "http://localhost:6364/admin/tdb-example-mydb")
```
{% /code-tab %}
{% /code-tabs %}

### List remotes

```bash
curl -u admin:root "http://localhost:6363/api/remote/admin/tdb-example-mydb"
```

**Expected response:**

```json
{"@type": "api:RemoteResponse", "api:remote_names": ["origin"], "api:status": "api:success"}
```

### Show remote details

```bash
curl -u admin:root "http://localhost:6363/api/remote/admin/tdb-example-mydb?remote_name=origin"
```

**Expected response:**

```json
{"@type": "api:RemoteResponse", "api:remote_name": "origin", "api:remote_url": "http://localhost:6364/admin/tdb-example-mydb", "api:status": "api:success"}
```

### Update a remote URL

```bash
curl -u admin:root -X PUT http://localhost:6363/api/remote/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"remote_name": "origin", "remote_location": "http://newhost:6363/admin/tdb-example-mydb"}'
```

**Expected response:**

```json
{"@type": "api:RemoteResponse", "api:status": "api:success"}
```

### Delete a remote

```bash
curl -u admin:root -X DELETE "http://localhost:6363/api/remote/admin/tdb-example-mydb?remote_name=origin"
```

**Expected response:**

```json
{"@type": "api:RemoteResponse", "api:status": "api:success"}
```

---

## Clone a database

Clone creates a full copy of a database (schema, all branches, all layers) from one TerminusDB instance to another. A remote named `origin` is automatically configured in the new database.

```bash
curl -u admin:root -X POST http://localhost:6363/api/clone/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{
    "comment": "Clone of remote tdb-example-mydb",
    "label": "My Database",
    "remote_url": "http://localhost:6364/admin/tdb-example-mydb"
  }'
```

**Expected response:**

```json
{"@type": "api:CloneResponse", "api:status": "api:success"}
```

The `Authorization-Remote` header provides credentials for the **remote** instance. The `-u admin:root` authenticates against the **local** instance.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
await client.clonedb({
  comment: "Clone of remote tdb-example-mydb",
  label: "My Database",
  remote_url: "http://localhost:6364/admin/tdb-example-mydb",
}, "admin", "tdb-example-mydb");
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.clonedb("http://localhost:6364/admin/tdb-example-mydb", label="My Database")
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" title="Clone direction matters" %}
Clone pulls data **from** the `remote_url` **into** the local instance. If the remote cannot reach your local instance (e.g. behind a firewall), use [reverse branch cloning](/docs/manual-reverse-branch-cloning/) instead.
{% /callout %}

---

## Fetch

Fetch retrieves layer information from a remote and updates local references — without changing any local branch data. This tells your local instance what the remote looks like.

```bash
curl -u admin:root -X POST http://localhost:6363/api/fetch/admin/tdb-example-mydb/origin/_commits \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA=="
```

**Expected response:**

```json
{"@type": "api:FetchRequest", "api:status": "api:success", "api:head_has_changed": true, "api:head": "Layer_ID_abc123"}
```

The path format is: `/api/fetch/{organization}/{db}/{remote_name}/_commits`

The `Authorization-Remote` header authenticates against the remote instance. The `-u` flag authenticates against the local instance.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const result = await client.fetch("origin");
console.log(result);
// { head_has_changed: true, head: "Layer_ID_abc123" }
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
result = client.fetch("origin")
print(result)
# {"head_has_changed": True, "head": "Layer_ID_abc123"}
```
{% /code-tab %}
{% /code-tabs %}

---

## Pull

Pull fetches remote changes **and** applies them to a local branch. Missing layers from the remote branch are transported and appended to the local branch.

```bash
curl -u admin:root -X POST http://localhost:6363/api/pull/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"remote": "origin", "remote_branch": "main"}'
```

**Expected response (new data pulled):**

```json
{"@type": "api:PullResponse", "api:status": "api:success", "api:head_has_changed": true, "api:head": "Layer_ID_def456"}
```

**Expected response (already up to date):**

```json
{"@type": "api:PullResponse", "api:status": "api:success", "api:head_has_changed": false}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const result = await client.pull({
  remote: "origin",
  remote_branch: "main",
});
console.log(result);
// { head_has_changed: true, head: "Layer_ID_def456" }
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
result = client.pull(remote="origin", remote_branch="main")
print(result)
# {"head_has_changed": True, "head": "Layer_ID_def456"}
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" title="Pull requires linear history" %}
Pull succeeds only if the history has not diverged. If both local and remote have commits the other does not, the pull fails. Use [rebase](#rebase) or [merge](/docs/merge-howto/) to resolve diverged histories first.
{% /callout %}

---

## Push

Push sends local branch changes to a remote branch. Missing layers from the local branch are transported and appended to the remote.

```bash
curl -u admin:root -X POST http://localhost:6363/api/push/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"remote": "origin", "remote_branch": "main"}'
```

**Expected response (new data pushed):**

```json
{"@type": "api:PushResponse", "api:status": "api:success", "api:repo_head_updated": true, "api:repo_head": "Layer_ID_ghi789"}
```

**Expected response (already up to date):**

```json
{"@type": "api:PushResponse", "api:status": "api:success", "api:repo_head_updated": false, "api:repo_head": "Layer_ID_current"}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const result = await client.push({
  remote: "origin",
  remote_branch: "main",
});
console.log(result);
// { repo_head_updated: true, repo_head: "Layer_ID_ghi789" }
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
result = client.push(remote="origin", remote_branch="main")
print(result)
# {"repo_head_updated": True, "repo_head": "Layer_ID_ghi789"}
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="warning" title="Push requires linear history" %}
Push succeeds only if the remote branch history is a strict ancestor of the local branch. If the remote has diverged (someone else pushed), you must pull and resolve first.
{% /callout %}

### Push with prefixes

To also push prefix/context mappings along with data:

```bash
curl -u admin:root -X POST http://localhost:6363/api/push/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"remote": "origin", "remote_branch": "main", "push_prefixes": true}'
```

---

## Rebase

Rebase replays commits from one branch onto another. Both branches must share a common ancestor commit. Commit messages are retained.

```bash
curl -u admin:root -X POST http://localhost:6363/api/rebase/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"author": "alice@example.com", "rebase_from": "admin/tdb-example-mydb/local/branch/feature"}'
```

**Expected response:**

```json
{
  "@type": "api:RebaseResponse",
  "api:status": "api:success",
  "api:forwarded_commits": ["commit/abc123", "commit/def456"],
  "api:rebase_report": [],
  "api:common_commit_id": "commit/ancestor789"
}
```

This replays all commits from `feature` onto `main`, starting from the common ancestor.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const result = await client.rebase({
  rebase_from: "admin/tdb-example-mydb/local/branch/feature",
  author: "alice@example.com",
});
console.log(result.forwarded_commits);
// ["commit/abc123", "commit/def456"]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
result = client.rebase(
    "admin/tdb-example-mydb/local/branch/feature",
    author="alice@example.com"
)
print(result["forwarded_commits"])
# ["commit/abc123", "commit/def456"]
```
{% /code-tab %}
{% /code-tabs %}

---

## Complete workflow: clone, edit, push

This workflow demonstrates a full collaboration cycle — clone a database, make changes locally, and push them back:

```bash
# 1. Clone the remote database
curl -u admin:root -X POST http://localhost:6363/api/clone/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"comment": "Working copy", "label": "My Database", "remote_url": "http://remote:6363/admin/tdb-example-mydb"}'

# 2. Add a document locally
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?author=alice&message=Add+new+product" \
  -H "Content-Type: application/json" \
  -d '{"@type": "Product", "name": "Widget", "price": 9.99}'

# 3. Push changes to remote
curl -u admin:root -X POST http://localhost:6363/api/push/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"remote": "origin", "remote_branch": "main"}'
```

## Complete workflow: pull remote changes

```bash
# 1. Fetch to see if remote has changed
curl -u admin:root -X POST http://localhost:6363/api/fetch/admin/tdb-example-mydb/origin/_commits \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA=="

# 2. Pull changes into local main
curl -u admin:root -X POST http://localhost:6363/api/pull/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -H "Authorization-Remote: Basic YWRtaW46cm9vdA==" \
  -d '{"remote": "origin", "remote_branch": "main"}'

# 3. Verify the new data is present
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb?type=Product&as_list=true"
```

---

## Schema and instance separation

Push and pull transport **instance data only** — schema changes are not synchronised automatically. You must maintain schema consistency manually across instances.

To push schemas along with instance data, pass `"push_prefixes": true` in the push request (see [Push with prefixes](#push-with-prefixes)).

---

## Operational technology environments

Git-for-Data is useful in environments with strict network segmentation (IEC 62443, Purdue model) where databases cannot maintain persistent connections. You can:

1. Clone a database to an isolated environment
2. Make changes offline
3. Push changes back when connectivity is available

For environments where the cloud instance cannot reach the local instance, see [Manual Reverse Branch Cloning](/docs/manual-reverse-branch-cloning/) and [Transfer Data in Operational Technologies Landscapes](/docs/operational-technologies-transfer/).

---

## Next steps

- [How to Branch](/docs/branch-howto/) — create and manage branches locally
- [How to Merge](/docs/merge-howto/) — merge branches with conflict detection
- [Manual Reverse Branch Cloning](/docs/manual-reverse-branch-cloning/) — clone when cloud cannot reach local
- [Transfer Data in OT Landscapes](/docs/operational-technologies-transfer/) — network-segmented environments
- [Version Control Operations Reference](/docs/version-control-operations/) — complete API reference

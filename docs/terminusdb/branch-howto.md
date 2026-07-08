---
tags:
  - how-to
  - version-control
  - beginner
title: How to Branch Your Database
nextjs:
  metadata:
    title: How to Branch Your Database — TerminusDB Branching Guide
    description: Step-by-step guide to database branching in TerminusDB — create, list, switch, diff, merge, and delete branches with HTTP API, TypeScript, and Python examples.
    keywords: branch database, create branch terminusdb, database branching, git branch for data, terminusdb branch, data branching, version control database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/branch-howto/
media: []
lastUpdated: "2026-05-01"
---

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will know how to create, switch, diff, merge, and delete branches in TerminusDB using the HTTP API, TypeScript, and Python.
{% /callout %}

This guide shows you how to work with branches in TerminusDB — create isolated workspaces, make changes, review diffs, and merge back to main. Every operation includes HTTP API, TypeScript, and Python examples.

{% callout type="note" title="Git branch for data" %}
Branching in TerminusDB works exactly like `git branch` — create a lightweight branch, make changes in isolation, then merge back when ready. Branches share history until they diverge, making them cheap to create.
{% /callout %}

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363` — see [installation guide](/docs/install-terminusdb-as-a-docker-container/)
- A database with a schema. Examples use `admin/tdb-example-mydb` with a `Product` class. Set up:
{% /callout %}

{% prerequisites-clone /%}


```bash
# Create the database
curl -u admin:root -X POST http://localhost:6363/api/db/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"label": "My Database"}'

# Add a Product schema class
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb?graph_type=schema&author=setup&message=Add+schema" \
  -H "Content-Type: application/json" \
  -d '{"@type": "Class", "@id": "Product", "name": "xsd:string", "price": "xsd:decimal", "category": "xsd:string"}'
```

---

## 1. Create a new branch

Create a branch from the current head of main. The new branch shares all existing history and diverges from this point forward.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/branch/admin/tdb-example-mydb/local/branch/feature \
  -H "Content-Type: application/json" \
  -d '{"origin": "admin/tdb-example-mydb/local/branch/main"}'
```

**Expected response:**

```json
{"@type":"api:BranchResponse","api:status":"api:success"}
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

await client.branch("feature");
// Branch "feature" created from current head of main
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="tdb-example-mydb")

client.branch("feature")
# Branch "feature" created from current head of main
```
{% /code-tab %}
{% /code-tabs %}

---

## 2. List all branches

See all branches in your database.

### HTTP API

```bash
curl -u admin:root "http://localhost:6363/api/db/admin/tdb-example-mydb?branches=true"
```

**Expected response:**

```json
{"path": "admin/tdb-example-mydb", "branches": ["feature", "main"]}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
// List branches by fetching db info
const info = await client.getDatabase("tdb-example-mydb");
console.log(info.branches);
// ["feature", "main"]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
# The Python client provides a branches property
branches = client.get_all_branches()
for branch in branches:
    print(branch)
# {"name": "main", ...}, {"name": "feature", ...}
```
{% /code-tab %}
{% /code-tabs %}

---

## 3. Switch to a branch

There is no "checkout" command — you simply target a different branch path in your API calls. Every TerminusDB API path includes the branch:

- **main:** `/api/document/admin/tdb-example-mydb/local/branch/main`
- **feature:** `/api/document/admin/tdb-example-mydb/local/branch/feature`

### HTTP API

```bash
# Read documents from main
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?type=Product&as_list=true"

# Read documents from feature branch — just change the path
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/feature?type=Product&as_list=true"
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
// Switch branch context for all subsequent operations
client.checkout("feature");

// All operations now target the feature branch
const docs = await client.getDocument({ type: "Product", as_list: true });
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
# Switch branch context
client.checkout("feature")

# All operations now target the feature branch
docs = client.get_all_documents()
```
{% /code-tab %}
{% /code-tabs %}

---

## 4. Make changes on a branch

Insert or update documents on your branch. Changes are isolated — main is unaffected until you merge.

### HTTP API

```bash
# Insert a new document on the feature branch
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/feature?author=alice&message=Add+Widget+product" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99, "category": "tools"}'
```

**Expected response:**

```json
["terminusdb:///data/Product/Widget"]
```

```bash
# Update an existing document on the feature branch
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/feature?author=alice&message=Update+Widget+price" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 14.99, "category": "tools"}'
```

**Expected response:**

```json
["terminusdb:///data/Product/Widget"]
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
client.checkout("feature");

// Insert
await client.addDocument(
  { "@type": "Product", "name": "Widget", "price": 9.99, "category": "tools" },
  { author: "alice", message: "Add Widget product" }
);

// Update
await client.updateDocument(
  { "@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 14.99, "category": "tools" },
  { author: "alice", message: "Update Widget price" }
);
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.checkout("feature")

# Insert
client.insert_document(
    {"@type": "Product", "name": "Widget", "price": 9.99, "category": "tools"},
    commit_msg="Add Widget product"
)

# Update
client.update_document(
    {"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 14.99, "category": "tools"},
    commit_msg="Update Widget price"
)
```
{% /code-tab %}
{% /code-tabs %}

---

## 5. Compare a branch to main (diff)

See exactly what changed on your branch compared to main — a structural, field-level diff.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "main", "after_data_version": "feature"}'
```

**Expected response:**

```json
[
  {
    "@insert": {
      "@id": "Product/Widget",
      "@type": "Product",
      "name": "Widget",
      "price": 14.99,
      "category": "tools"
    },
    "@op": "Insert"
  }
]
```

The diff shows typed operations:
- **Insert** — a new document exists on the branch but not on main
- **Delete** — a document exists on main but not on the branch
- **SwapValue** — a field value changed (when the document already existed on both branches)

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const diff = await client.getVersionDiff(
  "admin/tdb-example-mydb/local/branch/main",
  "admin/tdb-example-mydb/local/branch/feature"
);
console.log(JSON.stringify(diff, null, 2));
// Shows field-level changes between branches
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
diff = client.diff_version("main", "feature")
for change in diff:
    print(change)
# {"@id": "Product/Widget", "price": {"@op": "SwapValue", "@before": 9.99, "@after": 14.99}}
```
{% /code-tab %}
{% /code-tabs %}

---

## 6. Merge a branch back to main

Apply all changes from your branch onto main. If both branches modified the same field, the merge fails with a conflict report — no silent overwrites.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{
    "before_commit": "branch:main",
    "after_commit": "branch:feature",
    "commit_info": {
      "author": "alice@example.com",
      "message": "Merge feature branch: add Widget product"
    }
  }'
```

**Expected response (success):**

```json
{"@type":"api:ApplyResponse","api:status":"api:success"}
```

**If there are conflicts:**

```json
{
  "@type": "api:ApplyResponse",
  "api:status": "api:conflict",
  "api:witnesses": [
    {"@id": "Product/Widget", "price": {"@op": "SwapValue", "@before": 9.99, "@after_left": 12.99, "@after_right": 14.99}}
  ]
}
```

Conflicts must be resolved manually — TerminusDB never silently picks a winner.

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
await client.apply(
  "admin/tdb-example-mydb/local/branch/main",    // target
  "branch:main",                     // before (common ancestor)
  "branch:feature",                  // after (source of changes)
  { author: "alice@example.com", message: "Merge feature branch" }
);
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.checkout("main")
client.apply(
    before_commit="admin/tdb-example-mydb/local/branch/main",
    after_commit="admin/tdb-example-mydb/local/branch/feature",
    commit_msg="Merge feature branch: add Widget product",
    author="alice@example.com"
)
```
{% /code-tab %}
{% /code-tabs %}

---

## 7. Delete a branch

Remove a branch after merging. The commits remain in the database history but this branch name no longer reaches them.

### HTTP API

```bash
curl -u admin:root -X DELETE http://localhost:6363/api/branch/admin/tdb-example-mydb/local/branch/feature
```

**Expected response:**

```json
{"@type":"api:BranchResponse","api:status":"api:success"}
```

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
await client.deleteBranch("feature");
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.delete_branch("feature")
```
{% /code-tab %}
{% /code-tabs %}

---

## Complete workflow example

Here is the full branch workflow end-to-end — create, change, review, merge, clean up:

```bash
# 1. Create a feature branch
curl -u admin:root -X POST http://localhost:6363/api/branch/admin/tdb-example-mydb/local/branch/price-update \
  -H "Content-Type: application/json" \
  -d '{"origin": "admin/tdb-example-mydb/local/branch/main"}'

# 2. Make changes on the branch
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/price-update?author=alice&message=Add+new+product" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Gadget", "@type": "Product", "name": "Gadget", "price": 24.99, "category": "electronics"}'

# 3. Review what changed (diff against main)
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "main", "after_data_version": "price-update"}'

# 4. Merge into main
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"before_commit": "branch:main", "after_commit": "branch:price-update", "commit_info": {"author": "alice", "message": "Merge price-update into main"}}'

# 5. Clean up the branch
curl -u admin:root -X DELETE http://localhost:6363/api/branch/admin/tdb-example-mydb/local/branch/price-update
```

---

## Next steps

- [Version Control Operations Reference](/docs/version-control-operations/) — full HTTP API reference for all version control operations
- [Diff & Patch](/docs/diff-and-patch-operations/) — deeper dive into structural diffing
- [Time Travel](/docs/time-travel-howto/) — query the database at any historical commit
- [WOQL Common Patterns](/docs/woql-common-patterns/) — practical query recipes

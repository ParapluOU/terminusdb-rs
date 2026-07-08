---
tags:
  - how-to
  - version-control
  - beginner
title: How to Merge Branches
nextjs:
  metadata:
    title: How to Merge Branches in TerminusDB — Conflict Detection & Resolution
    description: Step-by-step guide to merging branches in TerminusDB — apply changes, detect conflicts, resolve them, and roll back failed merges. HTTP API, TypeScript, and Python examples.
    keywords: merge branches database, terminusdb merge, database merge conflict, git merge for data, apply changes branch, conflict resolution database, three-way merge
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/merge-howto/
media: []
lastUpdated: "2026-05-01"
---

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will know how to merge branches, detect conflicts, resolve them, and roll back failed merges in TerminusDB.
{% /callout %}

**Merging** in TerminusDB applies changes from one branch onto another — like `git merge`. TerminusDB uses a three-way merge that detects conflicts at the field level. If both branches modified the same field on the same document, the merge fails with a precise conflict report — no silent data loss.

{% callout type="note" title="No silent overwrites" %}
TerminusDB never silently resolves conflicts. If two branches changed the same field, the merge fails and reports exactly what conflicted. You decide which value wins.
{% /callout %}

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363`
- A database with at least two branches. See [How to Branch](/docs/branch-howto/) to create one.
- Examples use `admin/tdb-example-mydb` with branches `main` and `feature`.
{% /callout %}

{% prerequisites-clone /%}


---

## 1. Merge a branch into main

Apply all changes from `feature` onto `main`. This is the most common merge operation.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{
    "before_commit": "admin/tdb-example-mydb/local/branch/main",
    "after_commit": "admin/tdb-example-mydb/local/branch/feature",
    "commit_info": {
      "author": "alice@example.com",
      "message": "Merge feature into main"
    }
  }'
```

**Expected response (success):**

```json
{"@type": "api:ApplyResponse", "api:status": "api:success"}
```

### TypeScript

```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
  db: "tdb-example-mydb",
});

await client.apply(
  "admin/tdb-example-mydb/local/branch/main",    // target branch
  "admin/tdb-example-mydb/local/branch/main",    // before (common ancestor)
  "admin/tdb-example-mydb/local/branch/feature", // after (source of changes)
  { author: "alice@example.com", message: "Merge feature into main" }
);
```

### Python

```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="tdb-example-mydb")

client.apply(
    before_commit="admin/tdb-example-mydb/local/branch/main",
    after_commit="admin/tdb-example-mydb/local/branch/feature",
    commit_msg="Merge feature into main",
    author="alice@example.com"
)
```

---

## 2. Preview changes before merging (diff)

Always review what will change before merging. The diff shows exactly which documents and fields were modified.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "main", "after_data_version": "feature"}'
```

**Expected response:**

```json
[
  {"@id": "terminusdb:///data/Product/Widget", "price": {"@op": "SwapValue", "@before": 9.99, "@after": 14.99}},
  {"@id": "terminusdb:///data/Product/Gadget", "@op": "Insert"}
]
```

**Operations explained:**
- `SwapValue` — a field value changed (shows before and after)
- `Insert` — a new document exists on the source branch but not on the target
- `Delete` — a document exists on the target but not on the source branch

---

## 3. Handle merge conflicts

A conflict occurs when both branches changed the same field on the same document. TerminusDB detects this and rejects the merge.

### Conflict response

```json
{
  "@type": "api:ApplyResponse",
  "api:status": "api:conflict",
  "api:witnesses": [
    {
      "@id": "terminusdb:///data/Product/Widget",
      "price": {
        "@op": "SwapValue",
        "@before": 9.99,
        "@after_left": 12.99,
        "@after_right": 14.99
      }
    }
  ]
}
```

This tells you:
- **Document:** `Product/Widget`
- **Field:** `price`
- **Original value:** 9.99
- **main changed it to:** 12.99 (`@after_left`)
- **feature changed it to:** 14.99 (`@after_right`)

### Resolving conflicts

To resolve, you must decide which value wins and apply it manually:

```bash
# Option A: Accept the feature branch value (14.99)
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=alice&message=Resolve+conflict:+accept+feature+price" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 14.99}'

# Then retry the merge (remaining non-conflicting changes will apply)
```

```bash
# Option B: Keep main's value (12.99) — no action needed on main
# Just update the feature branch to match main before re-merging:
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/feature?author=alice&message=Resolve+conflict:+accept+main+price" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 12.99}'
```

---

## 4. Squash before merging

Collapse all commits on a branch into a single commit — useful for cleaning up history before merging.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/squash/admin/tdb-example-mydb/local/branch/feature \
  -H "Content-Type: application/json" \
  -d '{"commit_info": {"author": "alice@example.com", "message": "Squash feature branch"}}'
```

**Expected response:**

```json
{"@type": "api:SquashResponse", "api:status": "api:success", "api:commit": "system:data/admin/tdb-example-mydb/local/branch/feature/commit/abc123"}
```

After squashing, the feature branch has one clean commit that you can merge into main.

---

## 5. Merge from main into feature (rebase pattern)

Keep your feature branch up-to-date with main by merging main's changes into your branch:

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/tdb-example-mydb/local/branch/feature \
  -H "Content-Type: application/json" \
  -d '{
    "before_commit": "admin/tdb-example-mydb/local/branch/feature",
    "after_commit": "admin/tdb-example-mydb/local/branch/main",
    "commit_info": {
      "author": "alice@example.com",
      "message": "Merge latest main into feature"
    }
  }'
```

This brings your feature branch up-to-date with main's latest changes, reducing the chance of conflicts when you later merge back.

---

## Complete merge workflow

```bash
# 1. Check what changed on feature vs main
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "main", "after_data_version": "feature"}'

# 2. Squash feature branch for clean history
curl -u admin:root -X POST http://localhost:6363/api/squash/admin/tdb-example-mydb/local/branch/feature \
  -H "Content-Type: application/json" \
  -d '{"commit_info": {"author": "alice", "message": "Feature: add Widget product"}}'

# 3. Merge into main
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"before_commit": "admin/tdb-example-mydb/local/branch/main", "after_commit": "admin/tdb-example-mydb/local/branch/feature", "commit_info": {"author": "alice", "message": "Merge feature: add Widget product"}}'

# 4. Clean up the branch
curl -u admin:root -X DELETE http://localhost:6363/api/branch/admin/tdb-example-mydb/local/branch/feature
```

---

## Next steps

- [How to Branch](/docs/branch-howto/) — create and manage branches
- [How to Undo Changes](/docs/undo-reset-howto/) — reset, revert, and rollback
- [Version Control Operations Reference](/docs/version-control-operations/) — complete API reference
- [Diff & Patch](/docs/diff-and-patch-operations/) — advanced structural diffing

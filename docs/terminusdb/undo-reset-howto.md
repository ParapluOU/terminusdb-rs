---
tags:
  - how-to
  - version-control
  - beginner
title: How to Undo Changes (Reset, Revert, Squash)
nextjs:
  metadata:
    title: How to Undo Changes in TerminusDB — Reset, Revert, and Squash
    description: Step-by-step guide to undoing changes in TerminusDB — reset to a previous commit, revert specific changes, squash commits, and understand the difference between reset and revert. HTTP API, TypeScript, and Python examples.
    keywords: undo database changes, reset database, revert commit database, database rollback, terminusdb reset, squash commits database, undo last commit, database undo history
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/undo-reset-howto/
media: []
lastUpdated: "2026-05-01"
---

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will know how to reset branches, revert specific changes, squash commits, and recover from accidental resets.
{% /callout %}

**Undoing changes** in TerminusDB is safe because the database is immutable — it preserves every commit in history. You can reset a branch to a previous state, revert specific changes, or squash messy commit history into a clean single commit.

{% callout type="note" title="Nothing is truly lost" %}
TerminusDB never deletes commits. Even after a reset, the "undone" commits still exist in the immutable commit graph — the branch pointer just no longer points to them. You can always recover by resetting forward again.
{% /callout %}

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363`
- A database with commit history. Examples use `admin/tdb-example-mydb`.
- Know the commit identifier you want to return to. Use `GET /api/log/...` to find it (see [Time-Travel How-To](/docs/time-travel-howto/)).
{% /callout %}

{% prerequisites-clone /%}


---

## Reset vs Revert — which do you need?

| Operation | What it does | When to use |
|-----------|--------------|-------------|
| **Reset** | Moves the branch pointer back to an earlier commit. Subsequent commits become unreachable. | "Throw away everything after this point" |
| **Revert** | Creates a *new* commit that undoes specific changes while preserving history. | "Undo one change but keep later ones" |
| **Squash** | Collapses all commits on a branch into a single commit. | "Clean up messy history before merging" |

---

## 1. Undo the last commit (reset)

Move the branch pointer back to the previous commit. The latest commit becomes unreachable (but still exists in the immutable store).

### HTTP API

First, get the commit history to find the commit you want to reset to:

```bash
# Get recent commits
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=3"
```

```json
[
  {"identifier": "abc123", "message": "Unwanted change", "timestamp": "2026-04-30T14:22:01.000Z"},
  {"identifier": "def456", "message": "Good change", "timestamp": "2026-04-29T10:15:30.000Z"},
  {"identifier": "ghi789", "message": "Initial data", "timestamp": "2026-04-28T09:00:00.000Z"}
]
```

Now reset to the commit *before* the unwanted change:

```bash
curl -u admin:root -X POST http://localhost:6363/api/reset/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/tdb-example-mydb/local/branch/main/commit/def456"}'
```

**Expected response:**

```json
{"@type": "api:ResetResponse", "api:status": "api:success"}
```

The branch now points to `def456`. The `abc123` commit still exists but `main` no longer reaches it.

### TypeScript

```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
  db: "tdb-example-mydb",
});

// Reset main to the previous commit
await client.resetBranch("main", "admin/tdb-example-mydb/local/branch/main/commit/def456");
```

### Python

```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="tdb-example-mydb")

# Reset main to the previous commit
client.reset("admin/tdb-example-mydb/local/branch/main/commit/def456")
```

---

## 2. Reset to any commit in history

You can reset to any commit — not just the previous one. This is useful for rolling back multiple commits at once.

### HTTP API

```bash
# Reset to a commit from 3 days ago
curl -u admin:root -X POST http://localhost:6363/api/reset/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/tdb-example-mydb/local/branch/main/commit/ghi789"}'
```

**Expected response:**

```json
{"@type": "api:ResetResponse", "api:status": "api:success"}
```

{% callout type="warning" title="Reset is destructive (at the branch level)" %}
All commits after the target become unreachable from this branch. They still exist in the immutable store, so you can recover by resetting *forward* to them if you know their identifier. But if you have not noted the commit ID, they become hard to find.
{% /callout %}

---

## 3. Revert a specific change (preserve history)

If you want to undo a specific change without losing later commits, read the document from before the change and write it back as a new commit:

### HTTP API

```bash
# 1. Get the document as it was before the unwanted change
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/def456?id=terminusdb:///data/Product/Widget"
# Response: {"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}

# 2. Write it back to main — this creates a NEW commit (history preserved)
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=alice&message=Revert:+restore+Widget+original+price" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}'
```

**Expected response:**

```json
["terminusdb:///data/Product/Widget"]
```

The commit history now shows: original → change → revert. You lose nothing.

### TypeScript

```typescript
// Read from the commit before the change
const original = await client.getDocument({
  id: "terminusdb:///data/Product/Widget",
  commit: "def456"
});

// Write it back (creates a new "revert" commit)
client.checkout("main");
await client.updateDocument(original, {
  author: "alice",
  message: "Revert: restore Widget original price"
});
```

### Python

```python
# Read from the commit before the change
original = client.get_document("terminusdb:///data/Product/Widget", commit="def456")

# Write it back (creates a new "revert" commit)
client.checkout("main")
client.update_document(original, commit_msg="Revert: restore Widget original price")
```

---

## 4. Squash commits (clean up history)

Collapse all commits on a branch into a single commit. Useful before merging a feature branch with many small intermediate commits.

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/squash/admin/tdb-example-mydb/local/branch/feature \
  -H "Content-Type: application/json" \
  -d '{"commit_info": {"author": "alice@example.com", "message": "Feature: add product catalogue"}}'
```

**Expected response:**

```json
{"@type": "api:SquashResponse", "api:status": "api:success", "api:commit": "system:data/admin/tdb-example-mydb/local/branch/feature/commit/new123"}
```

The branch now has a single commit containing all the cumulative changes. This branch no longer reaches the individual intermediate commits.

### TypeScript

```typescript
await client.squashBranch("feature", {
  author: "alice@example.com",
  message: "Feature: add product catalogue"
});
```

### Python

```python
client.squash("Feature: add product catalogue", author="alice@example.com")
```

---

## 5. Recover from an accidental reset

If you reset too far back and need to recover, you can reset *forward* to the commit you accidentally abandoned — as long as you know its identifier:

```bash
# You accidentally reset to ghi789 but wanted to keep abc123
# If you noted abc123's identifier, simply reset forward:
curl -u admin:root -X POST http://localhost:6363/api/reset/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/tdb-example-mydb/local/branch/main/commit/abc123"}'
```

This is possible because commits are immutable — TerminusDB never deletes them, it only unreferences them.

---

## Complete workflow: undo a bad deploy

```bash
# 1. Get commit history to find the last good state
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=5"

# 2. Identify the last good commit (e.g., def456)

# 3. Verify what the database looked like at that commit
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/def456?type=Product&as_list=true"

# 4. Diff current state vs the good state
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "admin/tdb-example-mydb/local/commit/def456", "after_data_version": "main"}'

# 5. Reset to the good state
curl -u admin:root -X POST http://localhost:6363/api/reset/admin/tdb-example-mydb/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/tdb-example-mydb/local/branch/main/commit/def456"}'

# Done — main is back to the good state. The bad commits are unreachable but still exist.
```

---

## Next steps

- [How to Time-Travel](/docs/time-travel-howto/) — query data at any historical commit
- [How to Branch](/docs/branch-howto/) — create isolated workspaces for changes
- [How to Merge](/docs/merge-howto/) — merge branches with conflict detection
- [Version Control Operations Reference](/docs/version-control-operations/) — complete API reference

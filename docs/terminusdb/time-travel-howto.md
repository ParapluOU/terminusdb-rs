---
tags:
  - how-to
  - version-control
  - beginner
title: How to Time-Travel (Query Historical Data)
nextjs:
  metadata:
    title: How to Time-Travel in TerminusDB — Query Data at Any Point in History
    description: Step-by-step guide to querying historical data in TerminusDB — get commit history, query at a specific commit, compare points in time, and restore previous states. HTTP API, TypeScript, and Python examples.
    keywords: time travel database, query historical data, database history, point-in-time query, terminusdb time travel, database audit trail, immutable database history, version history database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/time-travel-howto/
media: []
lastUpdated: "2026-05-18"
---

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will know how to query your database at any historical commit, compare points in time, and restore previous states.
{% /callout %}

**Time-travel** in TerminusDB lets you query the database as it was at any point in history — without modifying anything. Because every write creates an immutable commit, you can "go back in time" to see the exact state of your data at any previous commit. No backups, no log replay, no special configuration.

{% callout type="note" title="How it works" %}
TerminusDB stores data as immutable delta layers. When you time-travel, you read a snapshot frozen at that commit — the current database stays unchanged. This is read-only; you cannot accidentally modify history.
{% /callout %}

{% callout type="note" title="Prerequisites" %}
- TerminusDB running on `localhost:6363`
- A database with some commit history. Examples use `admin/tdb-example-mydb`.
{% /callout %}

{% prerequisites-clone /%}


---

## 1. Get the commit history

View the commit log to find the commit you want to travel to.

### HTTP API

```bash
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=5"
```

**Expected response:**

```json
[
  {
    "@id": "commit/abc123def456",
    "author": "alice@example.com",
    "message": "Update Widget price to 14.99",
    "timestamp": "2026-04-30T14:22:01.000Z",
    "identifier": "abc123def456"
  },
  {
    "@id": "commit/789xyz012345",
    "author": "alice@example.com",
    "message": "Add Widget product",
    "timestamp": "2026-04-29T10:15:30.000Z",
    "identifier": "789xyz012345"
  },
  {
    "@id": "commit/initial000001",
    "author": "admin",
    "message": "Initial schema",
    "timestamp": "2026-04-28T09:00:00.000Z",
    "identifier": "initial000001"
  }
]
```

Each commit has an `identifier` that you use for time-travel queries.

### TypeScript

```typescript
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  key: "root",
  organization: "admin",
  db: "tdb-example-mydb",
});

const log = await client.getCommitHistory({ count: 5 });
for (const commit of log) {
  console.log(`${commit.identifier} — ${commit.author}: ${commit.message}`);
}
```

### Python

```python
from terminusdb_client import Client

client = Client("http://localhost:6363")
client.connect(user="admin", key="root", db="tdb-example-mydb")

log = client.get_commit_history(count=5)
for commit in log:
    print(f"{commit['identifier']} — {commit['author']}: {commit['message']}")
```

---

## 2. Query data at a specific commit

{% callout type="note" title="Connected workflow" %}
Pick a commit `identifier` from the history response above (e.g., `789xyz012345`) and use it in the URL below. This is the core time-travel operation: replace `local/branch/main` with `local/commit/{identifier}` to query the database as it was at that exact moment.
{% /callout %}

Replace the branch path with a commit path to see the database at that exact moment.

### HTTP API

```bash
# Query all Product documents as they were at commit 789xyz012345
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?type=Product&as_list=true"
```

**Expected response:**

```json
[
  {"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}
]
```

Note: the price is 9.99 (the value at that commit), not 14.99 (the current value). You are looking at a frozen snapshot.

### TypeScript

```typescript
// Query at a specific commit
const historicalDocs = await client.getDocument({
  type: "Product",
  as_list: true,
  commit: "789xyz012345"
});
console.log(historicalDocs);
// [{name: "Widget", price: 9.99}] — the value at that point in time
```

### Python

```python
# Query at a specific commit
docs = client.get_all_documents(commit="789xyz012345")
for doc in docs:
    print(doc)
# {"name": "Widget", "price": 9.99} — the value at that point in time
```

---

## 3. Compare two points in time

Use diff to see exactly what changed between any two commits:

### HTTP API

```bash
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "admin/tdb-example-mydb/local/commit/789xyz012345",
    "after_data_version": "admin/tdb-example-mydb/local/commit/abc123def456"
  }'
```

**Expected response:**

```json
[
  {
    "@id": "terminusdb:///data/Product/Widget",
    "price": {"@op": "SwapValue", "@before": 9.99, "@after": 14.99}
  }
]
```

This shows you exactly what changed between those two commits — in this case, the Widget price went from 9.99 to 14.99.

---

## 4. Query a specific document's history

To see how a single document evolved over time, query it at successive commits:

### HTTP API

```bash
# Get the document at the current state
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?id=terminusdb:///data/Product/Widget"

# Get the same document at a previous commit
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?id=terminusdb:///data/Product/Widget"
```

Compare the responses to see how the document changed over time.

---

## 5. Restore a document to a previous state

If you want to revert a document to its historical value, read it from the commit and write it back to the current branch:

### HTTP API

```bash
# 1. Read the document at the historical commit
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?id=terminusdb:///data/Product/Widget"
# Response: {"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}

# 2. Write it back to main (this creates a new commit — history is preserved)
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=alice&message=Restore+Widget+to+original+price" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}'
```

**Expected response:**

```json
["terminusdb:///data/Product/Widget"]
```

This does not erase history — it creates a new commit that sets the document back to its previous value. TerminusDB preserves the full history of changes.

### TypeScript

```typescript
// Read from historical commit
const historical = await client.getDocument({
  id: "terminusdb:///data/Product/Widget",
  commit: "789xyz012345"
});

// Write back to current branch
client.checkout("main");
await client.updateDocument(historical, {
  author: "alice",
  message: "Restore Widget to original price"
});
```

### Python

```python
# Read from historical commit
historical = client.get_document("terminusdb:///data/Product/Widget", commit="789xyz012345")

# Write back to current branch
client.checkout("main")
client.update_document(historical, commit_msg="Restore Widget to original price")
```

---

## Complete workflow: audit and restore

```bash
# 1. Get recent commit history
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=10"

# 2. Find the commit before the unwanted change (e.g., 789xyz012345)

# 3. See what the database looked like at that commit
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?type=Product&as_list=true"

# 4. Diff current state vs that historical commit
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/tdb-example-mydb \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "admin/tdb-example-mydb/local/commit/789xyz012345", "after_data_version": "main"}'

# 5. If you want to restore to that state, use reset (see How to Undo Changes)
#    or selectively restore individual documents as shown above
```

---

## Next steps

- [How to Undo Changes](/docs/undo-reset-howto/) — reset branches, revert commits, squash history
- [How to Branch](/docs/branch-howto/) — create and manage branches
- [How to Merge](/docs/merge-howto/) — merge branches with conflict handling
- [Version Control Operations Reference](/docs/version-control-operations/) — complete API reference

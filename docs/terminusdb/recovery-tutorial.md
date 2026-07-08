---
tags:
  - version-control
  - tutorial
  - curl
title: Recover Data from Version History
nextjs:
  metadata:
    title: Recover Data from Version History — TerminusDB Tutorial
    description: Step-by-step tutorial to recover deleted or corrupted data using TerminusDB's immutable commit history. View the log, branch from a known-good commit, and reset.
    keywords: recovery, reset, commit, version history, rollback, undo
    alternates:
      canonical: https://terminusdb.org/docs/recovery-tutorial/
---

Every write in TerminusDB creates an immutable commit. Data is never lost — only the branch HEAD moves forward. This tutorial shows how to use the commit log to identify a known-good state and reset your branch to recover from a bad change.

**Time:** ~10 minutes
**Prerequisites:** TerminusDB running on `localhost:6363` ([Install guide →](/docs/install-terminusdb-as-a-docker-container))

## Setup

```bash
export AUTH="admin:root"
export SERVER="http://localhost:6363"
export DB="admin/MyDatabase"
```

## Step 1 — Create a database with initial data

Create the database:

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Recovery tutorial"}
{% /http-example %}

Insert an initial document:

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=admin&message=Add+initial+product+data&raw_json=true" %}
{"@id": "terminusdb:///data/product-001", "name": "Widget", "price": 9.99, "status": "active"}
{% /http-example %}

## Step 2 — Make a second commit (the "good" state)

Update the product price — this creates a second commit that we will later identify as "last known good":

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=admin&message=Update+widget+price+to+12.50&raw_json=true" %}
{"@id": "terminusdb:///data/product-001", "name": "Widget", "price": 12.50, "status": "active"}
{% /http-example %}

## Step 3 — Make a bad change (simulate data corruption)

Delete the product entirely — this is the change we want to recover from:

{% http-example method="DELETE" path="/api/document/admin/MyDatabase?author=admin&message=Accidentally+deleted+product&id=terminusdb:///data/product-001" /%}

Verify it is gone:

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=terminusdb:///data/product-001&raw_json=true" /%}

You should get an empty response or an error — the document no longer exists on `main`.

## Step 4 — View the commit log

Use the `/api/log/{path}` endpoint to list recent commits:

{% http-example method="GET" path="/api/log/admin/MyDatabase?count=10" /%}

**Expected output:**

```json
[
  {
    "@id": "ValidCommit/<sha-of-delete-commit>",
    "@type": "ValidCommit",
    "author": "admin",
    "identifier": "<sha-of-delete-commit>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Accidentally deleted product",
    "parent": "ValidCommit/<sha-of-good-commit>",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714400000.0
  },
  {
    "@id": "ValidCommit/<sha-of-good-commit>",
    "@type": "ValidCommit",
    "author": "admin",
    "identifier": "<sha-of-good-commit>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Update widget price to 12.50",
    "parent": "ValidCommit/<sha-of-first-commit>",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714399900.0
  },
  {
    "@id": "ValidCommit/<sha-of-first-commit>",
    "@type": "InitialCommit",
    "author": "admin",
    "identifier": "<sha-of-first-commit>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Add initial product data",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714399800.0
  }
]
```

The response includes additional internal fields: `@id` (commit IRI), `instance` and `schema` (storage layer references), and `parent` (link to previous commit, absent on the initial commit). For recovery purposes, focus on `identifier`, `message`, and `timestamp`.

Identify the commit you want to return to. In this case it is the second commit ("Update widget price to 12.50"). Copy its `identifier` value.

{% callout type="note" title="Reading timestamps" %}
Timestamps are Unix epoch seconds. Convert to human-readable with: `date -d @1714399900` (Linux) or `date -r 1714399900` (macOS).
{% /callout %}

## Step 5 — Create a branch from the good commit (verify before reset)

Before resetting `main`, create a branch from the good commit to inspect the data safely:

```bash
curl -s -u $AUTH -X POST \
  "$SERVER/api/branch/$DB/local/branch/recovery-check" \
  -H "Content-Type: application/json" \
  -d '{"origin": "admin/MyDatabase/local/commit/<sha-of-good-commit>"}'
```

Replace `<sha-of-good-commit>` with the actual identifier from your log output.

## Step 6 — Verify the data on the recovery branch

Query the recovery branch to confirm it contains the expected data:

```bash
curl -s -u $AUTH \
  "$SERVER/api/document/$DB/local/branch/recovery-check?id=terminusdb:///data/product-001&raw_json=true" | jq
```

**Expected:** The document is present with `"price": 12.50`.

## Step 7 — Reset main to the good commit

Now that we have confirmed the data is correct, reset `main` to the good commit:

```bash
curl -s -u $AUTH -X POST "$SERVER/api/reset/$DB" \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/MyDatabase/local/commit/<sha-of-good-commit>"}'
```

**Expected response:**
```json
{"@type": "api:ResetResponse", "api:status": "api:success"}
```

## Step 8 — Confirm the recovery

Verify that `main` now has the document restored:

```bash
curl -s -u $AUTH "$SERVER/api/document/$DB?id=terminusdb:///data/product-001&raw_json=true" | jq
```

**Expected:** The document is present with `"price": 12.50` and `"status": "active"`.

## Cleanup

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}

## What you learned

- **Every write creates an immutable commit** — data is never lost, only HEAD moves forward
- **The commit log is the audit trail** — `author`, `message`, and `timestamp` on every commit
- **Reset moves the branch HEAD backward** — it does not delete commits from history
- **Branch from a commit to inspect safely** — verify before resetting production data

## Next steps

- [How to Set Commit Messages](/docs/commit-message-howto/) — make the log useful for recovery
- [Audit Data Changes](/docs/audit-tutorial/) — use the commit log for compliance auditing
- [Diff and Patch Operations](/docs/diff-and-patch-operations/) — compare specific commits

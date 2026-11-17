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
      canonical: https://terminusdb.com/docs/recovery-tutorial/
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

## Step 1 — Create a database with initial data {% step %}

Create the database:

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Recovery tutorial"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:database_uri":"*","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

Insert an initial document:

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=admin&message=Add+initial+product+data&raw_json=true" %}
{"@id": "terminusdb:///data/product-001", "name": "Widget", "price": 9.99, "status": "active"}
{% http-expected %}
["product-001" ]
{% /http-expected %}
{% /http-example %}

## Step 2 — Make a second commit (the "good" state) {% step %}

Update the product price — this creates a second commit that we will later identify as "last known good":

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=admin&message=Update+widget+price+to+12.50&raw_json=true" %}
{"@id": "terminusdb:///data/product-001", "name": "Widget", "price": 12.50, "status": "active"}
{% http-expected %}
["product-001" ]
{% /http-expected %}
{% /http-example %}

## Step 3 — Make a bad change (simulate data corruption) {% step %}

Delete the product entirely — this is the change we want to recover from:

{% http-example method="DELETE" path="/api/document/admin/MyDatabase?author=admin&message=Accidentally+deleted+product&id=terminusdb:///data/product-001" /%}
{% http-expected %}
[]
{% /http-expected %}

Verify it is gone:

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=terminusdb:///data/product-001&raw_json=true" /%}
{% http-expected %}
{"@type":"api:GetDocumentErrorResponse","api:error":{"@type":"api:DocumentNotFound","api:document_id":"terminusdb:///data/product-001"},"api:message":"Document not found: 'terminusdb:///data/product-001'","api:status":"api:not_found"}
{% /http-expected %}

You should get a document-not-found error — the document no longer exists on `main`.

## Step 4 — View the commit log {% step %}

Use the `/api/log/{path}` endpoint to list recent commits:

{% http-example method="GET" path="/api/log/admin/MyDatabase?count=10" /%}
{% http-expected %}
[
  {"@id":"ValidCommit/3yofxp42zi0lax2ikwt76bfgj5oec59","@type":"ValidCommit","author":"admin","identifier":"3yofxp42zi0lax2ikwt76bfgj5oec59","message":"Accidentally deleted product","parent":"ValidCommit/416tubx6qsch8nsdvuxb9jckas1citd","timestamp":1793753717.140962,"user":"terminusdb://system/data/User/admin"},
  {"@id":"ValidCommit/416tubx6qsch8nsdvuxb9jckas1citd","@type":"ValidCommit","author":"admin","identifier":"416tubx6qsch8nsdvuxb9jckas1citd","message":"Update widget price to 12.50","parent":"ValidCommit/y28hkemt7bvulv5iq773wv4kexl68g9","timestamp":1793753717.0989,"user":"terminusdb://system/data/User/admin"},
  {"@id":"ValidCommit/y28hkemt7bvulv5iq773wv4kexl68g9","@type":"InitialCommit","author":"admin","identifier":"y28hkemt7bvulv5iq773wv4kexl68g9","message":"Add initial product data","timestamp":1793753717.050699,"user":"terminusdb://system/data/User/admin"}
]
{% /http-expected %}

The response includes additional internal fields: `@id` (commit IRI), `instance` and `schema` (storage layer references), and `parent` (link to previous commit, absent on the initial commit). For recovery purposes, focus on `identifier`, `message`, and `timestamp`.

Identify the commit you want to return to. In this case it is the second commit ("Update widget price to 12.50") — the last one *before* "Accidentally deleted product".

{% callout type="note" title="Reading timestamps" %}
Timestamps are Unix epoch seconds. Convert to human-readable with: `date -d @1714399900` (Linux) or `date -r 1714399900` (macOS).
{% /callout %}

## Step 5 — Create a branch from the good commit (verify before reset) {% step %}

Before resetting `main`, create a branch from the good commit to inspect the data safely. First fetch the identifier of the commit just before the bad one — `start=1` skips the newest (the delete) entry:

```bash publishes="good_commit" publishColumn="identifier" publishLabel="message"
curl -s -u admin:root \
  "http://localhost:6363/api/log/admin/MyDatabase?count=1&start=1"
```
{% http-expected %}
[{"@id":"ValidCommit/416tubx6qsch8nsdvuxb9jckas1citd","@type":"ValidCommit","author":"admin","identifier":"416tubx6qsch8nsdvuxb9jckas1citd","message":"Update widget price to 12.50","timestamp":1793753717.0989,"user":"terminusdb://system/data/User/admin"}]
{% /http-expected %}

Then create the branch from that commit — the identifier flows in automatically:

```bash slot="good_commit" placeholder="GOOD_COMMIT"
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/branch/admin/MyDatabase/local/branch/recovery-check" \
  -H "Content-Type: application/json" \
  -d '{"origin": "admin/MyDatabase/local/commit/GOOD_COMMIT"}'
```
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}

## Step 6 — Verify the data on the recovery branch {% step %}

Query the recovery branch to confirm it contains the expected data:

{% http-example method="GET" path="/api/document/admin/MyDatabase/local/branch/recovery-check?id=terminusdb:///data/product-001&raw_json=true" runnable="false" /%}
{% http-expected %}
{"@id":"product-001","name":"Widget","price":12.5,"status":"active"}
{% /http-expected %}

The document is present with `"price": 12.50` — the state before the accidental delete.

## Step 7 — Reset main to the good commit {% step %}

Now that we have confirmed the data is correct, reset `main` to the same good commit:

```bash slot="good_commit" placeholder="GOOD_COMMIT"
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/reset/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{"commit_descriptor": "admin/MyDatabase/local/commit/GOOD_COMMIT"}'
```
{% http-expected %}
{"@type":"api:ResetResponse","api:status":"api:success"}
{% /http-expected %}

Reset moves the branch HEAD back to the chosen commit — it does not delete the bad commit, history stays intact.

## Step 8 — Confirm the recovery {% step %}

Verify that `main` now has the document restored:

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=terminusdb:///data/product-001&raw_json=true" runnable="false" /%}
{% http-expected %}
{"@id":"product-001","name":"Widget","price":12.5,"status":"active"}
{% /http-expected %}

The document is present with `"price": 12.50` and `"status": "active"` — recovery complete.

## Cleanup {% step %}

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## What you learned

- **Every write creates an immutable commit** — data is never lost, only HEAD moves forward
- **The commit log is the audit trail** — `author`, `message`, and `timestamp` on every commit
- **Reset moves the branch HEAD backward** — it does not delete commits from history
- **Branch from a commit to inspect safely** — verify before resetting production data

## Next steps

- [How to Set Commit Messages](/docs/commit-message-howto/) — make the log useful for recovery
- [Audit Data Changes](/docs/audit-tutorial/) — use the commit log for compliance auditing
- [Diff and Patch Operations](/docs/diff-and-patch-operations/) — compare specific commits

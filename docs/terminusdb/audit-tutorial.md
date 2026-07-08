---
tags:
  - version-control
  - tutorial
  - curl
title: Audit Data Changes
nextjs:
  metadata:
    title: Audit Data Changes — TerminusDB Tutorial
    description: Step-by-step tutorial to audit who changed what and when using TerminusDB's immutable commit log. Query history, diff commits, and trace field-level changes.
    keywords: audit, commit log, history, diff, compliance, traceability, author
    alternates:
      canonical: https://terminusdb.org/docs/audit-tutorial/
---

Every write in TerminusDB creates an immutable commit that records who made the change, when, and why. This makes the commit log a tamper-evident audit trail — useful for compliance, debugging, and accountability.

**Time:** ~10 minutes
**Prerequisites:** TerminusDB running on `localhost:6363` ([Install guide →](/docs/install-terminusdb-as-a-docker-container))

## Setup

```bash
export AUTH="admin:root"
export SERVER="http://localhost:6363"
export DB="admin/MyDatabase"
```

## Step 1 — Create a database

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Audit tutorial"}
{% /http-example %}

## Step 2 — Insert data with meaningful commit metadata

The `author` and `message` query parameters on the Document API are recorded in the commit log. They are the foundation of your audit trail.

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=jane.ops@example.com&message=Onboard+new+customer+ACME+Corp&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "standard", "credit_limit": 50000}
{% /http-example %}

Use the operator's real identity as `author` and a human-readable description as `message`. These become your audit record — "who did what and why" for every change.

{% callout type="note" title="author vs HTTP auth" %}
The HTTP Basic Auth credentials (`admin:root`) authorise the request. The `author` query parameter records who is *logically responsible* for the change. In production, pass the end user's identity as `author`.
{% /callout %}

## Step 3 — Make a second change (different author)

Another team member updates the customer's credit limit:

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=bob.finance@example.com&message=Increase+ACME+credit+limit+after+Q1+review&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "standard", "credit_limit": 100000}
{% /http-example %}

## Step 4 — Make a third change (tier upgrade)

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=jane.ops@example.com&message=Upgrade+ACME+to+premium+tier&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "premium", "credit_limit": 100000}
{% /http-example %}

## Step 5 — Query the commit log

Use the `/api/log/{path}` endpoint to see the full branch history:

{% http-example method="GET" path="/api/log/admin/MyDatabase?count=10" /%}

**Expected output:**

```json
[
  {
    "@id": "ValidCommit/<sha-3>",
    "@type": "ValidCommit",
    "author": "jane.ops@example.com",
    "identifier": "<sha-3>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Upgrade ACME to premium tier",
    "parent": "ValidCommit/<sha-2>",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714400200.0
  },
  {
    "@id": "ValidCommit/<sha-2>",
    "@type": "ValidCommit",
    "author": "bob.finance@example.com",
    "identifier": "<sha-2>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Increase ACME credit limit after Q1 review",
    "parent": "ValidCommit/<sha-1>",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714400100.0
  },
  {
    "@id": "ValidCommit/<sha-1>",
    "@type": "InitialCommit",
    "author": "jane.ops@example.com",
    "identifier": "<sha-1>",
    "instance": "layer_data:Layer_<hash>",
    "message": "Onboard new customer ACME Corp",
    "schema": "layer_data:Layer_<hash>",
    "timestamp": 1714400000.0
  }
]
```

The log tells you who (`author`), when (`timestamp`), and why (`message`) for every change. This is your complete audit trail.

Additional fields provide internal references: `@id` is the commit's full IRI, `instance` and `schema` point to storage layers, and `parent` links to the previous commit (absent on the initial commit which has `@type: "InitialCommit"`).

{% callout type="note" title="Enterprise edition" %}
The audit trail query performance shown here scales well for moderate data volumes. The TerminusDB Enterprise Edition includes vastly improved audit trail performance for high-volume production workloads — query millions of commits with sub-second response times.
{% /callout %}

{% callout type="note" title="Reading timestamps" %}
Timestamps are Unix epoch seconds. Convert to human-readable with: `date -d @1714400200` (Linux) or `date -r 1714400200` (macOS).
{% /callout %}

## Step 6 — Get document-level history

The `/api/log` endpoint shows all commits on a branch. For a specific document, use `/api/history/{path}` to see only commits where that document was modified:

{% http-example method="GET" path="/api/history/admin/MyDatabase?id=customer-acme" /%}

**Expected output:**

```json
[
  {
    "author": "jane.ops@example.com",
    "identifier": "<sha-3>",
    "message": "Upgrade ACME to premium tier",
    "timestamp": 1714400200.0,
    "user": "terminusdb://system/data/User/admin"
  },
  {
    "author": "bob.finance@example.com",
    "identifier": "<sha-2>",
    "message": "Increase ACME credit limit after Q1 review",
    "timestamp": 1714400100.0,
    "user": "terminusdb://system/data/User/admin"
  },
  {
    "author": "jane.ops@example.com",
    "identifier": "<sha-1>",
    "message": "Onboard new customer ACME Corp",
    "timestamp": 1714400000.0,
    "user": "terminusdb://system/data/User/admin"
  }
]
```

This shows only commits where `customer-acme` was modified — filtering out unrelated changes to other documents. The `user` field shows the authenticated system user who performed the HTTP request (distinct from `author`, which records logical responsibility).

## Step 7 — Diff two commits to see exactly what changed

Use the `/api/diff` endpoint to see the structural difference between any two points in history. Compare commit 1 (insert) and commit 2 (credit limit increase):

```bash
curl -s -u $AUTH -X POST "$SERVER/api/diff/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "<sha-1>",
    "after_data_version": "<sha-2>",
    "document_id": "terminusdb:///data/customer-acme"
  }' | jq
```

**Expected output:**

```json
{
  "credit_limit": {
    "@op": "SwapValue",
    "@before": 50000,
    "@after": 100000
  }
}
```

This tells you the exact field that changed (`credit_limit`), what it was before (`50000`), and what it became (`100000`). The diff is structural — not a line-by-line text comparison.

## Step 8 — Diff the tier upgrade

Compare commit 2 and commit 3 to see the tier change:

```bash
curl -s -u $AUTH -X POST "$SERVER/api/diff/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "<sha-2>",
    "after_data_version": "<sha-3>",
    "document_id": "terminusdb:///data/customer-acme"
  }' | jq
```

**Expected output:**

```json
{
  "tier": {
    "@op": "SwapValue",
    "@before": "standard",
    "@after": "premium"
  }
}
```

## Cleanup

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}

## What you learned

- **The audit trail is automatic** — every write creates an immutable commit with author, timestamp, and message
- **Author is application-controlled** — pass the real user's identity in the `author` parameter; TerminusDB records it faithfully
- **Messages should explain intent** — "Increase ACME credit limit after Q1 review" is far more useful than "updated document" when auditing six months later
- **Diffs are structural** — TerminusDB computes semantic differences (field-level changes with before/after values), not text diffs
- **History is immutable** — commits cannot be edited or deleted; the log is a tamper-evident record

## Next steps

- [Set Commit Messages](/docs/commit-message-howto/) — quick reference for the `author` and `message` parameters
- [Recover Data from Version History](/docs/recovery-tutorial/) — using the log to identify and roll back to a good state
- [Diff and Patch Operations](/docs/diff-and-patch-operations/) — advanced diff usage with the JS client

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
      canonical: https://terminusdb.com/docs/audit-tutorial/
---

Every write in TerminusDB creates an immutable commit that records who made the change, when, and why. This makes the commit log a tamper-evident audit trail — useful for compliance, debugging, and accountability.

*The history API was added in [TerminusDB 11.0.4](/docs/releases/11-0-4-document-history-api/), and gained diffs and streaming in [12.0.5](/docs/releases/12-0-5-streaming-history-diffs/).*

**Time:** ~10 minutes
**Prerequisites:** TerminusDB running on `localhost:6363` ([Install guide →](/docs/install-terminusdb-as-a-docker-container))

## Setup

```bash
export AUTH="admin:root"
export SERVER="http://localhost:6363"
export DB="admin/MyDatabase"
```

## Step 1 — Create a database {% step %}

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Audit tutorial"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:database_uri":"*","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## Step 2 — Insert data with meaningful commit metadata {% step %}

The `author` and `message` query parameters on the Document API are recorded in the commit log. They are the foundation of your audit trail.

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=jane.ops@example.com&message=Onboard+new+customer+ACME+Corp&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "standard", "credit_limit": 50000}
{% http-expected %}
["customer-acme" ]
{% /http-expected %}
{% /http-example %}

Use the operator's real identity as `author` and a human-readable description as `message`. These become your audit record — "who did what and why" for every change.

{% callout type="note" title="author vs HTTP auth" %}
The HTTP Basic Auth credentials (`admin:root`) authorise the request. The `author` query parameter records who is *logically responsible* for the change. In production, pass the end user's identity as `author`.
{% /callout %}

## Step 3 — Make a second change (different author) {% step %}

Another team member updates the customer's credit limit:

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=bob.finance@example.com&message=Increase+ACME+credit+limit+after+Q1+review&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "standard", "credit_limit": 100000}
{% http-expected %}
["customer-acme" ]
{% /http-expected %}
{% /http-example %}

## Step 4 — Make a third change (tier upgrade) {% step %}

{% http-example method="PUT" path="/api/document/admin/MyDatabase?author=jane.ops@example.com&message=Upgrade+ACME+to+premium+tier&raw_json=true" %}
{"@id": "terminusdb:///data/customer-acme", "name": "ACME Corp", "tier": "premium", "credit_limit": 100000}
{% http-expected %}
["customer-acme" ]
{% /http-expected %}
{% /http-example %}

## Step 5 — Query the commit log {% step %}

Use the `/api/log/{path}` endpoint to see the full branch history:

{% http-example method="GET" path="/api/log/admin/MyDatabase?count=10" /%}
{% http-expected %}
[
  {"@id":"ValidCommit/betivnc84t5z7ox6zihpus95j9wlfuq","@type":"ValidCommit","author":"jane.ops@example.com","identifier":"betivnc84t5z7ox6zihpus95j9wlfuq","message":"Upgrade ACME to premium tier","parent":"ValidCommit/gdt49i663sobmo3xsra4pmw63794vzf","timestamp":1793753655.912056,"user":"terminusdb://system/data/User/admin"},
  {"@id":"ValidCommit/gdt49i663sobmo3xsra4pmw63794vzf","@type":"ValidCommit","author":"bob.finance@example.com","identifier":"gdt49i663sobmo3xsra4pmw63794vzf","message":"Increase ACME credit limit after Q1 review","parent":"ValidCommit/ra929jaqk3w03uwwh67z6obgajnvyme","timestamp":1793753655.864094,"user":"terminusdb://system/data/User/admin"},
  {"@id":"ValidCommit/ra929jaqk3w03uwwh67z6obgajnvyme","@type":"InitialCommit","author":"jane.ops@example.com","identifier":"ra929jaqk3w03uwwh67z6obgajnvyme","message":"Onboard new customer ACME Corp","timestamp":1793753655.817107,"user":"terminusdb://system/data/User/admin"}
]
{% /http-expected %}

The log tells you who (`author`), when (`timestamp`), and why (`message`) for every change. This is your complete audit trail.

Additional fields provide internal references: `@id` is the commit's full IRI, `instance` and `schema` point to storage layers, and `parent` links to the previous commit (absent on the initial commit which has `@type: "InitialCommit"`).

{% callout type="note" title="Enterprise edition" %}
The audit trail query performance shown here scales well for moderate data volumes. The TerminusDB Enterprise Edition includes vastly improved audit trail performance for high-volume production workloads — query millions of commits with sub-second response times.
{% /callout %}

{% callout type="note" title="Reading timestamps" %}
Timestamps are Unix epoch seconds. Convert to human-readable with: `date -d @1714400200` (Linux) or `date -r 1714400200` (macOS).
{% /callout %}

## Step 6 — Get document-level history {% step %}

The `/api/log` endpoint shows all commits on a branch. For a specific document, use `/api/history/{path}` to see only commits where that document was modified:

{% http-example method="GET" path="/api/history/admin/MyDatabase?id=customer-acme" /%}
{% http-expected %}
[
  {"author":"jane.ops@example.com","identifier":"betivnc84t5z7ox6zihpus95j9wlfuq","message":"Upgrade ACME to premium tier","timestamp":1793753655.912056,"user":"terminusdb://system/data/User/admin"},
  {"author":"bob.finance@example.com","identifier":"gdt49i663sobmo3xsra4pmw63794vzf","message":"Increase ACME credit limit after Q1 review","timestamp":1793753655.864094,"user":"terminusdb://system/data/User/admin"},
  {"author":"jane.ops@example.com","identifier":"ra929jaqk3w03uwwh67z6obgajnvyme","message":"Onboard new customer ACME Corp","timestamp":1793753655.817107,"user":"terminusdb://system/data/User/admin"}
]
{% /http-expected %}

This shows only commits where `customer-acme` was modified — filtering out unrelated changes to other documents. The `user` field shows the authenticated system user who performed the HTTP request (distinct from `author`, which records logical responsibility).

## Step 7 — Diff against the initial commit {% step %}

Use the `/api/diff` endpoint to see the structural difference between any two points in history. First fetch the identifier of the *oldest* commit — `start=2` skips the two newest entries in the log:

```bash publishes="commit_identifiers" publishColumn="identifier" publishLabel="message"
curl -s -u admin:root \
  "http://localhost:6363/api/log/admin/MyDatabase?count=1&start=2"
```
{% http-expected %}
[{"@id":"ValidCommit/ra929jaqk3w03uwwh67z6obgajnvyme","@type":"InitialCommit","author":"jane.ops@example.com","identifier":"ra929jaqk3w03uwwh67z6obgajnvyme","message":"Onboard new customer ACME Corp","timestamp":1793753655.817107,"user":"terminusdb://system/data/User/admin"}]
{% /http-expected %}

Then diff that point against the current state of `main` — the identifier flows in automatically:

```bash slot="commit_identifiers" placeholder="BEFORE_VERSION"
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/diff/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "BEFORE_VERSION",
    "after_data_version": "main",
    "document_id": "terminusdb:///data/customer-acme"
  }'
```
{% http-expected %}
{"@id":"terminusdb:///data/customer-acme","credit_limit":{"@after":100000,"@before":50000,"@op":"SwapValue"},"tier":{"@after":"premium","@before":"standard","@op":"SwapValue"}}
{% /http-expected %}

The diff is structural — not a line-by-line text comparison. Since onboarding you see both field changes: `credit_limit` (`50000` → `100000`) and `tier` (`standard` → `premium`). To learn more about graph diffs, and how to apply a diff as a patch, see [Knowledge Graph Diff](/docs/knowledge-graph-diff/).

## Step 8 — Diff a single change {% step %}

To isolate one change, anchor the diff at the commit just before the latest — `start=1` fetches the second-newest entry ("Increase ACME credit limit"):

```bash publishes="commit_identifiers" publishColumn="identifier" publishLabel="message"
curl -s -u admin:root \
  "http://localhost:6363/api/log/admin/MyDatabase?count=1&start=1"
```
{% http-expected %}
[{"@id":"ValidCommit/gdt49i663sobmo3xsra4pmw63794vzf","@type":"ValidCommit","author":"bob.finance@example.com","identifier":"gdt49i663sobmo3xsra4pmw63794vzf","message":"Increase ACME credit limit after Q1 review","timestamp":1793753655.864094,"user":"terminusdb://system/data/User/admin"}]
{% /http-expected %}

```bash slot="commit_identifiers" placeholder="BEFORE_VERSION"
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/diff/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{
    "before_data_version": "BEFORE_VERSION",
    "after_data_version": "main",
    "document_id": "terminusdb:///data/customer-acme"
  }'
```
{% http-expected %}
{"@id":"terminusdb:///data/customer-acme","tier":{"@after":"premium","@before":"standard","@op":"SwapValue"}}
{% /http-expected %}

Now only the tier upgrade appears — everything before the anchor commit is excluded.

{% callout type="note" title="Commit-to-commit diffs" %}
`after_data_version` accepts commit identifiers too — pass two identifiers from the log to diff any pair of commits, or mix commits and branch names freely.
{% /callout %}

## Cleanup {% step %}

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

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

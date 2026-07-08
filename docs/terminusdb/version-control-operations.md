---
tags:
  - version-control
  - reference
title: "Version Control Operations — Git for Data"
nextjs:
  metadata:
    title: "Version Control Operations — Git for Data in TerminusDB"
    description: "TerminusDB is a git for data database. Complete reference for git-based version control operations — branch, merge, diff, time-travel, reset, and squash with HTTP API examples and expected responses."
    keywords: git for data, git-based database, git for data database, version control database, immutable database, branch merge diff, time travel database, database branching, data version control
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/version-control-operations/
media: []
lastUpdated: "2026-05-01"
---

TerminusDB brings **git-style version control to your data** — branch, merge, diff, and time-travel just like you would with code. Every write creates an immutable commit. This page is the single reference for all version control operations.

{% callout type="note" title="Git for Data" %}
TerminusDB is a **git for data** database. Every write creates an immutable commit. You can branch, merge, diff, and time-travel your data just like code in Git.
{% /callout %}

{% prerequisites-clone /%}

## TerminusDB as Git for Data

If you know Git, you already know how TerminusDB version control works. The concepts map directly:

| Git concept | TerminusDB equivalent |
|-------------|----------------------|
| `git branch` | Create a branch — isolated workspace for changes |
| `git commit` | Every write is a commit — with author and message |
| `git merge` | Merge branches — apply changes with conflict detection |
| `git diff` | Diff — structural field-level comparison of any two states |
| `git checkout <commit>` | Time-travel — query the database at any historical commit |
| `git reset` | Reset — move a branch pointer back to a previous commit |
| `git clone` | Clone — replicate a database between instances |

The difference: Git operates on text files with line-based diffs. TerminusDB operates on structured JSON documents with **field-level semantic diffs** — it knows that a price changed from 9.99 to 14.99, not just that "line 3 was modified".

---

{% callout type="note" %}
**Prerequisites**
- **TerminusDB running** on `localhost:6363` or use the public server at `data.terminusdb.org`.
- A database to work with. The examples below use `admin/tdb-example-mydb`. Create one with: `curl -u admin:root -X POST http://localhost:6363/api/db/admin/tdb-example-mydb -d '{"label":"My Database"}'`
{% /callout %}

## Create a branch

Create a new branch from an existing branch. Branches are cheap — they share history until they diverge.

{% http-example method="POST" path="/api/branch/admin/tdb-example-mydb/local/branch/feature" %}
{"origin": "admin/tdb-example-mydb/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## List branches

List all branches in a database.

{% http-example method="GET" path="/api/document/admin/tdb-example-mydb/_meta?type=Branch&as_list=true" runnable=false /%}

Expected response: an array of Branch documents, each with `name` and `head` (commit ID).

## Switch branch (query a branch)

There is no "checkout" — you simply query or write to the branch path you want:

- **main:** `/api/document/admin/tdb-example-mydb/local/branch/main`
- **feature:** `/api/document/admin/tdb-example-mydb/local/branch/feature`

Every API path that includes `/local/branch/{name}` targets that branch.

## Commit (write with message)

Every write is a commit. Add `author` and `message` query parameters to record who changed what and why:

{% http-example method="POST" path="/api/document/admin/tdb-example-mydb/local/branch/main?author=alice@example.com&message=Add+initial+product+data" runnable=false %}
{"@type": "Product", "name": "Widget", "price": 9.99}
{% http-expected %}
["terminusdb:///data/Product/Widget"]
{% /http-expected %}
{% /http-example %}

## Get commit history

View the commit log for a branch:

{% http-example method="GET" path="/api/log/admin/tdb-example-mydb/local/branch/main?count=5" runnable=false /%}

Expected response: an array of commit objects with `author`, `message`, `timestamp`, and `identifier` (commit ID).

## Diff (compare branches or commits)

Compare two branches to see exactly what changed — field-level structural diff, not line diff:

{% http-example method="POST" path="/api/diff/admin/tdb-example-mydb" runnable=false %}
{"before_data_version": "main", "after_data_version": "feature"}
{% http-expected %}
[{"@id": "terminusdb:///data/Product/Widget", "price": {"@op": "SwapValue", "@before": 9.99, "@after": 12.50}}]
{% /http-expected %}
{% /http-example %}

The diff shows typed operations: `SwapValue` (field changed), `Insert` (added), `Delete` (removed). No other fields changed.

## Merge a branch

Apply changes from one branch onto another:

{% http-example method="POST" path="/api/apply/admin/tdb-example-mydb/local/branch/main" %}
{"before_commit": "main", "after_commit": "feature", "commit_info": {"author": "alice@example.com", "message": "Merge feature into main"}}
{% http-expected %}
{"@type":"api:ApplyResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

If there are conflicts (both branches modified the same field), the merge fails with a conflict report — no silent overwrites.

## Squash commits

Collapse all commits on a branch into a single commit — useful for cleaning up history before merging:

{% http-example method="POST" path="/api/squash/admin/tdb-example-mydb/local/branch/feature" %}
{"commit_info": {"author": "alice@example.com", "message": "Squash feature branch into single commit"}}
{% http-expected %}
{"@type":"api:SquashResponse","api:status":"api:success","api:commit":"*"}
{% /http-expected %}
{% /http-example %}

## Time-travel (query at a previous commit)

Query the database as it was at any previous point in time. Use a commit path instead of a branch path:

{% http-example method="GET" path="/api/document/admin/tdb-example-mydb/local/commit/abc123def456?type=Product&as_list=true" runnable=false /%}

Replace `abc123def456` with an actual commit identifier from the log. The response shows the database state at that exact commit — you did not modify anything, you are simply looking at a snapshot.

## Reset a branch

Move a branch pointer back to a previous commit, discarding subsequent commits:

{% http-example method="POST" path="/api/reset/admin/tdb-example-mydb/local/branch/feature" runnable=false %}
{"commit_descriptor": "admin/tdb-example-mydb/local/branch/feature/commit/abc123def456"}
{% http-expected %}
{"@type":"api:ResetResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

{% callout type="warning" %}
Reset is destructive — commits after the target become unreachable (though the immutable commit graph still stores them). Use with care.
{% /callout %}

## Delete a branch

Remove a branch. The commits remain in the database graph but this branch name no longer reaches them:

{% http-example method="DELETE" path="/api/branch/admin/tdb-example-mydb/local/branch/feature" %}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## Patch (apply a diff)

Apply a previously obtained diff to a document. Useful for conflict resolution or programmatic updates:

{% http-example method="POST" path="/api/patch/admin/tdb-example-mydb/local/branch/main" runnable=false %}
{"before": {"@id": "terminusdb:///data/Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99}, "patch": {"price": {"@op": "SwapValue", "@before": 9.99, "@after": 14.99}}}
{% http-expected %}
{"@type":"api:PatchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

---

## Summary

| Operation | Method | Path pattern |
|-----------|--------|--------------|
| Create branch | POST | `/api/branch/{org}/{db}/local/branch/{name}` |
| Delete branch | DELETE | `/api/branch/{org}/{db}/local/branch/{name}` |
| Merge (apply) | POST | `/api/apply/{org}/{db}/local/branch/{target}` |
| Squash | POST | `/api/squash/{org}/{db}/local/branch/{name}` |
| Diff | POST | `/api/diff/{org}/{db}` |
| Patch | POST | `/api/patch/{org}/{db}/local/branch/{name}` |
| Reset | POST | `/api/reset/{org}/{db}/local/branch/{name}` |
| Commit log | GET | `/api/log/{org}/{db}/local/branch/{name}` |
| Time-travel | GET | `/api/document/{org}/{db}/local/commit/{id}` |

Every operation is an HTTP call. Every write is a commit. Every commit is immutable. This is git for data.

## Next steps

You've seen the full version control API. To go deeper:

- [**Diff and patch with the JS Client**](/docs/diff-and-patch-operations/) — compute and apply diffs programmatically in TypeScript
- [**Clone a project**](/docs/clone-a-project/) — replicate databases between TerminusDB instances
- [**Set commit messages**](/docs/commit-message-howto/) — write effective audit-trail messages on every write

---
tags:
  - explanation
  - version-control
  - knowledge-graph
  - curl
title: A Graph Database with Time Travel Queries
nextjs:
  metadata:
    title: Graph Database with Time Travel Queries — TerminusDB
    description: TerminusDB keeps every write as an immutable commit, so you can query your graph as it was at any commit or branch. No backups or log replay.
    keywords: graph database with time travel queries, time travel queries, time travel database, point-in-time query, query historical data, temporal graph database, graph database history, immutable database, versioned graph database, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/graph-database-with-time-travel-queries/
media: []
---

In TerminusDB, every write is an immutable commit. So you can query the database as it was at any commit or on any branch, with the same API you use for live data.

That is what we mean by "time travel queries". You do not need backups, log replay or extra history tables. The past is just another path you can read.

## How commits make it work

TerminusDB does not change data in place. Each write adds a new layer on top of the old ones:

- **Immutable layers.** A commit adds a layer with the triples it added and removed. Older layers stay as they were. See [Immutability](/docs/immutability-explanation/) for more detail.
- **Commit IDs.** Each commit has an `identifier`, plus an author, a timestamp and a message. You get these from the commit log.
- **Branches.** A branch is a name that points to the newest commit in a line of history. Reading a branch gives you the current state. Reading a commit gives you the state at that moment.

Because old layers never change, a read at an old commit is a normal read. TerminusDB does not rebuild the past from a log.

## A short example

These calls use a database `admin/tdb-example-mydb` on `localhost:6363`. First, get the recent commits on `main`:

```bash
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=5"
```

Each entry in the response has an `identifier`. Then, to read the data as it was at one of those commits, replace `local/branch/main` with `local/commit/<identifier>` in the path:

```bash
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?type=Product&as_list=true"
```

You get the `Product` documents exactly as they were at that commit. This is read-only, so the current branch does not change. The [time travel how-to](/docs/time-travel-howto/) shows the same steps in TypeScript and Python, and how to diff and restore.

## When it helps

Some common reasons to query the past:

- **Debugging.** See what the data looked like just before a bad import or a bug.
- **Audits.** Show what a record said on a given day, and who changed it.
- **Reports you can repeat.** Run a report against a fixed commit, so the numbers do not move.
- **Machine learning.** Train or test on the exact data snapshot you used before.
- **Safe restore.** Read an old version of a document and write it back as a new commit.

## How it compares

We try to keep this fair. Other tools solve parts of this problem well.

| | TerminusDB | SQL temporal tables | Neo4j |
|---|---|---|---|
| Where history lives | The whole database, per commit | Per table that you turn it on for | Not built in |
| How you ask for the past | Use a commit or branch in the path | `FOR SYSTEM_TIME AS OF` on each table | Model it yourself, for example with timestamps on nodes |
| Branches | Yes | No | No |
| Who changed what | Author and message on each commit | Not by default | Not by default |

- **SQL temporal tables** (system-versioned tables) are good at "what did this row look like at time T". You turn them on per table, and each table keeps its own history. TerminusDB versions the whole graph at once, so one commit is one consistent state of all your data.
- **Neo4j** is a strong graph database for traversal. As far as we know, it does not keep a version history of the graph out of the box. Teams often add timestamps or history nodes in their own model.

## Limits

Time travel is not free, and it is good to know the costs:

- **History uses storage.** Old layers are kept, so storage grows as you commit more changes.
- **You can shorten history.** If you do not need every step on a branch, you can [squash](/docs/undo-reset-howto/) its commits into one. Then the steps in between are no longer there to query on that branch.
- **It is not a time-series database.** If you need high-rate metrics, a time-series tool is a better fit.

Read speed is not one of the costs. Changes are stored as delta layers, and [delta rollup](/docs/delta-rollup/) runs automatically in standard setups to compact them. Together with the layer cache, this keeps reads at near-constant time as history grows. The effect of deep history on read speed is negligible.

## See also

- [How to Time-Travel](/docs/time-travel-howto/) — query, compare and restore past states, step by step
- [Time Travel in the Dashboard](/docs/time-travel/) — do the same thing in TerminusDB Cloud
- [Immutability](/docs/immutability-explanation/) — why TerminusDB never changes data in place
- [A Graph Database with Git-like Version Control](/docs/graph-database-with-git-like-version-control/) — commits, branches, diffs and merges
- [A Knowledge Graph for AI Agents](/docs/knowledge-graph-for-ai-agents/) — use commit history as temporal memory for agents

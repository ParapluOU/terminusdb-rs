---
tags:
  - explanation
  - documents
  - intermediate
title: Immutable Database Explained
nextjs:
  metadata:
    title: "Immutable Database — How TerminusDB Keeps Every Version"
    description: "Immutable database explained: what it is, how TerminusDB stores each change as an immutable commit, and what you get from it — audit, time travel, branches."
    keywords: immutable database, immutable databases, immutable db, append-only database, immutable storage, database audit trail, db, knowledge graph database, layers, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/immutability-explanation/
media: []
---

An **immutable database** is a database that never changes stored data in place. An update or a delete does not overwrite the old value. Instead, the database writes the change as new data and keeps the old state. So you can always see what the data was before, and who changed it.

TerminusDB is an immutable database with git-like version control for its RDF document graph. When you write, new data layers are appended on top of existing ones. When you delete, a new layer records the removed triples, and they stop showing in the current state. The original data remains intact and untouched underneath. This append-only architecture is what makes branching, time travel, and safe concurrent access possible.

## How immutable storage works in TerminusDB

TerminusDB stores data as a stack of **immutable delta layers**. Here is the short version:

- **A commit adds a layer.** Each commit writes one new layer that includes the triples it added, and the triples it removed. Once written, a layer is never modified.
- **Old layers stay as they were.** The layers under the new one do not change. Readers that still use them are not affected by any later changes.
- **Each commit has metadata.** A commit has an `identifier`, an author, a timestamp and a message. You get these from the commit log.
- **A branch is a pointer.** A branch points to the latest commit in a line of history. A write moves the pointer to a new commit. It does not touch older commits.

Because every past layer is still there, the state at any previous commit is readable. TerminusDB does not need to rebuild the past by replaying a log. The [graphs explanation](/docs/graphs-explanation/) shows how layers fit into the graph hierarchy, and [Immutability and Concurrency](/docs/immutability-and-concurrency/) goes deeper into the storage design and architecture.

## Advantages of immutability

### Transaction safety

Transactions are safer and more reliable in an immutable data store. Any issues during a transactions are easier to handle. Even when the system crashes, TerminusDB resumes operation with data integrity intact providing the operating filesystem synchronization is implemented correctly. Any incomplete transactions get undone automatically. A commit is one new layer, so it is applied all at once or not at all, by design.

### Lock-free concurrency

TerminusDB uses immutable data structures, which makes the system lock-free in most cases. The query engine uses optimistic concurrency, so a transaction can retry if its state changed while it ran. The lack of locking simplifies the engine and makes deadlocks very unlikely, while providing [ACID](/docs/acid-transactions-explanation/) guarantees. Readers see a stable snapshot, even when writers commit at the same time.

### Commit and branch time travel

The transaction history of TerminusDB databases is preserved. It is easy to travel back in time to a commit or branch, and to clone a database so that it starts at any previous commit. All data at a commit point is available immediately by design, without the need to rebuild the state of a past commit. See [time travel queries](/docs/graph-database-with-time-travel-queries/) for an overview.

### Change audit

Time travel comes with information about what was changed in a commit, at what date and time, and by whom. Data provenance metadata is tracked reliably. This adds value to data in regulated environments and for records management. You can also see commits that changed a document, and [diff](/docs/json-diff-and-patch/) is available out of the box between any two commits (that can be applied as a patch too, just like in software engineering).

### Safe branching and merging

Creating a branch only records a new pointer to an existing commit, so it is quick and copies no data. You can test a change on a branch, look at the diff, and then [merge](/docs/merge-howto/) it. If a change was wrong, you can [reset or revert](/docs/undo-reset-howto/). A reset moves the branch pointer back. The commits after that point still exist in the commit graph, so you can naturally reset forward again.

### Collaboration and synchronization

Historical commit information is also required for TerminusDB collaboration. The state of two databases that share a common lineage can be compared. Commits made by different authors can be rerun on the current database with a rebase, so both databases stay in sync. The [Git-for-Data reference](/docs/git-for-data-reference/) covers clone, fetch, push and pull.

### Reproducible results

Because a commit never changes, a query at a fixed commit always gives the same answer. You can run a report, a test or a machine learning job against one commit and get the same input next time. With the ability to have the same starting point, you get repeatable results.

## Immutable database vs append-only log vs history tables

There are several ways to keep history. Each one is good at something. This is a short and, we hope, fair comparison.

| | Immutable database (TerminusDB) | Append-only event log | Mutable database with history tables |
|---|---|---|---|
| What is stored | Every state, as layers of changes | A list of events | The current rows, plus old rows in extra tables |
| How you read the past | Read at a commit or branch | Replay events up to a point | Query the history tables, per table |
| One consistent state of all data | Yes, for every commit | After you replay or build a projection | Per table, unless you join with care |
| Branches and merges | Built in, Git-like | Not built in | Not built in |
| Replay history | Use diff and patch | Replay events | Not built in |
| Who changed what | Author and message on each commit | If you put it in the event | When you build columns or triggers for it |

- An **append-only log** (for example, an event store) is great for streams of events. To see the state at a point in time, you replay events or build a projection.
- A **mutable database with history tables** (for example, SQL temporal or system-versioned tables) is good at "what did this row look like at time T". You turn it on for each table.
- An **immutable database** like TerminusDB keeps the whole graph versioned at once. One commit is one consistent state of all your documents. The whole world, one commit at a time.

## What about performance?

Read and write speed amazingly does not suffer as history grows. Changes are stored as delta layers, and [delta rollup](/docs/delta-rollup/) runs automatically in the standard setup to compact them. Together with the layer cache, this keeps reads to at near constant time as history grows. Rollup builds storage layer caches at precise and controlled intervals in a way that keeps constant time. Time travel, branches and the full commit graph stay as they are.

The cost is disk storage. Old layers are kept, so storage grows as you commit more changes. If you do not need every step on a branch, you can [squash](/docs/squash-projects/) its commits into one.

## Is search enabled on historic versions?

Every historical version is enabled for hybrid search (full text and vector search) using the built in TerminusDB VectorLink search engine which works in a similar way for compact complete history, making every TerminusDB version, branch and commit fully searchable. The automatic document text embeddings, rendered using GraphQL queries and handlebars templates, get rendered by layer into vectors and made full text searchable.

## When an immutable database fits, and when it does not

An immutable database that includes versioned search is a good fit when:

- You must show what a record said on a given day, who changed it, and make it easy to find.
- Several people or systems change the same data, and you want to review changes before they go live.
- You want to debug a bad import by looking at the data just before it.
- You need reports or training data that you can repeat exactly.
- You need to process and search a digital world as it looked at a specific timestamp.

It may not be the best fit when:

- You write high-rate metrics or sensor readings. A time-series database is a better tool for that.
- You never need old states, and disk space matters more than history (although you don't need that much disk space with succinct data structures).
- You must remove a value from all past versions. A delete in a new commit removes the value from the current state, but older commits still contain it. Plan for this before you load such data. For example, keep it in its own database. Deleting a database (`DELETE /api/db/<org>/<db>`) removes it with its full commit history.
- You need to store personal information or identifiers that may need to be removed later. Use unique and non-personal identifiers.

## A small example: commit, then read an older commit

These calls use a database `admin/tdb-example-mydb` on `localhost:6363`. First, write a document. The author and message go into the commit:

```bash
curl -u admin:root -X POST \
  -H "Content-Type: application/json" \
  "http://localhost:6363/api/document/admin/tdb-example-mydb/local/branch/main?author=admin&message=Add+document&json_raw=true" \
  -d '{"@type": "Product", "name": "Widget", "price": 10}'
```

Then get the recent commits on `main`:

```bash
curl -u admin:root "http://localhost:6363/api/log/admin/tdb-example-mydb/local/branch/main?count=5"
```

Each entry has an `identifier`. To read the data as it was at one of those commits, replace `local/branch/main` with `local/commit/<identifier>`:

```bash
curl -u admin:root "http://localhost:6363/api/document/admin/tdb-example-mydb/local/commit/789xyz012345?type=Product&as_list=true"
```

This read does not change the branch. Later commits on `main` do not change what you see at that commit. The [time travel how-to](/docs/time-travel-howto/) shows the same steps in TypeScript and Python, and how to compare and restore old versions.

## Next steps

- [**Get started**](/docs/get-started/) — run TerminusDB and make your first commits
- [**Time travel how-to**](/docs/time-travel-howto/) — query, compare and restore past states
- [**Time travel queries**](/docs/graph-database-with-time-travel-queries/) — an overview, with a comparison to temporal tables and Neo4j
- [**A graph database with git-like version control**](/docs/graph-database-with-git-like-version-control/) — commits, branches, diffs and merges
- [**ACID transactions**](/docs/acid-transactions-explanation/) — how immutability enables atomic, consistent operations
- [**Immutability and concurrency**](/docs/immutability-and-concurrency/) — why readers and writers never block each other
- [**Graphs explained**](/docs/graphs-explanation/) — the layer hierarchy that makes immutability efficient
- [**Delta rollup**](/docs/delta-rollup/) — how accumulated layers are compacted

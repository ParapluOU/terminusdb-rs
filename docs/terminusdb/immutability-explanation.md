---
tags:
  - explanation
  - documents
  - intermediate
title: Immutable Database Explanation
nextjs:
  metadata:
    title: Immutable Database Explanation
    description: An explanation of how the TerminusDB immutable database with Git-like version control implements immutability, and handles deleted and new data.
    keywords: db, document graph database, documents, immutable database, layers, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/immutability-explanation/
media: []
---

TerminusDB is an immutable database with git-like version control for its RDF document graph — it never modifies data in place. When you write, new data layers are appended on top of existing ones; when you delete, a mask layer hides the removed triples. The original data remains intact underneath. This append-only architecture is what makes branching, time travel, and safe concurrent access possible.

## Advantages of immutability

### Transaction safety

Transactions are safer and more reliable in an immutable data store and any issues during transactions are easier to handle. In most cases, even in system crashes, TerminusDB resumes operation with data integrity intact and any incomplete transactions are undone.

### Lock-free concurrency

TerminusDB uses immutable data structures making it lock-free in most cases. The query engine uses optimistic concurrency allowing transactions to retry if their state changed while executing. The lack of locking simplifies the engine and makes deadlocks very unlikely while providing [ACID](/docs/acid-transactions-explanation/) guarantees.

### Commit and branch time travel

The transaction history of TerminusDB databases is preserved. It is easy to travel back in time to a commit or branch and create a new database starting at any commit. All data and information at a commit point are immediately available, eliminating the requirement to rebuild the state of a past commit.

### Change audit

Time travel is supplemented with information about what was committed, at what date and time, and by whom. Data provenance is reliably tracked adding significant value to data in regulated environments and for records management.

### Collaboration and synchronization

Historical commit information is also required for TerminusDB collaboration functionality. The state of two databases that share a common lineage can be compared. Commits made by different authors can be rerun on the current database using a rebase operation, enabling the synchronization of both databases.

## Further reading

- [**ACID transactions**](/docs/acid-transactions-explanation/) — how immutability enables atomic, consistent operations
- [**Time travel**](/docs/time-travel-howto/) — query your database at any historical point
- [**Graphs explained**](/docs/graphs-explanation/) — the layer hierarchy that makes immutability efficient
- [**Delta rollup**](/docs/delta-rollup/) — compact accumulated layers for performance
---
tags:
  - explanation
  - documents
  - advanced
title: Immutability and Concurrency
nextjs:
  metadata:
    title: Immutability and Concurrency
    description: Why TerminusDB avoids locks and how its immutable delta architecture enables safe concurrent access.
    keywords: immutability, concurrency, locks, MVCC, delta layers, architecture
    alternates:
      canonical: https://terminusdb.org/docs/immutability-and-concurrency/
---

Traditional databases use locks to prevent concurrent readers and writers from corrupting shared state. TerminusDB takes a fundamentally different approach: by making all committed data immutable, it eliminates the need for locks entirely.

## The problem with locks

In a conventional database, a write transaction must acquire an exclusive lock on the rows or pages it modifies. This creates three well-known problems:

1. **Readers block writers** — a long-running read query holds a shared lock that prevents writes from proceeding until it completes.
2. **Writers block readers** — a write transaction holds an exclusive lock that forces all readers to wait until the write commits or aborts.
3. **Deadlocks** — when two transactions each hold a lock the other needs, neither can proceed. The database must detect the deadlock and abort one transaction, wasting work.

These problems grow worse under load. Lock contention becomes the bottleneck long before CPU or I/O are saturated. In systems that serve both real-time queries and batch writes (a common pattern in reporting and analytics), lock-based concurrency forces architects to separate read replicas from write primaries — adding operational complexity and introducing replication lag.

## How immutable layers avoid locks

TerminusDB stores data as a stack of **immutable delta layers**. Each layer records the additions and deletions made by a single commit relative to the layer beneath it. Once written, a layer is never modified — it is an append-only, content-addressed file on disk.

This architecture has a critical consequence: **readers never conflict with writers.**

- A reader opens a transaction at a specific layer (the current head of a branch). That layer and its entire parent chain are immutable — no writer can change what the reader sees, regardless of how long the read takes.
- A writer creates a *new* layer on top of the current head. It does not touch the layers that existing readers hold open. When the writer commits, a new head pointer is published atomically.

Because readers and writers operate on disjoint, immutable data, no locks are needed to coordinate them. There is no "dirty read" risk, no "phantom read" anomaly, and no isolation level to configure — every reader sees a perfectly consistent snapshot by construction, not by careful configuration.

## Multi-version concurrency control

This design implements a form of **MVCC** (multi-version concurrency control) — the same principle used by PostgreSQL's snapshots and Git's commit graph, but at the storage layer itself.

Every open transaction holds a reference to a point-in-time snapshot (a specific layer). Multiple readers can hold different snapshots simultaneously — one reader might see commit 47, another commit 52. Both see a consistent, isolated view without blocking each other or any concurrent writer.

Write transactions use **optimistic concurrency**: they proceed without acquiring locks, then check at commit time whether the branch head has moved since they started. If it has, the transaction retries against the new head. The server performs up to three retries by default (configurable via `TERMINUSDB_SERVER_MAX_TRANSACTION_RETRIES`).

Because reads are never blocked and write conflicts are detected only at commit time, throughput remains high even under concurrent load. The system degrades gracefully: under extreme write contention to the same branch, the retry count increases, but readers are never affected.

## What this means in practice

| Operation | Traditional DB | TerminusDB |
|-----------|---------------|------------|
| Long read query during writes | Blocks or sees inconsistent data | Sees a stable snapshot, unaffected by writes |
| Concurrent writes to same branch | Lock contention, possible deadlocks | Optimistic retry — no locks, no deadlocks |
| Read-heavy workload | Lock overhead on every read | Zero coordination cost for reads |
| Time-travel queries | Expensive reconstruction | Instant — just open the historical layer |

## How branches interact with concurrency

Branches in TerminusDB are lightweight pointers to a head layer. Creating a branch is instantaneous — it simply records a new pointer to the same layer. This means:

- Multiple branches can exist simultaneously, each with independent writers, without any coordination between them.
- A writer on branch A never conflicts with a writer on branch B, even if both are modifying the same document type — they are appending layers to different branch heads.
- Merging branches is a structured operation that compares layer stacks and produces a new layer representing the combined changes. Conflicts are detected structurally (at the field level), not by lock timing.

This contrasts sharply with traditional databases, where "branching" data typically means copying entire tables or creating schema-level partitions — expensive operations that consume storage proportional to the dataset size.

## Why this matters for applications

For application developers, the practical implications are:

- **No connection pooling bottlenecks** — read connections never contend with write connections, so you can scale read throughput by simply opening more connections without risking deadlocks or timeouts.
- **Safe long-running analytics** — a report that takes minutes to generate holds a snapshot that is never invalidated by concurrent writes. The report sees a consistent point-in-time view from start to finish.
- **Predictable latency** — because reads require no coordination, their latency is determined only by the layer traversal depth (bounded by rollup) and I/O speed, not by how many writers are active.
- **Simplified error handling** — the only failure mode for writes is an optimistic concurrency conflict, which the server retries automatically (up to a configurable limit). Applications never need to implement retry-on-deadlock logic.

## The trade-off: layer accumulation

The cost of immutability is that layers accumulate. After hundreds of commits, reading a single triple may require traversing a deep parent chain. This is where [delta rollup](/docs/delta-rollup/) comes in: a maintenance operation that compresses layers into fewer, flatter layers without changing semantics or commit history.

TerminusDB includes an auto-optimize plugin that triggers compaction probabilistically after commits, keeping layer depth manageable without manual intervention. The exponential strategy (base 3) means that even high-throughput workloads producing thousands of commits will only accumulate a logarithmic number of layers between rollups.

## Comparison with PostgreSQL MVCC

PostgreSQL also implements MVCC, but with a critical difference: old row versions (dead tuples) occupy space in the same table until `VACUUM` reclaims them. Under write-heavy workloads, table bloat and vacuum contention become operational concerns. TerminusDB's layers are separate, content-addressed files — they never bloat existing structures, and compaction (rollup) creates new files alongside existing ones without touching live data.

Additionally, PostgreSQL's MVCC still requires locks for DDL operations (schema changes), certain index operations, and explicit `SELECT FOR UPDATE` patterns. TerminusDB schema changes are themselves versioned commits — they follow the same immutable-layer pattern as data, so even schema modifications never lock readers. This makes TerminusDB particularly well-suited to environments where schemas evolve frequently alongside data.

## Summary

TerminusDB's immutable delta architecture eliminates the fundamental tension between readers and writers. Readers always see a consistent snapshot. Writers append new layers without disturbing existing data. No locks means no contention, no deadlocks, and predictable performance under concurrent load — at the cost of periodic compaction to manage layer depth.

The net result is an architecture where concurrency is not a problem to be managed — it is a natural consequence of the storage model.

## Further reading

- [Immutability](/docs/immutability-explanation/) — how TerminusDB implements immutable storage
- [ACID Transactions](/docs/acid-transactions-explanation/) — atomicity, consistency, isolation, and durability guarantees
- [Perform a Delta Rollup](/docs/delta-rollup/) — the maintenance operation that manages layer accumulation

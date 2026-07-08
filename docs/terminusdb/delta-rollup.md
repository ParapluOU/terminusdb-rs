---
tags:
  - how-to
  - version-control
  - curl
  - advanced
title: Perform a Delta Rollup
nextjs:
  metadata:
    title: Perform a Delta Rollup
    description: How delta rollup works in TerminusDB. Automatic in standard deployments via the built-in rollup plugin — manual optimisation is available for custom deployments.
    keywords: delta rollup, optimize, performance, layers, compaction, maintenance
    alternates:
      canonical: https://terminusdb.org/docs/delta-rollup/
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally
- A database with many delta layers (long history)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have compacted delta layers to improve query performance.
{% /callout %}

A delta rollup compresses the accumulated delta layers in a TerminusDB database into fewer (or a single) flat layer, improving read performance. **In most deployments this happens automatically** — TerminusDB ships with a rollup plugin enabled by default that triggers optimisation after commits. Manual rollups are only needed if you are calling the `optimize` endpoint directly, running a custom deployment without the plugin, or tuning performance in specific scenarios.

{% callout type="note" %}
**Automatic in standard deployments**
TerminusDB includes a rollup plugin that is enabled by default. It probabilistically triggers optimisation after commits, so most users never need to run a manual rollup. Continue reading only if you need to trigger optimisation manually, tune the plugin, or understand what it does under the hood.
{% /callout %}

## Why layers accumulate

Every commit in TerminusDB creates a new **delta layer** — an immutable record of additions and deletions relative to the layer beneath it. Over hundreds or thousands of commits, the layer stack grows deep. When TerminusDB resolves a triple, it must traverse the entire parent chain from newest to oldest layer, checking each for the presence or absence of that triple. A deep stack means slower reads.

## When to perform a rollup

**Note:** If you are running a standard TerminusDB installation, the auto-optimize plugin handles this automatically. The scenarios below apply when running manual optimisation or when the plugin is disabled.

Run a rollup when:

- Read queries have become noticeably slower after many commits
- You have completed a bulk import (hundreds or thousands of documents)
- You observe the layer stack depth exceeding ~50–100 layers
- Routine maintenance — for example, nightly or weekly on active databases

TerminusDB also ships an optional **auto-optimize plugin** that probabilistically triggers optimization after commits, removing the need for manual scheduling in most deployments.

## How to trigger a rollup

TerminusDB provides two mechanisms: an HTTP API endpoint and the CLI.

### HTTP API

Send a `POST` request to `/api/optimize/<path>` where `<path>` is the database descriptor.

**Optimize a branch (most common):**

```bash
curl -X POST http://localhost:6363/api/optimize/admin/MyDatabase/local/branch/main \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{}'
```

**Optimize the repository layer (commit graph):**

```bash
curl -X POST http://localhost:6363/api/optimize/admin/MyDatabase/local \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{}'
```

**Optimize the system database:**

```bash
curl -X POST http://localhost:6363/api/optimize/_system \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{}'
```

A successful response returns:

```json
{
  "@type": "api:OptimizeResponse",
  "api:status": "api:success"
}
```

### CLI

```bash
# Optimize a branch
terminusdb optimize admin/MyDatabase/local/branch/main

# Optimize the system database
terminusdb optimize _system
```

### Rollup (targeted)

The `rollup` command creates an optimisation layer for a specific commit without the full squash behaviour of `optimize`:

```bash
terminusdb rollup admin/MyDatabase/local/branch/main
```

## What rollup does to commit history

A rollup **preserves commit history**. It does not remove or rewrite commits — it only flattens the underlying storage layers that hold triple data. After a rollup, time-travel, branching, and the full commit graph remain intact.

This is different from a [squash](/docs/squash-projects/), which collapses multiple commits into a single commit and does discard intermediate commit history.

## How the exponential rollup strategy works

TerminusDB uses an exponential rollup strategy (base 3) rather than always flattening everything into one layer. This creates a hierarchy of rollup layers at exponentially increasing intervals:

- After 3 commits, layers 1–3 are rolled into one.
- After 9 commits, three 3-layer rollups are rolled into one.
- After 27 commits, three 9-layer rollups are rolled into one.

This logarithmic approach means that even a database with thousands of commits will have a maximum traversal depth proportional to log₃(commits) — typically single digits. It balances compaction effort against read performance: each rollup touches only a small proportion of total data.

For most users, the `optimize` endpoint handles this automatically. The auto-optimize plugin included in the standard Docker image triggers this strategy probabilistically after commits, so manual intervention is rarely needed.

## Caveats

- **Duration** — Rollup time is proportional to the total data size, not the number of layers. Large databases (millions of triples) may take seconds to minutes.
- **Concurrent access** — Rollups create parallel files alongside existing layers. Readers are never blocked; they continue using the existing layer chain until the rollup completes. There is no lock contention.
- **Idempotency** — Running optimize on an already-optimized database is a no-op and completes immediately.
- **Disk space** — Rolled-up layers coexist with their source layers temporarily. TerminusDB garbage-collects unreferenced layers over time.
- **Permissions** — The caller requires `meta_write_access` on the target resource (or must be a super-user for `_system`).

## Further reading

- [Immutability and Concurrency](/docs/immutability-and-concurrency/) — why the layer architecture avoids locks
- [ACID Transactions](/docs/acid-transactions-explanation/) — how immutability enables atomic, isolated transactions
- [Graphs](/docs/graphs-explanation/) — the full layer and graph hierarchy

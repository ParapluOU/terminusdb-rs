---
tags:
  - reference
  - installation
  - curl
  - enterprise
title: Enterprise Configuration Reference
nextjs:
  metadata:
    title: Enterprise Configuration Reference
    description: Complete reference for all TwinfoxDB Enterprise environment variables covering context caching, remote context processing, LRU cache tuning, and feature activation.
    keywords: configuration, environment variables, enterprise, context cache, lru cache, tuning, reference
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-configuration/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise adds several environment variables for controlling features, caching behavior, and performance tuning. These variables are read at server startup and apply to all requests.

## Feature activation

{% table %}

- Variable
- Default
- Description

---

- `TERMINUSDB_ENTERPRISE`
- `false`
- Set to `true` to enable enterprise features. In Docker enterprise images, this is set automatically.

{% /table %}

## Context caching

These variables control how remote JSON-LD contexts are cached and resolved. See [Context Cache](/docs/enterprise-context-cache/) for architecture details.

{% table %}

- Variable
- Default
- Description

---

- `TERMINUSDB_CONTEXT_CACHE_PATH`
- `<storage_dir>/context_cache`
- Directory for the context cache. Contains the catalog subdirectory (plain files) and the HTTP cache subdirectory for processing JSON-LD. The default is derived from the parent of the database path. This enables to have known good contexts and avoid dynamic resolution which both increases latency and can increase the attack surface where 3rd party HTTP connections are made.

---

- `TERMINUSDB_ALLOW_REMOTE_CONTEXT`
- `false`
- When `true`, cache misses for remote URL contexts trigger an HTTP fetch. When `false`, only pre-seeded and previously cached contexts are available. Set to `false` in production for deterministic behavior.

{% /table %}

### Context cache path resolution

The default cache path is computed as follows:

1. If `TERMINUSDB_CONTEXT_CACHE_PATH` is set, use it directly
2. Otherwise, take the database path (from `TERMINUSDB_SERVER_DB_PATH`, default `./storage/db`)
3. Go up one directory to the storage root (e.g., `./storage/`)
4. Append `context_cache` (e.g., `./storage/context_cache`)

Inside the cache directory, two subdirectories are created automatically:

- `catalog/` — plain inspectable files, index, and SGML catalog
- `http-cache/` — RFC-compliant HTTP response cache

### Seed file directory

The context cache looks for pre-seeded context files at startup. It searches for a `data/context-seeds/` directory relative to the enterprise module path. In Docker, this is `terminusdb-enterprise/data/context-seeds/`. Seed files are copied into the catalog directory on first use.

## LRU cache tuning

The LRU cache manages eviction of graph layers from memory. These variables control its behavior.

{% table %}

- Variable
- Default
- Description

---

- `TERMINUSDB_LRU_CACHE_SIZE`
- `1073741824` (1 GB)
- Maximum size in bytes for the in-memory layer cache. Layers beyond this limit are evicted least-recently-used first.

---

- `TERMINUSDB_LRU_EVICTION_PERCENT`
- `10`
- Percentage of the cache to evict when the limit is reached. A higher value means fewer but larger eviction passes. Enterprise default is 10%.

{% /table %}

### Tuning guidance

- **Small datasets, fast queries** — keep the default. 1 GB is generous for most workloads.
- **Large datasets, memory pressure** — reduce `TERMINUSDB_LRU_CACHE_SIZE` to leave headroom for the Prolog stack and OS caches.
- **Many concurrent databases** — increase the cache size if you see frequent evictions in the [Prometheus metrics](/docs/enterprise-observability/) (`terminusdb_lru_cache_used_bytes` staying near `terminusdb_lru_cache_limit_bytes`).
- **Eviction granularity** — the default 10% eviction percentage is a good balance. Lower values (e.g., 5%) mean more frequent but smaller evictions and a leaner memory usage.

## Metrics prefix

{% table %}

- Variable
- Default
- Description

---

- `TERMINUSDB_METRICS_PREFIX`
- `terminusdb`
- Prefix for all Prometheus metric names. Change this if you run multiple TerminusDB instances and want to distinguish them in your monitoring stack.

{% /table %}

## Standard TerminusDB variables

Enterprise inherits all standard TerminusDB environment variables. The most commonly used ones alongside enterprise features:

{% table %}

- Variable
- Default
- Description

---

- `TERMINUSDB_SERVER_DB_PATH`
- `./storage/db`
- Database storage path. Also determines the default context cache path.

---

- `TERMINUSDB_SERVER_PORT`
- `6363`
- HTTP server port.

---

- `TERMINUSDB_ADMIN_PASS`
- `root`
- Admin user password for basic authentication.

---

- `TERMINUSDB_JWT_ENABLED`
- `true` (Docker)
- Enable JWT authentication. Set to `false` for local development with basic auth only.

---

- `TERMINUSDB_PLUGINS_PATH`
- `<storage_dir>/plugins`
- Directory for server plugins (e.g., auto-optimize).

{% /table %}

## Example: production configuration

A typical production setup with all enterprise variables:

```bash
export TERMINUSDB_ENTERPRISE=true
export TERMINUSDB_SERVER_DB_PATH=/data/terminusdb/db
export TERMINUSDB_SERVER_PORT=6363
export TERMINUSDB_CONTEXT_CACHE_PATH=/data/terminusdb/context_cache
export TERMINUSDB_ALLOW_REMOTE_CONTEXT=false
export TERMINUSDB_LRU_CACHE_SIZE=2147483648  # 2 GB
export TERMINUSDB_LRU_EVICTION_PERCENT=10
export TERMINUSDB_ADMIN_PASS=your-secure-password
```

## Example: Docker Compose

```yaml
services:
  terminusdb:
    image: terminusdb/terminusdb:enterprise
    ports:
      - "6363:6363"
    environment:
      TERMINUSDB_ENTERPRISE: "true"
      TERMINUSDB_ADMIN_PASS: "your-secure-password"
      TERMINUSDB_LRU_CACHE_SIZE: "2147483648"
      TERMINUSDB_ALLOW_REMOTE_CONTEXT: "false"
    volumes:
      - terminusdb-data:/app/terminusdb/storage

volumes:
  terminusdb-data:
```

## Further reading

- [Context Cache](/docs/enterprise-context-cache/) — cache architecture and seed file management
- [Prometheus Observability](/docs/enterprise-observability/) — monitoring cache and memory metrics
- [Enterprise Overview](/docs/enterprise/) — all enterprise features

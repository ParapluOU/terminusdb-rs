---
tags:
  - reference
  - installation
  - curl
  - enterprise
title: Prometheus Observability
nextjs:
  metadata:
    title: Prometheus Observability
    description: How to use the TerminusDB Enterprise Edition Prometheus metrics endpoint for production monitoring, including available gauges for memory, cache, worker pool, and request tracking.
    keywords: prometheus, metrics, observability, monitoring, memory, cache, lru, enterprise
    alternates:
      canonical: https://terminusdb.com/docs/enterprise-observability/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise exposes a Prometheus-compatible metrics endpoint for production monitoring. The endpoint serves metrics in the standard Prometheus exposition format, ready for scraping by Prometheus, Grafana Agent, or any compatible collector.

## Metrics endpoint

```text
GET /api/metrics
```

The endpoint **requires authentication**. The caller must hold `meta_read_access` on the system store (`SystemDatabase/system`) through a capability assigned to their own user — capabilities held by organizations or teams the user belongs to do not count on their own; the grant must sit on the user.

- The built-in `admin` user already satisfies the check via its `Capability/server_access` grant on `Organization/admin`, which subsumes the system store.
- Requests without the capability receive `403` — including credential-less requests, which authenticate as `User/anonymous`.
- Malformed credentials receive `401`.

```bash
curl -s -u admin:root http://localhost:6363/api/metrics
```

### Granting a scrape account

For production monitoring, prefer a dedicated user over the admin account. The narrowest grant is a custom role carrying only `meta_read_access`, assigned on the `admin` organization (the scope that contains the system store). Note that an organization-scoped grant also covers the databases under that organization — capability scope checks follow the resource hierarchy.

```bash
# 1. Create the scrape user
curl -s -u admin:root -X POST http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "metrics-scraper", "password": "change-me"}'

# 2. Create a role with only meta_read_access
curl -s -u admin:root -X POST http://localhost:6363/api/roles \
  -H "Content-Type: application/json" \
  -d '{"name": "metrics-reader", "action": ["meta_read_access"]}'

# 3. Grant the role to the user on the admin organization
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "admin",
    "user": "metrics-scraper",
    "roles": ["metrics-reader"]
  }'
```

See [Capabilities API modes](/docs/capabilities-api-modes/) for the grant/revoke payload reference.

### Example response

```text
# HELP build_info Server version and build information
# TYPE build_info gauge
build_info{version="12.0.7",git_hash="abc1234",store_version="0.19.8",edition="enterprise"} 1
# HELP uptime_seconds Time since the server process started
# TYPE uptime_seconds gauge
uptime_seconds 3842.7
# HELP atoms Number of atoms in the Prolog atom table
# TYPE atoms gauge
atoms 29510
# HELP layer_cache_entries_total Total entries in the layer object cache
# TYPE layer_cache_entries_total gauge
layer_cache_entries_total 42
# HELP layer_cache_entries_live Live (in use) layer cache entries
# TYPE layer_cache_entries_live gauge
layer_cache_entries_live 38
# HELP layer_cache_entries_dead Dead (evicted, not yet freed) layer cache entries
# TYPE layer_cache_entries_dead gauge
layer_cache_entries_dead 4
# HELP worker_pool_busy Number of worker threads currently processing requests
# TYPE worker_pool_busy gauge
worker_pool_busy 2
# HELP lru_cache_limit_bytes Configured LRU archive cache limit
# TYPE lru_cache_limit_bytes gauge
lru_cache_limit_bytes 536870912
# HELP lru_cache_used_bytes Current LRU archive cache usage
# TYPE lru_cache_used_bytes gauge
lru_cache_used_bytes 8388608
```

Metric names have no prefix by default; a prefix can be configured in the enterprise config (`metrics_prefix/1`).

## Available metrics

### Server information

{% table %}

- Metric
- Type
- Description

---

- `build_info`
- gauge
- Always 1. Labels carry `version`, `git_hash`, `store_version`, and `edition`

---

- `uptime_seconds`
- gauge
- Seconds since server start

---

- `start_time_seconds`
- gauge
- Unix timestamp of server start

---

- `http_requests_active`
- gauge
- Number of HTTP requests currently being processed

---

- `http_connections_active`
- gauge
- Number of active HTTP connections in the Rust webserver

{% /table %}

### Memory and stack

{% table %}

- Metric
- Type
- Description

---

- `stack_used_bytes`
- gauge
- SWI-Prolog stack memory in use (global + local + trail)

---

- `process_rss_bytes`
- gauge
- Process resident set size (physical memory)

---

- `heap_used_bytes`
- gauge
- Process heap memory — omitted if 0 (deprecated in SWI-Prolog 10.x)

---

- `atom_space_used_bytes`
- gauge
- Memory used by the atom table

---

- `functor_space_used_bytes`
- gauge
- Memory used by the functor table

---

- `table_space_used_bytes`
- gauge
- Memory used by tabling (memoization of pinned graphs, config)

---

- `global_stack_allocated_bytes` / `global_stack_used_bytes`
- gauge
- Global (term) stack allocated vs in use

---

- `local_stack_allocated_bytes` / `local_stack_used_bytes`
- gauge
- Local (environment) stack allocated vs in use

---

- `trail_stack_allocated_bytes` / `trail_stack_used_bytes`
- gauge
- Trail stack allocated vs in use

{% /table %}

### Code and atoms

Growth in these metrics without matching traffic indicates retained data — asserted clauses pinning atoms, for example.

{% table %}

- Metric
- Type
- Description

---

- `atoms`
- gauge
- Atoms in the Prolog atom table

---

- `inferences_total`
- counter
- Total Prolog inferences

---

- `clauses`
- gauge
- Total clauses across all predicates

---

- `predicates`
- gauge
- Total defined predicates

---

- `dynamic_clauses`
- gauge
- Clauses held by dynamic predicates — growth here without traffic means a dynamic predicate accumulating clauses per request or transaction

---

- `atom_gc_events_total`
- counter
- Completed atom garbage collection passes

---

- `atom_gc_atoms_reclaimed_total`
- counter
- Atoms reclaimed by atom GC — if `atoms` climbs while this stays flat, reachable data is pinning atoms

{% /table %}

### Worker pool and threads

{% table %}

- Metric
- Type
- Description

---

- `worker_pool_total`
- gauge
- Worker threads in the request pool

---

- `worker_pool_idle`
- gauge
- Idle workers available for request processing

---

- `worker_pool_busy`
- gauge
- Workers currently processing requests

---

- `worker_busy_seconds`
- gauge
- Per-worker busy duration, labelled by worker `alias` — a worker stuck at high values indicates a long-running or hung request

---

- `threads_by_type`
- gauge
- Prolog threads by type label: `worker`, `watchdog`, `indexer`, `commits`, `other`

{% /table %}

### Foreign blobs

Each foreign blob atom can pin native memory (buffers, layer handles) that is invisible to the Prolog space metrics. Growth in a blob type without matching traffic means retained references.

{% table %}

- Metric
- Type
- Description

---

- `blobs`
- gauge
- Foreign blob atoms by type label, e.g. `memory_file`, `layer`, `named_graph`, `stream`, `text`, `trie`

{% /table %}

### Layer cache

The layer cache stores graph layers in memory for fast access. These metrics help you understand cache pressure and tune the LRU eviction policy.

{% table %}

- Metric
- Type
- Description

---

- `layer_cache_entries_total`
- gauge
- Total entries in the layer object cache

---

- `layer_cache_entries_live`
- gauge
- Live (in use) cache entries

---

- `layer_cache_entries_dead`
- gauge
- Dead (evicted, not yet freed) cache entries

---

- `layer_cache_bytes_total`
- gauge
- Backing bytes for all cache entries

---

- `layer_cache_bytes_live`
- gauge
- Backing bytes for live entries

---

- `layer_cache_bytes_dead`
- gauge
- Backing bytes for dead entries

---

- `layer_cache_pinned_databases`
- gauge
- Databases pinned in the layer cache

---

- `layer_cache_pinned_organizations`
- gauge
- Organizations with pinned databases

---

- `layer_cache_pinned_bytes`
- gauge
- Backing bytes for pinned database layers

{% /table %}

### LRU cache

The LRU (Least Recently Used) cache manages eviction of graph layers when memory pressure increases.

{% table %}

- Metric
- Type
- Description

---

- `lru_cache_limit_bytes`
- gauge
- Configured maximum cache size

---

- `lru_cache_used_bytes`
- gauge
- Current cache usage

{% /table %}

## Prometheus scrape configuration

Add TerminusDB to your Prometheus `scrape_configs`. The endpoint requires authentication, so provide credentials for a user holding `meta_read_access` on the system store:

```yaml
scrape_configs:
  - job_name: 'terminusdb'
    scrape_interval: 15s
    metrics_path: '/api/metrics'
    basic_auth:
      username: metrics-scraper
      password_file: /etc/prometheus/secrets/terminusdb-metrics-password
    static_configs:
      - targets: ['localhost:6363']
```

## Grafana dashboard tips

Useful panels for a TerminusDB Grafana dashboard:

- **Uptime** — `uptime_seconds`
- **Cache pressure** — `layer_cache_entries_dead` climbing relative to `layer_cache_entries_total`
- **Memory pressure** — `lru_cache_used_bytes / lru_cache_limit_bytes`
- **Pool saturation** — `worker_pool_busy` against `worker_pool_total`, plus `worker_busy_seconds` for stuck requests
- **Atom growth** — `atoms` alongside `atom_gc_atoms_reclaimed_total` to distinguish reachable data from an AGC backlog
- **Active requests** — `http_requests_active` and `http_connections_active` for concurrency monitoring
- **Cache eviction headroom** — `lru_cache_limit_bytes - lru_cache_used_bytes`

## Further reading

- [Configuration Reference](/docs/enterprise-configuration/) — LRU cache tuning variables
- [Capabilities API modes](/docs/capabilities-api-modes/) — granting roles and capabilities
- [Enterprise Overview](/docs/enterprise/) — all enterprise features

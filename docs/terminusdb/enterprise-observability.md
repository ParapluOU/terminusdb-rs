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
    description: How to use the TwinfoxDB Enterprise Prometheus metrics endpoint for production monitoring, including available gauges for memory, cache, and request tracking.
    keywords: prometheus, metrics, observability, monitoring, memory, cache, lru, enterprise
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-observability/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise exposes a Prometheus-compatible metrics endpoint for production monitoring. The endpoint serves metrics in the standard Prometheus exposition format, ready for scraping by Prometheus, Grafana Agent, or any compatible collector.

## Metrics endpoint

```
GET /api/metrics
```

The endpoint is accessible **without authentication** so that monitoring infrastructure can scrape it without storing credentials.

```bash
curl -s http://localhost:6363/api/metrics
```

### Example response

```
# HELP terminusdb_info Server information
# TYPE terminusdb_info gauge
terminusdb_info{version="11.2.0",git_hash="abc1234",edition="enterprise"} 1
# HELP terminusdb_uptime_seconds Server uptime in seconds
# TYPE terminusdb_uptime_seconds gauge
terminusdb_uptime_seconds 3842.7
# HELP terminusdb_global_stack_usage_bytes Global stack usage in bytes
# TYPE terminusdb_global_stack_usage_bytes gauge
terminusdb_global_stack_usage_bytes 524288
# HELP terminusdb_table_space_bytes Table space usage in bytes
# TYPE terminusdb_table_space_bytes gauge
terminusdb_table_space_bytes 1048576
# HELP terminusdb_active_requests Number of currently active HTTP requests
# TYPE terminusdb_active_requests gauge
terminusdb_active_requests 0
# HELP terminusdb_layer_cache_total Total layers ever loaded into cache
# TYPE terminusdb_layer_cache_total gauge
terminusdb_layer_cache_total 42
# HELP terminusdb_layer_cache_live Currently live (pinned or unpinned) layers
# TYPE terminusdb_layer_cache_live gauge
terminusdb_layer_cache_live 38
# HELP terminusdb_layer_cache_dead Evicted but not yet freed layers
# TYPE terminusdb_layer_cache_dead gauge
terminusdb_layer_cache_dead 4
# HELP terminusdb_layer_cache_memory_bytes Memory usage by layer cache
# TYPE terminusdb_layer_cache_memory_bytes gauge
terminusdb_layer_cache_memory_bytes{state="total"} 8388608
terminusdb_layer_cache_memory_bytes{state="live"} 7340032
terminusdb_layer_cache_memory_bytes{state="dead"} 1048576
# HELP terminusdb_lru_cache_limit_bytes LRU cache size limit
# TYPE terminusdb_lru_cache_limit_bytes gauge
terminusdb_lru_cache_limit_bytes 1073741824
# HELP terminusdb_lru_cache_used_bytes LRU cache currently used bytes
# TYPE terminusdb_lru_cache_used_bytes gauge
terminusdb_lru_cache_used_bytes 8388608
# HELP terminusdb_lru_cache_pinned_count Number of pinned entries in LRU cache
# TYPE terminusdb_lru_cache_pinned_count gauge
terminusdb_lru_cache_pinned_count 5
# HELP terminusdb_lru_cache_pinned_bytes Bytes used by pinned LRU entries
# TYPE terminusdb_lru_cache_pinned_bytes gauge
terminusdb_lru_cache_pinned_bytes 2097152
```

## Available metrics

### Server information

{% table %}

- Metric
- Type
- Description

---

- `terminusdb_info`
- gauge
- Always 1. Labels carry `version`, `git_hash`, and `edition`

---

- `terminusdb_uptime_seconds`
- gauge
- Seconds since server start

---

- `terminusdb_active_requests`
- gauge
- Number of HTTP requests currently being processed

{% /table %}

### Memory and stack

{% table %}

- Metric
- Type
- Description

---

- `terminusdb_global_stack_usage_bytes`
- gauge
- SWI-Prolog global stack usage

---

- `terminusdb_table_space_bytes`
- gauge
- SWI-Prolog tabling space usage

{% /table %}

### Layer cache

The layer cache stores graph layers in memory for fast access. These metrics help you understand cache pressure and tune the LRU eviction policy.

{% table %}

- Metric
- Type
- Description

---

- `terminusdb_layer_cache_total`
- gauge
- Total layers ever loaded

---

- `terminusdb_layer_cache_live`
- gauge
- Currently active layers (pinned + unpinned)

---

- `terminusdb_layer_cache_dead`
- gauge
- Evicted layers not yet freed

---

- `terminusdb_layer_cache_memory_bytes`
- gauge
- Memory by state: `total`, `live`, `dead`

{% /table %}

### LRU cache

The LRU (Least Recently Used) cache manages eviction of graph layers when memory pressure increases.

{% table %}

- Metric
- Type
- Description

---

- `terminusdb_lru_cache_limit_bytes`
- gauge
- Configured maximum cache size

---

- `terminusdb_lru_cache_used_bytes`
- gauge
- Current cache usage

---

- `terminusdb_lru_cache_pinned_count`
- gauge
- Number of pinned (non-evictable) entries

---

- `terminusdb_lru_cache_pinned_bytes`
- gauge
- Memory used by pinned entries

{% /table %}

## Prometheus scrape configuration

Add TerminusDB to your Prometheus `scrape_configs`:

```yaml
scrape_configs:
  - job_name: 'terminusdb'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:6363']
    metrics_path: '/api/metrics'
```

## Grafana dashboard tips

Useful panels for a TerminusDB Grafana dashboard:

- **Uptime** — `terminusdb_uptime_seconds`
- **Cache hit ratio** — derive from `terminusdb_layer_cache_live` vs `terminusdb_layer_cache_total`
- **Memory pressure** — `terminusdb_lru_cache_used_bytes / terminusdb_lru_cache_limit_bytes`
- **Active requests** — `terminusdb_active_requests` for concurrency monitoring
- **Cache eviction headroom** — `terminusdb_lru_cache_limit_bytes - terminusdb_lru_cache_used_bytes`

## Further reading

- [Configuration Reference](/docs/enterprise-configuration/) — LRU cache tuning variables
- [Enterprise Overview](/docs/enterprise/) — all enterprise features

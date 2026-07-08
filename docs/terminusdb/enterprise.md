---
tags:
  - explanation
  - enterprise
title: DFRNT TwinfoxDB Enterprise Edition
nextjs:
  metadata:
    title: Enterprise Edition
    description: DFRNT TwinfoxDB Enterprise extends TerminusDB with commercial support, multi-format document processing including JSON-LD @context support, RDF/XML, and Turtle, management tooling, and observability.
    keywords: enterprise, twinfoxdb, json-ld, rdf xml, turtle, ttl, backup, restore, prometheus, observability
    alternates:
      canonical: https://terminusdb.org/docs/enterprise/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

DFRNT TwinfoxDB Enterprise extends TerminusDB with increased performance and additional capabilities for organizations that need multi-format data exchange, standards-compliant linked data context processing, operational backup and restore, and production observability.

All enterprise features are additive — they extend the open-source TerminusDB core without changing its behavior. Existing databases, queries, and client integrations work unchanged.

[Contact DFRNT](https://dfrnt.com/contact?utm_source=terminusdb) for more information on how to deploy the enterprise version.

## What Enterprise adds

Enterprise unlocks several areas that matter when TerminusDB moves from development into production and cross-system integration:

- **Multi-format document API** — Read and write documents as JSON, JSON-LD, RDF/XML, or Turtle through the document API endpoint, using query parameters or HTTP content negotiation
- **W3C JSON-LD processing** — Full `@context` support for expansion and compaction, including remote URL contexts like schema.org
- **Context caching** — Pre-seeded local cache for remote JSON-LD contexts, eliminating network dependencies in production
- **RDF/XML serialization** — Standards-compliant RDF/XML output with proper namespace declarations, typed literals, and subdocument nesting
- **Turtle serialization** — Compact, human-readable RDF output with prefix declarations, blank node nesting, and typed literals
- **Backup and restore** — Binary bundle/unbundle API for database snapshots, migration, and high resolution disaster recovery
- **Prometheus metrics** — Production-grade observability with memory, cache, and request metrics in Prometheus exposition format
- **High resolution history** — which documents changed how and when is available as a knowledge graph, enabling very fast document history
- **Enterprise configuration** — Environment variables for tuning cache behavior, context resolution, and LRU eviction

## Getting started

If you are new to Enterprise, the [Multi-Format Document API Tutorial](/docs/enterprise-document-formats/) walks through a complete round-trip with `curl` — from database creation through JSON-LD and RDF/XML export and re-import.

For specific topics:

- [Document Formats & Content Negotiation](/docs/enterprise-document-formats/) — how to request and submit data in different formats
- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — how `@context` works for input and output
- [Context Cache](/docs/enterprise-context-cache/) — pre-seeded contexts and the disk cache architecture
- [RDF/XML Support](/docs/enterprise-rdfxml/) — namespace handling, typed literals, and subdocument serialization
- [Turtle Support](/docs/enterprise-turtle/) — compact, human-readable RDF with prefix declarations and blank node nesting
- [Backup & Restore](/docs/enterprise-backup-restore/) — the bundle/unbundle API
- [Prometheus Observability](/docs/enterprise-observability/) — metrics endpoint and available gauges
- [Configuration Reference](/docs/enterprise-configuration/) — all enterprise environment variables

## Enabling Enterprise

In Docker, the enterprise configuration is enabled automatically when using the enterprise image. For local builds, see the [build documentation](/docs/install-terminusdb-from-source-code/).

```bash
export TERMINUSDB_ENTERPRISE=true
./terminusdb serve
```

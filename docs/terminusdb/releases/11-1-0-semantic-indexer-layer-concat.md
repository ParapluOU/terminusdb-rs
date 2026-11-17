---
tags:
  - release-notes
  - vectorlink
  - version-control
ai: assisted
title: 'TerminusDB 11.1.0: Semantic Indexer and Layer Concat'
release:
  version: 11.1.0
  date: 2023-07-10
  line: '11.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v11.1.0
nextjs:
  metadata:
    title: 'Semantic Indexer and Layer Concat | TerminusDB 11.1.0'
    description: 'TerminusDB 11.1.0 lets you run semantic similarity search with the VectorLink indexer, so you can find documents by meaning. Also: layer concat.'
    keywords: terminusdb 11.1.0, vectorlink semantic indexer, ai vector search, concat base layers, embedding index database, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/11-1-0-semantic-indexer-layer-concat/
---

TerminusDB 11.1.0 adds the VectorLink semantic indexer for AI-powered similarity search, and a `concat` command for merging storage layers. It was released on **10 July 2023**.

- [**Semantic indexer**](#semantic-indexer) — VectorLink indexes documents for similarity search.
- [**`concat` command**](#concat-command) — merge base layers into one.
- [**Lower memory usage**](#lower-memory-usage) — leaner server, no Google Analytics in the dashboard.

## Semantic indexer

### What it does

Support for the TerminusDB semantic indexer — VectorLink — a sidecar that stores, indexes and retrieves embedding vectors by similarity. Documents can be rendered to text, embedded, and searched semantically on top of the knowledge graph.

### Why it matters

This VectorLink 1.0 brings Vector Search to TerminusDB and enables every structured document to get a semantic vector embedding automatically (GraphQL and Handlebars to render from a template), and at the same time having the document as a structural part of the knowledge graph too. Full git-for-data including for Vector Search is a powerful capability.

The roadmap version of VectorLink 2.0 will offer even more advanced features and capabilities. Check it out when released!

**Read more:** [Set up VectorLink](/docs/set-up-vectorlink/) · [Use VectorLink](/docs/use-vectorlink/) · [OpenAI & Handlebars configuration](/docs/openai-handlebars-config/)

## `concat` command

A new `concat` command merges base layers into a single layer — a manual, more aggressive variant of the automatic squashes the optimizer performs.

## Lower memory usage

General memory reductions across the server, and the dashboard no longer ships Google Analytics.

## Fixes

- Fixed GraphQL ordering of stringy numbers; added type information to decimals
- Corrected diffs using out-of-band values; prevented reserved words in GraphQL names
- Fixed loading of triples

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v11.1.0
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v11.0.6...v11.1.0)

---

**Previous:** [TerminusDB 11.0.6: Automatic Schema Migrations](/docs/releases/11-0-6-automatic-schema-migrations/) · **Next:** [TerminusDB 11.1.1: Multiple Inheritance for Properties](/docs/releases/11-1-1-multiple-inheritance-docker-compose/) · [TerminusDB 11.1 changelog](/docs/releases/11-1-changelog/) · [Release index](/docs/releases/)

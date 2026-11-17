---
tags:
  - explanation
  - beginner
title: "TerminusDB features at a glance"
nextjs:
  metadata:
    title: "TerminusDB Features at a Glance: Graph + Git for Data"
    description: "A quick fact sheet of TerminusDB features: JSON documents in a typed graph, Git-like branch, diff and merge, WOQL and GraphQL. Links to every topic."
    keywords: terminusdb, terminusdb features, features overview, fact sheet, document database, graph database, immutable database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/at-a-glance
---

A short, scannable list of what TerminusDB does, with a link to the docs for each feature. Want the full explanation? Read [What is TerminusDB?](/docs/terminusdb-explanation/). New to knowledge graphs? Start with [What is a knowledge graph database?](/docs/knowledge-graph-database/).

## Quick facts

| | |
|---|---|
| **Data model** | [Schema-enforced JSON documents](/docs/documents-explanation/), stored as a [typed graph of RDF triples](/docs/graphs-explanation/) |
| **Version control** | [Git-like](/docs/git-for-data-reference/) branch, diff, merge, clone, push and pull, plus [time-travel queries](/docs/knowledge-graph-database-version-control/) |
| **Transactions** | [ACID](/docs/acid-transactions-explanation/), on an [immutable storage layer](/docs/immutability-explanation/) |
| **Query languages** | [WOQL](/docs/woql-explanation/) (Datalog), [GraphQL](/docs/graphql-query-reference/), and the [HTTP document API](/docs/http-documents-api/) |
| **Search** | [Versioned semantic and hybrid search](/docs/versioned-search/) with the VectorLink indexer |
| **Clients** | Official: [JavaScript](/docs/use-the-javascript-client/) and [Python](/docs/use-the-python-client/). Community: [Rust](/docs/rust-client-quickstart/) and [Elixir](https://hex.pm/packages/terminusdb_client) |
| **License** | Open source, Apache License 2.0 ([GitHub](https://github.com/terminusdb/terminusdb)) |

Three things make it distinctive:

1. **Documents + Graph in one** — Write and read JSON documents; under the hood, data is stored as a typed graph of triples. No need to choose between document-store convenience and graph traversal power. See [The Document Model](/docs/documents-explanation/).

2. **Git-for-data** — Branch, merge, diff, reset, and time-travel through your data history. Push and pull between instances. See [Version Control](/docs/use-the-collaboration-features/).

3. **Datalog query engine (WOQL)** — A declarative query language based on Prolog foundations. Pattern-match across documents and relationships without JOINs, with built-in support for aggregation, path queries, and schema-aware traversal. See [WOQL Explained](/docs/woql-explanation/).

TerminusDB is maintained by [Gettier](https://gettier.ai) in collaboration with the open-source community.

## Overview of the system

* [A versatile knowledge graph database](/docs/terminusdb-explanation/)
* [flexible headless content management system](/docs/document-graph-api/)

### Document graph paradigm

* [model-based](/docs/schema-reference-guide/) and [structured JSON records](/docs/documents-explanation/)
* [git-for-data](/docs/git-for-data-reference/)
* [complex content](/docs/python-woql-customer-data-processing-example/)

### ACID RDF graph database with history and branches

* [RDF graph database with immutable history](/docs/graphs-explanation/)
* [graph database with ACID transactions](/docs/acid-transactions-explanation/)
* [full immutable history](/docs/immutability-explanation/)
* [advanced type model with inheritance](/docs/schema-reference-guide/)
* [taxonomies with both inherited fieldsets and values](/docs/cookbook-taxonomy-inheritance/)

### RDF semantics with a Closed World Assumption (rules engine)

* [collaborative semantic infrastructure](/docs/branch-howto/)
* [logic reasoning on information models](/docs/woql-basics/)
* [rules-engine for complex digital twin graphs](/docs/path-queries-in-woql/)

### Datalog query engine

* [logical reasoning](/docs/woql-explanation/)
* [immutable history](/docs/graphs-explanation/) built in
* [WOQL Abstract Syntax Tree (AST)](/docs/woql-class-reference-guide/)
* [how to query with WOQL](/docs/how-to-query-with-woql/)
* [Javascript WOQL Client](/docs/run-woql-query/)
* [Python WOQL Client](/docs/run-woql-query/)

### Data model and schema management

* [schema migration](/docs/schema-migration-reference-guide/)
* [XML Datatypes for its data structures](/docs/xsd-data-types/)
* [schema reference](/docs/schema-reference-guide/)

### Git-for-data

* [git-for-data (including push, pull clone and branching)](/docs/git-for-data-reference/)

### Storage engine

* [In-memory succinct data structures](/docs/terminusdb-explanation/) — documents decompose into content-addressed, immutable delta layers; all data persists to disk with ACID guarantees, while queries execute in-memory against the succinct representation (roughly 13 bytes per triple on large datasets)
* [Delta rollup](/docs/delta-rollup/) — periodic compaction that keeps the in-memory footprint bounded
* [Immutable storage layer](/docs/immutability-explanation/)

### Interoperability

* [REST (OpenAPI Reference)](/docs/openapi/)
* [WOQL datalog query language](/docs/woql-explanation/)
* [Rust-based high performance GraphQL Engine](/docs/graphql-query-reference/) 
* [Javascript Client](/docs/use-the-javascript-client/)
* [Python Client](/docs/use-the-python-client/)
* [Rust Client](/docs/rust-client-quickstart/) (community)
* [Elixir Client](https://hex.pm/packages/terminusdb_client) (community)

### Complete User Interface

The [TerminusDB Cloud](/docs/cloud/) offers a commercially packaged solution, with hosting API tokens and collaborative environments, modelling environment and document UI that help you go from idea to prototype and production in a few hours or a few days.
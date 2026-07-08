---
tags:
  - explanation
  - beginner
title: "TerminusDB: At a glance"
nextjs:
  metadata:
    title: "TerminusDB: At a glance"
    description: "Description of the key technical features and capabilities of TerminusDB at a glance with links to the documentation"
    keywords: terminusdb, document database, features overview, graph database, immutable database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/at-a-glance
---

This page gives you a high-level overview of TerminusDB's capabilities — what it does, how it differs from other databases, and where to go next depending on your use case.

**TerminusDB** is an open-source document graph database with built-in version control. It combines a [schema-enforced JSON document store](/docs/documents-explanation/) with a [knowledge graph](/docs/graphs-explanation/) engine, [git-like collaboration](/docs/git-for-data-reference/) (branch, diff, merge, clone), and [ACID transactions](/docs/acid-transactions-explanation/) backed by an [immutable storage layer](/docs/immutability-explanation/).

Three things make it distinctive:

1. **Documents + Graph in one** — Write and read JSON documents; under the hood, data is stored as a typed graph of triples. No need to choose between document-store convenience and graph traversal power. See [The Document Model](/docs/documents-explanation/).

2. **Git-for-data** — Branch, merge, diff, reset, and time-travel through your data history. Push and pull between instances. See [Version Control](/docs/use-the-collaboration-features/).

3. **Datalog query engine (WOQL)** — A declarative query language based on Prolog foundations. Pattern-match across documents and relationships without JOINs, with built-in support for aggregation, path queries, and schema-aware traversal. See [WOQL Explained](/docs/woql-explanation/).

TerminusDB is maintained by [DFRNT®](https://dfrnt.com?utm_source=terminusdb&utm_medium=referral&utm_campaign=terminusdb_docs_at_a_glance) in collaboration with the open-source community.

## Overview of the system

* [A versatile document graph database](/docs/terminusdb-explanation/)
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
* [XML Datatypes for its data structures](/docs/dfrnt-data-types/)
* [schema reference](/docs/schema-reference-guide/)

### Git-for-data

* [git-for-data (including push, pull clone and branching)](/docs/git-for-data-reference/)

### Interoperability

* [REST (OpenAPI Reference)](/docs/openapi/)
* [WOQL datalog query language](/docs/woql-explanation/)
* [Rust-based high performance GraphQL Engine](/docs/graphql-query-reference/) 
* [Javascript Client](/docs/use-the-javascript-client/)
* [Python Client](/docs/use-the-python-client/)

### Complete User Interface

The [DFRNT Studio for TerminusDB](https://dfrnt.com/hypergraph-content-studio/) offers a commercially packaged solution, with hosting API tokens and collaborative environments, modelling environment and document UI that help you go from idea to prototype and production in a few hours or a few days.
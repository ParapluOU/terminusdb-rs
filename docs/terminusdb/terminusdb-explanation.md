---
tags:
  - explanation
  - beginner
title: What is TerminusDB?
nextjs:
  metadata:
    title: What is TerminusDB?
    keywords: terminusdb, what is terminusdb, git for data, document database version control, immutable database, graph database, branch merge data, terminusdb vs postgresql
    description: TerminusDB is an open-source document graph database with built-in version control. Branch, diff, merge, and time-travel your data like Git — with ACID transactions.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/terminusdb-explanation/
media: []
---

TerminusDB is an open-source document graph database with built-in version control. It stores JSON documents in a schema-enforced graph, tracks every change as an immutable delta layer, and supports branch, diff, merge, clone, and time-travel operations — like Git, but for structured data rather than files.

TerminusDB is maintained by the team at [DFRNT](https://dfrnt.com) in collaboration with the open-source community. DFRNT assumed stewardship of the project in 2025.

## What makes TerminusDB different

Most databases overwrite data in place. TerminusDB never mutates existing data. Instead, every commit appends a new **delta layer** — an immutable record of what was added and what was removed. This single architectural choice enables a set of capabilities that are difficult or impossible to achieve with conventional databases:

- **Full history** — every commit is preserved. You can time-travel to any previous state instantly, without replaying logs or restoring backups.
- **Branch and merge** — create isolated branches of your data, make changes independently, then merge them back. This enables change-request workflows, staging environments, and collaborative editing.
- **Diff and patch** — compare any two states of a database to see exactly what changed, at the document or triple level.
- **Clone and sync** — replicate databases between instances and synchronise changes, enabling distributed data collaboration.
- **Lock-free concurrency** — because committed layers are immutable, readers never block writers and writers never block readers. No locks, no deadlocks.
- **ACID transactions** — every commit is atomic, consistent, isolated, and durable, with schema validation enforced on every write.

## Who TerminusDB is for

TerminusDB is designed for developers and data engineers who need:

- **Auditable data lineage** — regulated industries (finance, healthcare, compliance) where you must prove what changed, when, and by whom.
- **Complex domain models** — interconnected document structures with enforced schemas, where a flat relational model becomes unwieldy.
- **Collaborative data workflows** — teams that need branching, review, and merge workflows for data, not just code.
- **Content and knowledge management** — structured content that evolves over time and needs version history.

## How it works

TerminusDB stores data as **JSON documents** within a **graph structure**. A schema (written in JSON) defines document types, their properties, relationships, and constraints. The database enforces this schema on every transaction.

Under the hood, documents are decomposed into **RDF triples** stored in succinct data structures called delta layers. Each layer is content-addressed and immutable. When you query the database, TerminusDB traverses the layer stack to resolve the current state. Periodic [delta rollup](/docs/delta-rollup/) operations compress the stack to maintain read performance.

You interact with TerminusDB through:

- **HTTP API** — RESTful document and query endpoints
- **WOQL** — a Datalog-based query language for complex graph traversal and pattern matching
- **GraphQL** — standard GraphQL queries auto-generated from your schema
- **Client libraries** — official TypeScript/JavaScript, Python, and Rust SDKs

## How TerminusDB compares to alternatives

### vs PostgreSQL

PostgreSQL is a mature relational database with excellent tooling. Choose TerminusDB when you need first-class version control (branch, merge, time-travel) without bolting on triggers or temporal tables, or when your data model is naturally a graph of interconnected documents rather than flat rows and joins. PostgreSQL is better suited for applications that need relational analytics, mature ORMs, or a very large ecosystem of extensions.

### vs MongoDB

Both store JSON documents. TerminusDB adds:

- **Full version history** — every write is an immutable commit. You can query any past state, diff any two points in time, and branch the entire database — not just individual documents.
- **Built-in audit trail** — every commit records author, timestamp, and message. This is structural, not bolted on: you cannot accidentally bypass it. For compliance and regulated workloads, the audit log is tamper-evident by construction.
- **Schema validation by default** — documents are validated against a typed schema on every write. Schema evolution is versioned alongside the data.
- **Graph relationships** — documents link to other documents via typed edges. Traversal, path queries, and recursive graph operations are first-class.
- **HTTP-native API** — the entire interface (read, write, diff, branch, merge, clone, query) is a single HTTP API. No driver required: any HTTP client works out of the box.
- **Reasoning and inference** — WOQL supports Datalog-style reasoning: rules and constraints can be applied across the graph, and the query engine can infer new facts from existing data. This enables knowledge graph applications, ontology-based classification, and constraint checking that go beyond document retrieval.

MongoDB offers horizontal sharding, a larger ecosystem, more flexible (schemaless) operation, and operational tooling at scale. Choose TerminusDB when schema rigour, audit trails, data lineage, and collaboration workflows matter more than raw horizontal scale. The [Enterprise Edition](/docs/enterprise) adds further performance and compliance capabilities.

### vs Git / Git LFS

Git versions files (text or binary blobs). TerminusDB versions structured data at the field level — it understands document schemas, can diff semantically (not line-by-line), and supports queries across history. Git LFS handles large files; TerminusDB handles large datasets of interconnected documents with ACID guarantees.

TerminusDB also supports graph reasoning via WOQL — querying inferred relationships and applying rules across document history. Git has no equivalent: it stores files, not structured data with semantics.

### vs Neo4j

Both are graph databases. The key differences:

- **Document model vs property graph** — TerminusDB stores JSON-LD documents with typed edges; Neo4j uses a property graph (nodes and relationships with properties). TerminusDB's document model maps naturally to application data; Neo4j's property graph is more flexible for ad-hoc graph modelling.
- **Version control** — TerminusDB has built-in branch, diff, and merge on the entire graph. Neo4j has no native version control.
- **Closed-world semantics** — TerminusDB uses a closed-world assumption (CWA): if a fact is not in the database, it is false. This enables reliable constraint checking and schema validation. Neo4j uses open-world semantics by default.
- **HTTP-native** — TerminusDB's full API is HTTP; Neo4j primarily uses the Bolt protocol with language-specific drivers.
- **Reasoning** — TerminusDB's WOQL supports Datalog-style reasoning and rule inference. Neo4j's Cypher is a pattern-matching query language without native inference.
- **Audit trail** — TerminusDB's commit log provides a tamper-evident audit trail by construction. Neo4j requires external tooling for change tracking.

Choose TerminusDB when you need versioned graph data, audit trails, schema rigour, or HTTP-native integration. Choose Neo4j when you need a mature property graph ecosystem, advanced graph algorithms (GDS library), or Cypher's expressive pattern matching.

### vs Traditional RDF Triple Stores

TerminusDB is built on RDF and stores data as triples internally, but it takes a different philosophical stance from traditional triple stores:

- **Closed-world assumption** — Traditional triple stores (and SPARQL) use open-world semantics: the absence of a fact does not imply it is false. TerminusDB uses a closed-world assumption: the database is the complete description of what is true. This makes schema validation, constraint checking, and application development far more predictable.
- **Document interface over triple interface** — TerminusDB presents data as JSON-LD documents, not raw triples. Authors work with structured objects; the RDF layer is an implementation detail that ensures semantic interoperability.
- **Version control** — Traditional triple stores have no native version control. TerminusDB treats branching and merging as first-class operations.
- **WOQL vs SPARQL** — TerminusDB uses WOQL (a Datalog with unification) rather than SPARQL. WOQL is more expressive for recursive graph traversal and reasoning; SPARQL has a larger ecosystem and standardisation.
- **Developer experience** — TerminusDB's HTTP Document API requires no RDF tooling knowledge. Traditional triple stores typically require familiarity with RDF, OWL, and SPARQL.

Choose TerminusDB when you want RDF semantics and interoperability without the operational complexity of traditional triple stores, and when version control and developer ergonomics matter. Choose a traditional triple store when you need full OWL reasoning, SPARQL federation, or integration with the broader semantic web toolchain.

{% callout title="Ready to try it?" %}
**[Start the quickstart →](/docs/get-started/)** — branch, diff, and merge a database in 10 minutes. Requires only Docker.
{% /callout %}

## Frequently asked questions

### Is TerminusDB a graph database?

Yes. TerminusDB stores data as an RDF knowledge graph under the hood, but exposes it through JSON documents and schemas. You get the traversal power of a graph database with the developer experience of a document store.

### How does TerminusDB compare to PostgreSQL?

TerminusDB has native branching, merging, time-travel, and immutable history — features that require complex extensions or manual implementation in PostgreSQL. PostgreSQL has a larger ecosystem, relational analytics, and mature tooling. They solve different problems; some teams use both.

### Does TerminusDB have ACID transactions?

Yes. Every commit is atomic (all-or-nothing), consistent (schema-validated), isolated (snapshot-based reads), and durable (immutable layers on disk). See [ACID Transactions](/docs/acid-transactions-explanation/) for details.

### Can I use TerminusDB in production?

Yes. TerminusDB is used in production by organisations for financial reporting, content management, and data collaboration. DFRNT offers a managed cloud service ([DFRNT Hub](https://dfrnt.com)) for teams that prefer not to self-host.

### What is the difference between TerminusDB and Git?

Git versions files as binary blobs and diffs text line-by-line. TerminusDB versions structured data (JSON documents in a graph) and diffs at the field level with schema awareness. TerminusDB supports queries across the full history, ACID transactions, and schema validation — things Git was not designed for.

### What query languages does TerminusDB support?

TerminusDB supports [WOQL](/docs/woql-explanation/) (a Datalog-based language), [GraphQL](/docs/how-to-query-with-graphql/), and a RESTful [HTTP document API](/docs/http-documents-api/).

### Is TerminusDB open source?

Yes. TerminusDB is licensed under the Apache License 2.0. The source code is available on [GitHub](https://github.com/terminusdb/terminusdb).

### How does TerminusDB handle large datasets?

TerminusDB uses succinct data structures — compressed, content-addressed layers that operate fully in-memory with ACID persistence to disk. All data is persisted on disk, but queries execute in-memory against the succinct representation, making reads extremely fast. The succinct encoding is exceptionally space-efficient: benchmarks on billion-triple datasets show an average memory footprint of approximately 13 bytes per triple. Periodic [delta rollup](/docs/delta-rollup/) compresses accumulated layers to keep the in-memory footprint bounded.

## Further reading

- [Get Started](/docs/get-started/) — install TerminusDB and run your first queries
- [Documents & Schema](/docs/documents-explanation/) — how documents work in the graph
- [Immutability & Concurrency](/docs/immutability-and-concurrency/) — why the layer architecture avoids locks
- [WOQL Query Language](/docs/woql-explanation/) — the Datalog-based query language
- [Schema Reference](/docs/schema-reference-guide/) — types, constraints, and key strategies
- [Troubleshooting](/docs/troubleshooting-connection/) — fix common connection and auth errors

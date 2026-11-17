# TerminusDB Documentation Index

Mirrors the navigation of https://terminusdb.org/docs/. Each entry links to the flat
markdown file in this directory; release notes live in `releases/`. Regenerate with the
recipe in `README.md` after refreshing the mirror.


## Getting Started

- [Introduction](start-here.md)
- **Quickstart**
  - [Set up with an AI agent](ai-agent-setup.md)
  - [First 10 Minutes (clone)](get-started.md)
  - [First 15 Minutes (from scratch)](first-15-minutes.md)
  - [Git-like Version Control for JSON](version-controlled-json.md)
  - [Clone a Demo Database](clone-a-demo-project.md)
  - [Interactive Tutorials](https://terminusdb.org/docs/topics/tutorial) *(no markdown upstream — React-only page or external)*
  - [Explore a Real Dataset](explore-a-real-dataset.md)
  - [Explore an Ecommerce Dataset](explore-ecommerce-dataset.md)
  - [TypeScript Quickstart](connect-with-the-javascript-client.md)
  - [Python Quickstart](connect-with-python-client.md)
  - [Rust Quickstart](rust-client-quickstart.md)
- **Installation**
  - [Docker (Recommended)](install-terminusdb-as-a-docker-container.md)
  - [Advanced Docker Config](docker-advanced-configuration.md)
  - [Docker on Windows](install-terminusdb-docker-windows.md)
  - [Kubernetes](install-on-kubernetes.md)
  - [Build from Source](install-terminusdb-from-source-code.md)
  - [Self-Hosted (Production)](self-hosted-installation.md)
  - [Running in Production](run-terminusdb-in-production.md)
  - [Deploy on Azure](deploy-on-azure.md)
  - [Security](security.md)

## Your First Database

- [TerminusDB Cloud](cloud.md)
  - [Connect a Client](how-to-connect-terminusdb-cloud.md)
- [Browse by Topic](https://terminusdb.org/docs/topics) *(no markdown upstream — React-only page or external)*
- [What's New](https://terminusdb.org/docs/whats-new) *(no markdown upstream — React-only page or external)*
- [Releases](releases.md)
  - [12.0 Changelog](releases/12-0-changelog.md)

## How TerminusDB Works

- [What is TerminusDB?](terminusdb-explanation.md)
- [At a Glance](at-a-glance.md)
- [Who Maintains TerminusDB](about-gettier.md)
- [Knowledge Graph Database](knowledge-graph-database.md)
- [Knowledge Graph for AI Agents](knowledge-graph-for-ai-agents.md)
- **Documents & Schema**
  - [The Document Model](documents-explanation.md)
  - [Document Types Compared](document-types-comparison.md)
  - [Document Type Inference](https://terminusdb.org/docs/document-type-inference) *(no markdown upstream — React-only page or external)*
  - [Schema Weakening](what-is-schema-weakening.md)
  - [IRIs, Prefixes & Minting](iris-prefixes-and-minting.md)
- **Git-like Version Control**
  - [Overview](knowledge-graph-database-version-control.md)
  - [Knowledge Graph Versioning](knowledge-graph-versioning.md)
  - [Knowledge Graph Diff](knowledge-graph-diff.md)
  - [RDF Versioning](rdf-versioning.md)
  - [Versioned Graph Database](graph-database-with-git-like-version-control.md)
  - [Time Travel Queries](graph-database-with-time-travel-queries.md)
  - [Clone, Push, Pull](use-the-collaboration-features.md)
  - [Merge How-To](merge-howto.md)
  - [Time-Travel How-To](time-travel-howto.md)
  - [Undo & Reset How-To](undo-reset-howto.md)
  - [Version JSON Data](how-to-version-json-data.md)
  - [Diff & Patch](diff-and-patch-operations.md)
  - [Recover Data](recovery-tutorial.md)
  - [Audit Changes](audit-tutorial.md)
  - [Operations Reference](version-control-operations.md)
- **Query Languages**
  - [Choosing a Query Interface](querying-terminusdb.md)
  - [WOQL Overview](woql-explanation.md)
  - [WOQL Architecture & Security](woql-architecture.md)
  - [What is Datalog?](what-is-datalog.md)
  - [What is Unification?](what-is-unification.md)
  - [Backtracking & Streaming](woql-query-streaming.md)
- **Architecture**
  - [ACID Transactions](acid-transactions-explanation.md)
  - [ACID Verification (ALICE + Jepsen)](https://terminusdb.org/docs/acid-verification-alice-jepsen) *(no markdown upstream — React-only page or external)*
  - [Open-Source ACID Graph DB](open-source-graph-database-with-acid-transactions.md)
  - [Immutability](immutability-explanation.md)
  - [Immutability & Concurrency](immutability-and-concurrency.md)
  - [Graphs](graphs-explanation.md)
- **Use Cases**
  - [Customer Data Processing](python-woql-customer-data-processing-example.md)
  - [Taxonomy Inheritance](cookbook-taxonomy-inheritance.md)
  - [Segmented Environments](operational-technologies-transfer.md)
- **Comparisons**
  - [TerminusDB vs Neo4j](terminusdb-vs-neo4j.md)
  - [TerminusDB vs MongoDB](terminusdb-vs-mongodb.md)
  - [Migrate from MongoDB](https://terminusdb.org/docs/migrate-from-mongodb) *(no markdown upstream — React-only page or external)*

## Build with TerminusDB

- **Client SDKs**
  - [Install](install-terminusdb-js-client.md)
  - [Create Database](create-a-database.md)
  - **Documents (CRUD)**
    - [Add Documents](add-a-document.md)
    - [Get Documents](get-documents.md)
    - [Edit Documents](edit-a-document.md)
    - [Delete Documents](delete-a-document.md)
    - [Query Documents](query-documents.md)
  - **Schema**
    - [Add a Schema](add-a-schema.md)
    - [Schema CRUD Operations](schema-crud-operations.md)
  - [Run WOQL Queries](run-woql-query.md)
  - **Collaboration**
    - [Clone](clone-a-project.md)
    - [Pull](pull-from-project.md)
    - [Push](push-to-project.md)
    - [Branch](branch-howto.md)
    - [Reset](reset-a-project.md)
    - [Squash](squash-projects.md)
- [Using the Clients](use-the-clients.md)
  - [JavaScript Client](use-the-javascript-client.md)
  - [TypeScript Client](typescript-with-the-javascript-client.md)
  - [JS Client Typedefs](javascript-client-typedefs.md)
  - [Python Client](use-the-python-client.md)
  - [JS Collaboration](collaboration-with-javascript-client.md)
  - [Python Collaboration](collaboration-with-python-client.md)
  - [MCP (AI Agents)](https://terminusdb.org/docs/use-terminusdb-mcp) *(no markdown upstream — React-only page or external)*
- **Python Ecosystem**
  - [Import Data (CSV, JSON, Pandas)](import-data-with-python-client.md)
  - [Pydantic Quickstart](pydantic-quickstart.md)
  - [Pydantic Schema Management](pydantic-schema-management.md)
  - [Pydantic Schema Reference](pydantic-schema-reference.md)
  - [Certificate Issues](python-certificate-issues.md)

## Guides

- [WOQL Query Language](how-to-query-with-woql.md)
  - **Getting Started with WOQL**
    - [Interactive Tutorial](woql-tutorial.md)
    - [Bike Share Tutorial](https://terminusdb.org/docs/bike-tutorial) *(no markdown upstream — React-only page or external)*
    - [WOQL Basics](woql-basics.md)
    - [Getting Started](woql-getting-started.md)
    - [Variables](woql-variable-generation.md)
  - **Document Operations**
    - [Add Documents](add-documents-with-woql.md)
    - [Read Documents](read-documents-with-woql.md)
    - [Edit Documents](edit-documents-with-woql.md)
    - [Delete Documents](delete-documents-with-woql.md)
    - [Subdocument Handling](woql-subdocument-handling.md)
  - **Filtering & Sorting**
    - [Filtering](filter-with-woql.md)
    - [Sorting (Order By)](order-by-with-woql.md)
    - [Grouping Results](group-query-results.md)
  - **Advanced Queries**
    - [Path Queries](path-queries-in-woql.md)
    - [Math Operations](maths-based-queries-in-woql.md)
    - [Schema Queries](schema-queries-with-woql.md)
    - [JSON-LD Queries](woql-json-ld-queries.md)
    - [Arrays & Sets](query-arrays-and-sets-in-woql.md)
    - [Set Operations](woql-set-operations.md)
    - [Query Multiple Graphs](datalog-queries-between-data-products.md)
  - [Time & Date Processing](time-processing.md)
    - [1. Dates & Range Queries](time-tutorial-dates.md)
    - [2. Durations & Sequences](time-tutorial-durations.md)
    - [3. Intervals & Allen's Algebra](time-tutorial-intervals.md)
    - [4. Creative Patterns](time-tutorial-patterns.md)
    - [Time Predicate Reference](woql-time-handling.md)
    - [Allen's Interval Algebra](woql-interval-algebra.md)
    - [EOM Preservation Rules](woql-eom-rules.md)
  - **Patterns & Recipes**
    - [Common Patterns](woql-common-patterns.md)
    - [Working with Data](woql-data-handling.md)
    - [Datatypes](cookbook-woql-type-of-datatype.md)
    - [Pattern Generation](pattern-generation-cookbook.md)
    - [Array Matching](cookbook-woql-arrays.md)
    - [Tips and Tricks](woql-tips-and-tricks-1.md)
  - **Advanced Topics**
    - [Control Flow & Performance](woql-control-flow.md)
    - [Range Queries (triple_slice)](woql-triple-slice.md)
    - [RDF List Operations](woql-rdflist-operations.md)
      - [Creation](woql-rdflist-creation.md)
      - [Access](woql-rdflist-access.md)
      - [Modification](woql-rdflist-modification.md)
      - [Transformation](woql-rdflist-transformation.md)
      - [Queue Tutorial](woql-rdflist-queue-tutorial.md)
- [GraphQL Queries](how-to-query-with-graphql.md)
  - [Basics](graphql-basics.md)
  - [Filtering](filter-with-graphql.md)
  - [Advanced Filtering](advanced-filtering-with-graphql.md)
  - [Pagination (Limit/Offset)](limit-results-in-graphql.md)
    - [Offset for Paging](offset-to-provide-paging.md)
  - [Sorting (Order By)](order-by-in-graphql.md)
  - [Path Queries](path-queries-in-graphql.md)
  - [Back Links](back-links-in-graphql.md)
  - [IDs and RDF IRIs](graphql-with-rdf-iri.md)
- **Data Import & Export**
  - [Extract Table from JSON](extract-table-from-json-with-woql.md)
  - [Sync CSV files](compare-csv-values-with-woql.md)
  - [Import JSON-LD](import-jsonld-woql.md)
- [Browser CORS](browser-cors-howto.md)
- **Document Interface**
  - [HTTP API](http-documents-api.md)
  - [Document Graph API](document-graph-api.md)
  - [Insertion API](document-insertion.md)
  - [Set Commit Messages](commit-message-howto.md)
  - [Language Strings](language-strings-with-curl.md)
- **User & Access Management**
  - [Managing Users and Invitations](managing-users-and-invitations.md)
  - [CLI Access Control](access-control-cli.md)
- **Database Maintenance**
  - [Perform a Delta Rollup](delta-rollup.md)

## Reference

- [Javascript API](https://terminusdb.org/docs/javascript/) *(no markdown upstream — React-only page or external)*
- [Python API](https://terminusdb.org/docs/python/) *(no markdown upstream — React-only page or external)*
- [HTTP API (OpenAPI)](https://terminusdb.org/docs/openapi) *(no markdown upstream — React-only page or external)*
- [Schema Reference](schema-reference-guide.md)
  - [Data Types](xsd-data-types.md)
  - [A Tour of All of Time](https://terminusdb.org/docs/xdd-datetimeinterval-time-tour) *(no markdown upstream — React-only page or external)*
  - [QName and SQName](qname-sqname-data-types.md)
  - [WOQL Reference](woql-class-reference-guide.md)
  - [Path Queries](path-query-reference-guide.md)
  - [Document Unfolding](document-unfolding-reference.md)
  - [Schema Migration](schema-migration-reference-guide.md)
  - [Numeric Precision](numeric-precision-reference.md)
  - [GRAPH_SPEC, DB_SPEC](graph-spec-db-spec-database-path-identifiers.md)
- [GraphQL Reference](graphql-query-reference.md)
  - [Connecting](connecting-to-graphql-reference.md)
  - [Javascript GraphQL](connect-with-apollo-client.md)
  - [Mutations](graphql-mutations.md)
  - [Naming Conventions](graphql-naming-conventions-reference.md)
  - [System Graph Interface](system-graph-graphql-interface-reference.md)
  - [Querying Layers](querying-layers-with-graphql.md)
- [CLI Reference](terminusdb-cli-commands.md)
  - [CLI Querying](terminusdb-db-cli-querying.md)
- [Access Control](access-control.md)
  - [Tutorial](access-control-tutorial.md)
  - [Name Mode vs ID Mode](capabilities-api-modes.md)
  - [JavaScript Client](access-control-with-javascript.md)
  - [JS API Reference](js-access-control.md)
  - [Source Code Tutorial](access-control-tutorial-source.md)
- [Git-for-Data Reference](git-for-data-reference.md)
  - [Reverse Branch Cloning](manual-reverse-branch-cloning.md)
- [JSON Diff & Patch](json-diff-and-patch.md)
  - [Patch Endpoint](patch-endpoint.md)
- [Prefix Management](prefix-management.md)
- [Cloud Dashboard Reference](terminusdb-cms-dashboard-reference.md)
  - [Model Builder UI](use-the-model-builder-ui.md)
  - [JSON Schema Editor](use-the-json-editor.md)
  - [Model Schema](model-schema.md)
  - [Branch](branch.md)
  - [Clone](clone.md)
  - [Reset](reset.md)
  - [Squash](squash.md)
  - [Time Travel](time-travel.md)
  - [Change Requests](change-request-workflows.md)
- [VectorLink](set-up-vectorlink.md)
  - [How to Use VectorLink](use-vectorlink.md)
  - [OpenAI & Handlebars Config](openai-handlebars-config.md)
  - [Index Your Data](index-your-data.md)
- [TerminusDB Internals](terminusdb-internals.md)
  - [sys:JSON Deep Dive](terminusdb-internals-sysjson.md)
  - [Writing Plugins](writing-plugins.md)
  - [Dashboard Component](dashboard.md)
- **UI SDK**
  - [Document UI SDK](document-ui-sdk.md)
    - [Data Types](document-ui-sdk-data-types.md)
    - [GeoJSON](ui-sdk-geojson.md)
  - **Data Types**
    - [Array](array.md)
    - [List](list.md)
    - [Set](set.md)
    - [Optional](optional.md)
    - [Mandatory](mandatory.md)
    - [OneOf (@oneof)](oneof.md)
    - [sys:JSON](sysjson.md)
    - [@metadata orderBy](orderby.md)
    - [@metadata renderAs](render-as.md)
  - [Document UI Template](document-ui-template.md)
    - [TDB React Table](tdb-react-table.md)
    - [Documents GraphQL Table](documentsgraphqltable.md)
    - [Edit Document](edit-document-component.md)
    - [List Documents](list-documents-component.md)
    - [New Document](newdocumentcomponent.md)
- [Glossary](glossary.md)
- [Legacy: TerminusCMS and DFRNT Hub](legacy.md)

## Next Release: 12.1

- [Versioned Search](versioned-search.md)
  - [Concepts](versioned-search-concepts.md)
  - [Quickstart](versioned-search-quickstart.md)
  - [Embedded Engine](versioned-search-embedded.md)
  - [How Automatic Indexing Works](versioned-search-automatic-indexing.md)
  - [Indexing by Push](versioned-search-indexing.md)
  - [Embeddings Field Guide](versioned-search-embeddings.md)
  - [Searching](versioned-search-querying.md)
  - [Clustering Embeddings](versioned-search-clustering.md)
  - [History & Branching](versioned-search-history.md)
  - [Entity Resolution](versioned-search-entity-resolution.md)
  - [API Reference](versioned-search-api-reference.md)
- [MCP Server](https://terminusdb.org/docs/terminusdb-mcp) *(no markdown upstream — React-only page or external)*
  - [Tool Reference](https://terminusdb.org/docs/terminusdb-mcp-reference) *(no markdown upstream — React-only page or external)*
- [GraphQL Subscriptions](graphql-subscriptions.md)
- [Admin Console](admin-console.md)
- [The Write Path](terminusdb-write-path.md)

## Enterprise

- [Overview](enterprise.md)
- [Document Formats](enterprise-document-formats.md)
  - [JSON-LD Context](enterprise-jsonld-context.md)
  - [RDF/XML Support](enterprise-rdfxml.md)
  - [Turtle Support](enterprise-turtle.md)
  - [curl Tutorial](document-format-api-curl-tutorial.md)
- [Context Cache](enterprise-context-cache.md)
- [Backup & Restore](enterprise-backup-restore.md)
- [Observability](enterprise-observability.md)
- [Configuration](enterprise-configuration.md)

## Troubleshooting

- [Connection Failures](troubleshooting-connection.md)
- [Authentication Errors](troubleshooting-auth.md)
- [Schema Validation Errors](troubleshooting-schema.md)
- [Query Errors](troubleshooting-queries.md)
- [Data Model Mismatches](troubleshooting-data-model.md)
- **Migration Issues**
  - [Document ID Migration](troubleshooting-document-id-migration.md)

## Blog

- [All Posts](https://terminusdb.org/blog) *(no markdown upstream — React-only page or external)*

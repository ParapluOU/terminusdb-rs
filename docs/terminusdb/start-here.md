---
tags:
  - explanation
  - beginner
title: TerminusDB Documentation
nextjs:
  metadata:
    title: TerminusDB Documentation — Getting Started, Guides, and API Reference
    keywords: terminusdb, terminusdb documentation, git for data, document graph database, version control database, immutable database, datalog engine, branch merge data, terminusdb quickstart
    description: TerminusDB is an open-source document graph database with built-in version control. Branch, diff, and merge your data like code. Start in 10 minutes with Docker.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/start-here/
media: []
---

TerminusDB is an open-source document graph database with built-in version control.
Branch, diff, and merge your data like code.

**TerminusDB** is the open-source database engine you run yourself. **[DFRNT Hub](https://dfrnt.com/hypergraph-content-studio/)** is a managed cloud platform built on TerminusDB — no installation needed.

## Start here

Choose the path that matches where you are:

| Goal | Page | Time | What you need |
|------|------|------|---------------|
| Quickest possible start | [JSON Version Control in 5 Minutes](/docs/version-controlled-json/) | 5 min | Docker |
| See it work immediately | [Your First 10 Minutes](/docs/get-started/) **Recommended first →** | 10 min | Docker |
| Build from scratch (no clone) | [Your First 15 Minutes](/docs/first-15-minutes/) | 15 min | Docker |
| Learn the query language | [Learn WOQL: Interactive Tutorial](/docs/woql-tutorial/) | 20 min | Docker |
| Explore & query a dataset | [Explore a Real Dataset](/docs/explore-a-real-dataset/) | 15 min | Docker |
| Try a business scenario | [Explore an Ecommerce Dataset](/docs/explore-ecommerce-dataset/) | 15 min | Docker |

{% prerequisites /%}

## Choose your language

All quickstarts teach the same git-for-data workflow (branch, diff, merge). Choose the one matching your preferred language or environment:

| Environment | Quickstart |
|-------------|------------|
| **HTTP / curl** | Already covered in the tutorials above |
| **TypeScript / JavaScript** | [TypeScript Quickstart](/docs/connect-with-the-javascript-client/) |
| **Python** | [Python Quickstart](/docs/connect-with-python-client/) |
| **Rust** | [Rust Quickstart](/docs/rust-client-quickstart/) |
| **Cloud (no install)** | [Connect to DFRNT Hub](/docs/how-to-connect-dfrnt-hub/) |

Not sure which query language to use after connecting? See [Choosing a Query Interface](/docs/querying-terminusdb/) — WOQL, GraphQL, or HTTP API.

## Understand the concepts

- [How TerminusDB Works](/docs/terminusdb-explanation/) — Architecture and mental model
- [Documents & Schema](/docs/documents-explanation/) — The data model
- [Version Control for Data](/docs/git-for-data-reference/) — Branching, diffing, merging

## Find something specific

- [HTTP API Reference](/docs/openapi) — All endpoints
- [Schema Reference](/docs/schema-reference-guide/) — Types and constraints
- [JSON Diff & Patch](/docs/json-diff-and-patch/) — Structural diff specification
- [Troubleshooting](/docs/troubleshooting-connection/) — Fix common errors

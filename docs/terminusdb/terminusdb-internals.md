---
tags:
  - explanation
  - documents
  - rdf
  - advanced
title: TerminusDB Internals
nextjs:
  metadata:
    title: TerminusDB Internals
    description: Examples to use the internals of TerminusDB from the command line.
    keywords: terminusdb, document database, documents, json-ld, terminusdb internals
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/terminusdb-internals/
media: []
---

TerminusDB is built around a **datalog engine** written in Prolog with a very fast and memory-efficient immutable graph storage layer optimised for version control. This page documents the internal components that make the database work.

## sys:JSON Storage

TerminusDB provides the `sys:JSON` type for storing arbitrary JSON data with automatic content-addressed deduplication. JSON values are identified by SHA-1 hash, allowing multiple documents to safely share identical JSON structures without duplication or consistency issues.

The implementation uses copy-on-write semantics and reference counting to ensure safe concurrent operations. When you update or delete documents with `sys:JSON` fields, TerminusDB tracks which JSON nodes are still referenced and only removes unused nodes when their reference count reaches zero.

For detailed information on storage architecture, GraphQL behavior, supported types, and best practices, see the [sys:JSON Internals Guide](/docs/terminusdb-internals-sysjson).

## Document Unfolding

 TerminusDB provides automatic document unfolding for classes marked with the `@unfoldable` schema annotation. When retrieving documents through the Document API, GraphQL, or WOQL, referenced documents are automatically expanded inline instead of returning just ID references. This creates a seamless navigation experience for hierarchical and graph-structured data.

The unfolding implementation uses an optimized path stack for cycle detection, preventing infinite recursion when documents reference themselves directly or indirectly. When a cycle is detected, the system returns an ID reference instead of expanding the document again, ensuring all nodes are rendered without crashes. The `TERMINUSDB_DOC_WORK_LIMIT` environment variable (default: 500,000 operations) provides additional protection against excessive resource consumption during deep traversals of complex document graphs.

For detailed information on cycle detection behavior, performance characteristics, and best practices, see the [Document Unfolding Reference](/docs/document-unfolding-reference).

## TerminusDB Server

### Command line Prolog query interface

The TerminusDB server provides a command line datalog query interface. It has a distinct syntax that is similar to the WOQL syntax, and more aligned to the internals of TerminusDB. It offers a more direct way to query the database.

The query interface is available as `./terminusdb query`. Variables are dynamically bound using the `v/1` predicate, see below. To get the responses as JSON, use the `--json` flag.

```bash
./terminusdb query admin/sandbox/local/branch/main --json 't(v(a),v(b),v(c))'
```

### Start in interactive mode

To start the server in prolog interactive mode, use the `serve --interactive` flag. It will start the server on port 6363 and it enables runtime introspection and to change behaviour of the runtime engine. To be able to run commands, you need to initialize the environment and to load a specific database to operate on. This is beyond the scope of this tutorial.

```bash
./terminusdb serve --interactive
```
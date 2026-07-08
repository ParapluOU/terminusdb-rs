---
tags:
  - explanation
  - documents
  - beginner
title: Document Graph API Howto
nextjs:
  metadata:
    title: Document Graph API Howto
    description: A howto guide for the TerminusDB document API interface.
    keywords: terminusdb, api, document, document database, document graph api howto, documents, json-ld
    alternates:
      canonical: https://terminusdb.org/docs/document-graph-api/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
media: []
---

This page explains how TerminusDB's document interface combines the convenience of a JSON document store with the traversal power of a knowledge graph. You write and read familiar JSON objects; the database stores them as a typed, schema-enforced graph — giving you both document simplicity and graph relationships in one system.

Documents in TerminusDB follow a strict schema which makes document modelling both precise and easy. The JSON syntax is extended with types and identifiers for precise handling of both top-level documents and subdocuments.

## Documents offers the best of graph, documents and relations

The document interface implements an opionionated subset of the JSON-LD standard, with automatic ID generation and schemaful handling of documents and subdocuments, to form a special-purpose database for closed world assumption knowledge graph processesing. 

The document interface consists of two endpoints. The first endpoint, `document`, is how we get documents into and out of TerminusDB. Since schemas consist of documents too, this is also how you'd update the schema, they use the same semantics.

The second endpoint, `schema`, is how we can easily get schema information out of TerminusDB. While technically it is possible to get all schema information through the document interface, the schema interface is more convenient for this purpose, as it takes class inheritance into account to give a complete image of all the properties that are usable on a certain class.

## Using the document API

The easiest way to get started with the document API is to use the TerminusDB client libraries, which provide a high-level interface to the document API:

* [Javascript](/docs/use-the-javascript-client/)
* [Python](/docs/use-the-python-client/)

## Further Reading

* [Document Insertion Reference Guide](/docs/document-insertion/)
* [Documents in a knowledge graph and how to use them](/docs/documents-explanation/)
* [Immutability, version history, and revision control](/docs/immutability-explanation/)
* [GraphQL API](/docs/how-to-query-with-graphql/)

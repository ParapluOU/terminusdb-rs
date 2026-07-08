---
tags:
  - explanation
  - knowledge-graph
  - schema
  - beginner
title: The Open Source Knowledge Graph Database
nextjs:
  metadata:
    title: The Open Source Knowledge Graph Database
    description: An instructional overview of TerminusDB as an open source knowledge graph database — a NoSQL graph database built on the closed world assumption of RDF, with an immutable history, ACID transactions, and a document-shaped class system that keeps related triples on a shared lifecycle.
    keywords: open source knowledge graph database, knowledge graph database, NoSQL graph database, RDF database, closed world assumption, open world assumption, immutable database, ACID transactions, triple store, document graph database, subdocument, shared document, cascading delete, schema-enforced graph, JSON-LD database, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/knowledge-graph-database/
media: []
---

TerminusDB is an **open source knowledge graph database**: a NoSQL graph database that stores information as [RDF triples](/docs/graphs-explanation/), enforces a schema over those triples, and gives every change an [immutable](/docs/immutability-explanation/), [ACID-transactional](/docs/acid-transactions-explanation/) history. It is designed for the case that traditional triple stores and traditional databases each leave open — storing **complex, consistent, version-controlled information records** at graph scale.

This page introduces the foundations: what a knowledge graph database is, how the closed world assumption changes how data behaves, and how the TerminusDB class system — including `@subdocument` and `@shared` — lets you model rich documents on top of a triple-level store without losing referential integrity.

## What is a knowledge graph database?

A **knowledge graph database** is a graph database that does three things at the same time:

1. **Stores facts as a graph** — typed connections between entities, in TerminusDB's case as RDF subject–predicate–object triples.
2. **Enforces a schema** — every entity belongs to a class, every class declares which properties it can have, and the database refuses writes that violate the model.
3. **Treats related triples as a single information record** — the triples that describe one entity share a lifecycle, so they are created, updated, and deleted together.

That third property is what separates a knowledge graph database from a pure triple store, and what makes it usable as a system of record rather than only a search index.

## The two ends of the spectrum it sits between

Most databases sit at one of two extremes. A knowledge graph database is the middle ground that takes the strengths of each.

### Traditional triple stores: triples without consistency

Classical RDF triple stores accept any triple you give them. They operate under the **open world assumption** — the idea, inherited from formal semantics on the web, that anything not stated may still be true somewhere else. That is excellent for federating public data, but it is a poor fit for an operational system, because:

- A document with missing fields is *not* invalid — it is just incomplete.
- A class with no schema is *not* an error — it is just under-described.
- Deleting one triple does not delete the rest of the entity it described — they survive as orphans.
- Two contradicting statements both stand, because closing them out would require knowing the universe.

You can store triples this way, but you cannot trust the result as a record.

### Traditional databases: consistency without complex records

Relational and document databases enforce strong consistency, but make complex, deeply linked records expensive. Modelling a contract that has parties, clauses, schedules, signatures, and amendments — each with their own sub-entities and cross-references — typically means a sprawl of join tables, denormalised JSON blobs, or both. Migrations are painful, audit history is bolted on, and graph traversals are awkward.

You can keep things consistent this way, but the structures you actually want to store fight the model.

### The knowledge graph database: both

TerminusDB takes the **closed world assumption** instead: if a fact is not in the database, it is not true. A property that the schema declares as mandatory must be present, or the transaction is rejected. A reference must point to a real entity, or the transaction is rejected.

A class can be checked exhaustively, because the database knows it has seen everything there is to see about it. That gives you the consistency guarantees of a relational system on top of the expressive structure of a graph.

Layered on top, [**immutable history**](/docs/immutability-explanation/) means past states are never destroyed, and [**ACID transactions**](/docs/acid-transactions-explanation/) mean each commit is atomic, consistent, isolated, and durable. The result is an information store where accuracy and provenance are properties of the database, not of a discipline you have to enforce in application code.

## NoSQL, but schema-enforced

TerminusDB is a **NoSQL graph database** — there are no tables, no fixed row width, and no SQL — but it is not schemaless. Schema is expressed as documents that describe classes, properties, types, and relationships, and is itself stored as triples in the graph.

You get the flexibility of a document database (nested structures, lists, optionals, unions) with the integrity of a typed schema, and you can [evolve the schema](/docs/schema-migration-reference-guide/) with the same version-controlled commits that change the data.

## From tables to class frames

If you come from a relational background, the easiest way into the model is to think of a class as a **frame** — a term borrowed from frame theory in cognitive science, where a frame is a structured template for a kind of thing, with named slots that get filled in for each instance. A class frame in TerminusDB plays roughly the role a table plays in a relational schema: it names the kind of thing, lists the slots (properties), and constrains their types. An instance of the class is, by analogy, like a row — a particular filling-in of those slots.

The leap is what happens *between* frames. In a relational schema, a row is confined to its table; anything more complex than a single record has to be reconstructed by joining across tables on foreign keys, with the application or the query stitching the result back together from the client's perspective.

In a knowledge graph database, a **document is not confined to one frame**. A document instance is filled out against the `Document` frame, but its clauses are filled out against the `Clause` frame, its parties against `Party`, its amendments against `Amendment` — and all of those triples are part of the same document, on the same lifecycle, retrievable as one nested structure.

The frame describes the *shape* of one slice; the document is the *whole structure* that spans many frames.

That is what `@subdocument` and `@shared` are for. They tell the database which of those cross-frame links are part of a single document's lifecycle and which point out to independently-living entities. The frames stay clean and reusable, the documents stay whole, and the joins that you would have written in SQL are already implicit in the graph.

## The class system

Every entity in TerminusDB is an instance of a [class](/docs/schema-reference-guide/). A class declares its properties, each with a type — a primitive like `xsd:string`, a [collection](/docs/dfrnt-data-types/) like `Set` or `List`, an [optional](/docs/optional/) or [mandatory](/docs/mandatory/) field, a [`@oneof`](/docs/oneof/) choice, or a link to another class.

Inheritance is supported, so common properties can live on a parent class. Because the schema is itself a graph, you can [query the schema with WOQL](/docs/schema-queries-with-woql/) the same way you query the data.

## `@subdocument` and `@shared`: the lifecycle annotations

The two annotations that make complex documents practical in a triple store are `@subdocument` and `@shared`. They answer a single question for every linked entity: **does this thing have its own lifecycle, or does it live and die with its parent?**

### `@subdocument` — composition

A class marked `@subdocument` is **owned** by its parent. Its triples are part of the parent's information record. You cannot reference it from elsewhere; it has no independent identity outside the parent; and when the parent is deleted, the subdocument is deleted with it.

That is how a contract owns its clauses, an order owns its line items, an invoice owns its tax breakdown.

The practical consequence is that **cascading deletes are trivial**. There is no `ON DELETE CASCADE` to remember to add, no application-side cleanup, no orphaned rows. Deleting the root deletes the whole tree, by construction, because the subdocument triples are part of the parent's lifecycle. See [**The Document Model**](/docs/documents-explanation/) and [**Choice Subdocuments**](/docs/choice-subdocuments/) for the full mechanics.

### `@shared` — reference-counted lifecycle

A class marked `@shared` has its own identity and can exist on its own, exactly like a regular document. What makes it different is **reference-counted delete semantics**: while at least one document still links to it, the shared document stays alive; the moment the last reference is removed, it is cleaned up automatically.

That is how a footnote lives as long as some article cites it, how an address lives as long as some order ships to it, how a tag lives as long as something is tagged with it.

The bookkeeping happens at commit time and counts every inbound reference, no matter which parent, class, or field it comes from. Cascades are recursive — a reaped shared document can itself release the last reference to another, and even circular islands of mutually-referencing shared documents are collected together once nothing outside the island points in.

The practical consequence is that **shared entities never become orphans**, and you never have to remember to delete them. The graph stays free of dangling value-like objects by construction, the same way `@subdocument` keeps deeply nested records from being half-deleted.

See [**Shared documents in Document Types Compared**](/docs/document-types-comparison/#shared-documents-shared) and the [**`@shared` section of the schema reference**](/docs/schema-reference-guide/) for the full semantics, including a worked lifecycle example.

### Why this matters

With these two annotations, the same store handles both worlds at once: **deeply nested documents** that behave like JSON, and **richly linked entities** that behave like a graph — with the database itself, not the application, responsible for getting the lifecycle right.

The triples that compose one record share that record's fate (`@subdocument`); the triples that describe a shared entity live as long as anything references them and are reaped the moment nothing does (`@shared`).

Either way, the database — not your application code — is the thing that keeps the graph free of orphans. That is what we mean by a knowledge graph database where **triples share a lifecycle**.

For a deeper comparison of the document shapes available, see [**Document Types Compared**](/docs/document-types-comparison/).

## What this combination gives you

Putting the pieces together, TerminusDB is an **open source, immutable, ACID-transactional, schema-enforced knowledge graph database** in which:

- Information is stored as RDF triples, accessible through the [**REST JSON linked-data document interface**](/docs/http-documents-api/) and queryable with [**WOQL**](/docs/woql-explanation/) and [**GraphQL**](/docs/graphql-basics/). Additional [**document formats**](/docs/enterprise-document-formats/) — [JSON-LD](/docs/enterprise-jsonld-context/), [RDF/XML](/docs/enterprise-rdfxml/), and [Turtle](/docs/enterprise-turtle/) — are available for interoperability with the wider semantic-web ecosystem.
- The closed world assumption makes consistency provable, not aspirational.
- The class system, with `@subdocument` and `@shared`, lets complex records exist as first-class citizens.
- Every change is a [**versioned commit**](/docs/knowledge-graph-version-control/) you can branch, merge, diff, audit, and roll back.
- The history is [**immutable**](/docs/immutability-and-concurrency/), which is what makes lock-free concurrency and time-travel queries possible at the same time.

## Where to go next

- [**What is TerminusDB?**](/docs/terminusdb-explanation/) — the product-level introduction
- [**The Document Model**](/docs/documents-explanation/) — how json documents map onto triples
- [**Schema Reference**](/docs/schema-reference-guide/) — the full class system
- [**Document Types Compared**](/docs/document-types-comparison/) — when to use subdocument, shared, and ordinary documents
- [**Immutability**](/docs/immutability-explanation/) and [**ACID Transactions**](/docs/acid-transactions-explanation/) — the transactional guarantees
- [**Knowledge Graph Database with Git-like Version Control**](/docs/knowledge-graph-version-control/) — the version-control model on top
- [**Graphs explained**](/docs/graphs-explanation/) — the underlying triple architecture

If you want to see all of this in action on a real dataset, the [**explore a real dataset quickstart**](/docs/explore-a-real-dataset/) walks through a schema-enforced knowledge graph end to end.

---
tags:
  - reference
  - beginner
title: Glossary of Terms for TerminusDB
nextjs:
  metadata:
    title: Glossary of Terms for TerminusDB
    description: A glossary of terms for TerminusDB to ensure terms are clearly defined and understood in the same way across the community to enable better conversations
    keywords: terminusdb, glossary of terms for terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/glossary/
media: []
---

This glossary defines key terms used throughout the TerminusDB documentation — from graph database concepts to version-control operations. Use it when you encounter an unfamiliar term in the docs.

> **Context:** Unless otherwise stated, the context for all definitions is computer, data, or information science.
> 
> **Source:** Unless otherwise stated, the source of all definitions is [Wikipedia](https://en.wikipedia.org/wiki/Main_Page)

## Graph concepts

### Knowledge Graph

There are numerous definitions of knowledge graphs. One one the simpler definitions is: A digital structure that represents knowledge as concepts and the relationships between them (facts.) A knowledge graph can include an [ontology](#ontology) that allows both humans and machines to understand and reason about its contents.

[Read more](https://en.wikipedia.org/wiki/Knowledge_graph#Definitions)

### Graph database

A database that uses [graph structures](#graphstructure) for [semantic queries](#semanticquery) with [nodes](#node), [edges](#edge), and [properties](#property) to represent and store data. The edges form the graph. The graph relates the data items in the store to a collection of nodes and edges, the edges representing the relationships between the nodes. The relationships allow data in the store to be linked together directly and, in many cases, retrieved with one operation. Graph databases hold the relationships between data as a priority. Querying relationships is fast because they are perpetually stored in the database. Relationships can be intuitively visualized using graph databases, making them useful for heavily inter-connected data

[Read more](https://en.wikipedia.org/wiki/Graph_database)

### Graph Structure

An abstract data type for implementing the graph theory concepts of [Undirected](#undirectedgraph) and [Directed](#directedgraph) graphs. The former concept consists of **unordered pairs** of vertices (also called [nodes](#node) or points.) The latter consists of **ordered pairs** of vertices.

A graph data structure consists of a finite set of vertices, together with a set of unordered or ordered pairs of these vertices as defined above. These pairs are known as [edges](#edge) (also called links or lines, and sometimes called arrows or arcs for a directed graph.) The vertices may be part of the graph structure, or may be external [entities](#entity) represented by integer indices or references.

A graph data structure may also associate an edge value to each edge, such as a symbolic label or a numeric attribute (cost, capacity, length, etc.)

[Read more](https://en.wikipedia.org/wiki/Graph_\(abstract_data_type\))

### Directed Graph

A directed graph (or **digraph**) is a graph made up of vertices connected by directed [edges](#edge) often called **arcs**.

[Read more](https://en.wikipedia.org/wiki/Directed_graph)

### Ontology

A way of showing the properties of a subject area and how they are related, by defining a set of concepts and categories that represent the subject. An ontology encompasses a representation, formal naming and definition of the categories, properties and relations between the concepts, data and entities that substantiate one, many, or all [domains](#domain) of discourse.

[Read more](https://en.wikipedia.org/wiki/Ontology_\(information_science\))

### Semantic Query

Semantic queries allow for queries and analytics of an associative and contextual nature. Semantic queries enable the retrieval of both explicitly and implicitly derived information based on syntactic, semantic and structural information contained in data. They are designed to deliver precise results (possibly the distinctive selection of one single piece of information) or to answer more fuzzy and wide-open questions through pattern matching and digital reasoning.

[Read more](https://en.wikipedia.org/wiki/Semantic_query)

### Node

A **vertex** (plural vertices) or **node** is the fundamental unit of which graphs are formed: an undirected graph consists of a set of vertices and a set of [edges](#edge) (unordered pairs of vertices), while a directed graph consists of a set of vertices and a set of **arcs** (ordered pairs of vertices). In a diagram of a graph, a vertex is usually represented by a circle with a label, and an edge is represented by a line or arrow extending from one vertex to another.

[Read more](https://en.wikipedia.org/wiki/Vertex_\(graph_theory\))

### Edge

An edge (together with [vertices](#node)) is one of the two basic units out of which graphs are constructed. Each edge has two (or in hypergraphs, more) vertices to which it is attached, called its **endpoints**. Edges may be [directed](#directedgraph) or undirected; undirected edges are also called **lines** and directed edges are also called **arcs** or **arrows**. In an undirected simple graph, an edge may be represented as the set of its [vertices](#node), and in a directed simple graph it may be represented as an ordered pair of its vertices. An edge that connects vertices x and y is sometimes written xy.

[Read more](https://en.wikipedia.org/wiki/Glossary_of_graph_theory#edge)

## Data

### Delta Encoding

A way of storing and transmitting data in the form of **deltas** (differences) between sequential data rather than complete files; sometimes referred to as data differencing. Delta encoding is sometimes called delta compression, particularly where archival histories of changes are required, for example, in revision control systems such as [GitHub](https://github.com) and the **TerminusDB** version-controlled graph databases.

[Read more](https://en.wikipedia.org/wiki/Delta_encoding)

### Metadata

Metadata is data that provides information about other data, but not the content of the data, such as the text of a message or an image. There are many distinct types of metadata.

[Read more](https://en.wikipedia.org/wiki/Metadata)

### RDF

The **Resource Description Framework** (RDF) is a family of **W3C** specifications used as a general method for conceptual description or modeling of information that is implemented in web resources, using a variety of syntax notations and data [serialization](#serialization) formats.

### Triple

A **semantic triple**, or **RDF triple** or simply **triple**, is the atomic data entity in the [RDF](#rdf) data model. A triple is a set of three entities that codifies a statement about [semantic data](#semantic-data) in the form of [subject](#subject)–[predicate](#predicate)–[object](#object) expressions. Examples:

#### Table: Subject-Predicate-Object examples

{% table %}

- Subject
- Predicate
- Object

---

- Bob
- knows
- John

---

- Bob
- is
- 35

{% /table %}

This format enables knowledge to be represented in a machine-readable way. Every part of an RDF triple is individually addressable via unique URIs. For example, the statement "Bob knows John" might be represented in RDF as:

```rdf
http://example.name#Bob12 http://xmlns.com/foaf/0.1/knows http://example.name#John34
```

[Read more](https://en.wikipedia.org/wiki/Semantic_triple)

### Semantic Data

Semantic data or **Semantic Data Model** (SDM) is a high-level semantics-based database description or model. This database model is designed to capture more of the meaning of an application environment than is possible with contemporary database models.

[Read more](https://en.wikipedia.org/wiki/Semantic_data_model)

### Serialization

The process of translating a data structure or object state into a format that can be stored (for example, in a file or memory) or transmitted (for example, over a computer network) and reconstructed later (possibly in a different computer environment.)

[Read more](https://en.wikipedia.org/wiki/Serialization)

### Schema

The organization of data as a blueprint of how the database is constructed.

[Read more](https://en.wikipedia.org/wiki/Database_schema)

### Attribute

The columns of a table in relational databases. Similar to properties in graph databases.

### Entity

Tables in relational databases. Similar to nodes in graph databases.

### Relationship

The relationships between two tables in relational databases. In graph databases, relationships are directed and named connections between two nodes.

## TerminusDB specific

### Database

In TerminusDB, a **database** is the fundamental storage unit — a named container that holds JSON documents with graph relationships, maintains a full commit history, and supports branching and merging. You interact with databases through the API (creating, querying, cloning, pushing, pulling). Every database has at least one branch (typically `main`) and every write creates an immutable commit.

### Data product

A **data product** is the higher-level concept for a database that is published, shared, or collaborated on within an organisation. When you clone a database from a remote, push changes to a team, or browse available datasets in DFRNT, you are working with data products. In practice, "database" and "data product" often refer to the same underlying object — the distinction is one of perspective: *database* emphasises the storage engine and API operations, while *data product* emphasises collaboration, discovery, and organisational ownership.

### WOQL

WOQL (**Web Object Query Language**) is TerminusDB's query language for querying complex data patterns and structures. WOQL is based on three fundamental concepts: WOQL [triples](#triples), WOQL **variables** and WOQL **operators**. WOQL also provides [PCRE](#pcre).

> WOQL does not use the [SPARQL](https://en.wikipedia.org/wiki/SPARQL) protocol.

### Immutable

In programming, the state of an immutable object is unchangeable, i.e., its state cannot be modified after it is created. In TerminusDB, **immutable data** means an instance of data that cannot be changed after it is created. Newer instances or versions of that data can be created, but the tat version is immutable.

[Read more](https://en.wikipedia.org/wiki/Immutable_object)

### Mutable

In contrast to an [immutable](#immutable) object, a mutable object is changeable.

[Read more](https://en.wikipedia.org/wiki/Immutable_object)

### PCRE

**Perl Compatible Regular Expressions** (PCRE) is a library that implements a regular expression engine, inspired by the capabilities of the Perl programming language. PCRE's syntax is more powerful and flexible than many other regular-expression libraries.

[Read more](https://en.wikipedia.org/wiki/Perl_Compatible_Regular_Expressions)

### IRI

The **Internationalized Resource Identifier** (IRI) is an internet protocol standard that builds on the Uniform Resource Identifier (URI) protocol by greatly expanding the set of permitted characters. IRIs extend URIs by using the Universal Character Set, where URIs were limited to ASCII, with far fewer characters. IRIs may be represented by a sequence of octets but by definition are defined as a sequence of characters, because IRIs may be spoken or written by hand.

[Read more](https://en.wikipedia.org/wiki/Internationalized_Resource_Identifier)

## Schema annotations

### @shared

A **class-level** schema annotation (`"@shared": []`) that marks a document class as reference-counted. Instances of a `@shared` class are regular documents with their own IRI, but are automatically cascade-deleted when no other document references them. Mutually exclusive with `@subdocument`. See [Document Types Compared](/docs/document-types-comparison/) and the [Schema Reference Guide](/docs/schema-reference-guide/).

### @subdocument

A **class-level** schema annotation (`"@subdocument": []`) that marks a class as wholly owned by its containing document. Subdocuments cannot exist independently, are created/deleted with their parent, and can only be referenced by a single parent. Keys must be `Random` or `ValueHash`. See the [Schema Reference Guide](/docs/schema-reference-guide/).

### @unfoldable

A **class-level** schema annotation (`"@unfoldable": []`) that causes all references to the annotated class to be expanded inline during document retrieval. Orthogonal to lifecycle annotations (`@shared`, `@subdocument`). See the [Document Unfolding Reference](/docs/document-unfolding-reference/).

### @unfold

A **field-level** schema annotation (`"@unfold": true`) that causes a specific property to expand its target inline during retrieval, regardless of whether the target class is `@unfoldable`. See the [Document Unfolding Reference](/docs/document-unfolding-reference/).
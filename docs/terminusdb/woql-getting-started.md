---
tags:
  - woql
  - tutorial
  - beginner
title: WOQL Getting Started
nextjs:
  metadata:
    title: WOQL Getting Started
    description: Examples to Get Started with the TerminusDB Web Object Query Langauge (WOQL)
    keywords: terminusdb, datalog, fetch, get, query, query language, read, retrieve
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-getting-started/
media: []
---

This page teaches you the building blocks of WOQL — predicates, variables, and literals — through practical examples. By the end, you will understand how to construct queries, bind variables, and read results. For a conceptual overview of WOQL, see [WOQL Explained](/docs/woql-explanation/). For a hands-on interactive tutorial, see [WOQL Basics](/docs/woql-basics/).

WOQL is a formal language for querying and updating TerminusDB databases. It is evaluated by a **datalog engine** that binds variables as it reads the query's abstract syntax tree — a declarative approach where you describe patterns and the engine finds all matching solutions.

## Prerequisites

All examples use the TypeScript/JavaScript dialect of WOQL (the Python syntax follows traditional Python patterns but the logic is identical). Examples have been tested in the TerminusDB Logical Studio provided by [DFRNT](https://dfrnt.com/hypergraph-content-studio/). You should have a database with some schema and documents before working through these examples.

## Predicates, literals and variables in WOQL

WOQL as a language has three main parts, predicates, literals and variables.

### Predicates, the "functions" of WOQL

Predicates such as `triple()`, `eq()` and `read_document()` enable logical constraints to be put to match information stored in TerminusDB. The retrieved information can be seen as the full set of combinations, possibilities or solutions from the query that is sent. 

### Literals, the "values" of WOQL

Literals are values of a datatype. A variable can be bound to a value using `eq("v:MyVariable", "John Doe")` to let the variable `MyVariable` be bound to the literal `John Doe`. Valid solutions are only those where the variable `MyVariable` will be equal to the literal or value `John Doe`.

### Variables, the "glue" of WOQL

Variables can be specified in two ways in the Typescript and Python TerminusDB SDK, either they can be generated using the `Vars` function as a native variable in the language to leverage syntax coloring and a pure approach. Or, they can be specified using the `v:` prefix. In this tutorial we will use the `v:` prefix as we will need less boilerplate code and it's easy to copy and paste.  

Think of each "binding" with a set of bound variables that is returned from TerminusDB as a solution to the logical constraints that were put in the query. The logical constraints are not limited to reading information, updates, creations and deletions can also be made in the same query.

## How WOQL sees the world

The world that one WOQL query operates on is a closed world as defined by the layer that the query is active on, usually the most recently defined layer, but it could also operate in read-only mode on a commit in the past.

All operations happen atomically in one go in the query and nothing is created, updated, or deleted in the world that the query operates on. Every action is recorded during the query and are applied in creating the next layer in the instance and schema graphs. This means that a query can reference many layers in the past, but can only create a new world that exists after the query completes.

This means a query can't operate on information that comes out of the current query. How the query logically sees the world is an important consideration when working with declarative logic in a datalog language like WOQL. 

## Reading documents and triples from the instance graph

Information stored in TerminusDB is stored as hierarchical documents with a defined shape, similar in structure to JSON-LD documents. Each document is made up of frames, sets of triples that are connected, that some call such frames field sets or FieldSets. Triples that are disconnected from documents are not allowed.

All triples that form documents exist in the instance graph. There are two graph types in TerminusDB, the instance graph and the schema graph. The instance graph contains the technical documents that are stored in the database as a graph of connected triples of a certain shape. 

### The minimal document, a single defined triple

Documents can be minimal. The smallest possible document to define consists of a single Resource Description Framework triple. It contains the `@id` of the document as the subject, a field or property called `rdf:type` as the predicate and as the object of the triple, the type of the document, such as `@schema:Entity`.

The `@schema:` prefix to `Entity` is a prefix that is shared between schema and instance graphs of TerminusDB data products. It enables the properties of types to be looked up in the schema graph using `quad()` that will be addressed separately. The schema graph contains the schema that defines the data model for all documents stored in the instance graph.

The schema enforces a structure, the shape of documents, that all triples connecting the information stored in the instance graph. Reading documents and triples from the instance graph is done using the `read_document()` predicate.

### Triples are the foundation for the WOQL language

Documents are the basic unit to exchange facts and hierarchical records. They can be read into WOQL variables. WOQL however mostly operates on triples once the TerminusDB engine has ensured that a new instance layer can be recorded for the future. 

This means we need a bit of foundation for what a document is, and what triple is. A triple is in essence three glued part that mirrors the world or that stand on their own: a subject, a predicate and an object. The subject is identified by an `@id`. The predicate indicates something about the subject, and the object is the value of the predicate.

Example of a minimal document of the `Person` type:

```json
{
    "@id": "Person/JohnDoe",
    "@type": "Person"
}
```

This is how the document is seen by WOQL in the triple store when the prefixes have been applied from the context.

{% table %}

- Subject
- Predicate
- Object

---

- Person/JohnDoe
- rdf:type
- @schema:Person

{% /table %}

This is the triple as it is actually stored on file.

{% table %}

- Subject
- Predicate
- Object

---

- terminusdb:///data/Person/JohnDoe
- rdf:type
- terminusdb:///schema#Person

{% /table %}

The first table shows how default prefixes in TerminusDB enable WOQL to make for concise queries without the full IRI. The second row shows the full URL that is used internally inthe instance graph if the context has not been altered. 

If we would state that John Doe has an age, this is how it would look to WOQL. 34 is a literal value and could be an integer or one of the other datatypes in TerminusDB.

{% table %}

- Subject
- Predicate
- Object

---

- Person/JohnDoe
- age
- 34

{% /table %}

We could also use the equivalent values below when quering, or even write out the full schema IRI, depending on our needs.

{% table %}

- Subject
- Predicate
- Object

---

- terminusdb:///data/Person/JohnDoe
- @schema:age
- 34

{% /table %}

The default context for TerminusDB is shown below.

```json
{
  "@base": "terminusdb:///data/",
  "@schema": "terminusdb:///schema#"
}
```

The reason this is important is that the foundation of TerminusDB rests with the Semantic Web, but with a closed world interpretation of the Resource Description Framework, RDF. This means that the context defines the data that is stored in TerminusDB, and that TerminusDB is authoritative for the information within it's world, it is expected to have correct answers reasoning about that world it knows about, a "closed world". 

WOQL operates as a datalog semantic query language that enables declarative logic to be used for the semantic web and to traverse documents no matter their shape or how they are connected. The variables in WOQL use either shorthand IRIs, prefixed IRIs or the full IRI when resolving connected triples.

Let's continue the exporation by quering for documents.

### Reading the minimal document as a triple

First, let's look at how documents are structured in TerminusDB. A document is assembled from triples by TerminusDB into a set of JSON-LD-like documents, without the context object, including `@id` and `@type` for each frame or field set. Each frame is represented as a JSON object.

Documents in TerminusDB are hierarchical, where concrete types that can exist on their own are called documents, and documents that form parts of a document are called subdocuments. They are nested framed objects in the structure of a document. What connects the "levels" of a hierarchical document are triples that connect one document to a subdocument.

### Querying a document in TerminusDB

Querying a document in TerminusDB is done using the `read_document("v:id", "v:document")` predicate. It takes two arguments, the first is the document id to read and the second is the variable to bind the read document to in the response. Both arguments are variables, and both will be returned in the bindings.

The whole nested document will be resolved into a complete object in the bindings WOQL result, with all the subdocuments assembled automatically. In a regular relational database, the client bears the responsibility to resolve the nested structure of documents using various kinds of joins as part of the query, where as the unfolding structure of documents can be specified elegantly in the TerminusDB schema.

To only get the `v:read_document_variable` back, leverage the `select()` predicate that filters the bindings to only include the variables that specified by the arguments to `select()`. This is a common pattern when only interested in a specific part of a document.

Here we want just a list of documents, so we select the `v:docs` variable to not get the data in the `v:docId` variable back in our query bindings.

```javascript
WOQL.select("v:docs").read_document("v:docId", "v:docs");
```

If the individual WOQL keywords have been added to the Javascript context, the query can be written as below in the **fluent** style, where the WOQL parts are joined together in a flow-like manner. The easiest way to think of it is that the next WOQL keyword is either the last argument or flows from the first predicate to the next. 

```javascript
select("v:docs").read_document("v:docId", "v:docs");
```

The query can also be written as below in the **functional** style that we will be using mainly. Note the use of `v:` for specifying variables. Variables used in multiple places are bound by the how deterministic the predicate is and whether the variable becomes "grounded".

```javascript
select("v:docs", read_document("v:docId", "v:docs"));
```

In this example, `v:docs` was bound by the `read_document()` predicate ("read into the variable"), and `v:docId` is free floating. It should be noted that the first argument of `read_document()` is not fully floating, it only matches the first document it finds, unless it is preceded by a predicate that is fully free floating, such as `triple()` or `quad()` that bind variables to content in the triple store. 

To match all documents thus, the following query can be used to match the document equal to the `@id` of `Person/JohnDoe`, read_document it into the variable `v:docs`, and filter out to only retrieve `v:docs` in the bindings of the query.

```javascript
select("v:docs").and(
  eq("v:docId", "Person/JohnDoe")
  triple("v:docId", "rdf:type", "@schema:Person"),
  read_document("v:docId", "v:docs")
);
```

Changing the order of the eq, triple and read_document predicates can change performance characteristics and sometimes also the meaning of the query.

## Using and, or and opt and not in queries

An important part of querying is to use boolean logic to filter out the data you are interested in. WOQL provides the `and()`, `or()`, `opt()`, and `not()` predicates to enable this.

### The and() predicate

The `and()` predicate is used to combine multiple predicates into a single predicate where all predicates must be true for its solutions to match and return. The use of `and()` is straightforward and is a good way to structure the query into logical blocks that must hold true.

No row will be returned for the variable `v:var` from this example:

```javascript
and(
  eq("v:var", "a"),
  eq("v:var", "b")
)
```

One row will be returned for the variable `v:var` from this example, where `v:var_a` is grounded to "a" and `v:var_b` is grounded to "b".

```javascript
and(
  eq("v:var_a", "a"),
  eq("v:var_b", "b")
)
```

### The or() predicate

The `or()` predicate is used to combine multiple predicates into a single predicate where at least one predicate must be true for its solutions to match and return. The use of `or()` can be a bit tricky and sometimes it is better to use the `opt()`, optional, predicate.

The use of the `or()` predicate drives the cardinality of the solutions, where each possible solution stands on its own, meaning that more than one solution to the query will be returned. It is often the case that the query author means to use that certain parts of the query are optional and that the variable can be left ungrounded if the predicate does not hold true or fill a variable with a value.

Two rows will be returned for the variable `v:var` from this example:

```javascript
or(
  eq("v:var_a", "a"),
  eq("v:var_b", "b")
)
```

One row where `v:var_a` is grounded to "a" and one row where `v:var_b` is grounded to "b". The values where `v:var_a` and `v:var_b` are ungrounded have no values.

### The opt() predicate

The `opt()` predicate means that the predicate is options, if possible, it will ground the variable, but if it is already grounded or does not hold true, the predicate will be skipped. An example is to fill up the ungrounded values of the or statement, which could be done as follows:

```javascript
and(
    or(
        eq("v:var_a", "a"), 
        eq("v:var_b", "b")
    ), 
    opt().eq("v:var_a", "v:var_b")
)
```

Here we get one solution where both `v:var_a` and `v:var_b` are grounded to "a". And one solution where `v:var_a` isand `v:var_b` are grounded to "b". The `opt()` predicate is optional, and applied where possible, i.e. filling the ungrounded value through equality. This is useful for handling optional values and to avoid "exploding" cardinality.

### The not() predicate

The `not()` predicate is used to negate a predicate, i.e. to make sure that something should not match. This can be used to find all document subjects, where we need to filter out lists and subdocuments:

```javascript
select(
  "v:subject",
  "v:type",
  and(
    triple("v:subject", "rdf:type", "v:type"),
    not(quad("v:type", "sys:subdocument", "v:select", "schema")),
    not(eq("v:type", "rdf:List")),
  ),
)
```

Here we select subject and type for the resulting bindings. We match all subjects of documents, but there are certain special types of documents, such as Lists and Subdocuments, that are not top level documents and should thus be exluded from the result.

## Further Reading

### WOQL Explanation

[WOQL Explanation](/docs/woql-explanation/) for a more in-depth explanation of WOQL.

### WOQL Reference

[JavaScript](/docs/javascript/) and [Python](/docs/python/) WOQL Reference guides

### How-to guides

See the [How-to Guides](/docs/use-the-clients/) for further examples of using WOQL.

### Documents

[Documents](/docs/documents-explanation/) in a knowledge graph and how to use them.
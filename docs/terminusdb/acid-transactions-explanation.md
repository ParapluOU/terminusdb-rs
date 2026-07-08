---
tags:
  - explanation
  - documents
  - beginner
title: ACID Transactions Explanation
nextjs:
  metadata:
    title: ACID Transactions Explanation
    description: "An explanation about ACID Transactions and how TerminusDB ensures ACID compliance, Atomicity, Consistency, Isolation, and Durability"
    keywords: ACID transactions, immutable database, knowledge graph, database, RDF, consistency, atomic commits
    alternates:
      canonical: https://terminusdb.org/docs/acid-transactions-explanation
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

TerminusDB is a fully **ACID-compliant** immutable database — every write is an atomic commit, schema validation ensures consistency, snapshot isolation prevents dirty reads, and the append-only delta layer guarantees durability. This page explains how each ACID property works in TerminusDB.

## What is ACID?

ACID ([Atomicity](#atomicity), [Consistency](#consistency), [Isolation](#isolation), [Durability](#durability)) are properties of database transactions that are generally considered desirable for many applications.

### Atomicity

Atomicity is an all-or-nothing approach to database transactions. If a transaction starts but does not complete, then all data manipulation or modification operations carried out by that transaction are undone, and any affected data or objects remain unchanged. The database is returned to the state it was in before the transaction started. Atomicity, or atomic transactions, guarantee the consistency and integrity of data and objects, ensuring the database is not left in an inconsistent or partially changed state.

#### Atomicity and immutability

TerminusDB combines atomicity with [immutability](/docs/immutability-explanation/) to provide atomic transactions.

### Consistency

Consistency has multiple forms and can be interpreted in different ways. TerminusDB implements two forms of consistency - full and partial consistency.

#### Full consistency

Where a schema exists for a TerminusDB database, a transaction will not be completed unless all schema conditions are satisfied. The consistency of the schema is maintained under all conditions.

#### Partial consistency

When rebasing, transactions that complete under certain **read-conditions** can be _replayed_ by reordering their commits. Schema consistency is maintained but not under all conditions.

### Isolation

The isolation property gives a user the impression of being the sole user of a database. The user experiences no currency or conflicts with other users of the database.

#### Read isolation

TerminusDB uses inherent database [immutability](/docs/immutability-explanation/) to ensure each read query exists at a given layer providing each user with an isolated snapshot of the database.

#### Write isolation

Similar to read isolation, completing write transactions ensures isolation with optimistic concurrency, simply restarting any transactions failing mid-run.

### Durability

TerminusDB is durable. Transactions failing mid-run do not corrupt data. Data is protected from external sources of potential corruption such as operating system bugs. In the unlikely event of a partial commit, previous layers remain unchanged and recoverable. Backups are significantly simplified, requiring copy storage only to ensure a safely recoverable state.

## Further Reading

[**Documents in a knowledge graph and how to use them**](/docs/documents-explanation/).

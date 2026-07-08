---
tags:
  - woql
  - cookbook
title: TerminusDB Query Cookbook
nextjs:
  metadata:
    title: TerminusDB Query Cookbook
    description: Guides showing how to query using GraphQL, WOQL and in other ways
    keywords: TerminusDB, DFRNT Cloud, TerminusDB Open Source, TerminusDB Documentation, TerminusDB Query, TerminusDB Query Cookbook
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/how-to-query/
media: []
---
## Getting Started

TerminusDB Query Cookbook is a collection of guides showing how to query using GraphQL, WOQL and in other ways. TerminusDB is a graph database and document store that is great for data integration, knowledge management, knowledge engineering, content management, and data analytics.

* [Learn WOQL: An Interactive Tutorial](/docs/woql-tutorial)

This interactive tutorial teaches WOQL from scratch through hands-on examples you can edit and run directly in your browser. Each step builds on the last, covering variables, logic, data reading/writing, filtering, and shaping results.

* [Getting Started with WOQL](/docs/woql-getting-started)

In this guide, we introduce the basic concepts of WOQL and show how to use it to query a TerminusDB database.

## Datatypes

WOQL is powerful enough to support a wide range of datatypes. In this guide, we show how to use WOQL to query TerminusDB using different datatypes.

* [Cookbook: Datatypes](/docs/cookbook-woql-type-of-datatype)

This guide covers how to match datatypes, understand the datatype of a value and check that a value is of a datatype.

## Git-for-data, query across graphs

TerminusDB includes a Git-for-data feature that also allows you to query across knowledge graphs.

* [Cookbook: Many Graphs](/docs/datalog-queries-between-data-products/)

This guide shows how to use WOQL to query a TerminusDB data product that has many graphs, or between completely separate data products.

## Time Processing

TerminusDB has built-in support for ISO 8601 dates, times, durations, and intervals. These tutorials progress from basic date comparisons through to solving real business problems with temporal logic.

* [ISO 8601 Time Processing in TerminusDB](/docs/time-processing)

Overview of TerminusDB's temporal capabilities and why correct time handling matters for financial reporting, scheduling, and data integration.

* [Tutorial 1: Dates, Comparisons & Range Queries](/docs/time-tutorial-dates)

Date representation, comparison predicates, half-open range filtering, and the as-of vs period query pattern.

* [Tutorial 2: Durations, Month Arithmetic & Sequences](/docs/time-tutorial-durations)

Duration arithmetic, end-of-month preservation, sequence generation, weekday extraction, and month boundary predicates.

* [Tutorial 3: Intervals & Allen's Temporal Algebra](/docs/time-tutorial-intervals)

Interval construction, deconstruction, and Allen's 13 temporal relations for verifying how periods relate to each other.

* [Tutorial 4: Creative Temporal Patterns](/docs/time-tutorial-patterns)

Solving real business problems by composing temporal predicates — business day calendars, fiscal period construction, gap and overlap detection.

## Pattern Generation

In this guide, we show how to use WOQL to query a TerminusDB database with pattern generation.

* [Cookbook: Pattern Generation](/docs/pattern-generation-cookbook/)


## Querying TerminusDB Databases with GraphQL

In this guide, we show how to use GraphQL to query a TerminusDB database within and across documents that are stored as graph objects.

It covers how to use GraphQL to query for data in a TerminusDB database, including how to use the `query` keyword and how to use the `mutation` keyword.

* [GraphQL Query](/docs/how-to-query-with-graphql//)

### Querying TerminusDB Databases with WOQL


In this guide, we show how to query a TerminusDB database using WOQL. It covers how to use WOQL to query for graph data and triples stored in a TerminusDB database.

* [WOQL Query](/docs/how-to-query-with-woql/)
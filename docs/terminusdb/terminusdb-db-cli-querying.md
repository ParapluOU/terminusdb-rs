---
tags:
  - how-to
  - woql
  - curl
  - cli
  - intermediate
title: How to use the terminusdb CLI query interface
nextjs:
  metadata:
    title: How to use the terminusdb CLI query interface
    description: Quick introduction to the terminusdb CLI WOQL Query interface which has a distinct syntax to other flavours.
    keywords: terminusdb, datalog, find, query, search, terminusdb query, use the terminusdb cli query interface, woql
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/terminusdb-db-cli-querying/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally
- Access to the TerminusDB CLI tool
- A database to query
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query your database using the TerminusDB CLI.
{% /callout %}

This is a tutorial to learn the command line CLI interface to start exploring the local TerminusDB _system database. Be careful in not making incorrect or uncontrolled changes to the _system database as it can lock you out of your data products.

*Disclaimer*: Do not run this tutorial with data that you do care about. Here be dragons. You have hereby been warned!

The local TerminusDB database CLI has a WOQL query interface built in, with a distinct syntax to other flavours. When working with a local TerminusDB instance, it is often handy to perform offline commands against it, or extract information in a data pipeline for ML/Ops.

This syntax is for advanced users who have significant experience with TerminusDB and WOQL. If you are new to TerminusDB, we recommend starting with the [TerminusDB Quickstart](/docs/get-started-with-terminusdb/).

## Addressing TerminusDB data products with GRAPH_SPEC and DB_SPEC

To specify a graph to interact with, use the DB_SPEC in a query. Here are a couple of ways to specify a graph using the DB_SPEC. The admin team is the default team on local TerminusDB instances.

* `_system`: the system database
* `org/data_product`: main of a specific data product
* `admin/data_product/local/branch/main` the main branch of the data product data_product in the admin organisation/team
* `admin/data_product/local/commit/9w8hk3y6rb8tjdy961ed3i536ntkqd8` a specific commit in the data product data_product

This is the order of the components of the DB_SPEC:
* organization
* data product
* local (or remote)
* branch (or commit)
* #commit or #branch

The designation of databases, repositories, the associated commit
graph of a database, and various graphs as used in the above command
requires the use of an appropriate descriptor path which is referred to
as the DB_SPEC. Here are the details of each.

  * `_system`:
  This is the system meta-data, which contains the user information,
  organization information and database records.

  * `<organization>/<database>`:
  This is the most basic descriptor path and actually refers to
  the more specific default path `<organization>/<database>/local/branch/main`.

  * `<organization>/<database>/_meta`:
  This is the repository graph associated with `database`. It contains
  information about the local repository and all known remotes.

  * `<organization>/<database>/<repository>`:
  This is a longer form database designator which specifies which
  respository we would like to address. It refers implicitly to
  `<organization>/<database>/<repository>/branch/main`.

  * `<organization>/<database>/<repository>/_commits`:
  The commit graph associated with a given database. This graph
  contains metadata about branch histories with their commit objects
  including the authorship and time.

  * `<organization>/<database>/<repository>/branch/<branch>`:
  The most specific branch descriptor, allows you to address a
  branch other than main.

  * `<organization>/<database>/<repository>/commit/<commit>`:
  The descriptor which allows an individual commit to be addressed
  directly.

For commands that refer to a GRAPH_SPEC, it should be a DB_SPEC
(specifying the precise branch if a database) followed by one of:

 * `DB_SPEC/instance`

 * `DB_SPEC/schema`

For example:

`terminusdb triples dump admin/people/local/branch/main/schema`

## Syntax

The syntax of the WOQL query interface is distinct from other flavours, and is based more closely on the Prolog syntax. The WOQL logic is the same, but the syntax is different, especially for and, triple and other predicates expected in WOQL.

This documentation is incomplete and a stub. PRs are welcome to strengthen it and make it more complete. It only covers the syntax differences. Refer to the [WOQL Basics guide](/docs/woql-basics/) as the main language interface.

We will use the system database as the example database for this guide as it's available on the local system.

### Getting started with a simple query

An example for finding the databases in the system:

```bash
./terminusdb query _system "(
  t(Database,rdf:type,'@schema':'UserDatabase')
)"
```

A couple of notes:
* All variables must start with a capital letter
* The string values must be in single quotes
* IRIs with prefixes must have the two portions quoted independently with a colon inbetween due to the `@` sign
* the `triple` syntax is `t(A,B,C)` instead of `triple(A,B,C)`
* the `quad` syntax is `t(A,B,C,schema)` instead of `quad(A,B,C,schema)`
* the `and` syntax is `(statementA,statementB)` instead of `and(A,B)`
* the `or` syntax is `(statementA;statementB)` instead of `or(A,B)`

### Finding which capabilities users have

The terminusdb client does not yet have the ability to find which capabilities a user has on which databases or organizations. This command queries them.

```bash
./terminusdb query _system "select([User,Role,Scope],(
  t(Capability,rdf:type,'@schema':'Capability'),
  t(Capability,role,Role),
  t(Capability,scope,Scope),
  t(User,capability,Capability),
  User='User/philippe@hoij.net'
))"
```

Some additional notes:

* select works by binding variables to be returned
* `t()` is used to bind triples
* to bind a specific value, use bind variables to specific values using `=`
* Note that the variables are unified by the values stored in the database

Or, to see the capabilities of all users, don't check for a specific user:

```bash
./terminusdb query _system "select([User,Role,Scope],
  ( t(Capability,rdf:type,'@schema':'Capability')
  , t(Capability,role,Role)
  , t(Capability,scope,Scope)
  , t(User,capability,Capability)
  )
)"
```

### Other special syntaxes

The command line is in essence a datalog prolog operating on the graph provided by TerminusDB and is excellent for solving small problems quickly.

* Instead of `lexical`, use `0^^xsd:unsignedInt`, or `0^^_`
* Variables can either be specified with a capital initial character, like `Subject`, or using `v('subject')`
* `once(...t()...)` is used to only get one answer
* `_` is used to bind a variable to any value
---
tags:
  - woql
  - how-to
  - advanced
title: Queries between Data Products
nextjs:
  metadata:
    title: Queries between Data Products
    description: Learn how to use datalog with knowledge graphs in data products to encode data in a way that resembles the real world, and easily query and manipulate it.
    keywords: terminusdb, datalog, queries between data products, query, terminusdb query, woql
    alternates:
      canonical: https://medium.com/the-semantic-data-practitioner/datalog-with-knowledge-graphs-as-data-products-in-terminusdb-3239c800bb98
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- Two or more databases (data products) to query across
- Familiarity with WOQL basics ([getting started](/docs/woql-getting-started/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to write WOQL queries that span multiple data products.
{% /callout %}

Knowledge graphs enable domain experts to encode their data in a way that resembles the real world, more than traditional relational databases do out of the box. In general, relational databases get very complex very quickly. Most data storage engine make use of one type of language, datalog, that enables users to assert what they want from a data source, and get that back, usually in a tabular format. A good datalog lets the data engine figure out how to get to the data, not the user. Examples of datalog languages include SQL, Cypher, WOQL (TerminusDB), and others.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*bGWLqvujoDcM3pPFO71nIg.png" alt="How a TerminusDB Knowledge Graph can be used to visualize connections (dfrnt.com)" caption="How a TerminusDB Knowledge Graph can be used to visualize connections (dfrnt.com)" %}
{% /figure %}

## What is the difference between a hypergraph and a knowledge graph?

Prolog is a great language to implement a datalog engine in, as it allows formal logic to be expressed. Building Prolog programs however can drive enormous complexity, especially in the industrialization phase of a solution, and to enable users to make safe use of the Prolog engine. By leveraging TerminusDB, an industrialized solution with datalog unification, most of the power is available, in a safe subset of Prolog, a datalog with unification.

## On the information in a semantic knowledge graph

In our opinion, the best knowledge graphs for business use work on an object level, instead of a "knowledge atoms" level. When we refer to a "knowledge atom", it's what many call a triple, the smallest unit of knowledge. With a knowledge atom, we can know something specific about something, there is a subject, predicate and an object. Bob is a friend of Alice.

What is then an object, or a document? An object is many of these "knowledge atoms" together. Imagine a sales order or an invoice with multiple line items. A sales order line item might even have a sold-to, bill-to and ship-to on line item. We can think of a sales order printed on a paper, it has a unique ID, and connected with that ID, we have a tree of information attached. 

That tree of information can reference other documents through numeric or formal links, such as a purchase requisition or a contract. All the information elements of the sales order, and the links between the information elements are either a value as the object, or another subject if the chain continues, such as the line item of the sales order.

There is an important distinction to be made here, that between the object, and the document. 

It's subtle, but it's easy to get lost if not making a distinction between the two. An object exists in memory (or stored on disk), and a document is a representation of that object, a copy of it. Think of when you print a file, the file is stored on disk, and the printed copy is a document. A good knowledge graph works on the object level, and prevents stray knowledge atoms to exist.

The ability to work both on an object level, and an atom level is a feature we love in TerminusDB. The engine does not allow you to add random atoms, unless you turn off the guardrails (which you should avoid in our opinion).

## Data products and business object collaboration

TerminusDB adds an abstraction on top of the object level, the data product level, that contains is expected to hold all information about a slice of the world that we say it is authoritative for. It's really just a fancy way of saying how we want TerminusDB to respond. If it does not find anything in it's data store, it should say there is nothing, which is what we call a closed world assumption, as opposed to an open world assumption where not having a response means it could be there, but we don't know of it.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*3KR1AQAhAkdBpDdNMqAuiA.png" alt="Here we see the two data products in the instance that is used for the example below" caption="Here we see the two data products in the instance that is used for the example below" %}{% /figure %}

Businesses and enterprises usually prefer closed world assumptions because it makes it easier to build performant and accurate systems, the inaccuracy is to be handled by people, not the system. Combining data products and object information means we have a foundation to build advanced reasoning about business problems. Since TerminusDB allows a data product to be synchronised with clone, push and pull, in a similar fashion as software developers collaborate on code, it becomes possible for data teams to easily collaborate on accurate data.

The collaborative aspects of TerminusDB are important in relation to it's datalog engine that can be accessed with the WOQL language. Data products can exist in two places, to be synchronised between organisations or between teams in an enterprise. By allowing this open-ended collaboration, teams can increase the velocity of feedback by accepting change requests on data (by sending a "data patch") back, or creating a branch.

## Using datalog with multiple data products

We just talk about the importance of datalog, data objects and data products. Let's combine these into a powerful concept, that of querying information from multiple teams (each having their data product) using one powerful WOQL query across both of them. It's like querying multiple traditional databases in one go, but for knowledge graphs that are synchronizable between teams. Each team would have their own responsibility, but need each other's business information.

With the using WOQL keyword, you can reference multiple data products in one logical query. The data model I used here is extremely simplified, but is enough to show the concept in action.

I created one data product at first, and added the built-in DFRNT `Entity` data type to keep things easy and neat. It has just a single property (predicate), label, for object instances created from it. Then I cloned that data product so I had two data products, to not have to duplicate that work and to keep them the same. I then created an instance of `Entity` in each, named `DP1` and `DP2`.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*dsOLAdO_5FiJ6gnr-BcRyw.png" alt="Here we see the type `Entity`, and the instance `Entity/`DP1``, with the label `DP1` (dfrnt.com)" caption="Here we see the type `Entity`, and the instance `Entity/`DP1``, with the label `DP1` (dfrnt.com)" %}{% /figure %}

And then, with below logic saved in `logic-dp-1`, I query the two data products, that are both located in the same instance, `LogicalTwins`:

```javascript
or(
    using(
         "LogicalTwins/logic-dp-1/local/branch/main",   
        triple("v:subject", "@schema:label", "v:label"),
      ),
    using(
        "LogicalTwins/logic-dp-2/local/branch/main",
        triple("v:subject", "@schema:label", "v:label"),
  ),
)
```

If we look at the logic of the query, we see or which means that we should combine the answers for the variable `v:subject`. The `v:` prefix means that what is between the quotes is a variable. For the first data product we query data from, we have the using keyword refer to the `logic-dp-1` data product in the `LogicalTwins` instance, and the `main` branch.

A sidenote on the `main` branch is that it is the default branch that exists in every TerminusDB data product. A branch is like a workspace, and you can not only clone a data product, you can clone a branch within a data product too and keep it synchronized with other branches, between data products or even with a data product on your own laptop! You can even create empty branches to work from that are completely separate from other branches. As said, I'll write more about this!

The triple keyword specifies that we want to constrain the variable v:subject to only the triples (knowledge atoms) that have a predicate label, and put the labels of the subject in the variable `v:label`.

The result of this query across the two data products is the below one, as created by the just-released Logical Studio for TerminusDB in DFRNT that allows users to build datalog queries in a user interface and save them as part of the data product itself.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*bGWLqvujoDcM3pPFO71nIg.png" alt="Result from TerminusDB, including the labels and ID:s from the TerminusDB data products (dfrnt.com)" caption="Result from TerminusDB, including the labels and ID:s from the TerminusDB data products (dfrnt.com)" %}{% /figure %}

What we see above is that each of the objects, one from each data product, were selected by the query and returned.

---

This page was adapted from a Medium article by `@hoijnet` with permission: [Datalog with Knowledge Graphs as Data Products in TerminusDB](https://medium.com/the-semantic-data-practitioner/datalog-with-knowledge-graphs-as-data-products-in-terminusdb-3239c800bb98) 

---

Learn more:

* [Datalog explanation](/docs/what-is-datalog/)
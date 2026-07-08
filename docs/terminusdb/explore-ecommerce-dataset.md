---
tags:
  - woql
  - tutorial
  - beginner
title: Explore an Ecommerce Dataset
nextjs:
  metadata:
    title: Explore an Ecommerce Dataset — TerminusDB Tutorial
    description: Clone an ecommerce database, query orders across customers, branch to update an order status, and see the structural diff — in 15 minutes.
    keywords: terminusdb tutorial, ecommerce database, graph database orders, branch diff merge, git for data business example
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/explore-ecommerce-dataset/
media: []
lastUpdated: "2026-04-30"
---

Clone a pre-populated ecommerce database to your local TerminusDB instance, query orders across customers and products, update an order status on a branch, and see a field-level structural diff — in 15 minutes.

{% callout type="note" %}
**Prerequisites**
- **TerminusDB running on localhost:6363.** Verify: `curl -s -u admin:root http://localhost:6363/api/info` should return JSON containing `"authority": "admin"`. If you get "connection refused", [start TerminusDB first](/docs/get-started/#step-1).
- **Completed the [First 10 Minutes quickstart](/docs/get-started/)** — you should be comfortable with branches and diffs. This tutorial builds on those concepts with a business dataset.
{% /callout %}

## What you will build

You will clone a complete ecommerce database (customers, orders, order lines, products, categories) from a public server, run a query that traverses document relationships without JOINs, create a business-scenario branch ("fulfil an order"), and see a field-level structural diff of the change.

**The dataset:**

| Document type | Count | Key relationships |
|---------------|-------|-------------------|
| Category | 6 | Hierarchical (parent → child) |
| Product | 20 | Belongs to Category |
| Customer | 15 | — |
| Order | 30 | Placed by Customer |
| OrderLine | 84 | Belongs to Order, references Product |

**Relationship graph:** Customer ← Order ← OrderLine → Product → Category

## Step 1 — Clone the ecommerce database

Pull the entire ecommerce dataset from the public templates server to your local instance:

{% quickstart-clone remoteUrl="https://data.terminusdb.org/public/ecommerce" localPath="ecommerce" label="Clone Ecommerce Database" description="Get the ecommerce dataset on your local TerminusDB — ready to branch and diff." /%}

You just pulled ~155 documents — customers, orders, products, and their relationships — from a public TerminusDB server to your local instance. The data is now yours to query, branch, and modify.

## Step 2 — Explore what you have

List the document types defined in the schema:

{% http-example method="GET" path="/api/document/admin/ecommerce/local/branch/main?graph_type=schema&as_list=true&skip=1" /%}

The response lists 5 document types — `Category`, `Customer`, `Order`, `OrderLine`, and `Product`.

List all orders:

{% http-example method="GET" path="/api/document/admin/ecommerce/local/branch/main?type=Order&as_list=true" /%}

You see 30 Order documents. The database has 15 customers, 20 products, and 84 order lines — all interconnected, all versioned. Order totals are stored as `xsd:decimal` — arbitrary-precision exact arithmetic, not floating-point. Financial figures stay exact across every query, branch, and diff.

## Step 3 — Query: find processing orders with customer details

Show all orders still in "processing" status, with the customer's name and country.

In a relational database, this requires a JOIN: `SELECT o.order_id, o.total, c.name, c.country FROM orders o JOIN customers c ON o.customer_id = c.id WHERE o.status = 'processing'`. You declare the relationship in the query because the database does not know it intrinsically. In TerminusDB, an Order document has a `customer` field that *is* the link to a Customer document — you simply follow it:

{% http-example method="POST" path="/api/woql/admin/ecommerce/local/branch/main" %}
{% http-woql %}
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root",
});
client.db("ecommerce");

const WOQL = TerminusClient.WOQL;
const query = WOQL.and(
  WOQL.triple("v:Order", "status", WOQL.string("processing")),
  WOQL.triple("v:Order", "order_id", "v:OrderId"),
  WOQL.triple("v:Order", "total", "v:Total"),
  WOQL.triple("v:Order", "customer", "v:Customer"),
  WOQL.triple("v:Customer", "name", "v:CustomerName"),
  WOQL.triple("v:Customer", "country", "v:Country")
);

const result = await client.query(query);
console.log(result.bindings);
{% /http-woql %}
```json
{"query": {"@type": "And", "and": [{"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Order"}, "predicate": {"@type": "NodeValue", "node": "status"}, "object": {"@type": "DataValue", "data": "processing"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Order"}, "predicate": {"@type": "NodeValue", "node": "order_id"}, "object": {"@type": "DataValue", "variable": "OrderId"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Order"}, "predicate": {"@type": "NodeValue", "node": "total"}, "object": {"@type": "DataValue", "variable": "Total"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Order"}, "predicate": {"@type": "NodeValue", "node": "customer"}, "object": {"@type": "NodeValue", "variable": "Customer"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Customer"}, "predicate": {"@type": "NodeValue", "node": "name"}, "object": {"@type": "DataValue", "variable": "CustomerName"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Customer"}, "predicate": {"@type": "NodeValue", "node": "country"}, "object": {"@type": "DataValue", "variable": "Country"}}]}}
```
{% http-expected %}
[{"OrderId": "ORD-0002", "Total": 5235.93, "CustomerName": "Hana Tanaka", "Country": "Japan"}, {"OrderId": "ORD-0019", "Total": 1094.95, "CustomerName": "Leila Okafor", "Country": "Nigeria"}, {"OrderId": "ORD-0030", "Total": 649.91, "CustomerName": "Erik Lindström", "Country": "Sweden"}]
{% /http-expected %}
{% /http-example %}

You just traversed Order → Customer in a single query — no JOIN syntax, no ON clause, no foreign key declaration. TerminusDB follows document links natively. The `customer` field in Order *is* the link; the query simply walks it.

## Step 4 — Branch and modify (fulfil an order)

A processing order has been shipped. Update its status on a branch and see what TerminusDB tracks.

Create a branch called `fulfillment`:

{% http-example method="POST" path="/api/branch/admin/ecommerce/local/branch/fulfillment" %}
{"origin": "admin/ecommerce/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

Update order ORD-0002 to "shipped" on the branch:

{% http-example method="PUT" path="/api/document/admin/ecommerce/local/branch/fulfillment?author=warehouse@example.com&message=Ship+order+ORD-0002" %}
{"@id": "Order/ORD-0002", "@type": "Order", "order_id": "ORD-0002", "customer": "Customer/hana.tanaka@example.com", "order_date": "2024-03-19T03:10:07.294Z", "status": "shipped", "total": 5235.93}
{% http-expected %}
["terminusdb:///data/Order/ORD-0002"]
{% /http-expected %}
{% /http-example %}

You just updated one field — `status` from "processing" to "shipped" — on an isolated branch. Main still has the original state. Let's see exactly what changed.

## Step 5 — See what changed (the diff)

In any other database, answering "what exactly changed in this order?" means querying an audit table, parsing CDC events, or comparing snapshots you exported. In TerminusDB, you ask the database directly — compare your `fulfillment` branch against `main`:

{% http-example method="POST" path="/api/diff/admin/ecommerce" %}
{"before_data_version": "main", "after_data_version": "fulfillment"}
{% http-expected %}
[{"@id": "Order/ORD-0002", "status": {"@op": "SwapValue", "@before": "processing", "@after": "shipped"}}]
{% /http-expected %}
{% /http-example %}

One field changed. TerminusDB knows it was `status`, knows the old value ("processing") and the new value ("shipped"), and confirms nothing else was touched — not the customer reference, not the total, not the order date. This is **structural, not textual** — the database understands the document schema and reports typed operations (`SwapValue`), not line diffs.

This is your audit trail — automatic, precise, and queryable. No trigger tables writing to a separate audit schema. No CDC pipeline capturing change events. No application-level event sourcing. The database *is* the changelog, and this diff can be applied, reversed, or forwarded to any system that needs to know what happened.

## Step 6 — Merge back to main

When you are satisfied with the change, merge the fulfillment branch back:

{% http-example method="POST" path="/api/apply/admin/ecommerce/local/branch/main" %}
{"before_commit": "main", "after_commit": "fulfillment", "commit_info": {"author": "warehouse@example.com", "message": "Merge fulfillment into main"}}
{% http-expected %}
{"@type":"api:ApplyResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

The order status change is now on main. The branch remains as a record of who changed what and when.

---

## Clean up

{% callout type="warning" %}
This deletes the **ecommerce** data product permanently. Only run this when you are finished experimenting.
{% /callout %}

{% http-example method="DELETE" path="/api/db/admin/ecommerce" confirm="This will permanently delete the ecommerce data product." /%}

---

## What you just did

In 15 minutes, you:

1. **Cloned** a complete ecommerce database from a public server — one command, no account
2. **Queried** relationships across documents (orders to customers) without JOINs
3. **Branched** the database, updated an order status (a real business scenario)
4. **Diffed** the branch against main and saw a single field-level change
5. **Merged** the change back to main — completing the workflow

These operations — clone, query, branch, diff, merge — are the core of TerminusDB. Every operation you just ran works at any scale: 155 documents or 1.5 million.

## Next steps

- [Explore the Star Wars dataset](/docs/explore-a-real-dataset/) — the same workflow with a fun dataset
- [Write your own schema](/docs/schema-reference-guide/) — add validation and type safety to your documents
- [Time-travel to previous commits](/docs/time-travel-howto/) — view any previous state of your database
- [WOQL query language](/docs/woql-getting-started/) — learn the full query language
- [Connect with TypeScript](/docs/connect-with-the-javascript-client/) — use the SDK in your application

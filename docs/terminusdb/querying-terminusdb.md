---
tags:
  - explanation
  - woql
  - graphql
  - beginner
title: "Choosing a Query Interface: WOQL, GraphQL, or HTTP API"
nextjs:
  metadata:
    title: "Choosing a Query Interface: WOQL, GraphQL, or HTTP API"
    keywords: terminusdb woql vs graphql, terminusdb query language, terminusdb api comparison, woql or graphql, terminusdb query interfaces
    description: Compare TerminusDB's three query interfaces — HTTP Document API, GraphQL, and WOQL — and decide which to use for your workload.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/querying-terminusdb/
media: []
---

TerminusDB exposes three query interfaces. The **HTTP Document API** handles CRUD without a query language — fetch, create, update, and delete documents by ID using any HTTP client. **GraphQL** provides typed, declarative reads ideal for nested document retrieval and frontend integrations. **WOQL** (Web Object Query Language) is a Datalog-based query language for complex graph traversal, pattern matching, aggregation, and schema-aware operations. All three operate on the same versioned data — choose by task complexity, not by capability silo.

## The three interfaces

### HTTP Document API

The simplest way to interact with TerminusDB. Send JSON documents over HTTP to create, read, update, or delete them by ID. No query language to learn — if you can use `curl` or `fetch`, you can use the Document API. Supports bulk operations, raw JSON mode (no schema required), and full version control (every write is a commit with author and message).

**Best for:** CRUD operations, bulk imports, form submissions, any client that speaks HTTP.

[Document API Reference →](/docs/document-format-api-curl-tutorial)

### GraphQL

TerminusDB auto-generates a GraphQL schema from your document schema. Query documents with field selection, nested object traversal, filtering, ordering, and pagination — all using standard GraphQL syntax. Works with Apollo, urql, Relay, and any GraphQL client. Type-safe codegen available.

**Best for:** Frontend integrations, dashboards, nested document reads, teams already using GraphQL tooling.

[GraphQL Basics →](/docs/graphql-basics)

### WOQL (Web Object Query Language)

A Datalog-based query language with unification and backtracking. WOQL queries declare patterns and TerminusDB finds all matches across the graph. Supports recursive path traversal, aggregation (count, sum, group-by), data transformation, schema queries, and operations that span branches or commits.

**Best for:** Complex graph traversal, analytics, migrations, ETL, any query that the other two cannot express.

[WOQL Overview →](/docs/woql-explanation)

{% callout type="note" title="Enterprise edition: additional document formats" %}
The TerminusDB Enterprise Edition supports additional document interface formats for interoperability: **RDF/XML** and **Turtle** document input/output, and complete **JSON-LD `@context` processing** — enabling seamless ingestion of JSON-LD from external systems without manual IRI mapping. Standard and JSON payloads are also supported alongside JSON-LD, making it straightforward to integrate with both JSON and JSON-LD producers and consumers.
{% /callout %}

## Decision table — use X when Y

| You want to... | Use | Why |
|----------------|-----|-----|
| Create, read, update, or delete documents by ID | HTTP Document API | Simplest interface; no query language to learn; works with any HTTP client |
| Fetch documents with nested linked objects in one call | GraphQL | Declarative field selection; automatic traversal of document links; familiar to frontend developers |
| Filter documents by field values | GraphQL or WOQL | GraphQL for simple filters; WOQL for complex compound conditions |
| Traverse arbitrary graph paths (e.g. "all ancestors of X") | WOQL | Path queries with recursive traversal; not available in GraphQL |
| Compare two branches or commits (diff) | WOQL or HTTP Diff API | Structural diff is a core TerminusDB operation; WOQL can incorporate diffs into larger queries |
| Aggregate data (count, sum, group-by) | WOQL | Full aggregation support; GraphQL has no aggregation |
| Pattern-match across multiple document types | WOQL | Unification and backtracking explore the full graph; GraphQL queries are type-rooted |
| Build a frontend integration quickly | GraphQL | Standard tooling (Apollo, urql, Relay) works out of the box; type-safe codegen available |
| Automate bulk data operations (ETL, migration) | HTTP Document API or WOQL | Document API for bulk insert/replace; WOQL for conditional transforms |
| Query schema structure (introspection) | WOQL or GraphQL | Both support schema queries; WOQL via `schema` graph; GraphQL via `__schema` |
| Query a historical state (time-travel) | WOQL | Every WOQL query runs against a specific branch and commit; specify a historical commit to query past state without restoring backups |
| Discover unknown properties on a document | WOQL | Leave the predicate as a variable to find what fields a document has; GraphQL requires knowing the type upfront |

## When WOQL is the right choice

WOQL is the most powerful of the three interfaces, but also the one with the steepest learning curve. Use it when the task genuinely requires its capabilities — not as a default.

### Graph traversal — follow links without declaring relationships

WOQL's shared variables create implicit joins by unification: placing the same variable in the `object` position of one triple and the `subject` position of another follows the link — no JOIN syntax, no ON clause, no foreign key declaration required. This makes multi-hop traversals (Order → Customer → Country) natural single-query operations.

### Complex aggregations across document types

WOQL's `group_by` collects bindings into lists, and `aggregate` computes summary values across an entire result set. GraphQL has no aggregation support; the HTTP Document API returns raw documents without computation. Use WOQL when you need `group_by`, `count`, `sum`, or `aggregate` operations that span multiple document types — for example, "total order value per country" requires traversing Order → Customer → Country and summing the `total` field.

### Diff-aware queries — querying what changed between commits

While the HTTP `/api/diff` endpoint returns raw diff JSON, WOQL can query diffs programmatically within a larger expression — comparing states, filtering changes by document type, or joining diff results with current data. Use WOQL when you need to incorporate branch or commit differences into a larger query, for example: "which products had their price increased in the last commit?"

### Schema-aware pattern matching — variable predicates

GraphQL queries are type-rooted: you must specify the type upfront. WOQL can leave the predicate as a variable (`triple("v:Doc", "v:Property", "v:Value")`) to discover what properties a document has, or query the schema graph to find types that match a structural pattern. Use this when the query itself needs to discover structure — for example, "find all documents that have a field linking to a Customer."

### Time-travel queries — querying historical state

Every WOQL query runs against a specific branch and commit. By specifying a commit identifier (or a timestamp-resolved commit), you query the exact state of the data at that point in time. This enables audit queries ("what was this customer's address last January?") and regression analysis without restoring backups or replaying logs.

## Can I use more than one?

Yes. The interfaces are not mutually exclusive — they hit the same versioned data store. A common pattern in production:

- **GraphQL** for frontend reads (product pages, dashboards, user-facing queries)
- **HTTP Document API** for write operations (form submissions, imports, bulk updates)
- **WOQL** for analytics, migrations, and operations that need branch or diff awareness

You can mix interfaces freely within the same application. A single database serves all three simultaneously.

## Performance considerations

All three interfaces read from and write to the same storage layer (immutable delta layers). Performance differences come from query complexity and result size, not from interface choice. A GraphQL query that fetches 1000 nested documents performs comparably to a WOQL query that returns the same data. The HTTP Document API has the lowest overhead for single-document operations because it skips query parsing entirely.

For large result sets, consider pagination (GraphQL `limit`/`offset`, WOQL `limit`, Document API `count`/`start`) to avoid loading entire collections into memory.

## Next steps

- [WOQL Overview](/docs/woql-explanation) — understand the Datalog foundations
- [WOQL Basics](/docs/woql-basics) — hands-on pattern matching with worked examples
- [GraphQL Basics](/docs/graphql-basics) — get started with GraphQL queries
- [Document API Tutorial](/docs/document-format-api-curl-tutorial) — CRUD operations with curl
- [WOQL Class Reference](/docs/woql-class-reference-guide) — complete API reference for all WOQL operations
- [Full WOQL Tutorial](/docs/how-to-query-with-woql) — 12-step hands-on guide

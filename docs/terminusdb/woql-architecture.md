---
title: "WOQL Architecture: AST Design & Security Model"
nextjs:
  metadata:
    title: WOQL Architecture — AST Design & Security Model
    description: How WOQL's AST-based architecture prevents injection attacks by construction, provides type safety across languages, and differs from string-based query approaches.
    keywords: woql architecture, ast query, query injection, sql injection prevention, type safe queries, woql security, datalog ast, json-ld query, query builder pattern
    alternates:
      canonical: https://terminusdb.org/docs/woql-architecture/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
tags:
  - woql
  - explanation
  - intermediate
---

This page explains **why** WOQL is designed the way it is — not how to write queries (see [WOQL Basics](/docs/woql-basics/) for that), but the architectural decisions that make WOQL fundamentally different from string-based query languages. If you have ever dealt with SQL injection, parameterised queries, or ORM query-builder bugs, this context explains why those problems do not exist in WOQL.

---

## The core idea: queries are data structures, not strings

Most query languages — SQL, Cypher, SPARQL — are defined as **text grammars**. You write a query as a string, send it to a server, and the server parses it. This creates a gap between the structure you intend and the text representation that reaches the server. That gap is where injection attacks, syntax errors, and composition bugs live.

WOQL takes a different approach. A WOQL query is an **Abstract Syntax Tree** (AST) — a structured JSON-LD object that the server receives directly. There is no parsing step, no string interpolation, and no text grammar. The client libraries provide builder functions that construct the AST programmatically:

```typescript
// This code builds a data structure — it does NOT build a string
import { triple, select } from "terminusdb"

const query = select(["v:Name", "v:Age"],
  triple("v:Person", "name", "v:Name"),
  triple("v:Person", "age", "v:Age")
)
```

The `query` variable now holds a JavaScript object — a tree of typed nodes — that serialises to JSON-LD for transmission:

```json
{
  "@type": "Select",
  "variables": ["Name", "Age"],
  "query": {
    "@type": "And",
    "and": [
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "name"},
        "object": {"@type": "Value", "variable": "Name"}
      },
      {
        "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Person"},
        "predicate": {"@type": "NodeValue", "node": "age"},
        "object": {"@type": "Value", "variable": "Age"}
      }
    ]
  }
}
```

The server receives a typed tree, validates its structure against the WOQL schema, and executes it. No text parsing is involved at any point.

---

## Security by construction

### Why string-based queries are vulnerable

In SQL, a query is a string. If user input is concatenated into that string, the user controls the query structure:

```python
# ❌ VULNERABLE: user input becomes part of query structure
username = request.params["user"]
query = f"SELECT * FROM users WHERE name = '{username}'"
# If username = "'; DROP TABLE users; --" → catastrophic
```

Parameterised queries (prepared statements) mitigate this by separating data from structure, but they require developer discipline. Every concatenation is a potential vulnerability. ORMs help but still generate strings internally.

### Why WOQL cannot be injected

In WOQL, user input is always a **value** in a typed node — it can never become a structural element of the query:

```typescript
// ✅ SAFE: userInput is always a data value, never query structure
const userInput = request.params.user  // Even if malicious

const query = triple("v:Person", "name", userInput)
```

This produces:

```json
{
  "@type": "Triple",
  "subject": {"@type": "NodeValue", "variable": "Person"},
  "predicate": {"@type": "NodeValue", "node": "name"},
  "object": {"@type": "Value", "data": {"@type": "xsd:string", "@value": "'; DROP TABLE users; --"}}
}
```

The malicious string is trapped inside a `data` node. It cannot escape into the query structure because **there is no string-to-structure boundary to cross**. The AST enforces separation by construction — not by convention, not by developer discipline, but by the type system itself.

### The security guarantee

| Attack vector | String-based (SQL/Cypher/SPARQL) | WOQL (AST) |
|---------------|----------------------------------|-------------|
| First-order injection | Possible (concatenation) | Impossible (no string boundary) |
| Second-order injection | Possible (stored strings reused in queries) | Impossible (values never become structure) |
| Parameter pollution | Possible in some ORMs | Impossible (typed nodes) |
| Operator injection | Possible (e.g., LIKE wildcards) | Impossible (operators are typed AST nodes) |

This is not a claim that WOQL applications are invulnerable to all security issues. Authentication, authorisation, and business logic bugs exist regardless of query language. But the entire **class** of injection vulnerabilities — the most common database attack vector — is eliminated by architectural design.

---

## Execution model

The execution boundary is clean and explicit:

```
┌──────────────────┐          JSON-LD           ┌──────────────────┐
│   Client (TS/Py) │  ──── AST over HTTP ────▶  │  TerminusDB      │
│                  │                             │  Server           │
│  Build AST       │                             │  Validate AST     │
│  Serialise JSON  │                             │  Execute query     │
│  Send request    │                             │  Return bindings   │
└──────────────────┘                             └──────────────────┘
```

1. **Client side** — The builder functions (`triple()`, `select()`, `and()`) construct an AST object in memory. No query execution happens on the client. The AST is serialised to JSON-LD and sent over HTTP.

2. **Server side** — TerminusDB receives the JSON-LD, validates it against the WOQL schema (checking that the AST is well-formed), then executes the query against the specified branch or commit. Results are returned as JSON bindings.

3. **No local execution** — The client never interprets or executes query logic. This means the server is the single point of query validation and access control. Client code cannot bypass server-side security checks.

### What the server validates

When the server receives a WOQL AST, it checks:

- Every node has a valid `@type` from the WOQL schema
- Required fields are present (e.g., `Triple` must have `subject`, `predicate`, `object`)
- Variable references are consistent
- The target branch or commit exists and the user has read access

If any check fails, the query is rejected before execution. Malformed ASTs produce clear error messages rather than undefined behaviour.

---

## Type safety across languages

Because WOQL's wire format is a JSON-LD schema, every client library — TypeScript, Python, Rust — generates the same AST structure. A query written in Python produces identical JSON-LD to the same query written in TypeScript:

**TypeScript:**
```typescript
const q = triple("v:X", "rdf:type", "@schema:Person")
```

**Python:**
```python
q = WOQLQuery().triple("v:X", "rdf:type", "@schema:Person")
```

**Both produce:**
```json
{
  "@type": "Triple",
  "subject": {"@type": "NodeValue", "variable": "X"},
  "predicate": {"@type": "NodeValue", "node": "rdf:type"},
  "object": {"@type": "NodeValue", "node": "@schema:Person"}
}
```

This uniformity means:
- Queries can be stored, shared, and replayed across languages
- Server-side tooling needs only one query representation
- Testing can verify the AST structure independent of client language
- Query templates (stored as JSON-LD) work with any client

---

## Comparison to other approaches

| Aspect | SQL | GraphQL | Cypher | SPARQL | WOQL |
|--------|-----|---------|--------|--------|------|
| **Representation** | Text grammar | Text grammar | Text grammar | Text grammar | JSON-LD AST |
| **Injection risk** | High (strings) | Low (typed schema) | Medium (strings) | Medium (strings) | None (by design) |
| **Composition** | String concat or ORM | Fragment merging | String concat | String concat | Function composition |
| **Cross-language** | Each DB has own dialect | Uniform spec | Neo4j-specific | Uniform spec | Same AST everywhere |
| **Expressiveness** | Full relational | Selection/mutation | Graph patterns | Full RDF | Full Datalog |
| **Type checking** | Runtime (DB rejects) | Schema validation | Runtime | Runtime | Client + server |
| **Stored queries** | Strings in DB | Persisted queries | Strings | Strings | JSON-LD documents |

### GraphQL comparison

GraphQL is the closest parallel — it also uses a typed schema to validate queries before execution. The key differences:

- **GraphQL** validates query *shape* (which fields you request) but uses a text grammar that must be parsed
- **WOQL** validates query *structure* (the AST itself) and never parses text
- **GraphQL** is primarily for data fetching with limited expressiveness (no recursive traversal, aggregation, or complex logic)
- **WOQL** is a full Datalog language with path queries, aggregation, unification, and mathematical operations

TerminusDB supports both GraphQL (for simple data fetching) and WOQL (for complex queries). See [Choosing a Query Interface](/docs/querying-terminusdb/) for when to use which.

---

## Implications for application design

### You can safely pass user input to WOQL builders

Because values cannot escape into structure, you can construct queries from user input without sanitisation:

```typescript
// Safe — searchTerm is always a value, never structure
const searchByName = (searchTerm: string) =>
  select(["v:Doc"],
    triple("v:Doc", "name", searchTerm)
  )
```

### Queries are first-class data

Since queries are JSON-LD objects, you can:
- Store them in TerminusDB itself (queries about queries)
- Version them alongside your data (branch-aware query history)
- Compose them dynamically at runtime without string manipulation
- Serialise them for logging, debugging, or replay

### No need for an ORM

ORMs exist primarily to provide safe query construction over string-based languages. Because WOQL's builders ARE the query language (not a wrapper around strings), there is no ORM layer to add, maintain, or debug. The builder IS the interface.

---

## Further reading

- [WOQL Explained](/docs/woql-explanation/) — What WOQL is and how it differs from SQL
- [WOQL Basics](/docs/woql-basics/) — Hands-on pattern matching and traversal
- [JSON-LD Queries](/docs/woql-json-ld-queries/) — Work directly with the AST wire format
- [What is Datalog?](/docs/what-is-datalog/) — The formal foundations behind WOQL
- [What is Unification?](/docs/what-is-unification/) — How variable binding works
- [Choosing a Query Interface](/docs/querying-terminusdb/) — When to use WOQL vs GraphQL vs HTTP API

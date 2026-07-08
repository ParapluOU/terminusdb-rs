---
tags:
  - woql
  - explanation
  - advanced
title: How WOQL Finds and Streams Solutions
nextjs:
  metadata:
    title: How WOQL Finds and Streams Solutions
    keywords: woql backtracking streaming ndjson chunked encoding datalog query processing solutions bindings knowledge graph goal-seeking
    description: Understand how TerminusDB's Datalog engine uses goal-seeking backtracking to find solutions in the knowledge graph, how results can be collected or streamed as ndjson with chunked transfer encoding, and when to use each approach.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-query-streaming/
media: []
---

When you send a WOQL query to TerminusDB, something fundamentally different from a SQL database happens under the hood. Instead of scanning tables and assembling rows, the Datalog engine embarks on a *goal-seeking search* through the knowledge graph, discovering solutions one at a time through a process called **backtracking**. Each solution is a complete, self-consistent set of variable bindings that satisfies every constraint in your query.

This page explains how that search works, why it matters for how you design queries, and how TerminusDB lets you choose between collecting all solutions into a single response or streaming them progressively as they are found.

> **Prerequisites:** Familiarity with [What is Datalog?](/docs/what-is-datalog/) and [What is Unification?](/docs/what-is-unification/) will help, but isn't required.

---

## The Search: How Backtracking Finds Solutions

### One solution at a time

A WOQL query is a logical goal. The engine's job is to find every combination of variable bindings that makes that goal true. It does this by attempting to satisfy each predicate in the query left-to-right, and when it reaches a dead end, it *backtracks* to the most recent choice point and tries the next alternative.

Consider a simple two-hop query:

```javascript
WOQL.and(
  WOQL.triple("v:Person", "works_at", "v:Company"),
  WOQL.triple("v:Company", "located_in", "v:City")
)
```

The engine processes this as follows:

1. **Enter the first `triple`** — Find all `(Person, Company)` pairs where `Person works_at Company`. Pick the first one.
2. **Enter the second `triple`** — With `v:Company` now bound, look for `(Company, City)` pairs where `Company located_in City`. If one exists, *emit a solution* with all three variables bound.
3. **Backtrack into the second `triple`** — Are there more cities for this company? If so, emit another solution.
4. **Backtrack into the first `triple`** — No more cities. Try the next `(Person, Company)` pair from step 1 and repeat.
5. **Exhaust all choices** — When no more `(Person, Company)` pairs remain, the search is complete.

Each time the engine successfully binds all variables, that is one **solution**. The total number of solutions is not known in advance — it depends on how many paths through the knowledge graph satisfy the query.

### Choice points and the search tree

Every predicate that can match multiple facts in the graph creates a **choice point** — a fork in the search. The engine explores one branch, and if it leads to a dead end (a later predicate fails), it returns to the choice point and tries the next branch. This is backtracking.

```
triple(v:Person, works_at, v:Company)
├── Person=alice, Company=acme
│   └── triple(acme, located_in, v:City)
│       ├── City=london  → Solution 1 ✓
│       └── City=paris   → Solution 2 ✓
├── Person=bob, Company=widgets_inc
│   └── triple(widgets_inc, located_in, v:City)
│       └── City=berlin  → Solution 3 ✓
└── Person=carol, Company=acme
    └── triple(acme, located_in, v:City)
        ├── City=london  → Solution 4 ✓
        └── City=paris   → Solution 5 ✓
```

The engine walks this tree depth-first. It never builds the entire tree in memory — it follows one path at a time, backtracks, and follows the next. This is what makes Datalog memory-efficient even on large graphs: the working memory is proportional to the *depth* of the search (the number of predicates), not the *breadth* (the number of solutions).

### Why this matters for query design

Understanding backtracking changes how you think about query performance:

- **Predicate order affects efficiency.** Place the most selective predicate first. If you know the person's name, start there — it eliminates most branches immediately.
- **Shared variables prune the search.** When `v:Company` appears in both `triple` predicates, the second predicate only searches facts for the company already bound by the first. This is automatic join optimization through unification.
- **Generators expand, filters contract.** A `triple` with all variables unbound generates every edge in the graph. A `triple` with the subject bound generates only edges from that node. Adding constraints narrows the search.

---

## Two Ways to Receive Solutions

TerminusDB offers two modes for returning query results. The choice depends on whether you need all results at once or want to process them progressively.

### Collected mode (default)

In collected mode, the engine finds *all* solutions through backtracking, gathers them into a list, and returns them as a single JSON response. This is the default behavior.

```javascript
// Client sends:
{
  "query": { ... },
  "streaming": false       // default — can be omitted
}

// Server responds with a single JSON object:
{
  "@type": "api:WoqlResponse",
  "api:status": "api:success",
  "api:variable_names": ["Person", "Company", "City"],
  "bindings": [
    { "Person": "person/alice", "Company": "company/acme", "City": "city/london" },
    { "Person": "person/alice", "Company": "company/acme", "City": "city/paris" },
    { "Person": "person/bob",   "Company": "company/widgets_inc", "City": "city/berlin" }
  ],
  "inserts": 0,
  "deletes": 0
}
```

**Internally**, the engine uses Prolog's `findall` to collect all solutions produced by backtracking into a list, then serializes the entire list as JSON.

**When to use collected mode:**

- Result sets are small to medium (hundreds to low thousands of bindings)
- You need all results before processing (sorting, aggregation, display)
- You want a simple request/response interaction

### Streaming mode

In streaming mode, the engine writes each solution to the HTTP response *as it is found* during backtracking. The client receives results progressively, without waiting for the full search to complete.

```javascript
// Client sends:
{
  "query": { ... },
  "streaming": true
}
```

The server responds with `Transfer-Encoding: chunked` and writes **newline-delimited JSON (ndjson)** — one JSON object per line:

```
{"@type":"PrefaceRecord","names":["Person","Company","City"]}
{"@type":"Binding","Person":"person/alice","Company":"company/acme","City":"city/london"}
{"@type":"Binding","Person":"person/alice","Company":"company/acme","City":"city/paris"}
{"@type":"Binding","Person":"person/bob","Company":"company/widgets_inc","City":"city/berlin"}
{"@type":"PostscriptRecord","status":"success","inserts":0,"deletes":0}
```

**Internally**, the engine uses Prolog's `forall` instead of `findall`. Each time backtracking produces a solution, it is immediately serialized and flushed to the HTTP response stream. The solution is then discarded from memory before the engine backtracks to find the next one.

**When to use streaming mode:**

- Result sets are large (thousands to millions of bindings)
- You want to display or process results as they arrive (progressive loading)
- Memory efficiency matters — solutions are not accumulated server-side
- You need first-result latency to be low, even if the full query takes time

---

## The ndjson Stream Format

The streaming response is a well-defined three-phase protocol. Each phase serves a specific purpose.

### Phase 1: Preface record

The first line declares the variable names in the query, so the client knows what fields to expect in each binding:

```json
{"@type":"PrefaceRecord","names":["Person","Company","City"]}
```

This arrives immediately after the query is compiled, before the search begins. A client can use it to set up column headers or allocate data structures.

### Phase 2: Binding records

Zero or more lines, one per solution found by backtracking:

```json
{"@type":"Binding","Person":"person/alice","Company":"company/acme","City":"city/london"}
{"@type":"Binding","Person":"person/bob","Company":"company/widgets_inc","City":"city/berlin"}
```

Each line is a complete, self-contained JSON object. Unbound variables appear as `null`. The records arrive in the order the engine discovers them — depth-first through the search tree.

### Phase 3: Postscript record

The final line signals completion and includes transaction metadata:

```json
{"@type":"PostscriptRecord","status":"success","inserts":0,"deletes":0,"transaction_retry_count":0}
```

If the query includes write operations (`insert`, `delete`), the `inserts` and `deletes` counts reflect the total mutations committed. A `version` field may also be present for data versioning.

### Error handling in streaming mode

If an error occurs during the search, it is written as a JSON error object on the stream in place of the postscript:

```json
{"@type":"api:ErrorResponse","api:error":{ ... },"api:status":"api:failure"}
```

Because the HTTP status code (`200`) and headers have already been sent by the time streaming begins, the client must inspect the `@type` field of the last line to determine success or failure.

---

## Chunked Transfer Encoding: How It Works on the Wire

Standard HTTP responses require a `Content-Length` header, which means the server must know the total response size before sending the first byte. For streaming queries, the total size is unknowable in advance — it depends on how many solutions backtracking discovers.

**Chunked transfer encoding** solves this. The server sends the response in pieces (chunks), each prefixed with its size. The client reassembles them into a continuous stream. The final empty chunk signals the end of the response.

```
HTTP/1.1 200 OK
Content-Type: application/json
Transfer-Encoding: chunked

4f\r\n
{"@type":"PrefaceRecord","names":["Person","Company","City"]}\n
\r\n
5a\r\n
{"@type":"Binding","Person":"person/alice","Company":"company/acme","City":"city/london"}\n
\r\n
0\r\n
\r\n
```

In practice, most HTTP client libraries handle chunked decoding transparently. The application code simply reads lines from the response stream.

---

## Consuming a Streaming Response

### JavaScript (fetch + ndjson)

```javascript
const response = await fetch('/api/woql/myorg/mydb', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    query: { "@type": "Triple",
             "subject": { "@type": "NodeValue", "variable": "Person" },
             "predicate": { "@type": "NodeValue", "node": "works_at" },
             "object":  { "@type": "Value", "variable": "Company" } },
    streaming: true
  })
});

const reader = response.body.getReader();
const decoder = new TextDecoder();
let buffer = '';

while (true) {
  const { done, value } = await reader.read();
  if (done) break;

  buffer += decoder.decode(value, { stream: true });
  const lines = buffer.split('\n');
  buffer = lines.pop();  // keep incomplete last line in buffer

  for (const line of lines) {
    if (!line.trim()) continue;
    const record = JSON.parse(line);

    switch (record['@type']) {
      case 'PrefaceRecord':
        console.log('Variables:', record.names);
        break;
      case 'Binding':
        console.log('Solution:', record);
        break;
      case 'PostscriptRecord':
        console.log('Done. Inserts:', record.inserts, 'Deletes:', record.deletes);
        break;
    }
  }
}
```

### Python (requests + streaming)

```python
import requests
import json

response = requests.post(
    'http://localhost:6363/api/woql/myorg/mydb',
    json={
        "query": { ... },
        "streaming": True
    },
    stream=True
)

for line in response.iter_lines(decode_unicode=True):
    if not line.strip():
        continue
    record = json.loads(line)

    if record['@type'] == 'PrefaceRecord':
        print('Variables:', record['names'])
    elif record['@type'] == 'Binding':
        print('Solution:', record)
    elif record['@type'] == 'PostscriptRecord':
        print(f"Done. Inserts: {record['inserts']}, Deletes: {record['deletes']}")
```

### curl (line-by-line)

```bash
curl -s -X POST http://localhost:6363/api/woql/myorg/mydb \
  -H 'Content-Type: application/json' \
  -d '{"query": { ... }, "streaming": true}' \
  | while IFS= read -r line; do
    echo "$line" | python3 -m json.tool
  done
```

---

## Collected vs Streaming: Decision Guide

| Concern | Collected | Streaming |
|---------|-----------|-----------|
| **Result set size** | Small–medium | Large or unbounded |
| **First-result latency** | Must wait for all results | Immediate as found |
| **Server memory** | All bindings held in memory | One binding at a time |
| **Client complexity** | Simple JSON parse | ndjson line parser needed |
| **Error reporting** | HTTP status code + JSON body | Must inspect last ndjson line |
| **Write operations** | Transaction metadata in response | Transaction metadata in postscript |
| **Data versioning** | `TerminusDB-Data-Version` header | `version` field in postscript |

### Rule of thumb

Use **collected** mode for interactive queries, dashboards, and APIs where consumers expect a complete JSON response. Use **streaming** mode for data exports, ETL pipelines, large analytics queries, and any scenario where you want to start processing results before the query finishes.

---

## Under the Hood: From Query to Solutions

Here is the full lifecycle, from API call to response, showing where backtracking and streaming fit in.

### 1. Parse and compile

The JSON query body is deserialized into a WOQL AST (abstract syntax tree), then compiled into an executable Prolog program. Variable names are extracted for the preface record.

### 2. Open a transaction

The engine opens an immutable read snapshot of the knowledge graph (or a read-write transaction if the query contains mutations). This snapshot ensures consistent results even if other writes occur during the query.

### 3. Execute with backtracking

The compiled program runs against the snapshot. Each `triple`, `path`, `sequence`, or other generator predicate creates choice points. The engine explores the search tree depth-first, backtracking when predicates fail.

**In collected mode:** Every solution is accumulated via `findall`. The engine backtracks until no more solutions exist, then the entire binding list is serialized as JSON.

**In streaming mode:** Each solution is immediately serialized and written to the HTTP response via `forall`. The solution's memory is reclaimed before the engine backtracks for the next one.

### 4. Commit and finalize

If the query included write operations, the transaction is committed. In collected mode, the full response JSON includes the transaction metadata. In streaming mode, the postscript record carries it.

---

## Practical Patterns

### Progressive UI loading

Stream results to a frontend that renders rows as they arrive. The preface record provides column names; each binding record adds a row. Users see data immediately instead of staring at a spinner.

### Memory-bounded data export

Export millions of documents without the server holding them all in memory. Combine streaming mode with a query that selects documents:

```javascript
{
  "query": {
    "@type": "And",
    "and": [
      { "@type": "Triple",
        "subject": {"@type": "NodeValue", "variable": "Doc"},
        "predicate": {"@type": "NodeValue", "node": "rdf:type"},
        "object": {"@type": "NodeValue", "node": "@schema:LargeDataset"} },
      { "@type": "ReadDocument",
        "identifier": {"@type": "NodeValue", "variable": "Doc"},
        "document": {"@type": "Value", "variable": "Content"} }
    ]
  },
  "streaming": true
}
```

Each document is read, serialized, and flushed independently — the server never holds more than one document in memory at a time.

### Combining streaming with `limit`

You can combine streaming with `limit` to get the first N results as fast as possible:

```javascript
{
  "query": {
    "@type": "Limit",
    "limit": 100,
    "query": { ... }
  },
  "streaming": true
}
```

The engine stops backtracking after 100 solutions, writes the postscript, and closes the stream. This gives you streaming's low first-result latency with a bounded result set.

---

## Summary

- **Backtracking** is the Datalog engine's mechanism for finding solutions: it walks the search tree depth-first, exploring branches and retreating from dead ends.
- **Each solution** is a complete set of variable bindings that satisfies all predicates in the query.
- **Collected mode** gathers all solutions into a single JSON response — simple and familiar.
- **Streaming mode** writes each solution as an ndjson line with chunked transfer encoding — memory-efficient and low-latency.
- The ndjson stream has three phases: **preface** (variable names), **bindings** (one per solution), and **postscript** (status and metadata).
- Choose collected for small results and simple clients. Choose streaming for large results, progressive loading, and memory-bounded pipelines.

## Further Reading

- [What is Datalog?](/docs/what-is-datalog/) — the declarative query model behind WOQL
- [What is Unification?](/docs/what-is-unification/) — how variables get bound to values
- [WOQL Query Language](/docs/woql-explanation/) — the full WOQL language overview
- [Query with WOQL](/docs/how-to-query-with-woql/) — practical WOQL query how-to guides

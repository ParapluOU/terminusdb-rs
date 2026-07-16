# terminusdb-sparql

Query TerminusDB with **SPARQL**. This crate compiles a SPARQL `SELECT` query
**entirely** into a [WOQL](https://terminusdb.com/docs/woql-explanation/) query
(`terminusdb_woql2::Query`) — no Prolog, no server extension. The compiled query
runs through the normal client (`client.query(spec, compiled.query)`).

SPARQL and WOQL share the same triple/graph data model, so the mapping is
direct and the compiler is small.

```text
SPARQL string ─[spargebra]→ algebra ─[lower]→ ir::SparqlQuery ─[compile]→ woql2::Query ─[client]→ bindings
```

We reuse the spec-grade [`spargebra`](https://crates.io/crates/spargebra) parser
(from the oxigraph ecosystem) as the front end, project its algebra onto a narrow
[`ir`], and lower that to WOQL. This mirrors the sibling `terminusdb-xpath` and
`terminusdb-sql` experiments.

## Quick start

```rust
use std::collections::HashMap;

let compiled = terminusdb_sparql::compile(
    r#"PREFIX s: <http://terminusdb.com/schema#>
       SELECT ?name WHERE { ?p a s:Person . ?p s:name ?name }"#,
)?;

// compiled.query is a woql2::Query; run it with the client:
let res = client
    .query::<HashMap<String, serde_json::Value>>(Some(spec), compiled.query)
    .await?;
// compiled.variables names the projected variables (here: ["name"]).
```

## Data-model mapping

SPARQL uses full IRIs; TerminusDB's instance graph uses `@schema:`-prefixed
predicates/classes and `rdf:type`. By default, IRIs under
`http://terminusdb.com/schema#` map onto the `@schema:` prefix, so a natural
query reads:

```sparql
PREFIX s: <http://terminusdb.com/schema#>
SELECT ?name WHERE {
  ?p a s:Person .        # `a` is rdf:type  → triple(?p, rdf:type, @schema:Person)
  ?p s:name ?name .      # s:name          → triple(?p, @schema:name, ?name)
}
```

| SPARQL                       | WOQL                                            |
|------------------------------|-------------------------------------------------|
| `?p a s:Person`              | `triple(?p, rdf:type, @schema:Person)`          |
| `?p s:name ?n`               | `triple(?p, @schema:name, ?n)`                  |
| `{ A } { B }` (join)         | `and(A, B)`                                      |
| `{ A } UNION { B }`          | `or(A, B)`                                       |
| `OPTIONAL { ... }`           | `opt(...)`                                       |
| `FILTER(?a = "x")`           | `eq(?a, "x")`                                    |
| `FILTER(?a > 26)`            | `greater(?a, 26)`                                |
| `FILTER(A && B)`             | conjunction (sibling goals)                     |
| `FILTER(A \|\| B)`           | `or(A, B)`                                       |
| `FILTER(!A)`                 | `not(A)`                                         |
| `ORDER BY DESC(?a)`          | `order_by([desc(?a)], ...)`                     |
| `LIMIT n` / `OFFSET k`       | `limit(n, ...)` / `start(k, ...)`               |
| `SELECT DISTINCT`            | `distinct([...], ...)`                          |

The [`CompileOptions::schema_base`] namespace is configurable. Well-known
`rdf:`/`rdfs:`/`xsd:` IRIs collapse to their prefixed form; anything else (e.g. a
full `terminusdb:///data/...` instance IRI) passes through verbatim.

## A real join across a graph edge

If a model field is a `Ref<T>` (a traversable link), a two-triple SPARQL pattern
becomes a genuine join:

```sparql
PREFIX s: <http://terminusdb.com/schema#>
SELECT ?name ?cn WHERE {
  ?p s:name ?name .
  ?p s:employer ?c .     # follow the employer edge to the company node
  ?c s:name ?cn .
  FILTER(?cn = "Acme Corp")
}
```
compiles to
```text
select([$name, $cn], and(
  triple($p, "@schema:name", $name),
  triple($p, "@schema:employer", $c),
  triple($c, "@schema:name", $cn),
  eq($cn, "Acme Corp")))
```

## Inspecting the compiled WOQL

`terminusdb_sparql::explain(sparql)` returns an [`Explanation`] with the WOQL DSL
and JSON-LD — the first debugging tool when a query returns nothing:

```rust
println!("{}", terminusdb_sparql::explain(sparql)?.report());
```

The `cargo run -p terminusdb-sparql --example explore` example dumps the compiled
WOQL for a set of queries with no database; `--example showcase --features client`
runs them against a live embedded TerminusDB.

## Supported subset

`SELECT` with: basic graph patterns (triple patterns, `a`/rdf:type), `SELECT *`,
`JOIN`, `OPTIONAL`, `UNION`, `FILTER` (`= != < <= > >=`, `&&`, `||`, `!`),
`ORDER BY ?v` (asc/desc), `LIMIT`, `OFFSET`, `DISTINCT`, string/integer/decimal/
double/boolean literals. Anything else is rejected as
`SparqlError::Unsupported` naming the construct. See `ROADMAP.md`.

## Testing

- `tests/compile_tests.rs` — offline WOQL-shape assertions (fast, hermetic).
- `tests/spec.rs` — the live behavioural suite: compiles each query, runs it
  against a real embedded `TerminusDBServer`, and asserts the returned bindings.
  This is the only proof that a query "works".

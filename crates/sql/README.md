# terminusdb-sql

Query TerminusDB with SQL. This crate compiles SQL `SELECT` statements to WOQL and
runs them against TerminusDB — **no SQL executes in-process; TerminusDB is the only
engine**. It reuses DataFusion's *logical* layer as the parser, name-resolver, and
type-checker, then translates the resulting plan to WOQL.

```text
SQL string ─[datafusion-sql]→ LogicalPlan ─[emit]→ woql2::Query ─[client]→ rows
```

## Quickstart

```rust
use terminusdb_sql::Session;

// `client` implements the CatalogBackend trait (behind the `client` feature).
let session = Session::open(client, "my_db", Some("main")).await?;

let result = session.run("SELECT title, year FROM book WHERE year > 1950").await?;
for row in result.to_json_rows() {
    println!("{row:?}");
}
```

- `Session::open` validates db/branch/auth, resolves the branch to a concrete
  commit, reads the schema at that commit, and builds the catalog. Every query in
  the session is pinned to that commit (repeatable reads; schema drift is a
  session-level "reconnect", not a per-query concern).
- `Session::compile(sql)` gives you the WOQL without executing; `Session::run(sql)`
  executes and decodes rows; `terminusdb_sql::explain(sql, catalog)` renders the
  plan + WOQL for inspection.

## See it work

```text
cargo run -p terminusdb-sql --example showcase --features client
```

Boots an embedded TerminusDB, seeds a small Author/Book graph, and prints, for a
progression of queries, the **SQL**, the **compiled WOQL**, and the **live result
table** — including a real object-property join and a left join producing SQL NULL.

## Schema → tables

Each concrete document class becomes a table. A synthetic `iri` column exposes the
document's subject IRI. Datatype properties become typed columns; **object
properties** (fields typed `Ref<T>` / `TdbLazy<T>`) become IRI foreign keys you
join against the target's `iri`. A field typed `EntityIDFor<T>` is a *value*
(`xsd:string`), not a link.

Non-representable properties (multi-valued, subdocument-ranged, unsupported
datatypes) are omitted **with a recorded reason**, so selecting one yields
`column X exists but is unsupported (…)`, never "no such column".

## Supported subset (v1)

Single-class `SELECT` with projection, `WHERE` (`= <> < <= > >=`, `AND`/`OR`),
`ORDER BY`, `LIMIT`/`OFFSET`, `DISTINCT`, and inner + left equijoins across object
properties. Anything outside the subset returns an error naming the construct —
never an approximation. See `ROADMAP.md` for the full status, including what is
deferred (aggregates, `IS NULL`, subqueries, …).

## Design notes

- **We do not run DataFusion's optimizer.** The raw `SqlToRel` plan is the IR
  (stable, testable); the emitter prunes to referenced columns itself.
- **Emission order is execution order** — WOQL has no cost-based optimizer, so the
  emitter is the join planner (v1: emit in plan order).
- **SQL NULL vs datalog absence:** a missing property is *absent*, not NULL. Only at
  the projection boundary do we reconcile — optional columns are wrapped in
  `optional(...)` so the row survives and the cell decodes to NULL.
- The compiler core depends only on `datafusion-sql`/`-expr`/`-common` +
  `terminusdb-woql2`/`-schema`; the HTTP backend is optional (`client` feature).

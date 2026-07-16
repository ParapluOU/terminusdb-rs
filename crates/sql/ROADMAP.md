# terminusdb-sql — coverage roadmap

Compiles SQL `SELECT` statements to WOQL and runs them against TerminusDB. The
compiler consumes DataFusion's *logical* layer as its IR; TerminusDB is the only
execution engine. Legend: ✅ supported · 🚧 partial · ❌ not yet · 🚫 out of scope.

## Catalog (schema → tables)

- ✅ Concrete document class → one table
- ✅ Datatype property → typed value column
- ✅ Object property → `Utf8` IRI foreign key (joinable to the target's `iri`)
- ✅ Enum property → `Utf8` column (allowed values recorded)
- ✅ Synthetic `iri` column (the subject IRI); the model's own `id` field is a
  normal column alongside it
- ✅ Inheritance closure (inherited properties merged; abstract parent is not a table)
- ✅ Identifier mangling with hard collision errors (never last-write-wins)
- ✅ Total xsd → Arrow type map (no silent `Utf8` fallback)
- ✅ Omitted properties recorded with a reason → precise "unsupported column" error
- ✅ Commit pinning (schema read + queries pinned to one commit; `Session::check_drift`)
- 🚫 Multi-valued (Set/List/Array) properties — rejected at load with a reason
- 🚫 Subdocument classes as tables — rejected with a reason
- 🚫 Abstract classes as tables — rejected with a reason
- ❌ `xsd:duration` / partial-date (`gYear`, …) columns — rejected (no faithful SQL type)
- ❌ `sys:JSON` columns

## Query (SELECT → WOQL)

- ✅ Single-class projection, `SELECT *`
- ✅ `WHERE`: `=`, `<>`, `<`, `<=`, `>`, `>=`, `AND`, `OR`, `NOT`
- ✅ Literals: string, integer, float/double, boolean, date, datetime, time
- ✅ `ORDER BY <column> [ASC|DESC]`
- ✅ `LIMIT` / `OFFSET`
- ✅ `SELECT DISTINCT`
- ✅ Bag semantics preserved (no dedupe unless `DISTINCT`)
- ✅ Inner equijoin (`JOIN … ON a.ref = b.iri`, and value equijoins)
- ✅ `LEFT JOIN` (nullable side wrapped in `Optional`)
- ✅ Nullable column selection via `Optional` (absent → SQL NULL)
- 🚧 `NULLS FIRST/LAST` on ORDER BY — parsed, not expressible in WOQL (ignored)
- ❌ Aggregates (`COUNT`, `SUM`, `GROUP BY`, `HAVING`) — phase 2 (designed in `emit`)
- ❌ Computed / arithmetic projections and predicates
- ❌ `IS NULL` / `IS NOT NULL` / `NOT IN` / three-valued logic
- ❌ Scalar functions, `LIKE`, `CASE`, `BETWEEN`, `IN (list)`
- ❌ Subqueries, `UNION` / set operations
- ❌ `RIGHT` / `FULL` / semi / anti joins
- 🚫 Class subsumption (concrete classes only in v1)
- 🚫 Any DML / DDL (read-only compiler)

## Notes

- Emission order is execution order — WOQL has no cost-based optimizer, so the
  emitter is the join planner (v1: emit in plan order).
- We do **not** run DataFusion's optimizer; the raw `SqlToRel` plan is the IR, and
  the emitter prunes to referenced columns itself.
- Any unsupported node/expression returns `SqlError::Unsupported` naming the
  construct — never an approximation.

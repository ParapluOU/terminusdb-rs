# terminusdb-sparql — coverage & roadmap

Legend: ✅ supported · 🚧 partial · ❌ not yet · 🚫 out of scope (v1)

The compiler targets SPARQL `SELECT`. Everything outside the supported subset is
rejected as `SparqlError::Unsupported` / `SparqlError::UnsupportedForm` naming the
construct — never a silent approximation.

## Query form

| Form        | Status | Notes                                            |
|-------------|:------:|--------------------------------------------------|
| `SELECT`    | ✅     | The compile target.                              |
| `ASK`       | ❌     | Could map to `once(...)` + a boolean; not yet.   |
| `CONSTRUCT` | 🚫     | Builds RDF, not a solution sequence.             |
| `DESCRIBE`  | 🚫     | Server-defined semantics.                        |

## Graph patterns (the `WHERE` clause)

| Construct                         | Status | WOQL image                          |
|-----------------------------------|:------:|-------------------------------------|
| Basic graph pattern (triples)     | ✅     | `and(triple, ...)`                  |
| `a` / rdf:type shorthand          | ✅     | `triple(?s, rdf:type, @schema:C)`   |
| Variable predicate `?s ?p ?o`     | ✅     | `triple` with a predicate variable  |
| Group join `{ A } { B }`          | ✅     | flattened conjunction               |
| `OPTIONAL { ... }`                | ✅     | `opt(...)`                          |
| `OPTIONAL { ... FILTER(c) }`      | ✅     | filter folded inside the `opt`      |
| `{ A } UNION { B }`               | ✅     | `or(A, B)`                          |
| `FILTER(...)`                     | ✅     | see below                           |
| Property paths (`/ ^ * + ?`)      | ❌     | maps naturally to WOQL `path(...)`; next up |
| `MINUS`                           | ❌     | needs negation-as-difference        |
| `BIND` / `VALUES`                 | ❌     | `BIND` → `eval`/`eq`; `VALUES` → `or` of `eq` |
| `GRAPH` (named graphs)            | ❌     | WOQL graph selection exists         |
| Sub-`SELECT`                      | ❌     | —                                   |
| `SERVICE` (federation)            | 🚫     | —                                   |
| Blank nodes / RDF-star            | 🚫     | —                                   |

## FILTER expressions

| Operator                                  | Status | WOQL image                    |
|-------------------------------------------|:------:|-------------------------------|
| `=` (and `sameTerm`)                      | ✅     | `eq` (node-aware)             |
| `!=`                                       | ✅     | `not(eq(...))`                |
| `<` `<=` `>` `>=`                          | ✅     | `less`/`lte`/`greater`/`gte` |
| `&&`                                       | ✅     | conjunction (sibling goals)   |
| `\|\|`                                     | ✅     | `or(...)`                     |
| `!`                                        | ✅     | `not(...)`                    |
| `BOUND`, `IN`, `EXISTS`, `IF`, `COALESCE` | ❌     | —                             |
| Arithmetic (`+ - * /`)                    | ❌     | maps to WOQL `eval`           |
| Built-in functions (`REGEX`, `STR`, ...)  | ❌     | some map to WOQL string ops   |

## Solution modifiers

| Modifier                    | Status | WOQL image                       |
|-----------------------------|:------:|----------------------------------|
| Projection / `SELECT *`     | ✅     | `select([...], ...)`             |
| `DISTINCT`                  | ✅     | `distinct([...], ...)`           |
| `REDUCED`                   | 🚧     | treated as a no-op (keeps dups)  |
| `ORDER BY ?v` (asc/desc)    | ✅     | `order_by([asc/desc(?v)], ...)`  |
| `ORDER BY expr(...)`        | ❌     | only bare variables for now      |
| `LIMIT` / `OFFSET`          | ✅     | `limit(n, ...)` / `start(k, ...)`|
| `GROUP BY` / aggregates     | ❌     | WOQL has `count`/`sum`/`group_by`|
| `HAVING`                    | ❌     | —                                |
| Select-expressions (`AS`)   | ❌     | `BIND`-like; maps to `eval`      |

## Literals

| Datatype                                     | Status |
|----------------------------------------------|:------:|
| `xsd:string` / plain / language-tagged       | ✅     |
| `xsd:integer` (& int/long/short/unsigned...) | ✅     |
| `xsd:decimal`                                | ✅     |
| `xsd:double` / `xsd:float`                   | ✅     |
| `xsd:boolean`                                | ✅     |
| `xsd:dateTime` / `date` / `time`             | 🚧     | kept lexical (string) for now   |
| Other typed literals                         | 🚧     | kept lexical (string)           |

## IRI mapping

- IRIs under `CompileOptions::schema_base` (default
  `http://terminusdb.com/schema#`) → `@schema:local`.
- `rdf:` / `rdfs:` / `xsd:` well-known IRIs → their prefixed form.
- Everything else (e.g. `terminusdb:///data/...`) passes through verbatim.

## Design notes

- **spargebra is the parser + normalizer.** Its algebra is already resolved and
  regularized, so `lower.rs` just projects the supported subset onto the narrow
  [`crate::ir`]; `compile.rs` lowers the IR to WOQL.
- **Emission order = execution order.** WOQL has no cost optimizer, so the
  compiler emits triples in pattern order.
- Nothing is considered "working" until `tests/spec.rs` runs it against a real
  TerminusDB and checks the bindings.

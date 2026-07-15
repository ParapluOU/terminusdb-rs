# terminusdb-xpath

Compile XPath expressions into TerminusDB **WOQL** queries.

Navigate TerminusDB documents/graphs with familiar XPath syntax; the expression
is compiled **entirely** into a `terminusdb_woql2::Query` and run through the
normal client — no Prolog, no server extension.

```
XPath string → xee-xpath-ast AST → ir::XPathQuery → terminusdb_woql2::Query
```

The XPath parser is reused from [`xee-xpath-ast`](https://crates.io/crates/xee-xpath-ast)
(vendored for reference under `modules/x-rs`); this crate only lowers a supported
subset onto WOQL. See [`ROADMAP.md`](./ROADMAP.md) for feature coverage.

## Quick start

```rust
let compiled = terminusdb_xpath::compile(
    r#"document("Person/jane")/address/@city"#,
)?;
// compiled.query is a woql2::Query; run it with the client:
let res = client.query::<HashMap<String, Value>>(Some(spec), compiled.query).await?;
// compiled.result_var names the projected variable.
```

Data-model mapping: a child step `foo` follows an **object property** (a graph
hop), `@foo` reads a **value property** (a literal), and `//foo` reaches `foo`
through any chain of edges.

## Building queries in Rust (no strings)

The [`builder`] module constructs paths with `/` operator overloading and
compiles straight to WOQL (no text parsing). `doc::<T>()` takes the typed id.

```rust
use terminusdb_xpath::builder::{doc, child, attr, descendant};

// document("Person/jane")/employer[@founded > 1990]/@name
let compiled = (doc(jane_id)                       // doc::<Person>(impl Into<EntityIDFor<T>>)
    / child("employer").filter(attr("founded").gt(1990))
    / attr("name"))
    .compile()?;

// descendant (`a//city`) — `//` can't be written literally (it's a Rust line
// comment), so use `descendant(...)` or the `>>` operator:
let compiled = (doc(acme_id) / descendant("city")).compile()?;
let compiled = (doc(acme_id) >> child("city")).compile()?;   // same thing
```

- Navigation is `/`; **descendant** (`//`) is `>>` (or `descendant(...)`).
- Predicates are `.filter(pred)` where `pred` is
  `attr("x").eq(v)` / `.ne` / `.lt` / `.le` / `.gt` / `.ge` / `.exists()`.
- `[...]` predicate syntax is **not** offered: Rust's `Index` must return a
  reference into the receiver, so it can't grow a builder — hence `.filter(...)`.

## Typed results

`client.query::<T>(…)` is generic over the result type, so you don't have to stop
at `HashMap<String, serde_json::Value>`:

- **Scalars** — deserialize the projected variable (unwrap its `@value`) into a
  concrete type: `Vec<i64>` from `@salary`, `Vec<String>` from `.../@city`.
- **Whole models** — when a path ends at a *node* (a child step, or a bare
  `document("Employee/alice")`), [`CompiledXPath::read_documents_query`] wraps the
  query in a WOQL `read_document`, so each result deserializes into a full
  `#[derive(TerminusDBModel)]` value via `M::from_json` — nested subdocuments and
  all. The XPath effectively *filters out the models for you*.

See `examples/showcase.rs` (the "Typed result sets" section) for runnable
`run_typed::<T>` and `read_models::<M>` helpers.

## Working with document IRIs

`document(...)` accepts **either** a short id (`MyModel/1234`) **or** a full IRI
(`terminusdb:///data/MyModel/1234`); both resolve server-side.

When you build those id/IRI strings in Rust, **do not hand-format them** with
`format!` / string concatenation. Use the typed helpers from `terminusdb-schema`:

- **`EntityIDFor<T>`** — `EntityIDFor::<Person>::new("jane")?` auto-prefixes the
  type. `.typed()` → `"Person/jane"`, `.iri_string()` → `"terminusdb:///data/Person/jane"`,
  `.id()` → `"jane"`.
- **`TdbIRI`** — `TdbIRI::parse(iri_or_id)` accepts either form; `.to_iri_string()`
  (Display) → full IRI, `.typed_path()` → short form, `.with_default_base()` applies
  the default `terminusdb:///data` base.

```rust
let id = EntityIDFor::<Person>::new("jane")?;
let xpath = format!(r#"document("{}")/@name"#, id.typed());       // ok: id.typed() is the IRI
// NOT: format!("terminusdb:///data/Person/{}", "jane")          // ✗ hand-rolled IRI
```

(`format!` is fine for assembling the *XPath expression* — just not for
fabricating the IRI inside it.)

## Testing

- `tests/compile_tests.rs` — AST-level (no database).
- `tests/spec.rs` — live spec suite against an embedded `TerminusDBServer`
  (one shared per-process server, each case in its own isolated db).
- `examples/explore.rs` — a WOQL exploration harness: prints the compiled DSL +
  JSON-LD and the live result for each probe. Run with
  `cargo run -p terminusdb-xpath --example explore`.
- `terminusdb_xpath::explain(expr)` returns the compiled WOQL (DSL + JSON-LD)
  without a database — the first thing to reach for when a query misbehaves.

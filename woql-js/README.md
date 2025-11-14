# terminusdb-woql-js

JavaScript WOQL syntax parser for Rust, bridging to the official [terminusdb-client-js](https://github.com/terminusdb/terminusdb-client-js) library.

## Overview

This crate provides functionality to parse WOQL queries written in JavaScript syntax (as used by terminusdb-client-js and the TerminusDB dashboard) into JSON-LD format, which can then be used with the Rust TerminusDB client.

The implementation works by spawning a Node.js subprocess that uses the official terminusdb-client-js library to parse the query, ensuring 100% compatibility with the JavaScript syntax.

## Requirements

### Runtime Requirements
- **Node.js** >= 14.0.0 must be installed and available in your system PATH

### Build Requirements
- **Node.js** and **npm** must be available during build
- The build script (`build.rs`) automatically:
  1. Runs `npm install` to get dependencies
  2. Bundles the parser script with esbuild
  3. Embeds the bundled script into the Rust binary

**No runtime dependencies on node_modules** - the bundled script is self-contained and embedded in your binary!

## Usage

### Basic Parsing to JSON-LD

```rust
use terminusdb_woql_js::parse_js_woql;

let js_query = r#"
    select(
        "Name", "Age",
        and(
            triple("v:Person", "rdf:type", "@schema:Person"),
            triple("v:Person", "@schema:name", "v:Name"),
            triple("v:Person", "@schema:age", "v:Age")
        )
    )
"#;

let json_ld = parse_js_woql(js_query)?;
println!("{}", serde_json::to_string_pretty(&json_ld)?);
```

### Parsing Directly to Query Type

```rust
use terminusdb_woql_js::parse_js_woql_to_query;

let js_query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
let query = parse_js_woql_to_query(js_query)?;

// Now you have a terminusdb_woql2::query::Query object
```

## JavaScript Syntax vs Rust DSL

This crate parses **JavaScript syntax** as used by terminusdb-client-js, which is different from the Rust DSL syntax:

### JavaScript Syntax (this crate)

```javascript
select(
    "Name", "Age",
    and(
        triple("v:Person", "rdf:type", "@schema:Person"),
        triple("v:Person", "@schema:name", "v:Name"),
        triple("v:Person", "@schema:age", "v:Age"),
        greater("v:Age", 18)
    )
)
```

Key features:
- Variables in queries: `"v:Name"` (strings with `v:` prefix)
- Variables in select/distinct/etc: `"Name", "Age"` (variadic string arguments without prefix)
- Standard JavaScript function call syntax

### Rust DSL Syntax (terminusdb-woql-dsl)

```rust
select(
    [$Name, $Age],
    and(
        triple($Person, "rdf:type", "@schema:Person"),
        triple($Person, "@schema:name", $Name),
        triple($Person, "@schema:age", $Age),
        greater($Age, 18)
    )
)
```

Key features:
- Variables: `$Name` prefix
- More concise syntax

## Supported WOQL Operations

The parser supports all WOQL operations available in terminusdb-client-js, including:

### Triple Operations
- `triple(subject, predicate, object)`
- `quad(subject, predicate, object, graph)`

### Logical Operations
- `and(query1, query2, ...)`
- `or(query1, query2, ...)`
- `not(query)`
- `opt(query)` / `optional(query)`

### Control Flow
- `select([variables], query)`
- `distinct([variables], query)`
- `limit(n, query)`
- `start(n, query)`
- `order_by([ordering], query)`
- `group_by([group_vars], [template_vars], query)`

### Comparison Operations
- `eq(left, right)`
- `greater(left, right)`
- `less(left, right)`

### Type Operations
- `isa(value, type)`
- `type_of(value, type)`
- `subsumption(subtype, supertype)`

### String Operations
- `concat([strings], result)`
- `substring(string, before, length, after, substring)`
- `trim(untrimmed, trimmed)`
- `upper(mixed, upper)`
- `lower(mixed, lower)`
- `regexp(pattern, string, result)`

### Arithmetic Operations
- `eval(expression, result)`
- `plus(a, b)`
- `minus(a, b)`
- `times(a, b)`
- `div(a, b)`

### Document Operations
- `read_document(id, document)`
- `insert_document(document, id)`
- `update_document(document)`
- `delete_document(id)`

### Path Operations
- `path(start, pattern, end, path)`
- Path patterns: `pred()`, `inv()`, `star()`, `plus()`, `seq()`, `or()`

## How It Works

### Build Time
1. The `build.rs` script runs during `cargo build`:
   - Runs `npm install` in the `scripts/` directory
   - Runs `npm run build` which uses esbuild to bundle `parse-woql.js` with all dependencies
   - Creates `scripts/parse-woql.bundle.js` (~1MB self-contained file)
2. The bundled script is embedded into the Rust binary using `include_str!`

### Runtime
1. The Rust code calls `parse_js_woql()` with a JavaScript-syntax query string
2. The embedded bundled script is written to a temporary file
3. A Node.js process is spawned to execute the temporary script
4. The query is passed via stdin to the Node.js process
5. The Node.js script:
   - Uses `WOQL.emerge()` to generate a prelude with all WOQL functions
   - Evaluates the query string with `eval(prelude + query)`
   - Calls `.json()` on the result to get JSON-LD
   - Outputs the JSON-LD to stdout
6. The Rust code parses the JSON-LD output
7. Optionally deserializes it into a `terminusdb_woql2::query::Query` object
8. The temporary file is automatically cleaned up

## Error Handling

The parser provides detailed error messages for:
- Node.js not being installed or not in PATH
- Missing npm dependencies
- JavaScript syntax errors
- Invalid WOQL queries
- JSON-LD deserialization errors

Example error:

```
Node.js parser failed with exit code Some(1):
Parse error: ReferenceError: invalidFunction is not defined
    at eval (eval at <anonymous> (/path/to/parse-woql.js:30:28), <anonymous>:2:1)
```

## Testing

The crate includes comprehensive integration tests. Note that all tests are marked with `#[ignore]` because they require Node.js to be installed.

To run the tests:

```bash
# The build script will automatically install dependencies and bundle the script
cargo test -p terminusdb-woql-js -- --ignored
```

The tests verify:
- Parsing simple and complex queries
- Conversion to JSON-LD and Query types
- Error handling for invalid syntax
- All WOQL operations (select, and, or, triple, etc.)

## Performance Considerations

Each call to `parse_js_woql()` spawns a new Node.js process, which has some overhead (typically 10-50ms). For use cases that parse many queries:

- Consider caching parsed queries if they are reused
- The subprocess overhead is acceptable for most use cases (query parsing for user input, configuration, etc.)
- For extremely high-performance scenarios, consider using the Rust DSL directly

## When to Use This Crate

**Use this crate when:**
- You need to parse queries written by users who are familiar with the JavaScript WOQL syntax
- You're migrating from a JavaScript-based TerminusDB application to Rust
- You want 100% compatibility with terminusdb-client-js query syntax
- You're building a tool that needs to accept WOQL in JavaScript syntax (e.g., a dashboard, query editor)

**Don't use this crate when:**
- You're writing queries directly in Rust code (use the Rust DSL instead)
- Node.js is not available in your deployment environment
- You need maximum performance (subprocess overhead may be an issue)

## Comparison with terminusdb-woql-dsl

| Feature | terminusdb-woql-js | terminusdb-woql-dsl |
|---------|-------------------|---------------------|
| Syntax | JavaScript | Rust-like DSL |
| Variables | `"v:Name"` | `$Name` |
| Dependencies | Node.js required | Pure Rust |
| Compatibility | 100% with terminusdb-client-js | Custom syntax |
| Performance | ~10-50ms overhead | Instant parsing |
| Use Case | JS query strings | Rust query strings |

## License

Apache-2.0

## Contributing

This crate is part of the terminusdb-rs workspace. See the main repository for contribution guidelines.

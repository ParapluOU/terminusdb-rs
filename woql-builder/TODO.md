# WOQL Builder Crate Documentation & Feature Plan

## Core Concepts & Getting Started

- [x] Introduction: Purpose of the crate (fluent WOQL building). (Implicitly
      done by creating the crate)
- [x] Basic Query Structure: How to start a query, chain methods.
      (WoqlBuilder::new(), method chaining)
- [x] Using Variables (`Vars` equivalent). (Var struct)
- [x] Executing a Query (how to get the final WOQL JSON). (finalize() method)
- [x] Examples: Simple `triple` query. (Tests cover this)

## Basic Query Operations

- [x] `triple`: Selecting basic triples (Subject, Predicate, Object).
      (Implemented)
- [x] `limit`: Limiting the number of results.
- [x] `start`: Setting the starting offset for results.
- [x] `select`: Choosing which variables to return in the results.
- [x] Chaining operations (implicit `and`). (Handled by `triple`)

## Logical Operators

- [x] `and`: Explicitly combining query parts that must _all_ be true.
- [x] `or`: Combining query parts where _any_ can be true.
- [x] `not`: Negating a query part.
- [x] `opt` / `optional`: Handling optional query parts. (Implemented)
- [x] `when` / `if`: Conditional execution. (Implemented as
      `when`/`if_then_else`)
- [x] `once`: Limiting a subquery to one solution.
- [x] `immediately`: Unclear purpose from name, needs investigation based on
      `woql2::control::Immediately`.

## Triple & Data Manipulation

- [x] `add_triple`: Adding new triples.
- [x] `delete_triple`: Deleting existing triples.
- [ ] (Potentially separate sections for `*_link`, `*_data`, `added_*`,
      `deleted_*` variants if the builder exposes them distinctly).

## Document Operations

- [x] `insert_document`: Adding new documents.
- [x] `update_document`: Modifying existing documents.
- [x] `delete_document`: Removing documents.
- [x] `read_document`: Retrieving documents by ID.
- [ ] Filtering Documents (`filter-documents.md` suggests more complex logic
      here, maybe tied to comparisons/string ops - likely implicitly handled by
      combining `read_document` with other builder methods like `eq`, `triple`,
      etc.).

## Schema & Type Queries

- [x] `isa`: Checking the type of a resource (`@type` / `rdfs:type`).
      (Implemented)
- [x] `subsumption`: Checking class subsumption. (Implemented)
- [x] `type_of`: Getting the literal type of a value. (Implemented)
- [x] `typecast`: Casting values between types. (Implemented)

## Comparison Operators

- [x] `equals`: Equality check. (Implemented as `eq`)
- [x] `less`: Less than comparison. (Implemented)
- [x] `greater`: Greater than comparison. (Implemented)

## String Operations

- [x] `concat`: Concatenating strings.
- [x] `like`: SQL-style LIKE pattern matching.
- [x] `lower`: Converting to lowercase.
- [x] `pad`: Padding strings.
- [x] `regexp`: Regular expression matching.
- [x] `split`: Splitting strings.
- [x] `substring`: Extracting substrings.
- [x] `trim`: Trimming whitespace.
- [x] `upper`: Converting to uppercase.
- [x] `join`: Joining list elements into a string.

## Mathematical & Arithmetic Operations

- [x] `eval`: Evaluating arithmetic expressions (+, -, *, /, div, exp).
- [x] `plus`, `minus`, `times`, `divide`, `div`, `exp` (or integrated into
      `eval` via helper functions).

## Aggregation & Grouping

- [x] `group_by`: Grouping results by variable values.
- [x] `count`: Counting results.
- [x] `sum`: Summing numerical values.
- [-] `min`, `max`, `avg` (Not found in woql2 schema, maybe later).
- [x] `length`: Getting the length of lists/strings.

## Ordering Results

- [x] `order_by`: Sorting results based on variables (ascending/descending).

## Path Queries

- [x] `path`: Defining graph traversals using path patterns (+, *, |, :, seq,
      or, times).
- [ ] **TODO**: Binding intermediate segments within path patterns (removed
      `.bind()` from tests for now, needs investigation).

## Graph/Data Source Specification

- [x] `from`: Specifying the source graph for a subquery.
- [x] `into`: Specifying the target graph for write operations.
- [x] `using`: Defining the default database/repo context.

## Working with Collections (Lists/Arrays)

- [x] `member`: Checking for membership in a list.
- [x] `dot`: Accessing dictionary/object properties (potentially using dot
      notation on variables - likely implicitly handled via variables and other
      ops).

## Key Generation

- [ ] `lexical_key`: Generating keys based on field values.
- [ ] `hash_key`: Generating hash-based keys.
- [ ] `random_key`: Generating random keys.

## Miscellaneous

- [ ] `size`: Getting the size of a graph in bytes.
- [x] `triple_count`: Getting the number of triples in a graph.
- [x] `distinct`: Ensuring unique results.
- [ ] `pin`: Unclear purpose, needs investigation based on
      `woql2::control::Pin`.
- [ ] `get`: Reading data from external resources (like CSVs).

## Advanced Topics / Examples

- [ ] Combining multiple operations.
- [ ] Recursive queries (`Call` in `woql2`).
- [ ] Named Queries (`NamedQuery`, `NamedParametricQuery`).
- [ ] Error Handling (if the builder produces errors).
- [ ] Integration with `terminusdb-rust` client.

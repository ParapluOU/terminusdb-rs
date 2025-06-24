# TerminusDB Schema Derive Tests

This directory contains tests for the `parture_terminusdb_schema_derive` crate,
which provides the `TerminusDBModel` derive macro for automatic TerminusDB
schema generation.

## Test Structure

The tests are organized by the type of Rust data structure being converted to
TerminusDB schemas:

1. **struct_test.rs** - Tests for deriving schemas from Rust structs
   - Basic struct conversion
   - Custom attribute handling
   - Nested struct schemas

2. **enum_simple_test.rs** - Tests for simple enums (unit variants only)
   - Basic enum conversion
   - Custom attribute handling
   - Schema tree generation

3. **enum_union_test.rs** - Tests for tagged union enums (with variants carrying
   data)
   - Basic tagged union conversion
   - Complex variants with struct-like data
   - Tuple struct variants
   - Virtual struct generation

4. **integration_test.rs** - Comprehensive integration tests
   - Complex nested data structures
   - Combination of structs, enums, and tagged unions
   - Collection types
   - Custom attributes at multiple levels

## Running Tests

Run the tests with:

```bash
cargo test -p parture-terminusdb-schema-derive
```

## Test Coverage

These tests verify that:

1. The `TerminusDBModel` derive macro correctly generates schema definitions for
   different Rust types
2. Custom attributes like `class_name`, `base`, `key`, etc. are properly applied
3. The schema tree generation includes all necessary nested schemas
4. Virtual structs are correctly generated for complex enum variants
5. Field and property name customization works correctly
6. Documentation is properly extracted and applied to schemas

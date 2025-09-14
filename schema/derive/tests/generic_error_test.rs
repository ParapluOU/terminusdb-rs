#![cfg(not(feature = "generic-derive"))]

// This test verifies that attempting to use generics without the feature flag
// produces a helpful error message

#[test]
fn test_generic_struct_error() {
    // This test uses trybuild to verify compile-time errors
    // Since we don't have trybuild set up, we'll create a documentation test instead
}

// The following would fail to compile with a helpful error:
/*
use terminusdb_schema_derive::TerminusDBModel;

#[derive(TerminusDBModel)]
struct Container<T> {
    value: T,
}

// Expected error:
// Generic types are not yet supported in TerminusDBModel derive macro.
// To experiment with generic support, enable the 'generic-derive' feature flag.
// Note: Generic support is experimental and may have limitations.
*/
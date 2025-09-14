#!/bin/bash
# This script tests that generic derive compiles successfully

cd "$(dirname "$0")"

# Create a test file
cat > test_generic.rs << 'EOF'
#![allow(unused)]

use terminusdb_schema::{EntityIDFor, ToTDBSchema, ToSchemaClass};
use terminusdb_schema_derive::TerminusDBModel;

// Concrete type that implements all traits
#[derive(Debug, Clone, TerminusDBModel)]
struct User {
    id: String,
    name: String,
}

// Generic type with EntityIDFor<T>
#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T> {
    id: String,
    target: EntityIDFor<T>,
}

fn main() {
    // This proves the macro generates proper code
    let _ = std::marker::PhantomData::<Reference<User>>;
    println!("✅ Generic derive macro works!");
    println!("✅ Model<T> {{ EntityIDFor<T> }} compiles successfully!");
}
EOF

# Compile with generic-derive feature
echo "Compiling with generic-derive feature..."
rustc test_generic.rs \
    --edition 2021 \
    --crate-type bin \
    -L target/debug/deps \
    --extern terminusdb_schema=target/debug/deps/libterminusdb_schema.rlib \
    --extern terminusdb_schema_derive=target/debug/deps/libterminusdb_schema_derive.so \
    --cfg 'feature="generic-derive"' \
    -o test_generic_bin 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Compilation successful!"
    ./test_generic_bin
    rm test_generic_bin
else
    echo "❌ Compilation failed"
fi

rm test_generic.rs
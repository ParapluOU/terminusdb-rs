//! Build script for terminusdb-woql-js
//!
//! This build script is intentionally minimal. The bundled JS files are committed to
//! the repository to avoid requiring Node.js at build time. To regenerate the bundles
//! after updating npm dependencies, use `just regen` in this directory.

fn main() {
    // Track changes to the source files and bundles
    println!("cargo:rerun-if-changed=scripts/parse-woql.bundle.js");
    println!("cargo:rerun-if-changed=scripts/parse-woql.quickjs.js");
    println!("cargo:rerun-if-changed=scripts/parse-woql.js");
    println!("cargo:rerun-if-changed=scripts/parse-woql-quickjs.js");
    println!("cargo:rerun-if-changed=scripts/woql-only.js");
    println!("cargo:rerun-if-changed=scripts/package.json");
}

//! Demonstration of the TerminusDB typed API.
//!
//! Run with: cargo run --example api_demo --release

use terminusdb_bin::api::{DbCreateOptions, DbSpec, DocInsertOptions, ServeOptions, TerminusDB};

fn main() -> std::io::Result<()> {
    let client = TerminusDB::new();

    println!("=== TerminusDB Typed API Demo ===\n");

    // Example 1: Create a database
    println!("1. Creating database 'admin/testdb'...");
    let spec = DbSpec::new("admin", "testdb");

    let create_opts = DbCreateOptions {
        label: Some("Test Database".to_string()),
        comment: Some("Created via typed API".to_string()),
        ..Default::default()
    };

    // Note: This would actually create the database if server was running
    // client.db().create(spec.clone(), create_opts)?;
    println!("   Would execute: db create admin/testdb --label 'Test Database' ...\n");

    // Example 2: Insert a document
    println!("2. Inserting a document...");
    let insert_opts = DocInsertOptions {
        data: Some(r#"{"@type": "Person", "name": "Alice", "age": 30}"#.to_string()),
        message: "Add Alice".into(),
        ..Default::default()
    };

    // client.doc().insert(spec.branch("main"), insert_opts)?;
    println!("   Would execute: doc insert admin/testdb/local/branch/main --data '{{...}}' ...\n");

    // Example 3: Query
    println!("3. Running a query...");
    let query = "triple(X, rdf:type, Person)";
    // client.query(spec.clone(), query, Default::default())?;
    println!(
        "   Would execute: query admin/testdb/local/branch/main '{}' ...\n",
        query
    );

    // Example 4: Clone from remote
    println!("4. Cloning from remote...");
    let clone_spec = DbSpec::new("admin", "cloned_db");
    // client.git().clone("https://cloud.terminusdb.com/team/database", Some(clone_spec), Default::default())?;
    println!(
        "   Would execute: clone https://cloud.terminusdb.com/team/database admin/cloned_db ...\n"
    );

    // Example 5: Using builder pattern for specs
    println!("5. Using builder pattern for complex specs...");
    let complex_spec = DbSpec::new("myorg", "mydb")
        .repository("production")
        .branch("feature-x");
    println!("   DbSpec: {}\n", complex_spec);

    let schema_spec = DbSpec::new("admin", "testdb")
        .branch("main")
        .graph(terminusdb_bin::api::GraphType::Schema);
    println!("   GraphSpec: {}\n", schema_spec);

    println!("=== Demo Complete ===");
    println!("\nAll commands are type-safe and validated at compile-time!");
    println!("No stringly-typed CLI arguments needed.");

    Ok(())
}

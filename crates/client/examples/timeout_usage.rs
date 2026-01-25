//! Example demonstrating the use of request timeout with DocumentInsertArgs
//!
//! Run with: cargo run --example timeout_usage -p terminusdb-client

use std::time::Duration;
use terminusdb_client::{BranchSpec, DocumentInsertArgs};

fn main() {
    println!("DocumentInsertArgs timeout examples:\n");

    // Example 1: Create args with default settings (no timeout)
    let args1 = DocumentInsertArgs::default();
    println!("1. Default args timeout: {:?}", args1.timeout);

    // Example 2: Create args with a 5-second timeout
    let args2 = DocumentInsertArgs::default().with_timeout(Duration::from_secs(5));
    println!("2. Args with 5s timeout: {:?}", args2.timeout);

    // Example 3: Create args with a 100ms timeout for fast operations
    let args3 = DocumentInsertArgs::default().with_timeout(Duration::from_millis(100));
    println!("3. Args with 100ms timeout: {:?}", args3.timeout);

    // Example 4: Chain multiple builder methods
    let args4 = DocumentInsertArgs::default()
        .with_force(true)
        .with_timeout(Duration::from_secs(10));
    println!(
        "4. Args with force and 10s timeout: force={}, timeout={:?}",
        args4.force, args4.timeout
    );

    // Example 5: Creating from BranchSpec and adding timeout
    let branch_spec = BranchSpec {
        db: "mydb".to_string(),
        branch: Some("main".to_string()),
        ref_commit: None,
    };
    let args5 = DocumentInsertArgs::from(branch_spec).with_timeout(Duration::from_secs(30));
    println!(
        "5. Args from BranchSpec with 30s timeout: db={}, timeout={:?}",
        args5.spec.db, args5.timeout
    );

    println!("\nWhen these args are used with insert operations, the timeout");
    println!("will be applied to the HTTP request if specified.");
}

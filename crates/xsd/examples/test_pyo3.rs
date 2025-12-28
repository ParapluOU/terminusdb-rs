//! Test PyO3 integration with xmlschema
//!
//! Run with:
//! ```
//! cargo run --example test_pyo3
//! ```
//!
//! Prerequisites:
//! ```
//! pip install xmlschema
//! ```

use pyo3::prelude::*;
use pyo3::types::PyModule;
use terminusdb_xsd::schema_model::XsdSchema;
use terminusdb_xsd::Result;

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("=== PyO3 + xmlschema Test ===\n");

    println!("Testing xmlschema import...");
    Python::with_gil(|py| -> Result<()> {
        let xmlschema = PyModule::import(py, "xmlschema")?;
        let version: String = xmlschema.getattr("__version__")?.extract()?;
        println!("✓ SUCCESS! xmlschema version {} is available!", version);
        println!("  PyO3 can successfully import and use xmlschema!\n");
        Ok(())
    })?;

    // If we have a command line argument with an XSD file, try to parse it
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let xsd_path = &args[1];
        println!("Parsing XSD file: {}", xsd_path);

        let schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>)?;

        println!("✓ XSD parsed successfully!\n");
        println!("Schema info:");
        println!("  Target namespace: {:?}", schema.target_namespace);
        println!("  Root elements: {}", schema.root_elements.len());
        println!("  Complex types: {}", schema.complex_types.len());
        println!("  Simple types: {}", schema.simple_types.len());

        println!("\nComplex types:");
        for ct in schema.complex_types.iter().take(10) {
            println!("  - {}", ct.name);
        }
        if schema.complex_types.len() > 10 {
            println!("  ... and {} more", schema.complex_types.len() - 10);
        }
    } else {
        println!("No XSD file provided.");
        println!("\nTo test with an XSD file:");
        println!("  cargo run --example test_pyo3 -- path/to/schema.xsd\n");
    }

    println!("=== Test Complete ===");
    Ok(())
}

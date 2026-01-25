//! Example: Parse XSD Schema
//!
//! This example demonstrates how to use the XSD parser to load and parse
//! an XSD schema file using PyO3 and Python's xmlschema library.
//!
//! Run with:
//! ```
//! cargo run --example parse_xsd -- path/to/schema.xsd
//! ```

use std::env;
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
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

    println!("=== XSD to TerminusDB Parser ===\n");

    // Get XSD path from command line args
    let args: Vec<String> = env::args().collect();
    let xsd_path = if args.len() > 1 {
        &args[1]
    } else {
        println!("Usage: cargo run --example parse_xsd -- <path-to-xsd>");
        println!("\nNo XSD file provided.");
        return Ok(());
    };

    println!("Parsing XSD file: {}", xsd_path);

    // Parse XSD using PyO3 + xmlschema
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>)?;

    println!("✓ XSD parsed successfully!");
    println!("  Found {} complex types\n", xsd_schema.complex_types.len());

    // Show complex types
    println!("Complex types:");
    for ct in &xsd_schema.complex_types {
        println!("  - {}", ct.name);
        if let Some(attrs) = &ct.attributes {
            for attr in attrs {
                println!(
                    "      @{}: {} ({})",
                    attr.name, attr.attr_type, attr.use_type
                );
            }
        }
        if let Some(children) = &ct.child_elements {
            for child in children {
                println!("      <{}>: {}", child.name, child.element_type);
            }
        }
    }

    // Generate TerminusDB schemas
    println!("\nGenerating TerminusDB schemas...");
    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema)?;

    println!("✓ Generated {} schemas\n", schemas.len());

    // Show generated schemas
    for schema in &schemas {
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }

    Ok(())
}

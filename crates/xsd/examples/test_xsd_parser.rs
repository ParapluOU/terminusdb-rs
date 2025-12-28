//! Test xsd-parser crate for XSD introspection
//!
//! Run with:
//! ```
//! cargo run --example test_xsd_parser -- path/to/schema.xsd
//! ```

use std::env;
use xsd_parser::{
    config::Schema,
    exec_parser, exec_interpreter,
    Config,
    models::meta::MetaTypeVariant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing xsd-parser Crate ===\n");

    // Get XSD path
    let args: Vec<String> = env::args().collect();
    let xsd_path = if args.len() > 1 {
        &args[1]
    } else {
        eprintln!("Usage: {} <path-to-xsd>", args[0]);
        std::process::exit(1);
    };

    println!("Parsing XSD: {}\n", xsd_path);

    // Step 1: Create config and parse XSD
    println!("Step 1: Parsing XSD file...");
    let mut config = Config::default();
    config.parser.schemas.push(Schema::File(xsd_path.into()));

    let schemas = exec_parser(config.parser)?;
    println!("✓ Parsed schemas\n");

    // Step 2: Interpret to MetaTypes
    println!("Step 2: Interpreting to MetaTypes...");
    let meta_types = exec_interpreter(config.interpreter, &schemas)?;
    println!("✓ Interpreted to MetaTypes\n");

    // Step 3: Introspect MetaTypes
    println!("=== Schema Introspection ===\n");

    println!("Total types: {}", meta_types.items.len());

    // Count by type variant
    let mut complex_count = 0;
    let mut simple_count = 0;
    let mut other_count = 0;

    for (_ident, type_meta) in &meta_types.items {
        match &type_meta.variant {
            MetaTypeVariant::ComplexType(_) => complex_count += 1,
            MetaTypeVariant::SimpleType(_) => simple_count += 1,
            _ => other_count += 1,
        }
    }

    println!("\nType breakdown:");
    println!("  Complex types: {}", complex_count);
    println!("  Simple types: {}", simple_count);
    println!("  Other types: {}", other_count);

    // Show first 10 complex types
    println!("\nFirst 10 complex types:");
    let mut shown = 0;
    for (ident, type_meta) in &meta_types.items {
        if let MetaTypeVariant::ComplexType(complex) = &type_meta.variant {
            println!("  • {} (ns: {:?})", ident.name, ident.ns);
            println!("    Attributes: {}", complex.attributes.len());
            // ComplexMeta doesn't have a direct elements field, check content instead
            if let Some(content) = &complex.content {
                println!("    Content: {:?}", std::any::type_name_of_val(content));
            }
            shown += 1;
            if shown >= 10 {
                break;
            }
        }
    }

    if complex_count > 10 {
        println!("  ... and {} more complex types", complex_count - 10);
    }

    println!("\n=== Summary ===");
    println!("Successfully introspected XSD schema using pure Rust!");
    println!("Total types: {}", meta_types.items.len());

    Ok(())
}

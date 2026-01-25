//! Example: Parse XSD and use strongly-typed Rust models
//!
//! Run with:
//! ```
//! # Without catalog
//! cargo run --example parse_with_types -- path/to/schema.xsd
//!
//! # With catalog for URN resolution (e.g., DITA)
//! cargo run --example parse_with_types -- path/to/schema.xsd path/to/catalog.txt
//! ```

use std::env;
use terminusdb_xsd::{Result, XsdSchema};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("=== XSD Parser with Typed Models ===\n");

    // Get XSD path and optional catalog path
    let args: Vec<String> = env::args().collect();
    let (xsd_path, catalog_path) = if args.len() > 2 {
        (&args[1], Some(&args[2]))
    } else if args.len() > 1 {
        (&args[1], None)
    } else {
        eprintln!("Usage: {} <path-to-xsd> [catalog-path]", args[0]);
        std::process::exit(1);
    };

    // Parse XSD using typed API with optional catalog
    println!("Parsing XSD: {}", xsd_path);
    if let Some(catalog) = catalog_path {
        println!("Using catalog: {}", catalog);
    }

    let schema = if let Some(catalog) = catalog_path {
        XsdSchema::from_xsd_file(xsd_path, Some(catalog))?
    } else {
        XsdSchema::from_xsd_file(xsd_path, None::<&str>)?
    };

    println!("✓ Schema parsed\n");

    // Display schema information using typed API
    println!("=== Schema Information ===");
    println!("Target Namespace: {:?}", schema.target_namespace);
    println!("Schema Location: {:?}", schema.schema_location);
    println!("Element Form Default: {:?}", schema.element_form_default);
    println!();

    // Root elements
    println!("=== Root Elements ({}) ===", schema.root_elements.len());
    for elem in &schema.root_elements {
        println!("  • {}", elem.name);
        if let Some(ref type_info) = elem.type_info {
            println!(
                "    Type: {} ({})",
                type_info.name.as_deref().unwrap_or("anonymous"),
                type_info.category
            );

            if let Some(ref attrs) = type_info.attributes {
                println!("    Attributes: {}", attrs.len());
                for attr in attrs {
                    let required = if attr.is_required() {
                        "required"
                    } else {
                        "optional"
                    };
                    println!("      - {} [{}] ({})", attr.name, attr.attr_type, required);
                }
            }

            if let Some(ref children) = type_info.child_elements {
                println!("    Child Elements: {}", children.len());
                for child in children {
                    let required = if child.is_required() {
                        "required"
                    } else {
                        "optional"
                    };
                    let multiple = if child.is_multiple() { "[]" } else { "" };
                    println!(
                        "      - {}{}: {} ({})",
                        child.name, multiple, child.element_type, required
                    );
                }
            }
        }
        println!();
    }

    // Complex types
    println!("=== Complex Types ({}) ===", schema.complex_types.len());

    // Count named vs anonymous
    let named_count = schema
        .complex_types
        .iter()
        .filter(|t| !t.is_anonymous)
        .count();
    let anonymous_count = schema
        .complex_types
        .iter()
        .filter(|t| t.is_anonymous)
        .count();
    if named_count > 0 && anonymous_count > 0 {
        println!(
            "  Named types: {}, Anonymous types: {}\n",
            named_count, anonymous_count
        );
    } else if anonymous_count > 0 {
        println!("  All types are anonymous (inline element type definitions)\n");
    }

    for ctype in schema.complex_types.iter().take(10) {
        let type_kind = if ctype.is_anonymous {
            format!(
                "(anonymous, from element '{}')",
                ctype.element_name.as_deref().unwrap_or("unknown")
            )
        } else {
            "(named)".to_string()
        };
        println!("  • {} {}", ctype.name, type_kind);
        println!("    Content Model: {:?}", ctype.content_model);

        if let Some(ref attrs) = ctype.attributes {
            println!("    Attributes: {}", attrs.len());
            for attr in attrs {
                println!("      - {}: {}", attr.name, attr.attr_type);
            }
        }

        if let Some(ref children) = ctype.child_elements {
            println!("    Children: {}", children.len());
            for child in children.iter().take(5) {
                println!("      - {}: {}", child.name, child.element_type);
            }
            if children.len() > 5 {
                println!("      ... and {} more", children.len() - 5);
            }
        }
        println!();
    }

    if schema.complex_types.len() > 10 {
        println!("  ... and {} more types\n", schema.complex_types.len() - 10);
    }

    // Simple types
    if !schema.simple_types.is_empty() {
        println!("=== Simple Types ({}) ===", schema.simple_types.len());
        for stype in &schema.simple_types {
            println!("  • {}", stype.name);
            println!("    Base: {:?}", stype.base_type);
            println!();
        }
    }

    // Summary
    println!("=== Summary ===");
    println!("Total Root Elements: {}", schema.root_elements.len());
    println!("Total Complex Types: {}", schema.complex_types.len());
    println!("Total Simple Types: {}", schema.simple_types.len());

    Ok(())
}

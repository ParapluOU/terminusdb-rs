//! Generate schemas from an entire directory of XSD files.
//!
//! This demonstrates batch processing of schema bundles by pointing
//! to a directory rather than individual files.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Directory-Based Schema Generation ===\n");

    // Point to a schema bundle directory
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/dita/xsd/xsd1.2-url/base/xsd");

    if !schema_dir.exists() {
        eprintln!("ERROR: Schema directory not found: {:?}", schema_dir);
        eprintln!("This example expects DITA schemas in schemas/dita/");
        return Ok(());
    }

    println!("📂 Schema directory: {:?}\n", schema_dir);

    // Generate schemas from ALL XSD files in the directory
    let generator = XsdToSchemaGenerator::with_namespace("http://dita.oasis-open.org/terminusdb#");

    println!("🔧 Processing entire schema bundle...\n");
    let schemas = generator.generate_from_directory(&schema_dir, None::<PathBuf>)?;

    // Show statistics
    println!("\n{}", "=".repeat(80));
    println!("\n📊 Schema Statistics:\n");

    let mut by_namespace: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut subdocs = 0;

    for schema in &schemas {
        if let terminusdb_schema::Schema::Class { base, subdocument, .. } = schema {
            let ns = base.as_ref().map(|s| s.as_str()).unwrap_or("(none)");
            *by_namespace.entry(ns.to_string()).or_insert(0) += 1;
            if *subdocument {
                subdocs += 1;
            }
        }
    }

    println!("   Total schemas: {}", schemas.len());
    println!("   Subdocuments: {}", subdocs);
    println!("   Top-level classes: {}", schemas.len() - subdocs);
    println!("\n   Schemas by namespace:");
    for (ns, count) in &by_namespace {
        println!("     • {}: {} classes", ns, count);
    }

    // Show first 20 schemas
    println!("\n{}", "=".repeat(80));
    println!("\n📋 First 20 Generated Schemas:\n");

    for (i, schema) in schemas.iter().take(20).enumerate() {
        if let terminusdb_schema::Schema::Class { id, base, properties, subdocument, .. } = schema {
            println!("{}. {}{}",
                i + 1,
                id,
                if *subdocument { " (subdocument)" } else { "" }
            );
            println!("   @base: {}", base.as_ref().unwrap_or(&"(none)".to_string()));
            println!("   Properties: {}", properties.len());
        }
    }

    // Export as JSON
    println!("\n{}", "=".repeat(80));
    println!("\n💾 Exporting schemas to JSON...\n");

    use terminusdb_schema::json::ToJson;
    let all_json: Vec<_> = schemas.iter().map(|s| s.to_json()).collect();
    let json_str = serde_json::to_string_pretty(&all_json)?;

    // Save to file
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/generated_schemas.json");

    std::fs::write(&output_path, json_str)?;
    println!("   Exported {} schemas to: {:?}", schemas.len(), output_path);

    println!("\n{}", "=".repeat(80));
    println!("\n✅ Complete! Processed entire schema bundle in one operation.\n");

    println!("📝 Key Features:");
    println!("   ✓ Recursive directory scanning");
    println!("   ✓ Batch processing of multiple XSD files");
    println!("   ✓ Automatic deduplication by class ID");
    println!("   ✓ Error tolerance (continues on parse errors)");
    println!("   ✓ Progress reporting");
    println!("   ✓ Consolidated output");

    Ok(())
}

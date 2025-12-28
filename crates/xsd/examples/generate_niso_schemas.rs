//! Generate schemas from NISO-STS XSD files.
//!
//! Demonstrates both auto-detection and explicit entry point specification.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NISO-STS Schema Generation ===\n");

    let niso_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/niso/xsd/NISO-STS-extended-1-MathML3-XSD");

    if !niso_dir.exists() {
        eprintln!("ERROR: NISO schema directory not found: {:?}", niso_dir);
        return Ok(());
    }

    println!("📂 NISO schema directory: {:?}\n", niso_dir);

    let generator = XsdToSchemaGenerator::with_namespace("https://www.niso.org/standards/z39-102-2022#");

    // Method 1: Auto-detection
    println!("--- Method 1: Auto-Detection ---\n");
    test_auto_detection(&generator, &niso_dir)?;

    println!("\n{}\n", "=".repeat(80));

    // Method 2: Explicit entry points (recommended for production)
    println!("--- Method 2: Explicit Entry Points (Customer-Provided) ---\n");
    test_explicit_entry_points(&generator, &niso_dir)?;

    Ok(())
}

fn test_auto_detection(
    generator: &XsdToSchemaGenerator,
    niso_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Auto-detecting entry points...\n");

    let schemas = generator.generate_from_directory(niso_dir, None::<PathBuf>)?;

    show_schema_stats(&schemas);

    Ok(())
}

fn test_explicit_entry_points(
    generator: &XsdToSchemaGenerator,
    niso_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Customer provides explicit entry point paths
    let entry_points = vec![
        niso_dir.join("NISO-STS-extended-1-mathml3.xsd"),
    ];

    println!("📝 Customer specified entry points:");
    for ep in &entry_points {
        println!("   • {:?}", ep.file_name().unwrap_or_default());
    }
    println!();

    let schemas = generator.generate_from_entry_points(&entry_points, None::<PathBuf>)?;

    show_schema_stats(&schemas);

    // Export schemas
    println!("\n💾 Exporting to JSON...\n");

    use terminusdb_schema::json::ToJson;
    let all_json: Vec<_> = schemas.iter().map(|s| s.to_json()).collect();
    let json_str = serde_json::to_string_pretty(&all_json)?;

    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/niso_schemas.json");

    std::fs::write(&output_path, json_str)?;
    println!("   Exported {} schemas to: {:?}", schemas.len(), output_path);

    Ok(())
}

fn show_schema_stats(schemas: &[terminusdb_schema::Schema]) {
    println!("\n{}", "=".repeat(60));
    println!("\n📊 Schema Statistics:\n");

    let mut by_namespace: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut subdocs = 0;

    for schema in schemas {
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

    if !by_namespace.is_empty() {
        println!("\n   Schemas by namespace:");
        for (ns, count) in &by_namespace {
            println!("     • {}: {} classes", ns, count);
        }
    }

    // Show sample schemas
    println!("\n📋 Sample Schemas (first 15):\n");

    for (i, schema) in schemas.iter().take(15).enumerate() {
        if let terminusdb_schema::Schema::Class { id, base, properties, subdocument, .. } = schema {
            println!("{}. {}{}",
                i + 1,
                id,
                if *subdocument { " (subdocument)" } else { "" }
            );
            if let Some(ns) = base {
                if ns.len() < 60 {
                    println!("   @base: {}", ns);
                } else {
                    println!("   @base: {}...", &ns[..57]);
                }
            }
            println!("   Properties: {}", properties.len());
        }
    }

    println!("\n{}", "=".repeat(60));
}

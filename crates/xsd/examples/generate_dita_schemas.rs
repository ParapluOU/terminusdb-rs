//! Generate TerminusDB schemas from DITA XSD at runtime.
//!
//! This example demonstrates how to:
//! 1. Parse DITA XSD schemas using xmlschema via PyO3
//! 2. Generate TerminusDB Schema instances programmatically
//! 3. Output schemas as JSON for inspection or submission to TerminusDB
//!
//! Run with: cargo run --example generate_dita_schemas

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== DITA XSD to TerminusDB Schema Generator ===\n");

    // Path to DITA topic.xsd (relative to workspace root)
    let xsd_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/dita-ot/plugins/org.oasis-open.dita.v1_3/schema-url/technicalContent/xsd/topic.xsd");

    // Catalog path for URN resolution
    let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/dita-ot/plugins/org.oasis-open.dita.v1_3/catalog-dita.xml");

    if !xsd_path.exists() {
        eprintln!("ERROR: DITA XSD not found at: {}", xsd_path.display());
        eprintln!("Please ensure DITA schemas are available in vendor/dita-ot/");
        return Ok(());
    }

    // Step 1: Parse XSD schema
    println!("📖 Parsing XSD schema: {}", xsd_path.display());
    let xsd_schema = XsdSchema::from_xsd_file(&xsd_path, Some(&catalog_path))?;

    println!("   ✓ Found {} complex types", xsd_schema.complex_types.len());
    println!(
        "   ✓ {} named types, {} anonymous types\n",
        xsd_schema
            .complex_types
            .iter()
            .filter(|t| !t.is_anonymous)
            .count(),
        xsd_schema
            .complex_types
            .iter()
            .filter(|t| t.is_anonymous)
            .count()
    );

    // Step 2: Generate TerminusDB schemas
    println!("🔧 Generating TerminusDB schemas...");
    let generator = XsdToSchemaGenerator::with_namespace("http://dita.example.com/schema#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("   ✓ Generated {} TerminusDB schemas\n", schemas.len());

    // Step 3: Display sample schemas
    println!("📋 Sample Generated Schemas:\n");

    for (i, schema) in schemas.iter().take(5).enumerate() {
        match schema {
            terminusdb_schema::Schema::Class {
                id,
                properties,
                key,
                subdocument,
                ..
            } => {
                println!("{}. Class: {}", i + 1, id);
                println!("   Key: {:?}", key);
                println!("   Subdocument: {}", subdocument);
                println!("   Properties ({}):", properties.len());
                for prop in properties.iter().take(3) {
                    println!(
                        "     - {}: {} {:?}",
                        prop.name, prop.class, prop.r#type
                    );
                }
                if properties.len() > 3 {
                    println!("     ... and {} more", properties.len() - 3);
                }
                println!();
            }
            _ => {
                println!("{}. Other schema type: {:?}", i + 1, schema);
            }
        }
    }

    if schemas.len() > 5 {
        println!("... and {} more schemas\n", schemas.len() - 5);
    }

    // Step 4: Convert to JSON for inspection/submission
    println!("💾 Converting to JSON format...");

    use terminusdb_schema::json::ToJson;
    for (i, schema) in schemas.iter().take(2).enumerate() {
        let json = schema.to_json();
        let json_str = serde_json::to_string_pretty(&json)?;
        println!("\nSchema {} JSON:\n{}", i + 1, json_str);
    }

    println!("\n✅ Schema generation complete!");
    println!("\n💡 Next steps:");
    println!("   - Review generated schemas");
    println!("   - Submit to TerminusDB via API");
    println!("   - Parse XML documents to create Instances");

    Ok(())
}

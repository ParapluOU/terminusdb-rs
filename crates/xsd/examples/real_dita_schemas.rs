//! Real-world XSD to TerminusDB schema conversion example using DITA schemas.
//!
//! This example demonstrates schema generation from actual DITA 1.2 XSD files,
//! showcasing:
//! - Real XSD schema parsing with catalog-based URN resolution
//! - Namespace preservation from actual standards
//! - PascalCase conversion using the heck crate
//! - Complex type hierarchies from production schemas

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Real DITA XSD to TerminusDB Schema Conversion ===\n");

    // Path to real DITA XSD schemas in the repository
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/dita/xsd/xsd1.2");

    let xsd_path = schema_dir.join("base/xsd/topicMod.xsd");
    let catalog_path = schema_dir.join("catalog-dita-xsd.xml");

    if !xsd_path.exists() {
        eprintln!("ERROR: DITA XSD not found at: {:?}", xsd_path);
        eprintln!("Please ensure DITA schemas are available in schemas/dita/");
        return Ok(());
    }

    println!("📂 Parsing DITA Topic Module XSD:");
    println!("   Schema: {:?}", xsd_path.file_name().unwrap());
    println!("   Catalog: {:?}\n", catalog_path.file_name().unwrap());

    // Parse the XSD schema using Python xmlschema with catalog support
    println!("⏳ Parsing XSD (this may take a moment)...\n");
    let xsd_schema = XsdSchema::from_xsd_file(&xsd_path, Some(&catalog_path))?;

    println!("✅ Parsed XSD Schema:");
    println!("   Target namespace: {:?}", xsd_schema.target_namespace);
    println!("   Complex types: {}", xsd_schema.complex_types.len());
    println!("   Simple types: {}", xsd_schema.simple_types.len());
    println!("   Root elements: {}\n", xsd_schema.root_elements.len());

    // Generate TerminusDB schemas
    println!("🔧 Generating TerminusDB schemas...\n");
    let generator = XsdToSchemaGenerator::with_namespace("http://dita.oasis-open.org/terminusdb#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("✅ Generated {} TerminusDB schemas\n", schemas.len());
    println!("{}", "=".repeat(80));

    // Display first 10 schemas as examples
    println!("\n📋 First 10 Generated Schemas (sample):\n");

    for (i, schema) in schemas.iter().take(10).enumerate() {
        match schema {
            terminusdb_schema::Schema::Class {
                id,
                base,
                properties,
                key,
                subdocument,
                ..
            } => {
                println!("{}. Class: {}", i + 1, id);
                println!("   {}", "-".repeat(60));

                if let Some(ns) = base {
                    println!("   @base: {}", ns);
                }

                println!("   @key: {:?}", key);
                println!("   @subdocument: {}", subdocument);

                if !properties.is_empty() {
                    println!("   Properties ({}):", properties.len());
                    for prop in properties.iter().take(5) {
                        let type_info = match &prop.r#type {
                            None => format!("{} (required)", prop.class),
                            Some(tf) => format!("{} {:?}", prop.class, tf),
                        };
                        println!("     • {}: {}", prop.name, type_info);
                    }
                    if properties.len() > 5 {
                        println!("     ... and {} more properties", properties.len() - 5);
                    }
                }
                println!();
            }
            _ => {
                println!("{}. Other schema type\n", i + 1);
            }
        }
    }

    // Show detailed JSON for first 3 schemas
    println!("{}", "=".repeat(80));
    println!("\n💾 Detailed JSON Schemas (first 3):\n");

    use terminusdb_schema::json::ToJson;
    for (i, schema) in schemas.iter().take(3).enumerate() {
        let json = schema.to_json();
        let json_str = serde_json::to_string_pretty(&json)?;
        println!("--- Schema {} ---", i + 1);
        println!("{}\n", json_str);
    }

    println!("{}", "=".repeat(80));
    println!("\n✅ Real DITA XSD Schema Conversion Complete!\n");

    println!("📝 Statistics:");
    println!("   Total schemas generated: {}", schemas.len());

    let subdocs = schemas.iter().filter(|s| {
        matches!(s, terminusdb_schema::Schema::Class { subdocument: true, .. })
    }).count();

    println!("   Subdocuments: {}", subdocs);
    println!("   Top-level classes: {}", schemas.len() - subdocs);

    Ok(())
}

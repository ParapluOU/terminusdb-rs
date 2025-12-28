//! Parse real DITA map schema and show generated TerminusDB schema.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DITA Map Schema Conversion ===\n");

    // Use the URL-based schemas (no catalog needed)
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/dita/xsd/xsd1.2-url");

    let xsd_path = schema_dir.join("base/xsd/mapMod.xsd");

    if !xsd_path.exists() {
        eprintln!("ERROR: DITA map XSD not found at: {:?}", xsd_path);
        return Ok(());
    }

    println!("📂 Parsing DITA Map Module XSD:");
    println!("   Path: {:?}\n", xsd_path);

    // Parse without catalog (URL-based version)
    println!("⏳ Parsing XSD schema...\n");
    let xsd_schema = XsdSchema::from_xsd_file(&xsd_path, None::<PathBuf>)?;

    println!("✅ Parsed DITA Map XSD:");
    println!("   Target namespace: {:?}", xsd_schema.target_namespace);
    println!("   Complex types: {}", xsd_schema.complex_types.len());
    println!("   Simple types: {}", xsd_schema.simple_types.len());
    println!("   Root elements: {}\n", xsd_schema.root_elements.len());

    // Show some type names
    println!("📋 Sample complex types found:");
    for (i, ct) in xsd_schema.complex_types.iter().take(15).enumerate() {
        let name = ct.name.split('}').last().unwrap_or(&ct.name);
        println!("   {}. {} (anonymous: {})", i + 1, name, ct.is_anonymous);
    }
    println!();

    // Generate TerminusDB schemas
    println!("🔧 Generating TerminusDB schemas...\n");
    let generator = XsdToSchemaGenerator::with_namespace("http://dita.oasis-open.org/terminusdb#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("✅ Generated {} TerminusDB schemas\n", schemas.len());
    println!("{}", "=".repeat(80));

    // Find and display the main map-related schemas
    println!("\n📋 Key Map-Related Schemas:\n");

    for (i, schema) in schemas.iter().enumerate() {
        match schema {
            terminusdb_schema::Schema::Class {
                id,
                base,
                properties,
                key,
                subdocument,
                ..
            } if id.to_lowercase().contains("map") || id.to_lowercase().contains("topicref") => {
                println!("{}. Class: {} (PascalCase)", i + 1, id);
                println!("   {}", "-".repeat(60));

                if let Some(ns) = base {
                    println!("   @base: {}", ns);
                }

                println!("   @key: {:?}", key);
                println!("   @subdocument: {}", subdocument);
                println!("   Properties ({}):", properties.len());

                for prop in properties.iter().take(10) {
                    let type_info = match &prop.r#type {
                        None => format!("{} (required)", prop.class),
                        Some(tf) => format!("{} {:?}", prop.class, tf),
                    };
                    println!("     • {}: {}", prop.name, type_info);
                }

                if properties.len() > 10 {
                    println!("     ... and {} more properties", properties.len() - 10);
                }

                println!();
            }
            _ => {}
        }
    }

    // Show full JSON for the main map schema
    println!("{}", "=".repeat(80));
    println!("\n💾 Full JSON for Map Schema:\n");

    use terminusdb_schema::json::ToJson;
    for schema in schemas.iter() {
        if let terminusdb_schema::Schema::Class { id, .. } = schema {
            if id == "Map" || id == "MapType" {
                let json = schema.to_json();
                let json_str = serde_json::to_string_pretty(&json)?;
                println!("{}\n", json_str);
                break;
            }
        }
    }

    println!("{}", "=".repeat(80));
    println!("\n✅ DITA Map Schema Analysis Complete!\n");

    println!("📝 Statistics:");
    println!("   Total schemas: {}", schemas.len());

    let map_related = schemas.iter().filter(|s| {
        matches!(s, terminusdb_schema::Schema::Class { id, .. }
            if id.to_lowercase().contains("map") || id.to_lowercase().contains("topicref"))
    }).count();

    println!("   Map-related classes: {}", map_related);

    Ok(())
}

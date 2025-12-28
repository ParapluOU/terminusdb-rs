//! Generate schemas from S1000D Issue 6 XSD files.
//!
//! Tests the XSD to TerminusDB converter with S1000D aerospace/defense technical publication schemas.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== S1000D Issue 6 Schema Generation ===\n");

    let s1000d_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/s1000d/xml_schema_flat");

    if !s1000d_dir.exists() {
        eprintln!("ERROR: S1000D schema directory not found: {:?}", s1000d_dir);
        eprintln!("\nDownload S1000D Issue 6 schemas and extract to schemas/s1000d/");
        return Ok(());
    }

    println!("📂 S1000D schema directory: {:?}\n", s1000d_dir);

    let generator = XsdToSchemaGenerator::with_namespace("http://www.s1000d.org/S1000D_6-0#");

    // First, analyze entry point candidates
    println!("🔍 Analyzing entry point candidates...\n");
    let candidates = generator.analyze_entry_point_candidates(&s1000d_dir)?;

    println!("📊 Entry Point Analysis Results:\n");
    for (i, candidate) in candidates.iter().take(15).enumerate() {
        let file_name = candidate.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???");

        let score = &candidate.score;

        let (symbol, recommendation) = match score.total_score {
            90.. => ("🟢", "EXCELLENT"),
            60..=89 => ("🟡", "GOOD"),
            30..=59 => ("🟠", "POSSIBLE"),
            _ => ("⚪", "UNLIKELY"),
        };

        println!("{}. {} {} - {} points ({})",
            i + 1, symbol, file_name, score.total_score, recommendation);

        if i < 5 {
            // Show details for top 5
            println!("   ├─ Depth:    {:>3} pts  (depth: {})",
                score.depth_score, score.depth);
            println!("   ├─ Includes: {:>3} pts  ({} include/import directives)",
                score.include_count_score, score.include_count);
            println!("   ├─ Naming:   {:>3} pts",
                score.naming_score);
            if !score.reasons.is_empty() {
                println!("   └─ Reasons:");
                for reason in &score.reasons {
                    println!("      • {}", reason);
                }
            }
            println!();
        }
    }

    println!("\n{}\n", "=".repeat(80));

    // S1000D schemas are all flat (no entry point hierarchy)
    // Each schema is designed to be used independently
    println!("--- Method 1: Auto-Detection ---\n");
    let schemas_auto = generator.generate_from_directory(&s1000d_dir, None::<PathBuf>)?;

    show_schema_stats(&schemas_auto, "Auto-Detection");

    println!("\n{}\n", "=".repeat(80));

    // Generate from specific S1000D schemas
    println!("--- Method 2: Common S1000D Document Types ---\n");

    let common_schemas = vec![
        s1000d_dir.join("descript.xsd"),     // Descriptive data module
        s1000d_dir.join("proced.xsd"),       // Procedural data module
        s1000d_dir.join("pm.xsd"),           // Publication module
        s1000d_dir.join("dml.xsd"),          // Data module list
        s1000d_dir.join("brex.xsd"),         // Business rule exchange
        s1000d_dir.join("comrep.xsd"),       // Comment/reply
        s1000d_dir.join("ddn.xsd"),          // Data dispatch note
        s1000d_dir.join("fault.xsd"),        // Fault data module
    ];

    println!("📝 Testing common S1000D document types:");
    for schema in &common_schemas {
        if schema.exists() {
            println!("   ✓ {:?}", schema.file_name().unwrap_or_default());
        } else {
            println!("   ✗ {:?} (not found)", schema.file_name().unwrap_or_default());
        }
    }
    println!();

    let existing_schemas: Vec<_> = common_schemas.iter()
        .filter(|p| p.exists())
        .collect();

    if !existing_schemas.is_empty() {
        let schemas_common = generator.generate_from_entry_points(&existing_schemas, None::<PathBuf>)?;
        show_schema_stats(&schemas_common, "Common Document Types");
    }

    // Export schemas
    println!("\n{}\n", "=".repeat(80));
    println!("\n💾 Exporting to JSON...\n");

    use terminusdb_schema::json::ToJson;
    let all_json: Vec<_> = schemas_auto.iter().map(|s| s.to_json()).collect();
    let json_str = serde_json::to_string_pretty(&all_json)?;

    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/s1000d_schemas.json");

    std::fs::write(&output_path, json_str)?;
    println!("   Exported {} schemas to: {:?}", schemas_auto.len(), output_path);

    println!("\n{}\n", "=".repeat(80));
    println!("\n✅ S1000D Schema Generation Complete!\n");

    Ok(())
}

fn show_schema_stats(schemas: &[terminusdb_schema::Schema], method: &str) {
    println!("\n📊 Schema Statistics ({}):\n", method);

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
            let display_ns = if ns.len() > 50 {
                format!("{}...", &ns[..47])
            } else {
                ns.clone()
            };
            println!("     • {}: {} classes", display_ns, count);
        }
    }

    // Show sample S1000D-specific schemas
    println!("\n📋 Sample S1000D Schemas:\n");

    let s1000d_schemas: Vec<_> = schemas.iter()
        .filter(|s| {
            matches!(s, terminusdb_schema::Schema::Class { id, .. }
                if !id.contains("Xsd") && !id.starts_with("Xml"))
        })
        .take(20)
        .collect();

    for (i, schema) in s1000d_schemas.iter().enumerate() {
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
}

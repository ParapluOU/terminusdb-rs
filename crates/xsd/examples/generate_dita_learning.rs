//! Generate schemas from DITA Learning and Training (LCE) XSD files.
//!
//! Tests the XSD to TerminusDB converter with DITA Learning Content Exchange schemas.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DITA Learning (LCE) Schema Generation ===\n");

    // Use URL-based schemas (no catalog needed)
    let learning_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/dita/xsd/xsd1.2-url/learning/xsd");

    if !learning_dir.exists() {
        eprintln!("ERROR: DITA Learning directory not found: {:?}", learning_dir);
        return Ok(());
    }

    println!("📂 DITA Learning schema directory (URL-based): {:?}\n", learning_dir);

    let generator = XsdToSchemaGenerator::with_namespace("http://dita.oasis-open.org/learning#");

    // First, analyze entry point candidates
    println!("🔍 Analyzing entry point candidates...\n");
    let candidates = generator.analyze_entry_point_candidates(&learning_dir)?;

    println!("📊 Entry Point Analysis Results:\n");
    for (i, candidate) in candidates.iter().take(10).enumerate() {
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
    }

    println!("\n{}\n", "=".repeat(80));

    // Generate from auto-detected entry points (no catalog needed for URL-based schemas)
    println!("--- Method 1: Auto-Detection ---\n");
    let schemas_auto = generator.generate_from_directory(&learning_dir, None::<PathBuf>)?;

    show_schema_stats(&schemas_auto, "Auto-Detection");

    println!("\n{}\n", "=".repeat(80));

    // Generate from explicit entry points
    println!("--- Method 2: Explicit Entry Points ---\n");

    let entry_points = vec![
        learning_dir.join("learningAssessment.xsd"),
        learning_dir.join("learningContent.xsd"),
        learning_dir.join("learningOverview.xsd"),
        learning_dir.join("learningPlan.xsd"),
        learning_dir.join("learningSummary.xsd"),
        learning_dir.join("learningBookmap.xsd"),
        learning_dir.join("learningMap.xsd"),
    ];

    println!("📝 Specified entry points:");
    for ep in &entry_points {
        if ep.exists() {
            println!("   ✓ {:?}", ep.file_name().unwrap_or_default());
        } else {
            println!("   ✗ {:?} (not found)", ep.file_name().unwrap_or_default());
        }
    }
    println!();

    let existing_entry_points: Vec<_> = entry_points.iter()
        .filter(|p| p.exists())
        .collect();

    let schemas_explicit = generator.generate_from_entry_points(&existing_entry_points, None::<PathBuf>)?;

    show_schema_stats(&schemas_explicit, "Explicit Entry Points");

    // Export schemas
    println!("\n{}\n", "=".repeat(80));
    println!("\n💾 Exporting to JSON...\n");

    use terminusdb_schema::json::ToJson;
    let all_json: Vec<_> = schemas_explicit.iter().map(|s| s.to_json()).collect();
    let json_str = serde_json::to_string_pretty(&all_json)?;

    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/dita_learning_schemas.json");

    std::fs::write(&output_path, json_str)?;
    println!("   Exported {} schemas to: {:?}", schemas_explicit.len(), output_path);

    println!("\n{}\n", "=".repeat(80));
    println!("\n✅ DITA Learning Schema Generation Complete!\n");

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

    // Show sample learning-specific schemas
    println!("\n📋 Sample Learning Schemas:\n");

    let learning_schemas: Vec<_> = schemas.iter()
        .filter(|s| {
            matches!(s, terminusdb_schema::Schema::Class { id, .. }
                if id.to_lowercase().contains("learning"))
        })
        .take(15)
        .collect();

    for (i, schema) in learning_schemas.iter().enumerate() {
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

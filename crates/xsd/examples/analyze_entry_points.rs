//! Analyze entry point candidates in schema directories.
//!
//! Demonstrates intelligent entry point detection with scoring breakdown.
//! This shows how to present customers with a dropdown of entry point options.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Entry Point Analysis Demo ===\n");

    // Test with multiple schema directories
    let test_dirs = vec![
        ("DITA Base", "../../schemas/dita/xsd/xsd1.2-url/base/xsd"),
        ("NISO-STS", "../../schemas/niso/xsd/NISO-STS-extended-1-MathML3-XSD"),
    ];

    for (name, dir) in test_dirs {
        let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir);

        if !schema_dir.exists() {
            eprintln!("⚠️  Skipping {}: directory not found\n", name);
            continue;
        }

        analyze_directory(name, &schema_dir)?;
    }

    Ok(())
}

fn analyze_directory(name: &str, schema_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "=".repeat(80));
    println!("\n📂 Analyzing: {}", name);
    println!("   Path: {:?}\n", schema_dir);

    let generator = XsdToSchemaGenerator::new();
    let candidates = generator.analyze_entry_point_candidates(schema_dir)?;

    if candidates.is_empty() {
        println!("⚠️  No entry point candidates found\n");
        return Ok(());
    }

    println!("🎯 Found {} entry point candidate(s):\n", candidates.len());

    // Show top 10 candidates with detailed scoring
    for (i, candidate) in candidates.iter().take(10).enumerate() {
        let file_name = candidate.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???");

        let score = &candidate.score;

        // Determine recommendation level
        let (symbol, recommendation) = match score.total_score {
            90.. => ("🟢", "EXCELLENT"),
            60..=89 => ("🟡", "GOOD"),
            30..=59 => ("🟠", "POSSIBLE"),
            _ => ("⚪", "UNLIKELY"),
        };

        println!("{}. {} {} - {} points ({})",
            i + 1,
            symbol,
            file_name,
            score.total_score,
            recommendation
        );

        // Show score breakdown
        println!("   ├─ Depth:    {:>3} pts  (depth: {})",
            score.depth_score, score.depth);
        println!("   ├─ Includes: {:>3} pts  ({} include/import directives)",
            score.include_count_score, score.include_count);
        println!("   ├─ Naming:   {:>3} pts",
            score.naming_score);

        // Show reasons
        if !score.reasons.is_empty() {
            println!("   └─ Reasons:");
            for reason in &score.reasons {
                println!("      • {}", reason);
            }
        }

        println!();
    }

    if candidates.len() > 10 {
        println!("   ... and {} more candidate(s)\n", candidates.len() - 10);
    }

    // Simulate UI dropdown presentation
    println!("{}", "-".repeat(80));
    println!("\n💡 UI Dropdown Presentation:\n");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ Select Entry Point Schema(s):                          │");
    println!("   ├─────────────────────────────────────────────────────────┤");

    for (i, candidate) in candidates.iter().take(5).enumerate() {
        let file_name = candidate.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???");

        let (symbol, _) = match candidate.score.total_score {
            90.. => ("🟢", "EXCELLENT"),
            60..=89 => ("🟡", "GOOD"),
            30..=59 => ("🟠", "POSSIBLE"),
            _ => ("⚪", "UNLIKELY"),
        };

        let checkmark = if i == 0 { "☑" } else { "☐" };

        println!("   │ {} {} {:48} │",
            checkmark,
            symbol,
            truncate(file_name, 48)
        );
    }

    println!("   └─────────────────────────────────────────────────────────┘");
    println!("   [ Auto-select recommended ] [ Generate Schemas ]\n");

    // Show what would be generated
    println!("{}", "-".repeat(80));
    println!("\n📊 Recommended Selection:\n");

    let top_candidates: Vec<_> = candidates.iter()
        .filter(|c| c.score.total_score >= 60)
        .take(3)
        .collect();

    if top_candidates.is_empty() {
        println!("   No candidates scored above threshold (60 points)");
        println!("   Consider manual selection or review scoring heuristics\n");
    } else {
        println!("   Automatically selecting {} file(s) with scores ≥ 60:",
            top_candidates.len());

        for candidate in top_candidates {
            let file_name = candidate.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("???");

            println!("      ✓ {} ({} points)", file_name, candidate.score.total_score);
        }

        println!("\n   These entry points will be parsed, and xmlschema will");
        println!("   automatically resolve all includes/imports to generate");
        println!("   the complete type tree.\n");
    }

    println!("{}", "=".repeat(80));
    println!();

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

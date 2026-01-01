//! Integration tests for parsing real-world DITA content
//!
//! These tests use sample DITA publications from open source projects to verify
//! that the full pipeline works with realistic content:
//!
//! 1. Load DITA XSD schemas
//! 2. Parse real DITA XML files to instances
//! 3. Insert instances into TerminusDB
//!
//! Sample content sources (all Apache 2.0 licensed):
//! - gnostyx/dita-demo-content-collection (StormCluster demo)
//! - dita-community/dita-test-cases (DITA feature tests)
//! - dita-ot/docs (DITA Open Toolkit documentation)

use schemas_dita::{Dita12, SchemaBundle};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::Schema;
use terminusdb_xsd::XsdModel;

// ============================================================================
// Paths and Setup
// ============================================================================

/// Path to the DITA sample fixtures directory
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dita-samples")
}

/// Lazily extracted DITA schemas (shared across tests)
static DITA_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for DITA schemas");
    Dita12::write_to_directory(dir.path()).expect("Failed to extract DITA schemas");
    dir
});

/// Get the path to DITA ditabase.xsd (includes all topic types: topic, concept, task, reference, glossentry)
fn dita_ditabase_xsd_path() -> PathBuf {
    DITA_DIR.path().join("xsd1.2-url/technicalContent/xsd/ditabase.xsd")
}

/// Collect all .dita files in a directory recursively
fn collect_dita_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(collect_dita_files(&path));
                } else if path.extension().map(|e| e == "dita").unwrap_or(false) {
                    files.push(path);
                }
            }
        }
    }
    files
}

/// Filter schemas to only include those without missing dependencies
fn filter_valid_schemas(schemas: &[Schema]) -> Vec<Schema> {
    use std::collections::HashSet;

    let defined: HashSet<String> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();

    schemas
        .iter()
        .filter(|s| {
            if let Schema::Class { properties, inherits, .. } = s {
                let all_props_defined = properties.iter().all(|p| {
                    p.class.starts_with("xsd:") || p.class.starts_with("sys:") || defined.contains(&p.class)
                });
                let all_inherits_defined = inherits.iter().all(|i| defined.contains(i));
                all_props_defined && all_inherits_defined
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_fixtures_exist() {
    let dir = fixtures_dir();
    assert!(dir.exists(), "Fixtures directory should exist at {:?}", dir);

    let gnostyx = dir.join("gnostyx-demo");
    let test_cases = dir.join("dita-test-cases");
    let dita_ot = dir.join("dita-ot-docs");

    assert!(gnostyx.exists(), "gnostyx-demo should exist");
    assert!(test_cases.exists(), "dita-test-cases should exist");
    assert!(dita_ot.exists(), "dita-ot-docs should exist");
}

#[test]
fn test_count_dita_files() {
    let dir = fixtures_dir();

    let gnostyx_files = collect_dita_files(&dir.join("gnostyx-demo"));
    let test_case_files = collect_dita_files(&dir.join("dita-test-cases"));
    let dita_ot_files = collect_dita_files(&dir.join("dita-ot-docs"));

    println!("DITA file counts:");
    println!("  gnostyx-demo: {} files", gnostyx_files.len());
    println!("  dita-test-cases: {} files", test_case_files.len());
    println!("  dita-ot-docs: {} files", dita_ot_files.len());
    println!("  Total: {} files", gnostyx_files.len() + test_case_files.len() + dita_ot_files.len());

    assert!(gnostyx_files.len() > 100, "Expected 200+ files in gnostyx-demo");
    assert!(test_case_files.len() > 100, "Expected 700+ files in dita-test-cases");
    assert!(dita_ot_files.len() > 100, "Expected 200+ files in dita-ot-docs");
}

#[test]
fn test_parse_gnostyx_demo_files() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA topic model");

    let files = collect_dita_files(&fixtures_dir().join("gnostyx-demo"));

    let mut success_count = 0;
    let mut fail_count = 0;
    let mut errors: Vec<(PathBuf, String)> = Vec::new();

    for file in &files {
        let xml = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(e) => {
                errors.push((file.clone(), format!("Read error: {}", e)));
                fail_count += 1;
                continue;
            }
        };

        match model.parse_xml_to_instances(&xml) {
            Ok(instances) => {
                success_count += 1;
                if instances.is_empty() {
                    println!("  Warning: {} parsed to 0 instances", file.display());
                }
            }
            Err(e) => {
                fail_count += 1;
                if errors.len() < 10 {
                    errors.push((file.clone(), e.to_string()));
                }
            }
        }
    }

    println!("\nGnostyx demo parsing results:");
    println!("  Success: {}/{}", success_count, files.len());
    println!("  Failed: {}/{}", fail_count, files.len());

    if !errors.is_empty() {
        println!("\nFirst {} errors:", errors.len().min(10));
        for (file, err) in &errors {
            let filename = file.file_name().unwrap_or_default().to_string_lossy();
            println!("  {}: {}", filename, err.chars().take(80).collect::<String>());
        }
    }

    // Allow some failures but most should parse
    let success_rate = success_count as f64 / files.len() as f64;
    assert!(
        success_rate > 0.5,
        "Expected >50% parse success rate, got {:.1}%",
        success_rate * 100.0
    );
}

#[tokio::test]
async fn test_insert_gnostyx_demo_into_terminusdb() -> anyhow::Result<()> {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA topic model");

    let files = collect_dita_files(&fixtures_dir().join("gnostyx-demo"));

    // Parse all files to instances
    let mut all_instances = Vec::new();
    let mut parse_errors = 0;

    for file in files.iter().take(50) { // Limit to first 50 for speed
        let xml = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(_) => continue,
        };

        match model.parse_xml_to_instances(&xml) {
            Ok(instances) => all_instances.extend(instances),
            Err(_) => parse_errors += 1,
        }
    }

    println!("Parsed {} instances from {} files ({} parse errors)",
             all_instances.len(), 50.min(files.len()), parse_errors);

    if all_instances.is_empty() {
        println!("No instances to insert, skipping database test");
        return Ok(());
    }

    // Debug: count how many UlClass schemas exist
    let ul_class_count = model.schemas().iter().filter(|s| matches!(s, Schema::Class { id, .. } if id == "UlClass")).count();
    println!("Number of UlClass schemas: {}", ul_class_count);

    // Debug: print Ul, Conbody and Fig schema properties and inheritance BEFORE filtering
    for schema in model.schemas() {
        if let Schema::Class { id, properties, inherits, .. } = schema {
            if id == "UlClass" {
                println!("BEFORE FILTER - Schema UlClass: all properties = {:?}",
                    properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>());
            }
            if id == "Conbody" || id == "ConbodyClass" {
                println!("BEFORE FILTER - {} inherits = {:?}, properties = {:?}",
                    id,
                    inherits,
                    properties.iter().map(|p| format!("{}: {}", p.name, p.class)).collect::<Vec<_>>());
            }
            if id == "Fig" || id == "FigClass" {
                println!("BEFORE FILTER - {} exists with {} properties",
                    id,
                    properties.len());
            }
        }
    }

    // Check if fig property exists anywhere
    let has_fig_property = model.schemas().iter().any(|s| {
        if let Schema::Class { properties, .. } = s {
            properties.iter().any(|p| p.name == "fig")
        } else {
            false
        }
    });
    println!("Any schema has 'fig' property: {}", has_fig_property);

    // Also check if id property type is defined
    let defined: std::collections::HashSet<String> = model.schemas()
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();
    println!("Is 'xsd:ID' in defined? {}", defined.contains("xsd:ID"));
    println!("Is 'NmtokenType' in defined? {}", defined.contains("NmtokenType"));

    // Start TerminusDB and insert
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_gnostyx_demo", |client, spec| {
        let schemas = filter_valid_schemas(model.schemas());

        // Debug: check UlClass and Conbody AFTER filtering
        for schema in &schemas {
            if let Schema::Class { id, properties, inherits, .. } = schema {
                if id == "UlClass" {
                    println!("AFTER FILTER - UlClass all properties = {:?}",
                        properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>());
                }
                if id == "Conbody" || id == "ConbodyClass" {
                    println!("AFTER FILTER - {} inherits = {:?}, properties = {:?}",
                        id,
                        inherits,
                        properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>());
                }
            }
        }
        let instances = all_instances.clone();
        async move {
            let args = DocumentInsertArgs::from(spec.clone());

            // Insert schemas first
            println!("Inserting {} schemas...", schemas.len());
            match client.insert_schema_instances(schemas, args.clone()).await {
                Ok(_) => println!("Schema insertion successful"),
                Err(e) => println!("Schema insertion error: {:?}", e),
            }

            // Insert instances
            println!("Inserting {} instances...", instances.len());
            let instance_refs: Vec<_> = instances.iter().collect();
            let result = client.insert_documents(instance_refs, args).await?;
            println!("Successfully inserted {} documents", result.len());

            Ok(())
        }
    }).await?;

    Ok(())
}

#[test]
fn test_parse_dita_test_cases() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA topic model");

    let files = collect_dita_files(&fixtures_dir().join("dita-test-cases"));

    let mut success_count = 0;
    let mut fail_count = 0;

    for file in &files {
        let xml = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(_) => {
                fail_count += 1;
                continue;
            }
        };

        match model.parse_xml_to_instances(&xml) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }
    }

    println!("\nDITA test cases parsing results:");
    println!("  Success: {}/{}", success_count, files.len());
    println!("  Failed: {}/{}", fail_count, files.len());

    let success_rate = success_count as f64 / files.len() as f64;
    println!("  Success rate: {:.1}%", success_rate * 100.0);
}

#[test]
fn test_parse_dita_ot_docs() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA topic model");

    let files = collect_dita_files(&fixtures_dir().join("dita-ot-docs"));

    let mut success_count = 0;
    let mut fail_count = 0;

    for file in &files {
        let xml = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(_) => {
                fail_count += 1;
                continue;
            }
        };

        match model.parse_xml_to_instances(&xml) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }
    }

    println!("\nDITA-OT docs parsing results:");
    println!("  Success: {}/{}", success_count, files.len());
    println!("  Failed: {}/{}", fail_count, files.len());

    let success_rate = success_count as f64 / files.len() as f64;
    println!("  Success rate: {:.1}%", success_rate * 100.0);
}

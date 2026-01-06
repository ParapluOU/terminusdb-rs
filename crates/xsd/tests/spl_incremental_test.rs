//! Incremental SPL parsing test - find where the issue is
//!
//! Parse SPL XSD files from smallest to largest to isolate parsing issues.

use schemas::{SchemaBundle, Spl};
use terminusdb_xsd::XsdModel;
use std::io::Write;

/// Helper to print with immediate flush
fn log(msg: &str) {
    eprintln!("{}", msg);
    std::io::stderr().flush().ok();
}

/// Parse SPL files incrementally to find which file causes issues.
/// Runs WITHOUT threads for easier debugging.
#[test]
fn test_spl_parse_files_incrementally() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Spl::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    // Test files in order of size (smallest first)
    // These are leaf files that don't include many others
    let files_to_test = [
        ("infrastructureRoot-r2b.xsd", 24),
        ("datatypes-rX-cs.xsd", 55),
        ("POCP_MT060000UV.xsd", 42),
        ("POCP_MT070000UV.xsd", 43),
    ];

    for (file, expected_lines) in &files_to_test {
        let path = temp_dir.path().join(file);
        if !path.exists() {
            log(&format!("SKIP: {} not found", file));
            continue;
        }

        log(&format!("\n=== Parsing {} ({} lines) ===", file, expected_lines));
        let start = std::time::Instant::now();

        // Parse directly without thread
        let result = XsdModel::from_file(&path, None::<&str>);
        let elapsed = start.elapsed();

        match result {
            Ok(model) => {
                log(&format!(
                    "  ✓ OK: {} schemas in {:?}",
                    model.schemas().len(),
                    elapsed
                ));
            }
            Err(e) => {
                log(&format!("  ✗ Parse error: {}", e));
            }
        }

        // If parsing took too long, warn
        if elapsed.as_secs() > 5 {
            log(&format!("  ⚠ WARNING: Parsing took {:?} - potential infinite loop?", elapsed));
        }
    }
}

/// Test parsing datatypes-r2b.xsd which is the core datatypes file (1765 lines)
#[test]
fn test_spl_parse_datatypes() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Spl::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    let path = temp_dir.path().join("datatypes-r2b.xsd");
    eprintln!("\n=== Parsing datatypes-r2b.xsd (1765 lines) ===");

    let start = std::time::Instant::now();
    let path_clone = path.clone();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .name("parse-datatypes".to_string())
        .spawn(move || XsdModel::from_file(&path_clone, None::<&str>))
        .expect("Failed to spawn thread");

    match handle.join() {
        Ok(Ok(model)) => {
            let elapsed = start.elapsed();
            eprintln!(
                "  ✓ OK: {} schemas in {:?}",
                model.schemas().len(),
                elapsed
            );
        }
        Ok(Err(e)) => {
            eprintln!("  ✗ Parse error: {}", e);
        }
        Err(_) => {
            panic!("Thread panicked - likely stack overflow in datatypes parsing");
        }
    }
}

/// Analyze SPL include hierarchy to understand depth
#[test]
fn test_spl_include_hierarchy() {
    use std::collections::{HashMap, HashSet};

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Spl::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    // Build include graph
    let mut includes: HashMap<String, Vec<String>> = HashMap::new();

    for entry in std::fs::read_dir(temp_dir.path()).expect("Failed to read dir") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        if path.extension().map(|e| e == "xsd").unwrap_or(false) {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).expect("Failed to read file");

            let mut file_includes = Vec::new();
            for line in content.lines() {
                if line.contains("xs:include") || line.contains("xsd:include") {
                    if let Some(loc) = line.split("schemaLocation=").nth(1) {
                        let loc = loc.trim_start_matches('"').split('"').next().unwrap_or("");
                        file_includes.push(loc.to_string());
                    }
                }
            }
            includes.insert(filename, file_includes);
        }
    }

    // Find max depth for POCP_MT060000UV.xsd
    fn find_max_depth(file: &str, includes: &HashMap<String, Vec<String>>, visited: &mut HashSet<String>, depth: usize) -> usize {
        if visited.contains(file) {
            return depth; // Cycle detected
        }
        visited.insert(file.to_string());

        let mut max = depth;
        if let Some(inc_list) = includes.get(file) {
            for inc in inc_list {
                let child_depth = find_max_depth(inc, includes, visited, depth + 1);
                if child_depth > max {
                    max = child_depth;
                }
            }
        }
        max
    }

    let target = "POCP_MT060000UV.xsd";
    let mut visited = HashSet::new();
    let max_depth = find_max_depth(target, &includes, &mut visited, 0);

    log(&format!("\n=== Include hierarchy for {} ===", target));
    log(&format!("Max include depth: {}", max_depth));
    log(&format!("Total unique files in include tree: {}", visited.len()));

    // Print direct includes
    if let Some(direct) = includes.get(target) {
        log(&format!("Direct includes: {:?}", direct));
    }

    // Show some problematic deep chains
    fn print_include_chain(file: &str, includes: &HashMap<String, Vec<String>>, chain: &mut Vec<String>, max_len: usize) -> Option<Vec<String>> {
        chain.push(file.to_string());
        if chain.len() > max_len {
            return Some(chain.clone());
        }
        if let Some(inc_list) = includes.get(file) {
            for inc in inc_list {
                if !chain.contains(&inc.to_string()) {
                    if let Some(result) = print_include_chain(inc, includes, chain, max_len) {
                        return Some(result);
                    }
                }
            }
        }
        chain.pop();
        None
    }

    // Find a chain of length 15+ if exists
    let mut chain = Vec::new();
    if let Some(long_chain) = print_include_chain(target, &includes, &mut chain, 15) {
        log(&format!("\nExample long include chain ({} deep):", long_chain.len()));
        for (i, f) in long_chain.iter().enumerate() {
            log(&format!("  {}: {}", i, f));
        }
    }
}

/// Test RAW XSD parsing (without TerminusDB schema generation) to isolate the issue.
/// This uses xmlschema-rs directly, not terminusdb-xsd.
#[test]
fn test_spl_raw_xsd_parsing() {
    use xmlschema::validators::XsdSchema as RustXsdSchema;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Spl::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    let path = temp_dir.path().join("POCP_MT060000UV.xsd");
    log(&format!("\n=== Raw XSD parsing for POCP_MT060000UV.xsd ==="));

    let start = std::time::Instant::now();

    // This should use the spawned 32MB thread internally
    let result = RustXsdSchema::from_file_with_catalog(&path, None::<&str>);
    let elapsed = start.elapsed();

    match result {
        Ok(schema) => {
            log(&format!(
                "  ✓ OK: Parsed in {:?}, {} elements, {} types",
                elapsed,
                schema.elements().count(),
                schema.types().count()
            ));
        }
        Err(e) => {
            log(&format!("  ✗ Parse error: {}", e));
        }
    }
}

/// Test parsing voc-r2b.xsd which is the vocabulary file (6018 lines)
#[test]
fn test_spl_parse_vocabulary() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Spl::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    let path = temp_dir.path().join("voc-r2b.xsd");
    eprintln!("\n=== Parsing voc-r2b.xsd (6018 lines) ===");

    let start = std::time::Instant::now();
    let path_clone = path.clone();
    let handle = std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024) // Vocabulary might need more
        .name("parse-vocabulary".to_string())
        .spawn(move || XsdModel::from_file(&path_clone, None::<&str>))
        .expect("Failed to spawn thread");

    match handle.join() {
        Ok(Ok(model)) => {
            let elapsed = start.elapsed();
            eprintln!(
                "  ✓ OK: {} schemas in {:?}",
                model.schemas().len(),
                elapsed
            );
        }
        Ok(Err(e)) => {
            eprintln!("  ✗ Parse error: {}", e);
        }
        Err(_) => {
            panic!("Thread panicked - likely stack overflow in vocabulary parsing");
        }
    }
}

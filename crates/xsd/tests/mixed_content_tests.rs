//! Tests for mixed content handling in XSD to TerminusDB conversion
//!
//! Mixed content is when an XML element contains both text and child elements:
//! ```xml
//! <p>Hello <b>world</b> and <i>everyone</i>!</p>
//! ```
//!
//! This is common in DITA and other document-centric schemas.

use schemas_dita::{Dita12, SchemaBundle};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_xsd::XsdModel;

/// Lazily extracted DITA schemas
static DITA_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir");
    Dita12::write_to_directory(dir.path()).expect("Failed to extract DITA schemas");
    dir
});

fn dita_ditabase_xsd_path() -> PathBuf {
    DITA_DIR
        .path()
        .join("xsd1.2-url/technicalContent/xsd/ditabase.xsd")
}

/// Demonstrates the mixed content problem with multiple inline elements
#[test]
fn test_mixed_content_multiple_inline_elements() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA model");

    // A paragraph with multiple <term> elements - common in DITA
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE topic PUBLIC "-//OASIS//DTD DITA Topic//EN" "topic.dtd">
<topic id="test">
  <title>Mixed Content Test</title>
  <body>
    <p>The <term>first term</term> and <term>second term</term> are important.</p>
  </body>
</topic>"#;

    let instances = model
        .parse_xml_to_instances(xml)
        .expect("Should parse to instances");

    println!("\n=== Mixed Content Instance ===");
    for inst in &instances {
        let value: Value = serde_json::to_value(inst).unwrap();
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
    }

    // The parsing succeeds, but let's look at the structure
    assert!(!instances.is_empty());

    // Find the P element in the instance
    let topic = &instances[0];
    let topic_value: Value = serde_json::to_value(topic).unwrap();

    // Navigate to the p element - need to go through properties
    if let Some(props) = topic_value.get("properties") {
        if let Some(body_prop) = props.as_array().and_then(|arr| {
            arr.iter()
                .find(|p| p.get("name") == Some(&Value::String("body".to_string())))
        }) {
            println!("\n=== Body property ===");
            println!("{}", serde_json::to_string_pretty(body_prop).unwrap());
        }
    }
}

/// Shows the ideal mixed content representation
///
/// ## Design: MixedContent<T>
///
/// The schema generator should produce:
///
/// ```rust,ignore
/// // Generated union of all allowed inline elements in <p>
/// #[derive(TerminusDBModel)]
/// enum PInlineUnion {
///     Term(Term),
///     B(B),
///     I(I),
///     Ph(Ph),
///     // ... all inline elements allowed in <p>
/// }
///
/// // Generic MixedContent type (defined once in terminusdb-schema)
/// #[derive(TerminusDBModel)]
/// struct MixedContent<T> {
///     /// Text with {} placeholders marking where substitutions go
///     text: String,
///     /// Ordered list of substitutions (Vec = List in TerminusDB, preserves order)
///     subs: Vec<T>,
/// }
///
/// // P element uses MixedContent with its allowed inline types
/// #[derive(TerminusDBModel)]
/// struct P {
///     content: MixedContent<PInlineUnion>,
///     // ... other P attributes like @id, @class, etc.
/// }
/// ```
///
/// ## Instance Example
///
/// For: `<p>The <term>first</term> and <term>second</term> are important.</p>`
///
/// ```json
/// {
///   "@type": "P",
///   "content": {
///     "@type": "MixedContent<PInlineUnion>",
///     "text": "The {} and {} are important.",
///     "subs": [
///       { "@type": "Term", "_text": "first" },
///       { "@type": "Term", "_text": "second" }
///     ]
///   }
/// }
/// ```
///
/// ## Benefits
/// 1. **Order preserved**: subs is a List, substitutions applied in order
/// 2. **All elements captured**: no loss of repeated inline elements
/// 3. **Position preserved**: {} marks exactly where each element appeared
/// 4. **Type-safe**: T is constrained to only allowed inline types
/// 5. **Reconstructable**: original XML can be reconstructed from this
#[test]
fn test_mixed_content_ideal_representation() {
    println!("See test doc comments for MixedContent<T> design specification");
}

/// Test that shows the schema uses MixedContent for mixed content types
#[test]
fn test_schema_generates_mixed_content_structure() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA model");

    let schemas = model.schemas();

    // Find the P schema (or PClass)
    let p_schema = schemas.iter().find(|s| {
        if let terminusdb_schema::Schema::Class { id, .. } = s {
            id == "P" || id == "PClass"
        } else {
            false
        }
    });

    if let Some(terminusdb_schema::Schema::Class { id, properties, .. }) = p_schema {
        println!("\n=== {} Schema Properties ===", id);
        for prop in properties {
            println!("  {}: {} ({:?})", prop.name, prop.class, prop.r#type);
        }

        // Check if it has a 'content' property (mixed content structure)
        let content_prop = properties.iter().find(|p| p.name == "content");
        if let Some(prop) = content_prop {
            println!("\n✅ Found 'content' property for MixedContent!");
            println!("   class: {}", prop.class);

            // Find the MixedContent class
            let mixed_content_schema = schemas.iter().find(|s| s.class_name() == &prop.class);
            if let Some(terminusdb_schema::Schema::Class { id, properties, .. }) =
                mixed_content_schema
            {
                println!("\n=== {} Schema ===", id);
                for prop in properties {
                    println!("  {}: {} ({:?})", prop.name, prop.class, prop.r#type);
                }
            }
        }
    }

    // Find the inline union TaggedUnion
    let inline_unions: Vec<_> = schemas
        .iter()
        .filter(|s| s.class_name().ends_with("Inline") && s.is_tagged_union())
        .collect();

    println!("\n=== Inline Union TaggedUnions ===");
    for schema in &inline_unions {
        println!("  {}", schema.class_name());
    }
}

/// Test parsing a real DITA file with mixed content
#[test]
fn test_real_dita_mixed_content_parsing() {
    let model = XsdModel::from_file(&dita_ditabase_xsd_path(), None::<&str>)
        .expect("Failed to load DITA model");

    // Real DITA content from gnostyx-demo
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE concept PUBLIC "-//OASIS//DTD DITA Concept//EN" "concept.dtd">
<concept id="test">
 <title>Test Concept</title>
 <conbody>
  <p>The <term>submission cluster</term> is the job submission gateway. Through the
    submission cluster, jobs are forwarded for rapid scheduling in each <term>execution
    cluster</term>.</p>
 </conbody>
</concept>"#;

    match model.parse_xml_to_instances(xml) {
        Ok(instances) => {
            println!("\n=== Parsed {} instances ===", instances.len());
            for inst in &instances {
                let value: Value = serde_json::to_value(inst).unwrap();
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(e) => {
            println!("Parse error: {}", e);
        }
    }
}

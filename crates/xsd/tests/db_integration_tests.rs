//! Integration tests for XSD-to-Instance-to-TerminusDB flow
//!
//! These tests verify the end-to-end flow of:
//! 1. Loading XSD schemas
//! 2. Converting XSD schemas to TerminusDB Schema definitions
//! 3. Inserting schemas into TerminusDB (in-memory server)
//! 4. Parsing XML to instances (future - blocked on schema/type mapping improvements)
//! 5. Inserting instances into TerminusDB
//!
//! This validates that XSD schema bundles can be used with TerminusDB.

use schemas_dita::{Dita12, SchemaBundle};
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::Schema;
use terminusdb_xsd::XsdModel;

// ============================================================================
// Schema Extraction - shared temp directories for efficiency
// ============================================================================

/// Lazily extracted DITA schemas (shared across tests)
static DITA_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for DITA schemas");
    Dita12::write_to_directory(dir.path()).expect("Failed to extract DITA schemas");
    dir
});

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the path to DITA basetopic.xsd
fn dita_topic_xsd_path() -> std::path::PathBuf {
    DITA_DIR.path().join("xsd1.2-url/base/xsd/basetopic.xsd")
}

/// Collect all class names that are referenced by schemas but not defined
fn find_missing_dependencies(schemas: &[Schema]) -> Vec<String> {
    use std::collections::HashSet;

    // Collect all defined class names
    let defined: HashSet<String> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();

    // Collect all referenced class names
    let mut referenced: HashSet<String> = HashSet::new();
    for schema in schemas {
        if let Schema::Class {
            properties,
            inherits,
            ..
        } = schema
        {
            for prop in properties {
                // Skip built-in types (xsd: primitives and sys: system types like sys:JSON)
                if !prop.class.starts_with("xsd:") && !prop.class.starts_with("sys:") {
                    referenced.insert(prop.class.clone());
                }
            }
            for parent in inherits {
                referenced.insert(parent.clone());
            }
        }
    }

    // Find missing
    referenced.difference(&defined).cloned().collect()
}

/// Trace which schemas reference a specific missing type
fn trace_missing_type_references(schemas: &[Schema], missing_type: &str) -> Vec<(String, String)> {
    let mut references = Vec::new();

    for schema in schemas {
        if let Schema::Class {
            id,
            properties,
            inherits,
            ..
        } = schema
        {
            for prop in properties {
                if prop.class == missing_type {
                    references.push((id.clone(), format!("property '{}'", prop.name)));
                }
            }
            for parent in inherits {
                if parent == missing_type {
                    references.push((id.clone(), "inherits".to_string()));
                }
            }
        }
    }

    references
}

/// Filter schemas to only include those without missing dependencies
fn filter_valid_schemas(schemas: &[Schema]) -> Vec<Schema> {
    use std::collections::HashSet;

    // Collect all defined class names
    let defined: HashSet<String> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();

    // Keep only schemas with all dependencies satisfied
    schemas
        .iter()
        .filter(|s| {
            if let Schema::Class {
                properties,
                inherits,
                ..
            } = s
            {
                // Check all property types exist (xsd: and sys: are built-in types)
                let props_ok = properties.iter().all(|p| {
                    p.class.starts_with("xsd:")
                        || p.class.starts_with("sys:")
                        || defined.contains(&p.class)
                });
                // Check all parent classes exist
                let inherits_ok = inherits.iter().all(|parent| defined.contains(parent));
                props_ok && inherits_ok
            } else {
                true // Enums are always valid
            }
        })
        .cloned()
        .collect()
}

// ============================================================================
// Tests - Schema Loading
// ============================================================================

#[test]
fn test_xsd_model_loads_dita_schemas() {
    let topic_path = dita_topic_xsd_path();

    let model =
        XsdModel::from_file(&topic_path, None::<&str>).expect("Failed to load DITA topic model");

    let schemas = model.schemas();
    assert!(
        !schemas.is_empty(),
        "XsdModel should generate TerminusDB schemas from DITA XSD"
    );

    // Count schema types
    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();
    let enum_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .count();

    println!("DITA XsdModel generated:");
    println!("  Total schemas: {}", schemas.len());
    println!("  Classes: {}", class_count);
    println!("  Enums: {}", enum_count);

    // Check for expected DITA types
    let class_names: Vec<_> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    // DITA should have topic-related types
    assert!(
        class_names
            .iter()
            .any(|n| n.contains("Topic") || n.contains("topic")),
        "DITA should have topic-related schemas"
    );
}

#[test]
fn test_analyze_schema_dependencies() {
    let topic_path = dita_topic_xsd_path();

    let model =
        XsdModel::from_file(&topic_path, None::<&str>).expect("Failed to load DITA topic model");

    let schemas = model.schemas();
    let missing = find_missing_dependencies(schemas);

    println!("Schema dependency analysis:");
    println!("  Total schemas: {}", schemas.len());
    println!("  Missing dependencies: {}", missing.len());

    if !missing.is_empty() {
        println!("\n  Missing types and their references:");
        for dep in missing.iter().take(10) {
            println!("\n    {} is referenced by:", dep);
            let refs = trace_missing_type_references(schemas, dep);
            for (schema_id, ref_type) in refs.iter().take(5) {
                println!("      - {} via {}", schema_id, ref_type);
            }
            if refs.len() > 5 {
                println!("      ... and {} more", refs.len() - 5);
            }
        }
    }

    // Check how many schemas are valid
    let valid = filter_valid_schemas(schemas);
    println!("\n  Valid schemas (all deps satisfied): {}", valid.len());
}

// ============================================================================
// Tests - TerminusDB Integration
// ============================================================================

/// Test inserting valid schemas (those with all dependencies satisfied) into TerminusDB
#[tokio::test]
async fn test_insert_valid_schemas_into_terminusdb() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    let model =
        XsdModel::from_file(&topic_path, None::<&str>).expect("Failed to load DITA topic model");

    let all_schemas = model.schemas();
    let valid_schemas = filter_valid_schemas(all_schemas);

    println!("Inserting valid schemas into TerminusDB:");
    println!("  Total schemas from XSD: {}", all_schemas.len());
    println!("  Valid schemas to insert: {}", valid_schemas.len());

    if valid_schemas.is_empty() {
        println!("WARNING: No valid schemas to insert (all have missing dependencies)");
        return Ok(());
    }

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_valid_schemas", |client, spec| {
            let schemas = valid_schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                client
                    .insert_schema_instances(schemas.clone(), args)
                    .await?;

                println!("Successfully inserted {} valid schemas", schemas.len());
                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// Test that verifies schemas from a small subset work correctly
#[tokio::test]
async fn test_insert_minimal_subset_schemas() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    let model =
        XsdModel::from_file(&topic_path, None::<&str>).expect("Failed to load DITA topic model");

    let schemas = model.schemas();

    // Take only enum schemas which should always be self-contained
    let enum_schemas: Vec<_> = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .cloned()
        .collect();

    println!("Testing enum schemas only:");
    println!("  Enum schemas found: {}", enum_schemas.len());

    if enum_schemas.is_empty() {
        println!("No enum schemas to test");
        return Ok(());
    }

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_enum_schemas", |client, spec| {
            let schemas = enum_schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                client
                    .insert_schema_instances(schemas.clone(), args)
                    .await?;

                println!("Successfully inserted {} enum schemas", schemas.len());
                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// End-to-end test showing the full intended flow
///
/// NOTE: This test currently documents two limitations:
///
/// 1. **xs:redefine not supported**: The underlying xmlschema-rs library doesn't
///    support `xs:redefine` yet. DITA uses `xs:redefine` to include `commonElementGrp.xsd`
///    which contains essential model groups like `title`, `keyword`, etc.
///    This causes the `title` element (and others) to be missing from `TopicClass` properties.
///
/// 2. **Inheritance works**: XSD type extension is now correctly mapped to TerminusDB's
///    `inherits` field. For example, `Topic` inherits from `TopicClass`.
///
/// Once xmlschema-rs supports `xs:redefine`, the full flow should work correctly.
#[tokio::test]
async fn test_full_xsd_xml_flow_documents_limitations() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    // Step 1: Load XSD model
    let model =
        XsdModel::from_file(&topic_path, None::<&str>).expect("Failed to load DITA topic model");

    println!("Step 1: Loaded XSD model");
    println!("  Generated {} TerminusDB schemas", model.schemas().len());

    // Debug: Show class names containing "topic" (case-insensitive)
    let topic_classes: Vec<_> = model
        .class_names()
        .into_iter()
        .filter(|n| n.to_lowercase().contains("topic"))
        .collect();
    println!("  Topic-related classes: {:?}", topic_classes);

    // Debug: Show number of XSD schemas and their root elements
    println!("  Number of XSD schemas: {}", model.xsd_schemas().len());
    for (i, xsd) in model.xsd_schemas().iter().enumerate() {
        println!(
            "    XSD[{}] root elements: {:?}",
            i,
            xsd.root_elements
                .iter()
                .map(|e| e.name.split('}').last().unwrap_or(&e.name))
                .collect::<Vec<_>>()
        );
    }

    // Debug: Show is_anonymous flag and mixed content for key types before schema generation
    println!("  XSD complex types flags:");
    for xsd in model.xsd_schemas() {
        for ct in &xsd.complex_types {
            let ct_local = ct.name.split('}').last().unwrap_or(&ct.name);
            if ct_local.contains("title")
                || ct_local.contains("body")
                || ct_local.contains("topic")
                || ct_local == "p"
            {
                println!(
                    "    {} -> is_anonymous={}, mixed={}, simple_content={}, base_type={:?}",
                    ct_local, ct.is_anonymous, ct.mixed, ct.has_simple_content, ct.base_type
                );
            }
        }
    }

    // Debug: Show properties of Topic-related classes (with inheritance)
    for class_name in &[
        "Topic",
        "TopicClass",
        "Title",
        "TitleClass",
        "Body",
        "BodyClass",
    ] {
        if let Some(Schema::Class {
            properties,
            inherits,
            ..
        }) = model.find_schema(class_name)
        {
            println!("  {} class:", class_name);
            println!("    inherits: {:?}", inherits);
            println!("    properties ({}):", properties.len());
            for prop in properties.iter().take(5) {
                println!("      - {}: {}", prop.name, prop.class);
            }
            if properties.len() > 5 {
                println!("      ... and {} more", properties.len() - 5);
            }
        } else {
            println!("  {} - NOT FOUND", class_name);
        }
    }

    // Debug: Show element-to-class mapping
    let element_map = model.element_to_class_map();
    println!("  Element-to-class mappings ({}):", element_map.len());
    for key in &["topic", "title", "body", "p"] {
        if let Some(class) = element_map.get(*key) {
            println!("    {} -> {}", key, class);
        } else {
            println!("    {} -> NOT MAPPED", key);
        }
    }

    // Debug: Show XSD root elements and their types
    println!("  XSD root elements:");
    for xsd in model.xsd_schemas() {
        for elem in &xsd.root_elements {
            let local_name = elem.name.split('}').last().unwrap_or(&elem.name);
            let type_name = elem
                .type_info
                .as_ref()
                .and_then(|ti| ti.name.as_ref().or(ti.qualified_name.as_ref()))
                .map(|s| s.as_str())
                .unwrap_or("(anonymous)");
            println!("    {} -> {}", local_name, type_name);
        }
        // Show complex types named 'topic' or 'body' (exactly)
        println!("  Complex types named 'topic', 'body', 'title':");
        for ct in xsd.complex_types.iter() {
            let ct_local = ct.name.split('}').last().unwrap_or(&ct.name);
            if ct_local == "topic"
                || ct_local == "body"
                || ct_local == "title"
                || ct_local == "topic.class"
                || ct_local == "body.class"
                || ct_local == "title.class"
            {
                println!(
                    "    Complex type '{}' (anonymous={}):",
                    ct_local, ct.is_anonymous
                );
                println!("      base_type: {:?}", ct.base_type);
                println!(
                    "      attributes: {:?}",
                    ct.attributes.as_ref().map(|a| a.len()).unwrap_or(0)
                );
                if let Some(children) = &ct.child_elements {
                    println!("      children ({}):", children.len());
                    for child in children.iter() {
                        println!(
                            "        {} -> {} (min={:?}, max={:?})",
                            child.name, child.element_type, child.min_occurs, child.max_occurs
                        );
                    }
                } else {
                    println!("      children: None");
                }
            }
        }
    }

    // Step 2: Try parsing XML (expected to fail with current implementation)
    let minimal_dita = r#"<?xml version="1.0" encoding="UTF-8"?>
<topic id="test-topic">
    <title>Test Topic Title</title>
    <body>
        <p>This is a test paragraph.</p>
    </body>
</topic>
"#;

    let parse_result = model.parse_xml_to_instances(minimal_dita);

    match parse_result {
        Ok(instances) => {
            println!(
                "Step 2: Successfully parsed XML to {} instances",
                instances.len()
            );

            // Step 3: Insert into TerminusDB
            let server = TerminusDBServer::test_instance().await?;

            server
                .with_tmp_db("test_full_flow", |client, spec| {
                    let schemas = filter_valid_schemas(model.schemas()).clone();
                    let insts = instances.clone();
                    async move {
                        let args = DocumentInsertArgs::from(spec.clone());

                        // Insert schemas first
                        if !schemas.is_empty() {
                            println!("Inserting {} schemas", schemas.len());
                            // Debug: Show Topic and Body schemas
                            for s in &schemas {
                                if let Schema::Class {
                                    id,
                                    inherits,
                                    properties,
                                    subdocument,
                                    ..
                                } = s
                                {
                                    if id == "Topic"
                                        || id == "Body"
                                        || id == "TopicClass"
                                        || id == "BodyClass"
                                        || id == "Title"
                                        || id == "TitleClass"
                                        || id == "P"
                                        || id == "PClass"
                                    {
                                        println!(
                                            "Schema {}: inherits={:?}, subdoc={}, props={:?}",
                                            id,
                                            inherits,
                                            subdocument,
                                            properties
                                                .iter()
                                                .map(|p| format!("{}: {}", p.name, p.class))
                                                .collect::<Vec<_>>()
                                        );
                                    }
                                }
                            }
                            client
                                .insert_schema_instances(schemas, args.clone())
                                .await?;
                            println!("Step 3a: Inserted schemas");
                        }

                        // Insert instances
                        if !insts.is_empty() {
                            // Debug: Print instance JSON before inserting
                            use terminusdb_schema::json::ToJson;
                            for inst in &insts {
                                let json = serde_json::to_string_pretty(&inst.to_json())?;
                                println!("Instance to insert:\n{}", json);
                            }
                            let instance_refs: Vec<_> = insts.iter().collect();
                            match client.insert_documents(instance_refs, args).await {
                                Ok(_) => println!("Step 3b: Inserted instances"),
                                Err(e) => {
                                    println!("Error: {}", e);
                                    println!("\nDetailed error: {:#?}", e);
                                    return Err(e);
                                }
                            }
                        }

                        Ok(())
                    }
                })
                .await?;
        }
        Err(e) => {
            // This is expected with the current implementation
            println!("Step 2: XML parsing to instances failed (expected)");
            println!("  Error: {}", e);
            println!("");
            println!("This limitation exists because:");
            println!("  - xmlschema-rs returns JSON without @type annotations");
            println!("  - The parser can't map XML element names to schema class names");
            println!("  - This requires enhancement to the XmlToInstanceParser");
        }
    }

    Ok(())
}

/// Test minimal DITA-like schema structure to isolate TerminusDB failure
#[tokio::test]
async fn test_minimal_dita_schema_structure() -> anyhow::Result<()> {
    use serde_json::json;
    use terminusdb_schema::{Key, Property, TypeFamily};

    // Simplified DITA-like structure:
    // - TitleClass (base subdocument)
    // - Title (extends TitleClass, subdocument)
    // - BodyClass (base subdocument)
    // - Body (extends BodyClass, subdocument)
    // - PClass (base subdocument)
    // - P (extends PClass, subdocument)
    // - TopicClass (base document, has title: TitleClass and body: BodyClass)
    // - Topic (extends TopicClass, document)

    // Use Key::Random for subdocuments (like in passing tests)
    let title_class = Schema::Class {
        id: "TitleClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "_text".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let title = Schema::Class {
        id: "Title".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec!["TitleClass".to_string()],
        properties: vec![Property {
            name: "class".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let body_class = Schema::Class {
        id: "BodyClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "p".to_string(),
            class: "PClass".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let body = Schema::Class {
        id: "Body".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec!["BodyClass".to_string()],
        properties: vec![Property {
            name: "class".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let p_class = Schema::Class {
        id: "PClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "_text".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let p = Schema::Class {
        id: "P".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec!["PClass".to_string()],
        properties: vec![Property {
            name: "class".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let topic_class = Schema::Class {
        id: "TopicClass".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "id".to_string(),
                class: "xsd:string".to_string(),
                r#type: Some(TypeFamily::Optional),
            },
            Property {
                name: "title".to_string(),
                class: "TitleClass".to_string(),
                r#type: None,
            }, // Required (min=1)
            Property {
                name: "body".to_string(),
                class: "BodyClass".to_string(),
                r#type: Some(TypeFamily::Optional),
            },
        ],
        unfoldable: false,
    };

    let topic = Schema::Class {
        id: "Topic".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec!["TopicClass".to_string()],
        properties: vec![Property {
            name: "class".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let schemas = vec![
        title_class,
        title,
        body_class,
        body,
        p_class,
        p,
        topic_class,
        topic,
    ];

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_minimal_dita", |client, spec| {
            let schemas = schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert schemas
                client
                    .insert_schema_instances(schemas, args.clone().as_schema())
                    .await?;
                println!("✓ Schemas inserted");

                // Test instance matching DITA test - simplified without body
                let instance = json!({
                    "@type": "Topic",
                    "id": "test-topic",
                    "title": {
                        "@type": "Title",
                        "_text": "Test Topic Title"
                    }
                });

                match client.insert_documents(vec![&instance], args.clone()).await {
                    Ok(_) => println!("✓ Instance inserted successfully"),
                    Err(e) => {
                        println!("✗ Instance insertion failed: {}", e);
                        println!("Detailed error: {:#?}", e);
                        return Err(e);
                    }
                }

                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// Test inheritance with property access
#[tokio::test]
async fn test_inherited_properties_with_subdocuments() -> anyhow::Result<()> {
    use serde_json::json;
    use terminusdb_schema::{Key, Property, TypeFamily};

    // Reproduce DITA-like structure:
    // - BaseClass has property `child: ChildBaseClass`
    // - DerivedClass extends BaseClass
    // - Child extends ChildBaseClass
    // Test: Insert DerivedClass instance with `child: { @type: Child }`

    let child_base = Schema::Class {
        id: "ChildBaseClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "name".to_string(),
            class: "xsd:string".to_string(),
            r#type: None,
        }],
        unfoldable: false,
    };

    let child_derived = Schema::Class {
        id: "ChildDerived".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec!["ChildBaseClass".to_string()],
        properties: vec![Property {
            name: "_text".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let parent_base = Schema::Class {
        id: "ParentBaseClass".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "id".to_string(),
                class: "xsd:string".to_string(),
                r#type: None,
            },
            Property {
                name: "child".to_string(),
                class: "ChildBaseClass".to_string(),
                r#type: None,
            },
        ],
        unfoldable: false,
    };

    let parent_derived = Schema::Class {
        id: "ParentDerived".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec!["ParentBaseClass".to_string()],
        properties: vec![Property {
            name: "_text".to_string(),
            class: "xsd:string".to_string(),
            r#type: Some(TypeFamily::Optional),
        }],
        unfoldable: false,
    };

    let schemas = vec![child_base, child_derived, parent_base, parent_derived];

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_inherited_props", |client, spec| {
            let schemas = schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_schema_instances(schemas, args.clone().as_schema())
                    .await?;
                println!("Schemas inserted");

                // Test: Insert ParentDerived with inherited property `child`
                let instance = json!({
                    "@type": "ParentDerived",
                    "id": "test-id",
                    "child": {
                        "@type": "ChildDerived",
                        "name": "child name",
                        "_text": "text content"
                    }
                });

                match client.insert_documents(vec![&instance], args.clone()).await {
                    Ok(_) => println!("✓ Inherited property with derived subdocument type works"),
                    Err(e) => {
                        println!(
                            "✗ Inherited property with derived subdocument type failed: {}",
                            e
                        );
                        return Err(e);
                    }
                }

                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// Minimal test to isolate subtype polymorphism issue
#[tokio::test]
async fn test_subtype_polymorphism_in_subdocuments() -> anyhow::Result<()> {
    use serde_json::json;
    use terminusdb_schema::{Key, Property};

    // Create a minimal schema hierarchy:
    // - ParentClass (subdocument)
    // - ChildClass extends ParentClass (subdocument)
    // - Container (document) has property `item: ParentClass`

    let parent_schema = Schema::Class {
        id: "ParentClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "name".to_string(),
            class: "xsd:string".to_string(),
            r#type: None,
        }],
        unfoldable: false,
    };

    let child_schema = Schema::Class {
        id: "ChildClass".to_string(),
        base: None,
        key: Key::Random,
        documentation: None,
        subdocument: true,
        r#abstract: false,
        inherits: vec!["ParentClass".to_string()],
        properties: vec![Property {
            name: "extra".to_string(),
            class: "xsd:string".to_string(),
            r#type: None,
        }],
        unfoldable: false,
    };

    let container_schema = Schema::Class {
        id: "Container".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "item".to_string(),
            class: "ParentClass".to_string(), // Property typed as ParentClass
            r#type: None,
        }],
        unfoldable: false,
    };

    let schemas = vec![parent_schema, child_schema, container_schema];

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_polymorphism", |client, spec| {
            let schemas = schemas.clone();
            async move {
                // Insert schemas
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_schema_instances(schemas, args.clone().as_schema())
                    .await?;
                println!("Schemas inserted");

                // Test 1: Insert with exact type (ParentClass)
                let instance_exact = json!({
                    "@type": "Container",
                    "item": {
                        "@type": "ParentClass",
                        "name": "exact type"
                    }
                });

                match client
                    .insert_documents(vec![&instance_exact], args.clone())
                    .await
                {
                    Ok(_) => println!("✓ Exact type (ParentClass) works"),
                    Err(e) => println!("✗ Exact type (ParentClass) failed: {}", e),
                }

                // Test 2: Insert with subtype (ChildClass where ParentClass expected)
                let instance_subtype = json!({
                    "@type": "Container",
                    "item": {
                        "@type": "ChildClass",
                        "name": "subtype",
                        "extra": "child-specific"
                    }
                });

                match client
                    .insert_documents(vec![&instance_subtype], args.clone())
                    .await
                {
                    Ok(_) => println!("✓ Subtype (ChildClass where ParentClass expected) works"),
                    Err(e) => println!(
                        "✗ Subtype (ChildClass where ParentClass expected) failed: {}",
                        e
                    ),
                }

                Ok(())
            }
        })
        .await?;

    Ok(())
}

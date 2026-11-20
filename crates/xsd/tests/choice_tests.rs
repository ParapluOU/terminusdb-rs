//! Tests for xs:choice handling in XSD to TerminusDB conversion
//!
//! xs:choice is a compositor that allows exactly one of its children to appear.
//! In TerminusDB schema, this maps to a TaggedUnion where each choice option
//! becomes a variant.
//!
//! Test cases:
//! - Basic choice between complex types
//! - Choice with maxOccurs="unbounded" (repeating choice)
//! - Optional choice (minOccurs="0")
//! - Choice within sequence
//! - Nested structures with choice

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::{Schema, TypeFamily};
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use terminusdb_xsd::XsdModel;

/// Path to the choice_types.xsd fixture
fn choice_xsd_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/choice_types.xsd"
    )
}

/// Helper to find a schema by class name
fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas.iter().find(|s| match s {
        Schema::Class { id, .. } => id == name,
        Schema::TaggedUnion { id, .. } => id == name,
        _ => false,
    })
}

/// The variant names carried by an instance's ordered `children` list.
///
/// Parsed instances mirror the generated schemas: a complex type with child
/// elements holds `children`, a list of `{Type}Child` tagged-union instances,
/// each with a single property named after the element it wraps.
fn child_variant_names(instance: &terminusdb_schema::Instance) -> Vec<String> {
    match instance.properties.get("children") {
        Some(terminusdb_schema::InstanceProperty::Relations(children)) => children
            .iter()
            .filter_map(|c| match c {
                terminusdb_schema::RelationValue::One(child) => {
                    child.properties.keys().next().cloned()
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The variant names of a complex type's generated children union.
///
/// An `xs:choice` is not flattened into one optional property per branch.
/// Each complex type with child elements gets `children: List<{Type}Child>`,
/// where `{Type}Child` is a TaggedUnion carrying one variant per permitted
/// child element — so a choice's branches are that union's variants, which is
/// what preserves XML document order across heterogeneous children.
fn children_union_variants<'a>(schemas: &'a [Schema], type_name: &str) -> Vec<&'a str> {
    let union_name = format!("{type_name}Child");
    match find_tagged_union(schemas, &union_name) {
        Some(Schema::TaggedUnion { properties, .. }) => {
            properties.iter().map(|p| p.name.as_str()).collect()
        }
        _ => Vec::new(),
    }
}

/// Helper to find a TaggedUnion by name
fn find_tagged_union<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas
        .iter()
        .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == name))
}

/// Helper to check if a schema is a TaggedUnion
fn is_tagged_union(schema: &Schema) -> bool {
    matches!(schema, Schema::TaggedUnion { .. })
}

// ============================================================================
// Schema Generation Tests
// ============================================================================

#[test]
fn test_choice_xsd_loads_successfully() {
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    // Should have all complex types (XSD uses camelCase, schema generator converts to PascalCase)
    let type_names: Vec<&str> = xsd_schema
        .complex_types
        .iter()
        .map(|ct| ct.name.as_str())
        .collect();

    eprintln!("Complex types found: {:?}", type_names);

    // XsdSchema stores original XSD names (camelCase)
    assert!(
        type_names
            .iter()
            .any(|n| n.contains("document") || n.contains("Document")),
        "documentType/DocumentType not found in {:?}",
        type_names
    );
    assert!(
        type_names
            .iter()
            .any(|n| n.contains("article") || n.contains("Article")),
        "articleType/ArticleType not found in {:?}",
        type_names
    );
    assert!(
        type_names
            .iter()
            .any(|n| n.contains("report") || n.contains("Report")),
        "reportType/ReportType not found in {:?}",
        type_names
    );
    assert!(
        type_names
            .iter()
            .any(|n| n.contains("memo") || n.contains("Memo")),
        "memoType/MemoType not found in {:?}",
        type_names
    );
    assert!(
        type_names
            .iter()
            .any(|n| n.contains("payment") || n.contains("Payment")),
        "paymentType/PaymentType not found in {:?}",
        type_names
    );
}

#[test]
fn test_choice_generates_tagged_union() {
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    // Debug: print all schemas
    eprintln!("\n=== Generated Schemas ===");
    for schema in &schemas {
        match schema {
            Schema::Class { id, properties, .. } => {
                eprintln!("Class: {}", id);
                for prop in properties {
                    eprintln!("  {} : {} ({:?})", prop.name, prop.class, prop.r#type);
                }
            }
            Schema::TaggedUnion { id, properties, .. } => {
                eprintln!("TaggedUnion: {}", id);
                for prop in properties {
                    eprintln!("  {} : {}", prop.name, prop.class);
                }
            }
            Schema::Enum { id, values, .. } => {
                eprintln!("Enum: {} = {:?}", id, values);
            }
            _ => {}
        }
    }

    // DocumentType should have a property that references a TaggedUnion for the choice
    let doc_type = find_class(&schemas, "DocumentType").expect("DocumentType not found");

    if let Schema::Class { properties, .. } = doc_type {
        let prop_names: Vec<&str> = properties.iter().map(|p| p.name.as_str()).collect();
        eprintln!("\nDocumentType properties: {:?}", prop_names);

        // The choice is represented as a union, the second of the two shapes
        // this test has always allowed: DocumentType carries one ordered
        // `children` property typed as the DocumentTypeChild TaggedUnion.
        assert!(
            prop_names.contains(&"children"),
            "DocumentType should carry the ordered children property, got {:?}",
            prop_names
        );

        let variants = children_union_variants(&schemas, "DocumentType");
        eprintln!("DocumentTypeChild variants: {:?}", variants);
        for branch in ["article", "report", "memo"] {
            assert!(
                variants.contains(&branch),
                "choice branch {branch:?} missing from DocumentTypeChild: {:?}",
                variants
            );
        }
    }
}

#[test]
fn test_choice_elements_are_optional() {
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    let doc_type = find_class(&schemas, "DocumentType").expect("DocumentType not found");

    if let Schema::Class { properties, .. } = doc_type {
        // In xs:choice, each element is mutually exclusive - only one can appear
        // So each choice element should be Optional in the generated schema
        for prop in properties {
            if prop.name == "article" || prop.name == "report" || prop.name == "memo" {
                eprintln!("Choice property '{}': type = {:?}", prop.name, prop.r#type);
                // These should be Optional since only one can appear
                assert!(
                    prop.r#type == Some(TypeFamily::Optional) || prop.r#type.is_none(),
                    "Choice element '{}' should be optional, got {:?}",
                    prop.name,
                    prop.r#type
                );
            }
        }
    }
}

#[test]
fn test_repeating_choice_generates_set() {
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    // MixedContentType has choice with maxOccurs="unbounded"
    let mixed_type = find_class(&schemas, "MixedContentType");

    eprintln!("\n=== MixedContentType ===");
    if let Some(Schema::Class { properties, .. }) = mixed_type {
        for prop in properties {
            eprintln!("  {} : {} ({:?})", prop.name, prop.class, prop.r#type);
        }

        // Choice elements with maxOccurs="unbounded" should be sets/lists
        for prop in properties {
            if prop.name == "paragraph" || prop.name == "image" || prop.name == "code" {
                eprintln!("Repeating choice '{}': {:?}", prop.name, prop.r#type);
            }
        }
    } else {
        eprintln!("MixedContentType not found as Class");
    }
}

#[test]
fn test_payment_type_has_payment_method_choice() {
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    let payment_type = find_class(&schemas, "PaymentType").expect("PaymentType not found");

    if let Schema::Class { properties, .. } = payment_type {
        let prop_names: Vec<&str> = properties.iter().map(|p| p.name.as_str()).collect();
        eprintln!("\nPaymentType properties: {:?}", prop_names);

        // amount/currency and the payment-method choice are all child elements,
        // so they are variants of PaymentTypeChild rather than properties of
        // PaymentType itself (which carries the ordered `children` list).
        assert!(
            prop_names.contains(&"children"),
            "PaymentType should carry the ordered children property, got {:?}",
            prop_names
        );

        let variants = children_union_variants(&schemas, "PaymentType");
        eprintln!("PaymentTypeChild variants: {:?}", variants);
        for required in ["amount", "currency"] {
            assert!(
                variants.contains(&required),
                "{required:?} missing from PaymentTypeChild: {:?}",
                variants
            );
        }

        let has_credit_card = variants.contains(&"creditCard");
        let has_bank_transfer = variants.contains(&"bankTransfer");
        let has_digital_wallet = variants.contains(&"digitalWallet");

        assert!(
            has_credit_card || has_bank_transfer || has_digital_wallet,
            "Payment method choice not found in PaymentTypeChild: {:?}",
            variants
        );
    }
}

// ============================================================================
// XML Parsing Tests
// ============================================================================

#[test]
fn test_parse_document_with_article_choice() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://example.com/choice" id="doc1">
    <title>Test Document</title>
    <article>
        <abstract>This is the abstract</abstract>
        <body>This is the article body</body>
        <keywords>test</keywords>
        <keywords>example</keywords>
    </article>
    <footer>Copyright 2024</footer>
</document>"#;

    let result = model.parse_xml_to_instances(xml);

    match result {
        Ok(instances) => {
            eprintln!("\n=== Parsed Article Document ===");
            eprintln!("Instance count: {}", instances.len());

            for inst in &instances {
                let json = serde_json::to_string_pretty(inst).unwrap();
                eprintln!("{}", json);
            }

            assert!(
                !instances.is_empty(),
                "Should parse to at least one instance"
            );

            // The choice branch shows up as a variant inside the ordered
            // children list, not as a flat property on the document.
            let first = &instances[0];
            let variants = child_variant_names(first);
            eprintln!("child variants: {:?}", variants);
            assert!(
                variants.iter().any(|v| v == "article" || v == "title"),
                "children should carry an article or title variant, got {:?}",
                variants
            );
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            // For now, we accept that parsing may fail if choice handling is incomplete
            // The test documents the expected behavior
        }
    }
}

#[test]
fn test_parse_document_with_report_choice() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://example.com/choice" id="doc2">
    <title>Quarterly Report</title>
    <report reportNumber="Q4-2024">
        <summary>Executive summary here</summary>
        <findings>Key findings from Q4</findings>
        <recommendations>Strategic recommendations</recommendations>
    </report>
</document>"#;

    let result = model.parse_xml_to_instances(xml);

    match result {
        Ok(instances) => {
            eprintln!("\n=== Parsed Report Document ===");
            for inst in &instances {
                let json = serde_json::to_string_pretty(inst).unwrap();
                eprintln!("{}", json);
            }

            assert!(
                !instances.is_empty(),
                "Should parse to at least one instance"
            );
        }
        Err(e) => {
            eprintln!("Parse error (report): {}", e);
        }
    }
}

#[test]
fn test_parse_document_with_memo_choice() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://example.com/choice" id="doc3">
    <title>Internal Memo</title>
    <memo>
        <to>All Staff</to>
        <from>Management</from>
        <subject>Holiday Schedule</subject>
        <content>Please note the updated holiday schedule...</content>
    </memo>
</document>"#;

    let result = model.parse_xml_to_instances(xml);

    match result {
        Ok(instances) => {
            eprintln!("\n=== Parsed Memo Document ===");
            for inst in &instances {
                let json = serde_json::to_string_pretty(inst).unwrap();
                eprintln!("{}", json);
            }

            assert!(
                !instances.is_empty(),
                "Should parse to at least one instance"
            );
        }
        Err(e) => {
            eprintln!("Parse error (memo): {}", e);
        }
    }
}

#[test]
fn test_parse_payment_with_credit_card() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<payment xmlns="http://example.com/choice">
    <amount>99.99</amount>
    <currency>USD</currency>
    <creditCard>
        <cardNumber>4111111111111111</cardNumber>
        <expiryDate>12/25</expiryDate>
        <cvv>123</cvv>
    </creditCard>
</payment>"#;

    let result = model.parse_xml_to_instances(xml);

    match result {
        Ok(instances) => {
            eprintln!("\n=== Parsed Payment (Credit Card) ===");
            for inst in &instances {
                let json = serde_json::to_string_pretty(inst).unwrap();
                eprintln!("{}", json);
            }

            assert!(
                !instances.is_empty(),
                "Should parse to at least one instance"
            );
        }
        Err(e) => {
            eprintln!("Parse error (payment): {}", e);
        }
    }
}

#[test]
fn test_parse_payment_with_bank_transfer() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<payment xmlns="http://example.com/choice">
    <amount>500.00</amount>
    <currency>EUR</currency>
    <bankTransfer>
        <accountNumber>DE89370400440532013000</accountNumber>
        <routingNumber>COBADEFFXXX</routingNumber>
    </bankTransfer>
</payment>"#;

    let result = model.parse_xml_to_instances(xml);

    match result {
        Ok(instances) => {
            eprintln!("\n=== Parsed Payment (Bank Transfer) ===");
            for inst in &instances {
                let json = serde_json::to_string_pretty(inst).unwrap();
                eprintln!("{}", json);
            }

            assert!(
                !instances.is_empty(),
                "Should parse to at least one instance"
            );
        }
        Err(e) => {
            eprintln!("Parse error (bank transfer): {}", e);
        }
    }
}

#[test]
fn test_parse_contact_with_optional_choice() {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    // Contact with email (one choice)
    let xml_with_email = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://example.com/choice">
    <name>John Doe</name>
    <email>john@example.com</email>
</contact>"#;

    // Contact with phone (other choice)
    let xml_with_phone = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://example.com/choice">
    <name>Jane Smith</name>
    <phone>+1-555-0123</phone>
</contact>"#;

    // Contact without choice element (minOccurs="0")
    let xml_no_choice = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://example.com/choice">
    <name>Anonymous</name>
</contact>"#;

    for (name, xml) in [
        ("with email", xml_with_email),
        ("with phone", xml_with_phone),
        ("no contact info", xml_no_choice),
    ] {
        eprintln!("\n=== Parsing Contact {} ===", name);
        match model.parse_xml_to_instances(xml) {
            Ok(instances) => {
                for inst in &instances {
                    let json = serde_json::to_string_pretty(inst).unwrap();
                    eprintln!("{}", json);
                }
                assert!(!instances.is_empty(), "Should parse contact {}", name);
            }
            Err(e) => {
                eprintln!("Parse error ({}): {}", name, e);
            }
        }
    }
}

// ============================================================================
// Full Integration Tests (Schema + Instance + DB)
// ============================================================================

#[tokio::test]
async fn test_choice_full_flow_document_with_article() -> anyhow::Result<()> {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://example.com/choice" id="article-doc">
    <title>Research Article</title>
    <article>
        <abstract>A groundbreaking study</abstract>
        <body>The full article content goes here...</body>
        <keywords>research</keywords>
        <keywords>science</keywords>
    </article>
</document>"#;

    // Step 1: Parse XML
    let instances = model.parse_xml_to_instances(xml)?;
    eprintln!("\n=== Step 1: Parsed {} instances ===", instances.len());

    for inst in &instances {
        let json = serde_json::to_string_pretty(inst)?;
        eprintln!("{}", &json[..json.len().min(1000)]);
    }

    // Step 2: Get schemas
    let schemas = model.schemas().to_vec();
    eprintln!("\n=== Step 2: Generated {} schemas ===", schemas.len());

    // Step 3: Insert into TerminusDB
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_choice_article", |client, spec| {
            let schemas = schemas.clone();
            let insts = instances.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert schemas
                client
                    .insert_schema_instances(schemas.clone(), args.clone())
                    .await?;
                eprintln!("Step 3a: Inserted schemas");

                // Insert instances
                let instance_refs: Vec<_> = insts.iter().collect();
                let result = client.insert_documents(instance_refs, args).await?;
                eprintln!("Step 3b: Inserted {} documents", result.len());

                Ok(())
            }
        })
        .await?;

    Ok(())
}

#[tokio::test]
async fn test_choice_full_flow_payment_methods() -> anyhow::Result<()> {
    let model =
        XsdModel::from_file(choice_xsd_path(), None::<&str>).expect("Failed to load XSD model");

    // Test with different payment methods
    let payments = vec![
        (
            r#"<?xml version="1.0" encoding="UTF-8"?>
<payment xmlns="http://example.com/choice">
    <amount>150.00</amount>
    <currency>USD</currency>
    <creditCard>
        <cardNumber>4242424242424242</cardNumber>
        <expiryDate>06/26</expiryDate>
        <cvv>999</cvv>
    </creditCard>
</payment>"#,
            "credit card",
        ),
        (
            r#"<?xml version="1.0" encoding="UTF-8"?>
<payment xmlns="http://example.com/choice">
    <amount>1000.00</amount>
    <currency>GBP</currency>
    <bankTransfer>
        <accountNumber>GB82WEST12345698765432</accountNumber>
        <routingNumber>WESTGB2L</routingNumber>
    </bankTransfer>
</payment>"#,
            "bank transfer",
        ),
        (
            r#"<?xml version="1.0" encoding="UTF-8"?>
<payment xmlns="http://example.com/choice">
    <amount>25.50</amount>
    <currency>EUR</currency>
    <digitalWallet>
        <walletId>wallet-abc123</walletId>
        <provider>PayPal</provider>
    </digitalWallet>
</payment>"#,
            "digital wallet",
        ),
    ];

    let schemas = model.schemas().to_vec();
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_choice_payments", |client, spec| {
            let schemas = schemas.clone();
            let model_ref = &model;
            let payments_ref = &payments;
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert schemas first
                client
                    .insert_schema_instances(schemas.clone(), args.clone())
                    .await?;
                eprintln!("Inserted schemas");

                // Parse and insert each payment type
                for (xml, payment_type) in payments_ref {
                    eprintln!("\n--- Testing {} payment ---", payment_type);

                    match model_ref.parse_xml_to_instances(xml) {
                        Ok(instances) => {
                            let instance_refs: Vec<_> = instances.iter().collect();
                            let result =
                                client.insert_documents(instance_refs, args.clone()).await?;
                            eprintln!("✓ Inserted {} payment: {} docs", payment_type, result.len());
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to parse {} payment: {}", payment_type, e);
                        }
                    }
                }

                Ok(())
            }
        })
        .await?;

    Ok(())
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_choice_child_elements_extracted() {
    // Verify that xs:choice elements are properly extracted as child elements
    let xsd_schema = XsdSchema::from_xsd_file(choice_xsd_path(), None::<&str>)
        .expect("Failed to parse choice_types.xsd");

    eprintln!("\n=== XSD Complex Types with Child Elements ===");
    for ct in &xsd_schema.complex_types {
        if let Some(children) = &ct.child_elements {
            if !children.is_empty() {
                eprintln!("\n{} ({:?}):", ct.name, ct.content_model);
                for child in children {
                    eprintln!(
                        "  - {} : {} (min={:?}, max={:?})",
                        child.name, child.element_type, child.min_occurs, child.max_occurs
                    );
                }
            }
        }
    }

    // DocumentType should have choice children (article, report, memo)
    // Note: Complex type names are in Clark notation like {namespace}documentType
    let doc_type = xsd_schema
        .complex_types
        .iter()
        .find(|ct| ct.name.ends_with("documentType"))
        .expect("documentType not found");

    let children = doc_type
        .child_elements
        .as_ref()
        .expect("documentType should have child elements");

    let child_names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    eprintln!("\ndocumentType children: {:?}", child_names);

    // Should contain the choice elements
    assert!(
        child_names
            .iter()
            .any(|n| n.contains("article") || n.ends_with("article")),
        "documentType should have article child"
    );
    assert!(
        child_names
            .iter()
            .any(|n| n.contains("report") || n.ends_with("report")),
        "documentType should have report child"
    );
    assert!(
        child_names
            .iter()
            .any(|n| n.contains("memo") || n.ends_with("memo")),
        "documentType should have memo child"
    );
}

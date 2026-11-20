//! Tests for XSD to TerminusDB Schema conversion
//!
//! These tests verify that XSD schemas are correctly converted to TerminusDB Schema structures,
//! including proper handling of:
//! - Complex types → Schema::Class
//! - Attributes → Properties (required/optional)
//! - Child elements → Properties with correct cardinality
//! - XSD types → TerminusDB types (xsd:string, xsd:integer, etc.)

use terminusdb_schema::{Key, Property, Schema, TypeFamily};
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;

/// Helper to find a schema by class name
fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == name))
}

/// Helper to get properties from a Schema::Class
fn get_properties(schema: &Schema) -> Option<&Vec<Property>> {
    match schema {
        Schema::Class { properties, .. } => Some(properties),
        _ => None,
    }
}

/// Helper to find a property by name
fn find_property<'a>(props: &'a [Property], name: &str) -> Option<&'a Property> {
    props.iter().find(|p| p.name == name)
}

/// A child element's variant in the type's generated `{Type}Child` union.
///
/// Child elements are variants of that ordered union, not properties of the
/// class: the class carries one `children: List<{Type}Child>`. The variant keeps
/// the element's class; how often it may repeat is a property of the enclosing
/// List, so a variant has no type family — except for an `xs:list`-typed
/// element, whose value really is a list inside one element.
fn child_variant<'a>(schemas: &'a [Schema], type_name: &str, element: &str) -> Option<&'a Property> {
    let union_name = format!("{type_name}Child");
    schemas.iter().find_map(|s| match s {
        Schema::TaggedUnion { id, properties, .. } if *id == union_name => {
            properties.iter().find(|p| p.name == element)
        }
        _ => None,
    })
}

/// Assert that `type_name` carries the ordered children list.
fn assert_has_children_list(schemas: &[Schema], type_name: &str) {
    let class = find_class(schemas, type_name).expect("class not found");
    let props = get_properties(class).expect("no properties");
    let children = find_property(props, "children")
        .unwrap_or_else(|| panic!("{type_name} should carry a children property"));
    assert_eq!(children.r#type, Some(TypeFamily::List));
    assert_eq!(children.class, format!("{type_name}Child"));
}

// ============================================================================
// Simple Book Schema Tests
// ============================================================================

#[test]
fn test_simple_book_schema_generates_classes() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema =
        XsdSchema::from_xsd_file(xsd_path, None::<&str>).expect("Failed to parse simple_book.xsd");

    // Debug: print complex types
    eprintln!("\n=== XSD Complex Types ===");
    for ct in &xsd_schema.complex_types {
        eprintln!("  Type: {}", ct.name);
        if let Some(attrs) = &ct.attributes {
            for attr in attrs {
                eprintln!("    @{}: {} ({})", attr.name, attr.attr_type, attr.use_type);
            }
        }
        if let Some(children) = &ct.child_elements {
            for child in children {
                eprintln!("    <{}>: {}", child.name, child.element_type);
            }
        }
    }

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    // Debug: print generated schemas
    eprintln!("\n=== Generated Schemas ===");
    for schema in &schemas {
        if let Schema::Class { id, properties, .. } = schema {
            eprintln!("  Class: {}", id);
            for prop in properties {
                eprintln!("    {}: {} ({:?})", prop.name, prop.class, prop.r#type);
            }
        }
    }

    // Should generate BookType and PersonType classes
    assert!(
        schemas.len() >= 2,
        "Expected at least 2 schemas, got {}",
        schemas.len()
    );

    // Find BookType class
    let book_type = find_class(&schemas, "BookType");
    assert!(
        book_type.is_some(),
        "BookType class not found. Available: {:?}",
        schemas
            .iter()
            .filter_map(|s| match s {
                Schema::Class { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
    );

    // Find PersonType class
    let person_type = find_class(&schemas, "PersonType");
    assert!(person_type.is_some(), "PersonType class not found");
}

#[test]
fn test_book_type_has_correct_attributes() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let book_type = find_class(&schemas, "BookType").expect("BookType not found");
    let props = get_properties(book_type).expect("No properties");

    // Check isbn attribute (required)
    let isbn = find_property(props, "isbn").expect("isbn property not found");
    assert_eq!(isbn.class, "xsd:string");
    assert!(
        isbn.r#type.is_none(),
        "Required attribute should have no type family"
    );

    // Check edition attribute (optional)
    let edition = find_property(props, "edition").expect("edition property not found");
    assert_eq!(edition.class, "xsd:integer");
    assert_eq!(
        edition.r#type,
        Some(TypeFamily::Optional),
        "Optional attribute should have Optional type"
    );
}

#[test]
fn test_book_type_has_correct_child_elements() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    assert_has_children_list(&schemas, "BookType");

    // The elements keep their classes as variants; repetition (author is
    // unbounded) is carried by the enclosing children List.
    let title = child_variant(&schemas, "BookType", "title").expect("title variant not found");
    assert_eq!(title.class, "xsd:string");
    assert_eq!(title.r#type, None);

    let author = child_variant(&schemas, "BookType", "author").expect("author variant not found");
    assert_eq!(author.class, "PersonType");
    assert_eq!(author.r#type, None);

    let publisher =
        child_variant(&schemas, "BookType", "publisher").expect("publisher variant not found");
    assert_eq!(publisher.class, "xsd:string");
    assert_eq!(publisher.r#type, None);

    let year = child_variant(&schemas, "BookType", "year").expect("year variant not found");
    assert_eq!(year.class, "xsd:integer");
    assert_eq!(year.r#type, None);
}

#[test]
fn test_person_type_has_correct_structure() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    assert_has_children_list(&schemas, "PersonType");

    // firstName (required but treated as Optional - see schema_generator.rs:982-987)
    let first_name = child_variant(&schemas, "PersonType", "firstName").expect("firstName not found");
    assert_eq!(first_name.class, "xsd:string");
    assert_eq!(first_name.r#type, None);

    // lastName (required but treated as Optional)
    let last_name = child_variant(&schemas, "PersonType", "lastName").expect("lastName not found");
    assert_eq!(last_name.class, "xsd:string");
    assert_eq!(last_name.r#type, None);

    // email (optional)
    let email = child_variant(&schemas, "PersonType", "email").expect("email not found");
    assert_eq!(email.class, "xsd:string");
    assert_eq!(email.r#type, None);
}

// ============================================================================
// Catalog Schema Tests - More Complex XSD Features
// ============================================================================

#[test]
fn test_catalog_schema_generates_all_types() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema =
        XsdSchema::from_xsd_file(xsd_path, None::<&str>).expect("Failed to parse catalog.xsd");

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    // Should generate CatalogType, ProductType, CategoryType
    assert!(
        find_class(&schemas, "CatalogType").is_some(),
        "CatalogType not found"
    );
    assert!(
        find_class(&schemas, "ProductType").is_some(),
        "ProductType not found"
    );
    assert!(
        find_class(&schemas, "CategoryType").is_some(),
        "CategoryType not found"
    );
}

#[test]
fn test_product_type_various_xsd_types() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    assert_has_children_list(&schemas, "ProductType");

    // Check various XSD type mappings
    let price = child_variant(&schemas, "ProductType", "price").expect("price not found");
    assert_eq!(
        price.class, "xsd:decimal",
        "xs:decimal should map to xsd:decimal"
    );

    let quantity = child_variant(&schemas, "ProductType", "quantity").expect("quantity not found");
    assert_eq!(
        quantity.class, "xsd:integer",
        "xs:integer should map to xsd:integer"
    );

    let in_stock = child_variant(&schemas, "ProductType", "inStock").expect("inStock not found");
    assert_eq!(
        in_stock.class, "xsd:boolean",
        "xs:boolean should map to xsd:boolean"
    );

    // tags is an unbounded string element: a variant with no family of its own,
    // because the enclosing children List already admits any number of them.
    let tags = child_variant(&schemas, "ProductType", "tags").expect("tags not found");
    assert_eq!(tags.class, "xsd:string");
    assert_eq!(tags.r#type, None);
}

#[test]
fn test_category_type_self_reference() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    assert_has_children_list(&schemas, "CategoryType");

    // subcategory is a self-reference to CategoryType
    let subcategory = child_variant(&schemas, "CategoryType", "subcategory").expect("subcategory not found");
    assert_eq!(
        subcategory.class, "CategoryType",
        "Self-referencing type should use PascalCase class name"
    );
}

// ============================================================================
// Schema Class Structure Tests
// ============================================================================

#[test]
fn test_generated_schemas_use_correct_key_strategy() {
    // Key strategy:
    // - Documents (named types, non-subdocuments) use ValueHash for content-based addressing
    // - Subdocuments (anonymous types) use Random to avoid TerminusDB insertion failures
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    for schema in &schemas {
        if let Schema::Class {
            key,
            id,
            subdocument,
            ..
        } = schema
        {
            if *subdocument {
                assert_eq!(
                    *key,
                    Key::Random,
                    "Subdocument class {} should use Random key strategy",
                    id
                );
            } else {
                assert_eq!(
                    *key,
                    Key::ValueHash,
                    "Document class {} should use ValueHash key strategy",
                    id
                );
            }
        }
    }
}

#[test]
fn test_namespace_preserved_in_base_for_multi_namespace_support() {
    // Schema classes SHOULD set `base` to the XSD namespace because:
    // 1. This enables to_namespaced_json() to produce fully-qualified URIs
    // 2. Fully-qualified URIs allow multiple schemas with same class names to coexist
    //    (e.g., "http://example.com/book#DocumentType" vs "http://example.com/library#DocumentType")
    // 3. Instance @type uses fully-qualified URI to match the correct schema
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let book_type = find_class(&schemas, "BookType").expect("BookType not found");
    if let Schema::Class { base, .. } = book_type {
        assert!(
            base.is_some(),
            "Namespace should be preserved in base for multi-namespace support"
        );
        assert!(
            base.as_ref().unwrap().contains("example.com"),
            "Base should contain original namespace"
        );
    }
}

// ============================================================================
// XsdModel High-Level API Tests
// ============================================================================

#[test]
fn test_xsd_model_from_file() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load XsdModel");

    let schemas = model.schemas();
    assert!(!schemas.is_empty(), "XsdModel should generate schemas");

    let stats = model.stats();
    assert!(
        stats.total_complex_types >= 2,
        "Should have at least 2 complex types"
    );
}

#[test]
fn test_xsd_model_class_names() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load XsdModel");

    let class_names = model.class_names();
    assert!(
        class_names.contains(&"CatalogType"),
        "Should contain CatalogType"
    );
    assert!(
        class_names.contains(&"ProductType"),
        "Should contain ProductType"
    );
    assert!(
        class_names.contains(&"CategoryType"),
        "Should contain CategoryType"
    );
}

#[test]
fn test_xsd_model_find_schema() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load XsdModel");

    let book_schema = model.find_schema("BookType");
    assert!(book_schema.is_some(), "Should find BookType schema");

    let unknown = model.find_schema("UnknownType");
    assert!(unknown.is_none(), "Should not find unknown type");
}

// ============================================================================
// xs:list Type Tests - Verify TypeFamily::List is generated
// ============================================================================

#[test]
fn test_list_types_generate_type_family_list() {
    // Test that xs:list types correctly generate TypeFamily::List.
    //
    // xs:list is a simple type that represents a whitespace-separated list of values.
    // It's different from maxOccurs="unbounded" which represents multiple XML elements.
    //
    // Example XSD:
    //   <xs:simpleType name="IntegerList">
    //     <xs:list itemType="xs:integer"/>
    //   </xs:simpleType>
    //
    // XML instance: <scores>1 2 3 4 5</scores> (space-separated values)

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/list_types.xsd");
    let xsd_schema =
        XsdSchema::from_xsd_file(xsd_path, None::<&str>).expect("Failed to parse list_types.xsd");

    // Verify xs:list types have variety=List
    eprintln!("\n=== XSD Simple Types ===");
    for st in &xsd_schema.simple_types {
        eprintln!("  SimpleType: {} (variety: {:?})", st.name, st.variety);
        if st.name.contains("List") {
            assert_eq!(
                st.variety,
                Some(terminusdb_xsd::schema_model::SimpleTypeVariety::List),
                "xs:list types should have variety=List"
            );
        }
    }

    let mut generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    assert_has_children_list(&schemas, "DataContainer");

    // `scores` is an xs:list element, so unlike a merely repeating element its
    // variant keeps TypeFamily::List — the values live inside ONE element and
    // the enclosing children List cannot express that.
    let scores =
        child_variant(&schemas, "DataContainer", "scores").expect("scores variant not found");
    eprintln!(
        "scores variant: class={}, type={:?}",
        scores.class, scores.r#type
    );
    assert_eq!(
        scores.r#type,
        Some(TypeFamily::List),
        "an xs:list element should keep TypeFamily::List on its variant"
    );

    // `tags` is an optional xs:list element. Per-element optionality is not
    // represented (the children List carries multiplicity), so the variant
    // shows the same List family as a required one.
    let tags = child_variant(&schemas, "DataContainer", "tags").expect("tags variant not found");
    eprintln!("tags variant: class={}, type={:?}", tags.class, tags.r#type);
    assert_eq!(tags.r#type, Some(TypeFamily::List));
}

// ============================================================================
// Namespace Tests - Verify targetNamespace is used in Context
// ============================================================================

#[test]
fn test_context_uses_target_namespace() {
    // Verify that XsdModel extracts targetNamespace and sets it in Context.@schema
    //
    // simple_book.xsd has: targetNamespace="http://example.com/book"
    // This should become context.schema = "http://example.com/book#"
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load XsdModel");

    let context = model.context();

    // Context schema should be derived from targetNamespace
    assert!(
        context.schema.contains("example.com/book"),
        "Context @schema should contain the XSD targetNamespace. Got: {}",
        context.schema
    );

    // Should end with # or / for proper IRI formation
    assert!(
        context.schema.ends_with('#') || context.schema.ends_with('/'),
        "Context @schema should end with # or /. Got: {}",
        context.schema
    );

    // namespace() accessor should match context.schema
    assert_eq!(
        model.namespace(),
        &context.schema,
        "namespace() should return context.schema"
    );
}

#[test]
fn test_with_namespace_overrides_context() {
    // Verify that with_namespace() updates both namespace and context.schema
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_namespace("http://custom.org/schema#");

    let context = model.context();

    assert_eq!(
        context.schema, "http://custom.org/schema#",
        "with_namespace() should update context.schema"
    );

    assert_eq!(
        model.namespace(),
        "http://custom.org/schema#",
        "with_namespace() should update namespace()"
    );
}

#[test]
fn test_different_xsd_get_different_namespaces() {
    // Verify that different XSD files get their own namespaces
    // This is the core of the multi-schema solution
    use terminusdb_xsd::XsdModel;

    let book_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple_book.xsd"
    );
    let catalog_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");

    let book_model =
        XsdModel::from_file(book_path, None::<&str>).expect("Failed to load book model");
    let catalog_model =
        XsdModel::from_file(catalog_path, None::<&str>).expect("Failed to load catalog model");

    let book_ns = book_model.namespace();
    let catalog_ns = catalog_model.namespace();

    // They should have different namespaces (from their targetNamespace)
    eprintln!("Book namespace: {}", book_ns);
    eprintln!("Catalog namespace: {}", catalog_ns);

    // Book should have example.com/book namespace
    assert!(
        book_ns.contains("example.com/book"),
        "Book should have example.com/book namespace. Got: {}",
        book_ns
    );
}

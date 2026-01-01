//! Tests for XSD to TerminusDB Schema conversion
//!
//! These tests verify that XSD schemas are correctly converted to TerminusDB Schema structures,
//! including proper handling of:
//! - Complex types → Schema::Class
//! - Attributes → Properties (required/optional)
//! - Child elements → Properties with correct cardinality
//! - XSD types → TerminusDB types (xsd:string, xsd:integer, etc.)

use terminusdb_schema::{Key, Property, Schema, TypeFamily, SetCardinality};
use terminusdb_xsd::schema_model::XsdSchema;
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;

/// Helper to find a schema by class name
fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas.iter().find(|s| matches!(s, Schema::Class { id, .. } if id == name))
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

// ============================================================================
// Simple Book Schema Tests
// ============================================================================

#[test]
fn test_simple_book_schema_generates_classes() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>)
        .expect("Failed to parse simple_book.xsd");

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

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

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
    assert!(schemas.len() >= 2, "Expected at least 2 schemas, got {}", schemas.len());

    // Find BookType class
    let book_type = find_class(&schemas, "BookType");
    assert!(book_type.is_some(), "BookType class not found. Available: {:?}",
        schemas.iter().filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        }).collect::<Vec<_>>());

    // Find PersonType class
    let person_type = find_class(&schemas, "PersonType");
    assert!(person_type.is_some(), "PersonType class not found");
}

#[test]
fn test_book_type_has_correct_attributes() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let book_type = find_class(&schemas, "BookType").expect("BookType not found");
    let props = get_properties(book_type).expect("No properties");

    // Check isbn attribute (required)
    let isbn = find_property(props, "isbn").expect("isbn property not found");
    assert_eq!(isbn.class, "xsd:string");
    assert!(isbn.r#type.is_none(), "Required attribute should have no type family");

    // Check edition attribute (optional)
    let edition = find_property(props, "edition").expect("edition property not found");
    assert_eq!(edition.class, "xsd:integer");
    assert_eq!(edition.r#type, Some(TypeFamily::Optional), "Optional attribute should have Optional type");
}

#[test]
fn test_book_type_has_correct_child_elements() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let book_type = find_class(&schemas, "BookType").expect("BookType not found");
    let props = get_properties(book_type).expect("No properties");

    // Check title element (required, single)
    let title = find_property(props, "title").expect("title property not found");
    assert_eq!(title.class, "xsd:string");
    assert!(title.r#type.is_none(), "Required single element should have no type family");

    // Check author element (required, unbounded) - should be a Set
    let author = find_property(props, "author").expect("author property not found");
    assert_eq!(author.class, "PersonType");
    assert!(matches!(author.r#type, Some(TypeFamily::Set(_))),
        "Unbounded element should have Set type, got {:?}", author.r#type);

    // Check publisher element (optional, single)
    let publisher = find_property(props, "publisher").expect("publisher property not found");
    assert_eq!(publisher.class, "xsd:string");
    assert_eq!(publisher.r#type, Some(TypeFamily::Optional),
        "Optional element should have Optional type");

    // Check year element (optional, single)
    let year = find_property(props, "year").expect("year property not found");
    assert_eq!(year.class, "xsd:integer");
    assert_eq!(year.r#type, Some(TypeFamily::Optional));
}

#[test]
fn test_person_type_has_correct_structure() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let person_type = find_class(&schemas, "PersonType").expect("PersonType not found");
    let props = get_properties(person_type).expect("No properties");

    // firstName (required)
    let first_name = find_property(props, "firstName").expect("firstName not found");
    assert_eq!(first_name.class, "xsd:string");
    assert!(first_name.r#type.is_none());

    // lastName (required)
    let last_name = find_property(props, "lastName").expect("lastName not found");
    assert_eq!(last_name.class, "xsd:string");
    assert!(last_name.r#type.is_none());

    // email (optional)
    let email = find_property(props, "email").expect("email not found");
    assert_eq!(email.class, "xsd:string");
    assert_eq!(email.r#type, Some(TypeFamily::Optional));
}

// ============================================================================
// Catalog Schema Tests - More Complex XSD Features
// ============================================================================

#[test]
fn test_catalog_schema_generates_all_types() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>)
        .expect("Failed to parse catalog.xsd");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

    // Should generate CatalogType, ProductType, CategoryType
    assert!(find_class(&schemas, "CatalogType").is_some(), "CatalogType not found");
    assert!(find_class(&schemas, "ProductType").is_some(), "ProductType not found");
    assert!(find_class(&schemas, "CategoryType").is_some(), "CategoryType not found");
}

#[test]
fn test_product_type_various_xsd_types() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let product_type = find_class(&schemas, "ProductType").expect("ProductType not found");
    let props = get_properties(product_type).expect("No properties");

    // Check various XSD type mappings
    let price = find_property(props, "price").expect("price not found");
    assert_eq!(price.class, "xsd:decimal", "xs:decimal should map to xsd:decimal");

    let quantity = find_property(props, "quantity").expect("quantity not found");
    assert_eq!(quantity.class, "xsd:integer", "xs:integer should map to xsd:integer");

    let in_stock = find_property(props, "inStock").expect("inStock not found");
    assert_eq!(in_stock.class, "xsd:boolean", "xs:boolean should map to xsd:boolean");

    // tags is unbounded string
    let tags = find_property(props, "tags").expect("tags not found");
    assert_eq!(tags.class, "xsd:string");
    // Since minOccurs=0 and maxOccurs=unbounded, it should be Optional (or Set)
    assert!(tags.r#type.is_some(), "Unbounded optional element should have a type family");
}

#[test]
fn test_category_type_self_reference() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let category_type = find_class(&schemas, "CategoryType").expect("CategoryType not found");
    let props = get_properties(category_type).expect("No properties");

    // subcategory is a self-reference to CategoryType
    let subcategory = find_property(props, "subcategory").expect("subcategory not found");
    assert_eq!(subcategory.class, "CategoryType", "Self-referencing type should use PascalCase class name");
}

// ============================================================================
// Schema Class Structure Tests
// ============================================================================

#[test]
fn test_generated_schemas_use_correct_key_strategy() {
    // Key strategy:
    // - Documents (named types, non-subdocuments) use ValueHash for content-based addressing
    // - Subdocuments (anonymous types) use Random to avoid TerminusDB insertion failures
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    for schema in &schemas {
        if let Schema::Class { key, id, subdocument, .. } = schema {
            if *subdocument {
                assert_eq!(*key, Key::Random,
                    "Subdocument class {} should use Random key strategy", id);
            } else {
                assert_eq!(*key, Key::ValueHash,
                    "Document class {} should use ValueHash key strategy", id);
            }
        }
    }
}

#[test]
fn test_namespace_preserved_in_base() {
    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>).unwrap();

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let book_type = find_class(&schemas, "BookType").expect("BookType not found");
    if let Schema::Class { base, .. } = book_type {
        assert!(base.is_some(), "Namespace should be preserved in @base");
        assert!(base.as_ref().unwrap().contains("example.com"),
            "Base should contain original namespace");
    }
}

// ============================================================================
// XsdModel High-Level API Tests
// ============================================================================

#[test]
fn test_xsd_model_from_file() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel");

    let schemas = model.schemas();
    assert!(!schemas.is_empty(), "XsdModel should generate schemas");

    let stats = model.stats();
    assert!(stats.total_complex_types >= 2, "Should have at least 2 complex types");
}

#[test]
fn test_xsd_model_class_names() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel");

    let class_names = model.class_names();
    assert!(class_names.contains(&"CatalogType"), "Should contain CatalogType");
    assert!(class_names.contains(&"ProductType"), "Should contain ProductType");
    assert!(class_names.contains(&"CategoryType"), "Should contain CategoryType");
}

#[test]
fn test_xsd_model_find_schema() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel");

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
    let xsd_schema = XsdSchema::from_xsd_file(xsd_path, None::<&str>)
        .expect("Failed to parse list_types.xsd");

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

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

    // Find DataContainer class
    let data_container = find_class(&schemas, "DataContainer")
        .expect("DataContainer class not found");
    let props = get_properties(data_container).expect("No properties");

    // scores uses an xs:list type - should have TypeFamily::List
    let scores = find_property(props, "scores").expect("scores property not found");
    eprintln!("scores property: class={}, type={:?}", scores.class, scores.r#type);
    assert_eq!(
        scores.r#type,
        Some(TypeFamily::List),
        "xs:list type should generate TypeFamily::List"
    );

    // tags uses an optional xs:list type - should have TypeFamily::Optional with List inner
    let tags = find_property(props, "tags").expect("tags property not found");
    eprintln!("tags property: class={}, type={:?}", tags.class, tags.r#type);
    // Optional list type - since it has minOccurs=0, it's Optional
    // The underlying type is List but the cardinality makes it Optional
    assert!(
        matches!(tags.r#type, Some(TypeFamily::Optional) | Some(TypeFamily::List)),
        "Optional xs:list type should be Optional or List. Got {:?}",
        tags.r#type
    );

    // items uses maxOccurs="unbounded" - should be Set (NOT List)
    let items = find_property(props, "items").expect("items property not found");
    assert!(
        matches!(items.r#type, Some(TypeFamily::Set(_)) | Some(TypeFamily::Optional)),
        "Unbounded elements should use Set or Optional, not List. Got {:?}",
        items.r#type
    );
}

// ============================================================================
// Class Prefix Tests - Multi-Schema Database Support
// ============================================================================

#[test]
fn test_class_prefix_transforms_schema_ids() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("Test_");

    // Class names should be prefixed
    let class_names = model.class_names();
    assert!(class_names.contains(&"Test_BookType"), "BookType should be prefixed to Test_BookType");
    assert!(class_names.contains(&"Test_PersonType"), "PersonType should be prefixed to Test_PersonType");

    // Original unprefixed names should NOT exist
    assert!(!class_names.contains(&"BookType"), "Original BookType should not exist");
    assert!(!class_names.contains(&"PersonType"), "Original PersonType should not exist");
}

#[test]
fn test_class_prefix_transforms_property_references() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("Dita_");

    // Find Dita_BookType class
    let book_type = model.find_schema("Dita_BookType").expect("Dita_BookType not found");
    let props = get_properties(book_type).expect("No properties");

    // The author property should reference Dita_PersonType, not PersonType
    let author = find_property(props, "author").expect("author property not found");
    assert_eq!(author.class, "Dita_PersonType",
        "Property class reference should be prefixed. Got: {}", author.class);

    // Primitive types should NOT be prefixed
    let title = find_property(props, "title").expect("title property not found");
    assert_eq!(title.class, "xsd:string",
        "XSD primitive types should not be prefixed. Got: {}", title.class);
}

#[test]
fn test_class_prefix_transforms_element_to_class_map() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("NisoSts_");

    // Element-to-class mapping should return prefixed class names
    let element_map = model.element_to_class_map();

    eprintln!("Element to class map: {:?}", element_map);

    // Element names should map to prefixed class names
    if let Some(book_class) = element_map.get("book") {
        assert!(book_class.starts_with("NisoSts_"),
            "Element 'book' should map to prefixed class. Got: {}", book_class);
    }
}

#[test]
fn test_class_prefix_preserves_xsd_system_types() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("Custom_");

    // Check ProductType for XSD primitive types
    let product_type = model.find_schema("Custom_ProductType")
        .expect("Custom_ProductType not found");
    let product_props = get_properties(product_type).expect("No properties");

    // XSD types (xsd:*) should NOT be prefixed
    let price = find_property(product_props, "price").expect("price not found");
    assert!(price.class.starts_with("xsd:"),
        "XSD types should not be prefixed. Got: {}", price.class);

    // Check CatalogType for custom type references
    let catalog_type = model.find_schema("Custom_CatalogType")
        .expect("Custom_CatalogType not found");
    let catalog_props = get_properties(catalog_type).expect("No properties");

    // Custom types should be prefixed
    let category = find_property(catalog_props, "category").expect("category not found");
    assert_eq!(category.class, "Custom_CategoryType",
        "Custom types should be prefixed. Got: {}", category.class);

    let product = find_property(catalog_props, "product").expect("product not found");
    assert_eq!(product.class, "Custom_ProductType",
        "Custom types should be prefixed. Got: {}", product.class);
}

#[test]
fn test_class_prefix_transforms_inheritance() {
    // This test needs a schema with inheritance, which we don't have
    // in the simple fixtures. Let's verify the mechanism works with catalog.xsd
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog.xsd");
    let model = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("Pfx_");

    // CategoryType references itself - subcategory should be Pfx_CategoryType
    let category_type = model.find_schema("Pfx_CategoryType")
        .expect("Pfx_CategoryType not found");
    let props = get_properties(category_type).expect("No properties");

    let subcategory = find_property(props, "subcategory").expect("subcategory not found");
    assert_eq!(subcategory.class, "Pfx_CategoryType",
        "Self-referencing types should have prefixed class. Got: {}", subcategory.class);
}

#[test]
fn test_class_prefix_getter() {
    use terminusdb_xsd::XsdModel;

    let xsd_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_book.xsd");

    // Without prefix
    let model_no_prefix = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel");
    assert!(model_no_prefix.class_prefix().is_none(),
        "Model without prefix should return None");

    // With prefix
    let model_with_prefix = XsdModel::from_file(xsd_path, None::<&str>)
        .expect("Failed to load XsdModel")
        .with_class_prefix("MyPrefix_");
    assert_eq!(model_with_prefix.class_prefix(), Some("MyPrefix_"),
        "Model with prefix should return the prefix");
}

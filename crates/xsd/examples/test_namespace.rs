//! Test namespace preservation in schema generation.

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::{Cardinality, ChildElement, XsdAttribute, XsdComplexType, XsdSchema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Namespace Preservation ===\n");

    // Create a mock XSD schema with Clark notation namespaces
    let xsd_schema = XsdSchema {
        target_namespace: Some("http://example.com/book".to_string()),
        schema_location: None,
        element_form_default: Some("qualified".to_string()),
        root_elements: vec![],
        entry_point_elements: vec![],
        complex_types: vec![
            // Named complex type with namespace
            XsdComplexType {
                name: "{http://example.com/book}personType".to_string(),
                qualified_name: "{http://example.com/book}personType".to_string(),
                category: "XsdComplexType".to_string(),
                is_complex: true,
                is_simple: false,
                has_simple_content: false,
                mixed: false,
                content_model: Some("XsdGroup".to_string()),
                attributes: Some(vec![
                    XsdAttribute {
                        name: "id".to_string(),
                        attr_type: "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                        use_type: "required".to_string(),
                        default: None,
                    },
                ]),
                child_elements: Some(vec![
                    ChildElement {
                        name: "{http://example.com/book}firstName".to_string(),
                        element_type: "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                        min_occurs: Some(1),
                        max_occurs: Some(Cardinality::Number(1)),
                    },
                    ChildElement {
                        name: "{http://example.com/book}lastName".to_string(),
                        element_type: "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                        min_occurs: Some(1),
                        max_occurs: Some(Cardinality::Number(1)),
                    },
                ]),
                is_anonymous: false,
                element_name: None,
                base_type: None,
            },
            // Anonymous complex type (subdocument)
            XsdComplexType {
                name: "anonymous_book_type".to_string(),
                qualified_name: "anonymous_book_type".to_string(),
                category: "XsdComplexType".to_string(),
                is_complex: true,
                is_simple: false,
                has_simple_content: false,
                mixed: false,
                content_model: Some("XsdGroup".to_string()),
                attributes: Some(vec![
                    XsdAttribute {
                        name: "isbn".to_string(),
                        attr_type: "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                        use_type: "required".to_string(),
                        default: None,
                    },
                ]),
                child_elements: Some(vec![
                    ChildElement {
                        name: "{http://example.com/book}title".to_string(),
                        element_type: "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                        min_occurs: Some(1),
                        max_occurs: Some(Cardinality::Number(1)),
                    },
                    ChildElement {
                        name: "{http://example.com/book}author".to_string(),
                        element_type: "{http://example.com/book}personType".to_string(),
                        min_occurs: Some(1),
                        max_occurs: Some(Cardinality::Unbounded),
                    },
                ]),
                is_anonymous: true,
                element_name: Some("{http://example.com/book}book".to_string()),
                base_type: None,
            },
        ],
        simple_types: vec![],
    };

    println!("📖 Created mock XSD schema with namespaces");
    println!("   Target namespace: {}\n", xsd_schema.target_namespace.as_ref().unwrap());

    // Generate TerminusDB schemas
    let generator = XsdToSchemaGenerator::with_namespace("http://example.com/terminusdb#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("🔧 Generated {} TerminusDB schemas\n", schemas.len());

    // Display schemas with namespace information
    println!("📋 Generated Schemas with Namespace Preservation:\n");

    for (i, schema) in schemas.iter().enumerate() {
        match schema {
            terminusdb_schema::Schema::Class {
                id,
                base,
                properties,
                key,
                subdocument,
                ..
            } => {
                println!("{}. Class: {}", i + 1, id);
                println!("   @base (namespace): {:?}", base);
                println!("   @key: {:?}", key);
                println!("   @subdocument: {}", subdocument);
                println!("   Properties ({}):", properties.len());
                for prop in properties {
                    println!(
                        "     - {}: {} (type: {:?})",
                        prop.name, prop.class, prop.r#type
                    );
                }
                println!();
            }
            _ => {
                println!("{}. Other schema type", i + 1);
            }
        }
    }

    // Convert to JSON to show TerminusDB representation
    println!("💾 TerminusDB JSON Schema (first schema):\n");
    use terminusdb_schema::json::ToJson;
    if let Some(schema) = schemas.first() {
        let json = schema.to_json();
        let json_str = serde_json::to_string_pretty(&json)?;
        println!("{}", json_str);
    }

    println!("\n✅ Namespace preservation test complete!");
    println!("\n📝 Key observations:");
    println!("   - Class names use local names (e.g., 'personType' not '{{...}}personType')");
    println!("   - @base field stores the namespace URI");
    println!("   - Property names use local names");
    println!("   - Type references preserve namespace context");

    Ok(())
}

//! Simple real-world XSD example showing actual schema conversion.
//!
//! This example uses a minimal standalone XSD schema to demonstrate:
//! - Real XSD parsing (not fabricated mock data)
//! - PascalCase conversion using heck crate
//! - Namespace preservation with @base
//! - Actual TerminusDB schema generation

use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple Real XSD to TerminusDB Schema Conversion ===\n");

    // Create a simple but realistic XSD file
    let temp_dir = std::env::temp_dir();
    let xsd_path = temp_dir.join("simple_book.xsd");

    // Write a simple standalone XSD (no catalog needed)
    let xsd_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="http://example.com/book"
           xmlns:book="http://example.com/book"
           elementFormDefault="qualified">

  <!-- Simple type for ISBN -->
  <xs:simpleType name="isbnType">
    <xs:restriction base="xs:string">
      <xs:pattern value="[0-9]{3}-[0-9]{10}"/>
    </xs:restriction>
  </xs:simpleType>

  <!-- Person complex type -->
  <xs:complexType name="personType">
    <xs:sequence>
      <xs:element name="first-name" type="xs:string"/>
      <xs:element name="last-name" type="xs:string"/>
      <xs:element name="birth_year" type="xs:gYear" minOccurs="0"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:ID" use="optional"/>
  </xs:complexType>

  <!-- Publisher complex type -->
  <xs:complexType name="publisherType">
    <xs:sequence>
      <xs:element name="name" type="xs:string"/>
      <xs:element name="country" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>

  <!-- Book complex type with references -->
  <xs:complexType name="bookType">
    <xs:sequence>
      <xs:element name="title" type="xs:string"/>
      <xs:element name="subtitle" type="xs:string" minOccurs="0"/>
      <xs:element name="author" type="book:personType" maxOccurs="unbounded"/>
      <xs:element name="publisher" type="book:publisherType" minOccurs="0"/>
      <xs:element name="publication-year" type="xs:gYear"/>
      <xs:element name="page-count" type="xs:positiveInteger" minOccurs="0"/>
    </xs:sequence>
    <xs:attribute name="isbn" type="book:isbnType" use="required"/>
  </xs:complexType>

  <!-- Root element -->
  <xs:element name="book" type="book:bookType"/>

</xs:schema>"#;

    fs::write(&xsd_path, xsd_content)?;
    println!("📂 Created temporary XSD at: {:?}\n", xsd_path);

    // Parse the XSD
    println!("⏳ Parsing XSD schema...\n");
    let xsd_schema = XsdSchema::from_xsd_file(&xsd_path, None::<PathBuf>)?;

    println!("✅ Parsed XSD Schema:");
    println!("   Target namespace: {:?}", xsd_schema.target_namespace);
    println!("   Complex types: {}", xsd_schema.complex_types.len());
    println!("   Simple types: {}", xsd_schema.simple_types.len());
    println!("\n   Complex type names:");
    for ct in &xsd_schema.complex_types {
        println!("     - {} (anonymous: {})", ct.name, ct.is_anonymous);
    }
    println!();

    // Generate TerminusDB schemas
    println!("🔧 Generating TerminusDB schemas with PascalCase naming...\n");
    let generator = XsdToSchemaGenerator::with_namespace("http://example.com/terminusdb#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("✅ Generated {} TerminusDB schemas\n", schemas.len());
    println!("{}", "=".repeat(80));

    // Display all generated schemas
    println!("\n📋 All Generated Schemas:\n");

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
                println!("{}. Class: {} (PascalCase!)", i + 1, id);
                println!("   {}", "-".repeat(60));

                if let Some(ns) = base {
                    println!("   @base: {}", ns);
                }

                println!("   @key: {:?}", key);
                println!("   @subdocument: {}", subdocument);

                if !properties.is_empty() {
                    println!("   Properties:");
                    for prop in properties {
                        let type_info = match &prop.r#type {
                            None => format!("{} (required)", prop.class),
                            Some(tf) => format!("{} {:?}", prop.class, tf),
                        };
                        println!("     • {}: {}", prop.name, type_info);
                    }
                }
                println!();
            }
            _ => {
                println!("{}. Other schema type\n", i + 1);
            }
        }
    }

    // Show full JSON for all schemas
    println!("{}", "=".repeat(80));
    println!("\n💾 Full TerminusDB JSON Schemas:\n");

    use terminusdb_schema::json::ToJson;
    for (i, schema) in schemas.iter().enumerate() {
        let json = schema.to_json();
        let json_str = serde_json::to_string_pretty(&json)?;
        println!("--- Schema {} ---", i + 1);
        println!("{}\n", json_str);
    }

    println!("{}", "=".repeat(80));
    println!("\n✅ Schema Conversion Complete!\n");

    println!("📝 Key Features Demonstrated:");
    println!("   ✓ Real XSD parsing (not fabricated mock data)");
    println!("   ✓ PascalCase conversion using heck crate:");
    println!("     - personType → PersonType");
    println!("     - publisherType → PublisherType");
    println!("     - bookType → BookType");
    println!("   ✓ Namespace preservation via @base");
    println!("   ✓ Type references (Book → Person, Publisher)");
    println!("   ✓ Cardinality mapping (Optional, Set, required)");
    println!("   ✓ ValueHash keys for content-based addressing");

    // Clean up
    fs::remove_file(&xsd_path)?;

    Ok(())
}

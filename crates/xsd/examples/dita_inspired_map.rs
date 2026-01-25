//! DITA-inspired map schema example showing realistic document structure.
//!
//! Based on actual DITA map XSD patterns, this demonstrates:
//! - Hierarchical topic references (topicref)
//! - Relationship tables (reltable)
//! - Metadata structures
//! - Multiple namespaces and references

use std::fs;
use std::path::PathBuf;
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DITA-Inspired Map Schema ===\n");

    let temp_dir = std::env::temp_dir();
    let xsd_path = temp_dir.join("dita_map.xsd");

    // Create a DITA-inspired map XSD based on real DITA patterns
    let xsd_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="http://dita.oasis-open.org/architecture/2005/"
           xmlns:dita="http://dita.oasis-open.org/architecture/2005/"
           elementFormDefault="qualified">

  <!-- Enumeration types (like in real DITA) -->
  <xs:simpleType name="collection-type">
    <xs:restriction base="xs:string">
      <xs:enumeration value="choice"/>
      <xs:enumeration value="unordered"/>
      <xs:enumeration value="sequence"/>
      <xs:enumeration value="family"/>
    </xs:restriction>
  </xs:simpleType>

  <xs:simpleType name="linking-type">
    <xs:restriction base="xs:string">
      <xs:enumeration value="targetonly"/>
      <xs:enumeration value="sourceonly"/>
      <xs:enumeration value="normal"/>
      <xs:enumeration value="none"/>
    </xs:restriction>
  </xs:simpleType>

  <xs:simpleType name="scope-type">
    <xs:restriction base="xs:string">
      <xs:enumeration value="local"/>
      <xs:enumeration value="peer"/>
      <xs:enumeration value="external"/>
    </xs:restriction>
  </xs:simpleType>

  <!-- Metadata elements -->
  <xs:complexType name="navtitleType">
    <xs:simpleContent>
      <xs:extension base="xs:string">
        <xs:attribute name="id" type="xs:ID"/>
      </xs:extension>
    </xs:simpleContent>
  </xs:complexType>

  <xs:complexType name="topicmetaType">
    <xs:sequence>
      <xs:element name="navtitle" type="dita:navtitleType" minOccurs="0"/>
      <xs:element name="shortdesc" type="xs:string" minOccurs="0"/>
      <xs:element name="keywords" type="xs:string" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>

  <!-- Topic reference - core of DITA maps -->
  <xs:complexType name="topicrefType">
    <xs:sequence>
      <xs:element name="topicmeta" type="dita:topicmetaType" minOccurs="0"/>
      <xs:element name="topicref" type="dita:topicrefType" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:ID"/>
    <xs:attribute name="href" type="xs:anyURI"/>
    <xs:attribute name="keys" type="xs:string"/>
    <xs:attribute name="collection-type" type="dita:collection-type"/>
    <xs:attribute name="linking" type="dita:linking-type"/>
    <xs:attribute name="scope" type="dita:scope-type"/>
    <xs:attribute name="format" type="xs:string" default="dita"/>
    <xs:attribute name="toc" type="xs:boolean" default="true"/>
    <xs:attribute name="print" type="xs:boolean" default="true"/>
    <xs:attribute name="processing-role" type="xs:string" default="normal"/>
  </xs:complexType>

  <!-- Relationship table structures -->
  <xs:complexType name="relcellType">
    <xs:sequence>
      <xs:element name="topicref" type="dita:topicrefType" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="collection-type" type="dita:collection-type"/>
  </xs:complexType>

  <xs:complexType name="relrowType">
    <xs:sequence>
      <xs:element name="relcell" type="dita:relcellType" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="relheaderType">
    <xs:sequence>
      <xs:element name="relcell" type="dita:relcellType" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="reltableType">
    <xs:sequence>
      <xs:element name="title" type="xs:string" minOccurs="0"/>
      <xs:element name="relheader" type="dita:relheaderType" minOccurs="0"/>
      <xs:element name="relrow" type="dita:relrowType" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:ID"/>
    <xs:attribute name="collection-type" type="dita:collection-type"/>
  </xs:complexType>

  <!-- Main map structure -->
  <xs:complexType name="mapType">
    <xs:sequence>
      <xs:element name="title" type="xs:string" minOccurs="0"/>
      <xs:element name="topicmeta" type="dita:topicmetaType" minOccurs="0"/>
      <xs:element name="topicref" type="dita:topicrefType" minOccurs="0" maxOccurs="unbounded"/>
      <xs:element name="reltable" type="dita:reltableType" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:ID"/>
    <xs:attribute name="title" type="xs:string"/>
    <xs:attribute name="lang" type="xs:language"/>
  </xs:complexType>

  <!-- Root element -->
  <xs:element name="map" type="dita:mapType"/>

</xs:schema>"#;

    fs::write(&xsd_path, xsd_content)?;
    println!("📂 Created DITA-inspired map XSD\n");

    // Parse the XSD
    println!("⏳ Parsing schema...\n");
    let xsd_schema = XsdSchema::from_xsd_file(&xsd_path, None::<PathBuf>)?;

    println!("✅ Parsed Schema:");
    println!("   Target namespace: {:?}", xsd_schema.target_namespace);
    println!("   Complex types: {}", xsd_schema.complex_types.len());
    println!("   Simple types: {}\n", xsd_schema.simple_types.len());

    // Generate TerminusDB schemas
    println!("🔧 Generating TerminusDB schemas...\n");
    let generator = XsdToSchemaGenerator::with_namespace("http://dita.oasis-open.org/terminusdb#");
    let schemas = generator.generate(&xsd_schema)?;

    println!("✅ Generated {} TerminusDB schemas\n", schemas.len());
    println!("{}", "=".repeat(80));

    // Display all schemas
    println!("\n📋 All Generated DITA Map Schemas:\n");

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
                println!("   {}", "-".repeat(60));
                println!(
                    "   @base: {}",
                    base.as_ref().unwrap_or(&"(none)".to_string())
                );
                println!("   @key: {:?}", key);
                println!("   @subdocument: {}", subdocument);

                if !properties.is_empty() {
                    println!("   Properties ({}):", properties.len());
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
            _ => {}
        }
    }

    // Show JSON for key schemas
    println!("{}", "=".repeat(80));
    println!("\n💾 Key DITA Map Schemas as JSON:\n");

    use terminusdb_schema::json::ToJson;

    let key_schemas = ["MapType", "TopicrefType", "ReltableType"];

    for schema in schemas.iter() {
        if let terminusdb_schema::Schema::Class { id, .. } = schema {
            if key_schemas.contains(&id.as_str()) {
                let json = schema.to_json();
                let json_str = serde_json::to_string_pretty(&json)?;
                println!("--- {} ---", id);
                println!("{}\n", json_str);
            }
        }
    }

    println!("{}", "=".repeat(80));
    println!("\n✅ DITA Map Schema Generation Complete!\n");

    println!("📝 DITA Map Structure:");
    println!("   • MapType: Root container with title, metadata, topicrefs, reltables");
    println!("   • TopicrefType: Hierarchical topic references (self-referencing)");
    println!("   • TopicmetaType: Metadata for topics and maps");
    println!("   • ReltableType: Relationship tables for linking");
    println!("   • RelrowType, RelcellType: Table structure components");
    println!("   • Enumerations: collection-type, linking-type, scope-type");
    println!("\n   All with PascalCase naming via heck crate!");

    fs::remove_file(&xsd_path)?;

    Ok(())
}

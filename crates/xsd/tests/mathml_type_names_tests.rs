//! A type named like one of MathML's simple types, outside MathML, keeps its
//! own declared type.
//!
//! The generator carries a small table of MathML's simple types, because MathML
//! is embedded in JATS, BITS and DITA and those schemas cannot be changed. The
//! table was matched against the **local name alone**: the namespace is parsed
//! out of the Clark notation two lines above and then thrown away. So anything
//! named `number`, `length`, `color` or `character`, in any namespace or in no
//! namespace at all, was given MathML's mapping.
//!
//! What that cost, measured 2026-11-24 in Pubbin: an element `<number>` with an
//! inline complex type is given a synthesized type name of `number`, which hit
//! the table and became `xsd:integer`. The class the document then supplied —
//! `Number`, a subdocument with four attributes — was refused with
//! `ascribed_type_not_subsumed`, so **no chunk holding a `<number>` could ever
//! be stored**, and its room retried the save for ever. The element's own
//! `@value` was declared `xs:string` and was never read.
//!
//! Two claims here, and they are deliberately in both directions: a `number`
//! outside MathML keeps what its schema says, and a MathML one still maps.

use terminusdb_schema::{Property, Schema};
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;

const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";

/// A schema of our own with no target namespace, whose `<number>` is a complex
/// type of four attributes — the shape Pubbin's specification schema has.
const OURS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" elementFormDefault="unqualified">
  <xs:element name="evidence">
    <xs:complexType>
      <xs:sequence>
        <xs:element ref="number"/>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
  <xs:element name="number">
    <xs:complexType mixed="true">
      <xs:sequence/>
      <xs:attribute name="value" type="xs:string" use="required"/>
      <xs:attribute name="unit" type="xs:string"/>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

/// MathML's own `number`, as `mathml3-presentation.xsd` declares it: a
/// restriction of `xs:decimal`, used by `@scriptsizemultiplier`.
const MATHML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           xmlns:m="http://www.w3.org/1998/Math/MathML"
           targetNamespace="http://www.w3.org/1998/Math/MathML"
           elementFormDefault="qualified">
  <xs:simpleType name="number">
    <xs:restriction base="xs:decimal"/>
  </xs:simpleType>
  <xs:simpleType name="length">
    <xs:restriction base="xs:string"/>
  </xs:simpleType>
  <xs:element name="mstyle">
    <xs:complexType>
      <xs:attribute name="scriptsizemultiplier" type="m:number"/>
      <xs:attribute name="scriptminsize" type="m:length"/>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

fn generated(xsd: &str) -> (tempfile::TempDir, Vec<Schema>) {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let path = dir.path().join("schema.xsd");
    std::fs::write(&path, xsd).expect("the XSD is written");
    let parsed = XsdSchema::from_xsd_file(&path, None::<&str>).expect("the XSD parses");
    let schemas = XsdToSchemaGenerator::new()
        .generate(&parsed)
        .expect("the schemas generate");
    (dir, schemas)
}

/// Every property of every class and tagged union, by name, with its class.
fn properties(schemas: &[Schema]) -> Vec<(String, String, Property)> {
    schemas
        .iter()
        .flat_map(|schema| match schema {
            Schema::Class { id, properties, .. } | Schema::TaggedUnion { id, properties, .. } => {
                properties.iter().map(|p| (id.clone(), p.name.clone(), p.clone())).collect::<Vec<_>>()
            }
            _ => Vec::new(),
        })
        .collect()
}

fn class_of(schemas: &[Schema], holder_contains: &str, property: &str) -> Option<String> {
    properties(schemas)
        .into_iter()
        .find(|(holder, name, _)| holder.contains(holder_contains) && name == property)
        .map(|(_, _, p)| p.class)
}

#[test]
fn a_number_outside_mathml_keeps_what_its_own_schema_says() {
    let (_dir, schemas) = generated(OURS);
    // The element is a complex type of its own, so the property that holds it
    // is a link to that class — not a bare integer. Before the fix this was
    // `xsd:integer` and the class it names could never be stored in it.
    let held = class_of(&schemas, "Evidence", "number").expect("evidence holds a number");
    assert_eq!(
        held, "Number",
        "a `<number>` of our own is a class of its own, and naming it `number` is not a statement about MathML"
    );
    // And the class exists, with the attribute its schema declared.
    let number = schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == "Number"))
        .expect("the Number class is generated");
    let value = match number {
        Schema::Class { properties, .. } => properties.iter().find(|p| p.name == "value").cloned(),
        _ => None,
    }
    .expect("it has the @value its schema declares");
    assert_eq!(value.class, "xsd:string", "@value is `xs:string`, which is what the schema says and what was never read");
}

#[test]
fn mathmls_own_number_is_a_decimal_because_that_is_what_mathml_says() {
    let (_dir, schemas) = generated(MATHML);
    let multiplier = class_of(&schemas, "Mstyle", "scriptsizemultiplier").expect("mstyle has it");
    // Measured against `mathml3-presentation.xsd`: `<xs:simpleType name="number">
    // <xs:restriction base="xs:decimal"/></xs:simpleType>`. The table mapped it
    // to `xsd:integer`, which was wrong for MathML too — `@scriptsizemultiplier`
    // defaults to 0.71.
    assert_eq!(
        multiplier, "xsd:decimal",
        "MathML's `number` restricts `xs:decimal`, so an integer loses 0.71"
    );
    let minsize = class_of(&schemas, "Mstyle", "scriptminsize").expect("mstyle has it");
    assert_eq!(minsize, "xsd:string", "and MathML's `length` is still a string");
}

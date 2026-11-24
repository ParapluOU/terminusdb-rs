//! An **empty** simple-typed child element must reach the graph.
//!
//! A child element whose type is an XSD primitive (`xs:string`) or a
//! `simpleContent` extension of one does not become a TerminusDB class: it is
//! stored as a string beside its name in its parent's ordered children. An
//! empty one used to be left out altogether, which lost the element — a
//! document came back out of the graph with fewer elements than went into it.
//!
//! That is invisible in a schema whose leaves are complex types (DITA's and
//! JATS's are, so they never showed it) and fatal in one whose leaves are not:
//! a caller that counts the elements of what it stored against what it read
//! finds them disagreeing, with nothing to say which element went missing.

use std::path::PathBuf;
use terminusdb_schema::json::ToJson;
use terminusdb_xsd::XsdModel;

/// `title` is an `xs:string`; `step` is a `simpleContent` extension of one, so
/// that it can carry an attribute. Neither is a class.
const XSD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" elementFormDefault="qualified">
  <xs:element name="recipe">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="title" type="xs:string"/>
        <xs:element name="step" maxOccurs="unbounded">
          <xs:complexType>
            <xs:simpleContent>
              <xs:extension base="xs:string">
                <xs:attribute name="minutes" type="xs:string"/>
              </xs:extension>
            </xs:simpleContent>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

fn recipe_model() -> (tempfile::TempDir, XsdModel) {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let path: PathBuf = dir.path().join("recipe.xsd");
    std::fs::write(&path, XSD).expect("the XSD is written");
    let model = XsdModel::from_file(&path, None::<&str>).expect("the recipe schema loads");
    (dir, model)
}

/// The names the root's children are stored under, in order. A text run is
/// `text`; an element is its own name.
fn children_of(xml: &str) -> Vec<String> {
    let (_dir, model) = recipe_model();
    let instances = model
        .parse_xml_to_instances_ordered(xml)
        .expect("the recipe parses");
    let json = instances.first().expect("one instance").to_json();
    json["children"]
        .as_array()
        .expect("an ordered list of children")
        .iter()
        .map(|child| {
            child
                .as_object()
                .expect("a child is an object")
                .keys()
                .find(|key| !key.starts_with('@'))
                .expect("a child holds exactly one variant")
                .clone()
        })
        .collect()
}

#[test]
fn an_empty_simple_child_is_stored_among_full_ones() {
    assert_eq!(
        children_of("<recipe><title></title><step minutes=\"10\">Knead</step></recipe>"),
        vec!["title".to_string(), "step".to_string()],
        "the empty <title> is an element of the document and is stored",
    );
}

#[test]
fn a_document_of_nothing_but_empty_simple_children_keeps_them_all() {
    assert_eq!(
        children_of("<recipe><title></title><step></step></recipe>"),
        vec!["title".to_string(), "step".to_string()],
    );
    assert_eq!(
        children_of("<recipe><title/><step/></recipe>"),
        vec!["title".to_string(), "step".to_string()],
        "written as a self-closing tag it is the same document",
    );
}

#[test]
fn an_empty_simple_child_is_stored_as_the_empty_string() {
    let (_dir, model) = recipe_model();
    let instances = model
        .parse_xml_to_instances_ordered("<recipe><title></title><step>Knead</step></recipe>")
        .expect("the recipe parses");
    let json = instances.first().expect("one instance").to_json();
    assert_eq!(json["children"][0]["title"], "", "no text, rather than no element");
}

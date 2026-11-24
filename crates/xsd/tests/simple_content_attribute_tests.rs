//! An element that holds **text and attributes** is stored as a class, so that
//! it has somewhere to keep them.
//!
//! The rule, in one sentence: an element is stored as a class exactly when its
//! type declares attributes; otherwise it is stored as its text alone.
//!
//! A *named* `simpleContent` type with attributes has always been stored that
//! way — `_text` for the text, a property per attribute. An **anonymous** one
//! was flattened to its base simple type, so the element became a bare string
//! in its parent's ordered children and had nowhere to keep them:
//! `<step minutes="10">Mix.</step>` went in and `<step>Mix.</step>` came back,
//! on every save, saying nothing.
//!
//! What is *not* changed is as much of the claim as what is: an element whose
//! type is a plain `xs:string` can never carry an attribute and is still stored
//! as its text, and so is an anonymous `simpleContent` type that declares no
//! attributes — there is nothing else in it to lose. No schema that stored
//! correctly before stores differently now.

use terminusdb_schema::json::ToJson;
use terminusdb_xsd::XsdModel;

/// Four shapes of "an element that holds text", one per storage decision.
const XSD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" elementFormDefault="qualified">
  <xs:complexType name="stepType">
    <xs:simpleContent>
      <xs:extension base="xs:string">
        <xs:attribute name="minutes" type="xs:string"/>
      </xs:extension>
    </xs:simpleContent>
  </xs:complexType>
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
        <xs:element name="named" type="stepType"/>
        <xs:element name="bare">
          <xs:complexType>
            <xs:simpleContent>
              <xs:extension base="xs:string"/>
            </xs:simpleContent>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

fn recipe() -> (tempfile::TempDir, XsdModel) {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let path = dir.path().join("recipe.xsd");
    std::fs::write(&path, XSD).expect("the XSD is written");
    let model = XsdModel::from_file(&path, None::<&str>).expect("the recipe schema loads");
    (dir, model)
}

/// What each child of the root is stored as: the name it is held under, and the
/// class of the property — a primitive for an element stored as its text, a
/// class name for one stored as a class.
fn children_are_stored_as(model: &XsdModel) -> Vec<(String, String)> {
    model
        .schemas()
        .iter()
        .find_map(|schema| match schema {
            terminusdb_schema::Schema::TaggedUnion { id, properties, .. } if id == "RecipeChild" => Some(
                properties
                    .iter()
                    .map(|property| (property.name.clone(), property.class.clone()))
                    .collect(),
            ),
            _ => None,
        })
        .expect("the root's children are a tagged union")
}

#[test]
fn an_element_is_a_class_exactly_when_its_type_declares_attributes() {
    let (_dir, model) = recipe();
    assert_eq!(
        children_are_stored_as(&model),
        vec![
            // Plain `xs:string`: it can never carry an attribute.
            ("title".to_string(), "xsd:string".to_string()),
            // Anonymous `simpleContent` with an attribute: a class of its own,
            // named after the element. This is what used to be `xsd:string`.
            ("step".to_string(), "Step".to_string()),
            // The same by a named type, which was always a class.
            ("named".to_string(), "StepType".to_string()),
            // Anonymous `simpleContent` with no attributes: still its text.
            ("bare".to_string(), "xsd:string".to_string()),
        ],
    );
}

#[test]
fn the_class_holds_its_text_and_a_property_per_attribute() {
    let (_dir, model) = recipe();
    let step = model
        .schemas()
        .iter()
        .find_map(|schema| match schema {
            terminusdb_schema::Schema::Class { id, properties, .. } if id == "Step" => Some(
                properties
                    .iter()
                    .map(|property| (property.name.clone(), property.class.clone()))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .expect("the anonymous type of `step` has a class");
    assert!(step.contains(&("_text".to_string(), "xsd:string".to_string())), "{step:?}");
    assert!(step.contains(&("minutes".to_string(), "xsd:string".to_string())), "{step:?}");
}

#[test]
fn the_attribute_reaches_the_instance() {
    let (_dir, model) = recipe();
    let xml = "<recipe><title>Bread</title><step minutes=\"10\">Mix.</step><named minutes=\"60\">Wait.</named><bare>Rest.</bare></recipe>";
    let instances = model.parse_xml_to_instances_ordered(xml).expect("the recipe parses");
    let json = instances.first().expect("one instance").to_json();
    let children = json["children"].as_array().expect("ordered children");
    assert_eq!(children[1]["step"]["_text"], "Mix.");
    assert_eq!(children[1]["step"]["minutes"], "10", "the attribute is stored: {}", children[1]);
    // Unchanged, and as much the point: the other three are as they were.
    assert_eq!(children[0]["title"], "Bread");
    assert_eq!(children[2]["named"]["minutes"], "60");
    assert_eq!(children[3]["bare"], "Rest.");
}

#[test]
fn an_attribute_that_is_not_given_is_not_invented() {
    let (_dir, model) = recipe();
    let instances = model
        .parse_xml_to_instances_ordered("<recipe><title>Bread</title><step>Mix.</step></recipe>")
        .expect("the recipe parses");
    let json = instances.first().expect("one instance").to_json();
    let step = &json["children"][1]["step"];
    assert_eq!(step["_text"], "Mix.");
    assert!(step.get("minutes").is_none_or(|given| given.is_null()), "{step}");
}

//! XML to TerminusDB Instance parser
//!
//! This module provides functionality to parse XML documents into TerminusDB
//! instances using the generated schemas from XSD.

use std::collections::BTreeMap;
use terminusdb_schema::{Instance, InstanceProperty, PrimitiveValue, RelationValue, Schema};
use thiserror::Error;

// xmlschema-rs imports for XML parsing and conversion
use xmlschema::converters::{create_converter, ConverterType, ElementData};
use xmlschema::documents::{Document, Element};
use xmlschema::validators::XsdSchema as RustXsdSchema;

/// Errors that can occur during XML parsing to TerminusDB instances.
#[derive(Debug, Error)]
pub enum XmlParseError {
    /// No schema found for the given element type
    #[error("No schema found for element type '{element_type}'")]
    NoSchemaForElement { element_type: String },

    /// Property type mismatch
    #[error("Property '{property}' expected type '{expected}' but got '{actual}'")]
    PropertyTypeMismatch {
        property: String,
        expected: String,
        actual: String,
    },

    /// Required property missing
    #[error("Required property '{property}' missing in element '{element}'")]
    MissingRequiredProperty { property: String, element: String },

    /// Invalid XML structure
    #[error("Invalid XML structure: {message}")]
    InvalidStructure { message: String },

    /// Python/xmlschema error
    #[error("XML parsing error: {message}")]
    ParseError { message: String },

    /// Multiple errors occurred
    #[error("Multiple parsing errors occurred")]
    Multiple(Vec<XmlParseError>),
}

impl XmlParseError {
    /// Create a new error for missing schema
    pub fn no_schema(element_type: impl Into<String>) -> Self {
        Self::NoSchemaForElement {
            element_type: element_type.into(),
        }
    }

    /// Create a new parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create an invalid structure error
    pub fn invalid_structure(message: impl Into<String>) -> Self {
        Self::InvalidStructure {
            message: message.into(),
        }
    }
}

/// Result type for XML parsing operations
pub type ParseResult<T> = std::result::Result<T, XmlParseError>;

/// Parser for converting XML to TerminusDB instances
pub struct XmlToInstanceParser<'a> {
    /// Available schemas indexed by class name
    schemas: BTreeMap<String, &'a Schema>,
    /// Mapping from element names to TerminusDB class names
    element_to_class: std::collections::HashMap<String, String>,
}

impl<'a> XmlToInstanceParser<'a> {
    /// Create a new parser with the given schemas
    pub fn new(schemas: &'a [Schema]) -> Self {
        let mut schema_map = BTreeMap::new();
        for schema in schemas {
            if let Schema::Class { id, .. } = schema {
                schema_map.insert(id.clone(), schema);
            }
        }
        Self {
            schemas: schema_map,
            element_to_class: std::collections::HashMap::new(),
        }
    }

    /// Create a parser with element-to-class mapping from XSD schema.
    ///
    /// This allows the parser to correctly resolve XML element names to
    /// their corresponding TerminusDB class types using the XSD schema's
    /// element type declarations.
    pub fn with_element_mapping(
        schemas: &'a [Schema],
        element_to_class: std::collections::HashMap<String, String>,
    ) -> Self {
        let mut schema_map = BTreeMap::new();
        for schema in schemas {
            if let Schema::Class { id, .. } = schema {
                schema_map.insert(id.clone(), schema);
            }
        }
        Self {
            schemas: schema_map,
            element_to_class,
        }
    }

    /// Resolve an element name to its TerminusDB class name.
    ///
    /// Tries in order:
    /// 1. XSD element-to-type mapping (if available)
    /// 2. PascalCase conversion of the element name
    fn resolve_class_name(&self, element_name: &str) -> String {
        // Try XSD mapping first
        if let Some(class) = self.element_to_class.get(&element_name.to_lowercase()) {
            return class.clone();
        }
        // Fall back to PascalCase
        to_pascal_case(element_name)
    }

    /// Parse XML content into TerminusDB instances.
    ///
    /// This uses xmlschema-rs to parse and validate the XML, then converts
    /// the result to TerminusDB instances.
    ///
    /// # Arguments
    ///
    /// * `xml` - XML content as a string
    /// * `schema_path` - Path to the XSD schema file
    ///
    /// # Returns
    ///
    /// A vector of TerminusDB instances representing the parsed XML.
    pub fn parse_xml(&self, xml: &str, schema_path: &str) -> ParseResult<Vec<Instance>> {
        // Load the XSD schema from file
        let rust_schema = RustXsdSchema::from_file(std::path::Path::new(schema_path))
            .map_err(|e| XmlParseError::parse(format!("Failed to load XSD schema: {}", e)))?;

        // Validate the XML against the schema
        let validation_result = rust_schema.validate_string(xml);
        if !validation_result.valid {
            let errors = validation_result.errors.join("; ");
            return Err(XmlParseError::parse(format!(
                "XML validation failed: {}",
                errors
            )));
        }

        // Parse the XML document
        let doc = Document::from_string(xml)
            .map_err(|e| XmlParseError::parse(format!("Failed to parse XML: {}", e)))?;

        // Get the root element
        let root = doc
            .root
            .as_ref()
            .ok_or_else(|| XmlParseError::parse("XML document has no root element"))?;

        // Convert Element to ElementData
        let element_data = element_to_element_data(root);

        // Use the default XMLSchema converter
        let converter = create_converter(ConverterType::Default);

        // Convert to JSON
        let json_value = converter.decode(&element_data, 0);

        // Wrap in root element name for consistency
        let output_json = serde_json::json!({
            root.local_name(): json_value
        });

        // Convert JSON to instances
        self.json_to_instances(&output_json)
    }

    /// Parse JSON value (from xmlschema) to TerminusDB instances
    ///
    /// The JSON is expected to be in the format produced by `parse_xml_to_json`:
    /// ```json
    /// {"elementName": { ... }}
    /// ```
    ///
    /// The element name is converted to PascalCase to find the matching schema class.
    pub fn json_to_instances(
        &self,
        json: &serde_json::Value,
    ) -> ParseResult<Vec<Instance>> {
        let mut instances = Vec::new();
        let mut errors = Vec::new();

        match json {
            serde_json::Value::Object(obj) => {
                // Check if this is a wrapper object like {"topic": {...}}
                // where the key is the element name
                if obj.len() == 1 {
                    if let Some((element_name, inner_value)) = obj.iter().next() {
                        // Skip @-prefixed keys (metadata)
                        if !element_name.starts_with('@') {
                            // Resolve element name to class using XSD mapping
                            let type_hint = self.resolve_class_name(element_name);

                            if let serde_json::Value::Object(inner_obj) = inner_value {
                                match self.json_object_to_instance(inner_obj, Some(&type_hint)) {
                                    Ok(inst) => {
                                        instances.push(inst);
                                        return Ok(instances);
                                    }
                                    Err(e) => errors.push(e),
                                }
                            }
                        }
                    }
                }

                // Fall back to trying to parse the object directly
                match self.json_object_to_instance(obj, None) {
                    Ok(inst) => {
                        instances.push(inst);
                    }
                    Err(e) => errors.push(e),
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let serde_json::Value::Object(obj) = item {
                        match self.json_object_to_instance(obj, None) {
                            Ok(inst) => instances.push(inst),
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            _ => {
                return Err(XmlParseError::invalid_structure(
                    "Expected object or array at root level",
                ));
            }
        }

        if !errors.is_empty() && instances.is_empty() {
            return Err(XmlParseError::Multiple(errors));
        }

        Ok(instances)
    }

    /// Convert a JSON object to a TerminusDB instance
    fn json_object_to_instance(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
        type_hint: Option<&str>,
    ) -> ParseResult<Instance> {
        // Determine the type
        let type_name = obj
            .get("@type")
            .and_then(|v| v.as_str())
            .or(type_hint)
            .ok_or_else(|| {
                // Try to infer from the first key that looks like an element name
                let first_key = obj.keys().find(|k| !k.starts_with('@') && !k.starts_with('$'));
                XmlParseError::invalid_structure(format!(
                    "Cannot determine type for object. Keys: {:?}",
                    first_key
                ))
            })?;

        // Look up the schema
        let schema = self
            .schemas
            .get(type_name)
            .ok_or_else(|| XmlParseError::no_schema(type_name))?;

        // Get the schema properties
        let schema_props = match schema {
            Schema::Class { properties, .. } => properties,
            _ => return Err(XmlParseError::no_schema(type_name)),
        };

        // Convert properties
        let mut instance_props = BTreeMap::new();

        for (key, value) in obj {
            // Skip TerminusDB metadata keys (but keep XML attributes like @id, @class)
            // TerminusDB uses @type, @ref, $, etc. for its own metadata
            if key == "@type" || key == "@ref" || key.starts_with('$') {
                continue;
            }

            // Normalize key: strip @ prefix for XML attributes to match schema property names
            let property_name = if key.starts_with('@') && key != "@id" {
                // XML attributes like @class -> class
                key.trim_start_matches('@').to_string()
            } else if key == "@id" {
                // @id is special - it's both a TerminusDB ID and often an XML attribute
                // Include it as "id" property if schema has an id property
                "id".to_string()
            } else {
                key.clone()
            };

            // Find the matching schema property
            let _schema_prop = schema_props.iter().find(|p| p.name == property_name);

            // Convert the value to an InstanceProperty
            let instance_prop = self.json_value_to_property(value, &property_name)?;
            instance_props.insert(property_name, instance_prop);
        }

        // Don't set TerminusDB @id from XML id attribute
        // - TerminusDB uses ValueHash key generation for deduplication
        // - XML id attribute is stored as a property (lines 304-307 above)
        // - Setting @id would conflict with ValueHash-generated IDs

        Ok(Instance {
            schema: (*schema).clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: instance_props,
        })
    }

    /// Convert a JSON value to an InstanceProperty
    fn json_value_to_property(
        &self,
        value: &serde_json::Value,
        field_name: &str,
    ) -> ParseResult<InstanceProperty> {
        match value {
            serde_json::Value::Null => Ok(InstanceProperty::Primitive(PrimitiveValue::Null)),

            serde_json::Value::Bool(b) => {
                Ok(InstanceProperty::Primitive(PrimitiveValue::Bool(*b)))
            }

            serde_json::Value::Number(n) => {
                Ok(InstanceProperty::Primitive(PrimitiveValue::Number(n.clone())))
            }

            serde_json::Value::String(s) => {
                // Check if this field should be a complex type (has a schema)
                // If so, wrap the string in an Instance with _text property for text content
                let class_name = self.resolve_class_name(field_name);
                if let Some(schema) = self.schemas.get(&class_name) {
                    // This is a complex type - wrap the text in an Instance
                    let mut properties = BTreeMap::new();
                    properties.insert(
                        "_text".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String(s.clone())),
                    );
                    let inst = Instance {
                        schema: (*schema).clone(),
                        id: None,
                        capture: false,
                        ref_props: false,
                        properties,
                    };
                    Ok(InstanceProperty::Relation(RelationValue::One(inst)))
                } else {
                    // No schema found - treat as primitive string
                    Ok(InstanceProperty::Primitive(PrimitiveValue::String(s.clone())))
                }
            }

            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(InstanceProperty::Primitives(vec![]));
                }

                // Check if array contains objects (relations) or primitives
                let first = &arr[0];
                if first.is_object() {
                    // Array of nested instances
                    let mut relations = Vec::new();
                    for item in arr {
                        if let serde_json::Value::Object(obj) = item {
                            let inst = self.json_object_to_instance(obj, None)?;
                            relations.push(RelationValue::One(inst));
                        }
                    }
                    Ok(InstanceProperty::Relations(relations))
                } else {
                    // Array of primitives
                    let mut primitives = Vec::new();
                    for item in arr {
                        let prim = match item {
                            serde_json::Value::Null => PrimitiveValue::Null,
                            serde_json::Value::Bool(b) => PrimitiveValue::Bool(*b),
                            serde_json::Value::Number(n) => PrimitiveValue::Number(n.clone()),
                            serde_json::Value::String(s) => PrimitiveValue::String(s.clone()),
                            _ => PrimitiveValue::Object(item.clone()),
                        };
                        primitives.push(prim);
                    }
                    Ok(InstanceProperty::Primitives(primitives))
                }
            }

            serde_json::Value::Object(obj) => {
                // Check for @ref (external reference)
                if let Some(ref_id) = obj.get("@ref").and_then(|v| v.as_str()) {
                    return Ok(InstanceProperty::Relation(
                        RelationValue::ExternalReference(ref_id.to_string()),
                    ));
                }

                // Try to parse as a nested instance
                // Resolve field name to class using XSD mapping
                let type_hint = self.resolve_class_name(field_name);
                match self.json_object_to_instance(obj, Some(&type_hint)) {
                    Ok(inst) => Ok(InstanceProperty::Relation(RelationValue::One(inst))),
                    Err(_) => {
                        // Fall back to treating it as a generic object
                        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(
                            serde_json::Value::Object(obj.clone()),
                        )))
                    }
                }
            }
        }
    }
}

/// Convert a string to PascalCase
fn to_pascal_case(s: &str) -> String {
    use heck::ToPascalCase;
    s.to_pascal_case()
}

/// Convert a Document Element to an ElementData for the converter.
///
/// This recursively converts the element tree to the format expected by
/// the xmlschema-rs converters.
fn element_to_element_data(elem: &Element) -> ElementData {
    let mut data = ElementData::new(elem.local_name());

    // Add text content
    if let Some(text) = &elem.text {
        data = data.with_text(text.clone());
    }

    // Add attributes
    for (qname, value) in &elem.attributes {
        data = data.with_attribute(qname.local_name.clone(), value.clone());
    }

    // Add xmlns declarations
    for (prefix, uri) in elem.namespaces.iter() {
        data = data.with_xmlns(prefix, uri);
    }

    // Add child elements recursively
    for child in &elem.children {
        let child_data = element_to_element_data(child);
        let child_json = create_converter(ConverterType::Default).decode(&child_data, 1);
        data = data.with_child(child.local_name(), child_json);
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::{Key, Property, TypeFamily};

    fn create_test_schema() -> Vec<Schema> {
        vec![
            Schema::Class {
                id: "Person".to_string(),
                base: None,
                key: Key::ValueHash,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: false,
                properties: vec![
                    Property {
                        name: "name".to_string(),
                        r#type: None,
                        class: "xsd:string".to_string(),
                    },
                    Property {
                        name: "age".to_string(),
                        r#type: Some(TypeFamily::Optional),
                        class: "xsd:integer".to_string(),
                    },
                ],
            },
        ]
    }

    #[test]
    fn test_simple_json_to_instance() {
        let schemas = create_test_schema();
        let parser = XmlToInstanceParser::new(&schemas);

        let json: serde_json::Value = serde_json::json!({
            "@type": "Person",
            "name": "Alice",
            "age": 30
        });

        let instances = parser.json_to_instances(&json).unwrap();
        assert_eq!(instances.len(), 1);

        let person = &instances[0];
        assert_eq!(person.schema.class_name(), "Person");

        let name = person.get_property("name").unwrap();
        assert!(matches!(
            name,
            InstanceProperty::Primitive(PrimitiveValue::String(s)) if s == "Alice"
        ));
    }

    #[test]
    fn test_parser_no_schema() {
        let schemas = create_test_schema();
        let parser = XmlToInstanceParser::new(&schemas);

        let json: serde_json::Value = serde_json::json!({
            "@type": "Unknown",
            "field": "value"
        });

        let result = parser.json_to_instances(&json);
        assert!(result.is_err());
    }
}

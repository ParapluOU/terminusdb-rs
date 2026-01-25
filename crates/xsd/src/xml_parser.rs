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
            match schema {
                Schema::Class { id, .. } => {
                    schema_map.insert(id.clone(), schema);
                }
                Schema::TaggedUnion { id, .. } => {
                    schema_map.insert(id.clone(), schema);
                }
                _ => {}
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
            match schema {
                Schema::Class { id, .. } => {
                    schema_map.insert(id.clone(), schema);
                }
                Schema::TaggedUnion { id, .. } => {
                    schema_map.insert(id.clone(), schema);
                }
                _ => {}
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
    pub fn json_to_instances(&self, json: &serde_json::Value) -> ParseResult<Vec<Instance>> {
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
                let first_key = obj
                    .keys()
                    .find(|k| !k.starts_with('@') && !k.starts_with('$'));
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

        // Check if this is a mixed content type (has a `content` property pointing to MixedContent*)
        // First check own properties, then check inherited classes
        let mixed_content_prop = schema_props
            .iter()
            .find(|p| p.name == "content" && p.class.starts_with("MixedContent"));

        let inherited_mixed_content = if mixed_content_prop.is_none() {
            // Check inherited classes for mixed content
            self.find_inherited_mixed_content(schema)
        } else {
            None
        };

        if let Some(content_prop) = mixed_content_prop {
            // This is a mixed content type - build MixedContent instance
            return self.build_mixed_content_instance(obj, schema, &content_prop.class);
        } else if let Some((_base_schema, content_class)) = inherited_mixed_content {
            // This type inherits from a mixed content base - use original schema but base's MixedContent class
            return self.build_mixed_content_instance(obj, schema, &content_class);
        }

        // Convert properties (non-mixed content path)
        let mut instance_props = BTreeMap::new();

        for (key, value) in obj {
            // Skip TerminusDB metadata keys (but keep XML attributes like @id, @class)
            // TerminusDB uses @type, @ref, $, etc. for its own metadata
            if key == "@type" || key == "@ref" || key.starts_with('$') {
                continue;
            }

            // Skip XML namespace declarations - these are not data properties
            // Example: xmlns:ali, xmlns:mml, xmlns:xlink, xmlns:xsi, xmlns
            if key.starts_with("xmlns:") || key == "xmlns" {
                continue;
            }

            // Skip dtd-version and similar DTD-related attributes
            // Also skip xml:lang which becomes "lang" after prefix stripping
            if key == "dtd-version"
                || key == "@dtd-version"
                || key == "lang"
                || key == "@lang"
                || key == "xml:lang"
            {
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

    /// Build a MixedContent instance for mixed content types.
    ///
    /// Mixed content (text interleaved with elements) is represented as:
    /// - `text`: String with `{}` placeholders marking substitution positions
    /// - `subs`: List of inline element instances in order
    ///
    /// For example, `<p>The <term>first</term> and <term>second</term> are important.</p>`
    /// becomes:
    /// ```json
    /// {
    ///   "content": {
    ///     "@type": "MixedContentPClass",
    ///     "text": "The {} and {} are important.",
    ///     "subs": [{"@type": "TermClass", "_text": "first"}, {"@type": "TermClass", "_text": "second"}]
    ///   }
    /// }
    /// ```
    fn build_mixed_content_instance(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
        parent_schema: &Schema,
        mixed_content_class: &str,
    ) -> ParseResult<Instance> {
        // Get the parent schema properties for attributes
        let parent_props = match parent_schema {
            Schema::Class { properties, .. } => properties,
            _ => return Err(XmlParseError::no_schema(parent_schema.class_name())),
        };

        // Look up the MixedContent schema
        let mixed_content_schema = self
            .schemas
            .get(mixed_content_class)
            .ok_or_else(|| XmlParseError::no_schema(mixed_content_class))?;

        // Get the inline union type from MixedContent's `subs` property
        let inline_union_name = match mixed_content_schema {
            Schema::Class { properties, .. } => properties
                .iter()
                .find(|p| p.name == "subs")
                .map(|p| p.class.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };

        // Debug: print the inline union name
        tracing::debug!("Mixed content inline union: {}", inline_union_name);

        // Build the parent instance properties (attributes only)
        let mut instance_props = BTreeMap::new();

        // Collect text content and child elements
        let mut text_content = obj
            .get("$")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut subs: Vec<RelationValue> = Vec::new();

        for (key, value) in obj {
            if key == "@type" || key == "@ref" {
                continue;
            }

            if key == "$" {
                // Text content - already captured above
                continue;
            }

            // Skip XML namespace declarations - these are not data properties
            if key.starts_with("xmlns:") || key == "xmlns" {
                continue;
            }

            // Skip dtd-version and similar DTD-related attributes
            if key == "dtd-version" || key == "xml:lang" {
                continue;
            }

            // Check if this is an attribute (@ prefix) or a child element
            if key.starts_with('@') {
                // Attribute - add to parent instance
                let property_name = if key == "@id" {
                    "id".to_string()
                } else {
                    key.trim_start_matches('@').to_string()
                };

                // Only add if it's a known property on the parent (not `content`)
                if parent_props
                    .iter()
                    .any(|p| p.name == property_name && property_name != "content")
                {
                    let instance_prop = self.json_value_to_property(value, &property_name)?;
                    instance_props.insert(property_name, instance_prop);
                }
            } else {
                // Child element - add to subs
                // Resolve element name to class for the TaggedUnion variant
                let element_class = self.resolve_class_name(key);

                match value {
                    serde_json::Value::Array(arr) => {
                        // Multiple child elements with same name
                        for item in arr {
                            if let Some(inst) =
                                self.build_inline_element(&element_class, item, &inline_union_name)?
                            {
                                subs.push(RelationValue::One(inst));
                                // Add placeholder to text if we have text
                                if !text_content.is_empty() {
                                    text_content = format!("{} {{}}", text_content);
                                }
                            }
                        }
                    }
                    _ => {
                        if let Some(inst) =
                            self.build_inline_element(&element_class, value, &inline_union_name)?
                        {
                            subs.push(RelationValue::One(inst));
                        }
                    }
                }
            }
        }

        // Build the MixedContent instance
        let mut mixed_props = BTreeMap::new();

        // Add text property (with {} placeholders - simplified for now)
        // Note: We can't perfectly reconstruct placeholder positions without changes to xmlschema-rs
        // For now, use the raw text content
        mixed_props.insert(
            "text".to_string(),
            InstanceProperty::Primitive(PrimitiveValue::String(text_content)),
        );

        // Add subs property (List of inline elements) - always required, even when empty
        mixed_props.insert("subs".to_string(), InstanceProperty::Relations(subs));

        let mixed_content_inst = Instance {
            schema: (*mixed_content_schema).clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: mixed_props,
        };

        // Add MixedContent to parent instance
        instance_props.insert(
            "content".to_string(),
            InstanceProperty::Relation(RelationValue::One(mixed_content_inst)),
        );

        Ok(Instance {
            schema: (*parent_schema).clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: instance_props,
        })
    }

    /// Build an inline element instance for mixed content subs.
    ///
    /// This wraps child elements in the appropriate type for the inline TaggedUnion.
    /// The returned instance is a TaggedUnion instance with the variant name as the property.
    fn build_inline_element(
        &self,
        element_class: &str,
        value: &serde_json::Value,
        inline_union_name: &str,
    ) -> ParseResult<Option<Instance>> {
        // Look up the element's schema
        let schema = match self.schemas.get(element_class) {
            Some(s) => s,
            None => {
                // Try with "Class" suffix (e.g., "Term" -> "TermClass")
                let class_name = format!("{}Class", element_class);
                match self.schemas.get(&class_name) {
                    Some(s) => s,
                    None => return Ok(None), // Unknown element type, skip
                }
            }
        };

        // Build the variant instance based on the value type
        let variant_instance = match value {
            serde_json::Value::String(s) => {
                // Check if this schema (or its base) has mixed content
                // For example, <institution>Mapbox</institution> should use MixedContentInstitution
                if let Some(mixed_content_class) = self.find_mixed_content_class(schema) {
                    // This inline element has its own mixed content structure
                    self.build_simple_mixed_content_instance(schema, &mixed_content_class, s)?
                } else {
                    // Simple text content without mixed content structure
                    let mut properties = BTreeMap::new();
                    properties.insert(
                        "_text".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String(s.clone())),
                    );
                    Instance {
                        schema: (*schema).clone(),
                        id: None,
                        capture: false,
                        ref_props: false,
                        properties,
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                // Complex inline element
                self.json_object_to_instance(obj, Some(schema.class_name()))?
            }
            _ => return Ok(None),
        };

        // Wrap the variant instance in a TaggedUnion instance
        // The TaggedUnion expects { "@type": "AddrLineInline", "institution": { ... } }
        tracing::debug!(
            "build_inline_element: element_class={}, inline_union_name={}",
            element_class,
            inline_union_name
        );
        let union_schema = match self.schemas.get(inline_union_name) {
            Some(s) => {
                tracing::debug!("Found union schema: {}", s.class_name());
                s
            }
            None => {
                tracing::debug!("Union schema NOT found, returning variant directly");
                // Fallback: return the variant directly if union schema not found
                return Ok(Some(variant_instance));
            }
        };

        // Find the variant name from the TaggedUnion schema properties
        // The variant name corresponds to the element class (e.g., "institution" for Institution)
        let variant_name = match union_schema {
            Schema::TaggedUnion { properties, .. } => {
                // Find the property whose class matches element_class
                properties
                    .iter()
                    .find(|p| p.class == element_class)
                    .map(|p| p.name.clone())
            }
            _ => None,
        };

        let variant_name = match variant_name {
            Some(name) => name,
            None => {
                // Fallback: convert class name to kebab-case for variant name
                // e.g., "Institution" -> "institution"
                element_class
                    .chars()
                    .enumerate()
                    .flat_map(|(i, c)| {
                        if c.is_uppercase() && i > 0 {
                            vec!['-', c.to_ascii_lowercase()]
                        } else {
                            vec![c.to_ascii_lowercase()]
                        }
                    })
                    .collect()
            }
        };

        // Build the TaggedUnion instance
        let mut union_props = BTreeMap::new();
        union_props.insert(
            variant_name,
            InstanceProperty::Relation(RelationValue::One(variant_instance)),
        );

        Ok(Some(Instance {
            schema: (*union_schema).clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: union_props,
        }))
    }

    /// Find mixed content property in inherited classes.
    ///
    /// Returns the base schema and mixed content class name if found.
    fn find_inherited_mixed_content<'b>(
        &'b self,
        schema: &'b Schema,
    ) -> Option<(&'b Schema, String)> {
        let inherits = match schema {
            Schema::Class { inherits, .. } => inherits,
            _ => return None,
        };

        for base_name in inherits {
            if let Some(base_schema) = self.schemas.get(base_name) {
                if let Schema::Class { properties, .. } = base_schema {
                    // Check if base has mixed content
                    if let Some(content_prop) = properties
                        .iter()
                        .find(|p| p.name == "content" && p.class.starts_with("MixedContent"))
                    {
                        return Some((base_schema, content_prop.class.clone()));
                    }

                    // Recursively check base's parents
                    if let Some(result) = self.find_inherited_mixed_content(base_schema) {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Find the MixedContent class for a schema (checking own properties and inherited classes).
    ///
    /// Returns the MixedContent class name if found.
    fn find_mixed_content_class(&self, schema: &Schema) -> Option<String> {
        // First check own properties
        if let Schema::Class { properties, .. } = schema {
            if let Some(content_prop) = properties
                .iter()
                .find(|p| p.name == "content" && p.class.starts_with("MixedContent"))
            {
                return Some(content_prop.class.clone());
            }
        }

        // Check inherited classes
        if let Some((_, content_class)) = self.find_inherited_mixed_content(schema) {
            return Some(content_class);
        }

        None
    }

    /// Build an instance with MixedContent for simple text content.
    ///
    /// This is used when an XML element contains only text (no child elements).
    fn build_simple_mixed_content_instance(
        &self,
        parent_schema: &Schema,
        mixed_content_class: &str,
        text: &str,
    ) -> ParseResult<Instance> {
        // Look up the MixedContent schema
        let mixed_content_schema = self
            .schemas
            .get(mixed_content_class)
            .ok_or_else(|| XmlParseError::no_schema(mixed_content_class))?;

        // Build the MixedContent instance
        let mut mixed_props = BTreeMap::new();

        // Add text property (no placeholders since there are no inline elements)
        mixed_props.insert(
            "text".to_string(),
            InstanceProperty::Primitive(PrimitiveValue::String(text.to_string())),
        );

        // Add empty subs list (required by schema, even when empty)
        mixed_props.insert("subs".to_string(), InstanceProperty::Relations(vec![]));

        let mixed_content_inst = Instance {
            schema: (*mixed_content_schema).clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: mixed_props,
        };

        // Build the parent instance with the content property
        let mut instance_props = BTreeMap::new();
        instance_props.insert(
            "content".to_string(),
            InstanceProperty::Relation(RelationValue::One(mixed_content_inst)),
        );

        Ok(Instance {
            schema: (*parent_schema).clone(),
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

            serde_json::Value::Bool(b) => Ok(InstanceProperty::Primitive(PrimitiveValue::Bool(*b))),

            serde_json::Value::Number(n) => Ok(InstanceProperty::Primitive(
                PrimitiveValue::Number(n.clone()),
            )),

            serde_json::Value::String(s) => {
                // Check if this field should be a complex type (has a schema)
                let class_name = self.resolve_class_name(field_name);
                if let Some(schema) = self.schemas.get(&class_name) {
                    // Check if this schema (or its base) has mixed content
                    let mixed_content_class = self.find_mixed_content_class(schema);

                    if let Some(mixed_class) = mixed_content_class {
                        // This is a mixed content type - build MixedContent instance with the text
                        let inst =
                            self.build_simple_mixed_content_instance(schema, &mixed_class, s)?;
                        Ok(InstanceProperty::Relation(RelationValue::One(inst)))
                    } else {
                        // Regular complex type - wrap the text in an Instance with _text property
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
                    }
                } else {
                    // No schema found - treat as primitive string
                    Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                        s.clone(),
                    )))
                }
            }

            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(InstanceProperty::Primitives(vec![]));
                }

                // Check if array contains objects (relations) or primitives
                let first = &arr[0];

                // Resolve the element type for this field
                let class_name = self.resolve_class_name(field_name);
                let element_schema = self.schemas.get(&class_name);

                if first.is_object() {
                    // Array of nested instances
                    // e.g., "sec" array items should be type "Sec"
                    let mut relations = Vec::new();
                    for item in arr {
                        if let serde_json::Value::Object(obj) = item {
                            let inst = self.json_object_to_instance(obj, Some(&class_name))?;
                            relations.push(RelationValue::One(inst));
                        }
                    }
                    Ok(InstanceProperty::Relations(relations))
                } else if first.is_string() && element_schema.is_some() {
                    // Array of strings but we have a schema for the element type
                    // This happens with elements like <p>text</p> repeated
                    let schema = element_schema.unwrap();

                    // Check if this schema (or its base) has mixed content
                    let mixed_content_class = self.find_mixed_content_class(schema);

                    let mut relations = Vec::new();
                    for item in arr {
                        if let serde_json::Value::String(s) = item {
                            let inst = if let Some(ref mixed_class) = mixed_content_class {
                                // This is a mixed content type - build MixedContent instance with the text
                                self.build_simple_mixed_content_instance(schema, mixed_class, s)?
                            } else {
                                // Regular complex type - wrap the text in an Instance with _text property
                                let mut properties = BTreeMap::new();
                                properties.insert(
                                    "_text".to_string(),
                                    InstanceProperty::Primitive(PrimitiveValue::String(s.clone())),
                                );
                                Instance {
                                    schema: (*schema).clone(),
                                    id: None,
                                    capture: false,
                                    ref_props: false,
                                    properties,
                                }
                            };
                            relations.push(RelationValue::One(inst));
                        } else if let serde_json::Value::Object(obj) = item {
                            // Mixed array (some strings, some objects) - try to parse object
                            if let Ok(inst) = self.json_object_to_instance(obj, Some(&class_name)) {
                                relations.push(RelationValue::One(inst));
                            }
                        }
                    }
                    Ok(InstanceProperty::Relations(relations))
                } else {
                    // Array of primitives (no schema for element type)
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
                    Err(e) => {
                        // Unknown element type - skip by returning null
                        // Don't fall back to raw JSON as it creates documents without @type
                        // which TerminusDB cannot validate
                        tracing::warn!(
                            "Could not parse element '{}' as type '{}': {}. Property will be skipped.",
                            field_name, type_hint, e
                        );
                        Ok(InstanceProperty::Primitive(PrimitiveValue::Null))
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

/// Filter and normalize XML-specific attributes for TerminusDB storage.
///
/// This:
/// - Removes xmlns namespace declarations (xmlns:*, xmlns)
/// - Removes DTD-related attributes (dtd-version)
/// - Removes xml: prefixed attributes (xml:lang, xml:space)
/// - Removes xsi: prefixed attributes (xsi:schemaLocation, xsi:noNamespaceSchemaLocation)
/// - Renames @id to id (TerminusDB uses @id specially for document IDs)
/// - Renames other @-prefixed attributes to remove the prefix
fn filter_xml_attributes(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    obj.iter()
        .filter(|(key, _)| {
            !key.starts_with("xmlns:")
                && *key != "xmlns"
                && !key.starts_with("xml:")
                && !key.starts_with("xsi:")
                && *key != "dtd-version"
        })
        .map(|(k, v)| {
            // Recursively filter nested objects
            let filtered_value = filter_xml_attributes_value(v);

            // Rename @-prefixed attributes (XML attributes in JSON convention)
            // to remove the prefix, especially @id which TerminusDB interprets specially
            let normalized_key = if k.starts_with('@') {
                k.trim_start_matches('@').to_string()
            } else {
                k.clone()
            };

            (normalized_key, filtered_value)
        })
        .collect()
}

/// Recursively filter XML attributes from any JSON value
fn filter_xml_attributes_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(obj) => serde_json::Value::Object(filter_xml_attributes(obj)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(filter_xml_attributes_value).collect())
        }
        other => other.clone(),
    }
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

    // NOTE: We intentionally do NOT add xmlns declarations to ElementData.
    // Namespace declarations are XML metadata for parsing, not data content.
    // They would be converted to JSON keys like "xmlns:ali" which cause
    // "Unknown prefix" errors when inserting into TerminusDB.
    // for (prefix, uri) in elem.namespaces.iter() {
    //     data = data.with_xmlns(prefix, uri);
    // }

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
        vec![Schema::Class {
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
        }]
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

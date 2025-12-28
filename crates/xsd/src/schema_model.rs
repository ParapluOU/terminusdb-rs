use serde::{Deserialize, Serialize};
use pyo3::types::IntoPyDict;

/// Top-level XSD schema representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdSchema {
    pub target_namespace: Option<String>,
    pub schema_location: Option<String>,
    pub element_form_default: Option<String>,
    pub root_elements: Vec<XsdElement>,
    pub complex_types: Vec<XsdComplexType>,
    pub simple_types: Vec<XsdSimpleType>,
}

/// XSD element declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdElement {
    pub name: String,
    pub qualified_name: String,
    #[serde(rename = "type")]
    pub type_info: Option<XsdTypeInfo>,
    pub min_occurs: Option<u32>,
    pub max_occurs: Option<Cardinality>,
    pub nillable: bool,
    pub default: Option<String>,
}

/// Cardinality for max_occurs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cardinality {
    Number(u32),
    Unbounded, // null in JSON means unbounded
}

/// Type information (inline or reference)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdTypeInfo {
    pub name: Option<String>,
    pub qualified_name: Option<String>,
    pub category: String,
    pub is_complex: bool,
    pub is_simple: bool,
    pub content_model: Option<String>,
    pub attributes: Option<Vec<XsdAttribute>>,
    pub child_elements: Option<Vec<ChildElement>>,
}

/// XSD complex type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdComplexType {
    pub name: String,
    pub qualified_name: String,
    pub category: String,
    pub is_complex: bool,
    pub is_simple: bool,
    pub content_model: Option<String>,
    pub attributes: Option<Vec<XsdAttribute>>,
    pub child_elements: Option<Vec<ChildElement>>,
    /// True if this is an anonymous inline type definition
    #[serde(default)]
    pub is_anonymous: bool,
    /// For anonymous types, the element name this type is defined within
    pub element_name: Option<String>,
}

/// XSD simple type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdSimpleType {
    pub name: String,
    pub qualified_name: String,
    pub category: String,
    pub base_type: Option<String>,
    pub restrictions: Option<Vec<Restriction>>,
}

/// XSD attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsdAttribute {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: String,
    #[serde(rename = "use")]
    pub use_type: String, // "required" | "optional" | "prohibited"
    pub default: Option<String>,
}

/// Child element reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildElement {
    pub name: String,
    #[serde(rename = "type")]
    pub element_type: String,
    pub min_occurs: Option<u32>,
    pub max_occurs: Option<Cardinality>,
}

/// XSD restriction (for simple types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Restriction {
    MinLength { value: u32 },
    MaxLength { value: u32 },
    Pattern { value: String },
    Enumeration { values: Vec<String> },
    MinInclusive { value: String },
    MaxInclusive { value: String },
    MinExclusive { value: String },
    MaxExclusive { value: String },
}

impl XsdSchema {
    /// Parse an XSD schema from a file path using Python xmlschema
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the XSD schema file
    /// * `catalog_path` - Optional path to XML catalog file for URN resolution
    pub fn from_xsd_file(
        path: impl AsRef<std::path::Path>,
        catalog_path: Option<impl AsRef<std::path::Path>>,
    ) -> crate::Result<Self> {
        use pyo3::prelude::*;
        use pyo3::types::PyDict;

        let path = path.as_ref();

        Python::with_gil(|py| {
            // Import xmlschema
            let xmlschema = pyo3::types::PyModule::import(py, "xmlschema")?;

            // Run our comprehensive extraction Python code first to get helper functions
            let code = include_str!("extract_schema.py");
            let locals = PyDict::new(py);
            let code_cstr = std::ffi::CString::new(code)
                .expect("Python code contains null bytes");

            py.run(code_cstr.as_c_str(), Some(&locals), Some(&locals))?;

            // Load the schema with optional catalog-based URI mapper
            let schema = if let Some(catalog) = catalog_path {
                let catalog_str = catalog.as_ref().to_str().unwrap();

                // Create URI mapper using catalog
                let create_mapper = locals.get_item("create_uri_mapper")?.unwrap();
                let uri_mapper = create_mapper.call1((catalog_str,))?;

                // Load schema with URI mapper
                let kwargs = [("uri_mapper", uri_mapper)].into_py_dict(py)?;
                xmlschema.call_method(
                    "XMLSchema",
                    (path.to_str().unwrap(),),
                    Some(&kwargs),
                )?
            } else {
                // Load schema without URI mapper
                xmlschema.call_method1("XMLSchema", (path.to_str().unwrap(),))?
            };

            // Set schema in locals for extraction
            locals.set_item("schema", schema)?;

            // Extract schema data (the main extraction code at the bottom of extract_schema.py)
            let extract_code = std::ffi::CString::new(r#"
# Extract schema data
schema_data = {
    'target_namespace': schema.target_namespace,
    'schema_location': schema.url if hasattr(schema, 'url') else None,
    'element_form_default': schema.element_form_default if hasattr(schema, 'element_form_default') else None,
    'root_elements': [],
    'complex_types': [],
    'simple_types': [],
}

# Extract root elements
if hasattr(schema, 'elements') and schema.elements:
    for name, element in schema.elements.items():
        schema_data['root_elements'].append(extract_element_info(element))

# Extract named types
if hasattr(schema, 'types') and schema.types:
    for name, type_obj in schema.types.items():
        type_name = type(type_obj).__name__
        if 'Complex' in type_name:
            schema_data['complex_types'].append(extract_complex_type_info(type_obj))
        elif 'Simple' in type_name:
            schema_data['simple_types'].append(extract_simple_type_info(type_obj))

# Also extract anonymous complex types from elements (for schemas like NISO-STS)
# Track unique anonymous types by their structure
anonymous_complex_types = []
if hasattr(schema, 'elements') and schema.elements:
    for name, element in schema.elements.items():
        if element.type and hasattr(element.type, 'name'):
            # Only extract if it's an anonymous type (no name)
            if element.type.name is None:
                type_name = type(element.type).__name__
                if 'Complex' in type_name:
                    # Add element name to distinguish anonymous types
                    type_info = extract_complex_type_info(element.type)
                    type_info['name'] = f"anonymous_{name}_type"
                    type_info['qualified_name'] = f"anonymous_{name}_type"
                    type_info['is_anonymous'] = True
                    type_info['element_name'] = name
                    anonymous_complex_types.append(type_info)

# Add anonymous types to complex_types list
schema_data['complex_types'].extend(anonymous_complex_types)

# Convert to JSON string for Rust to parse
schema_data = json.dumps(schema_data)
"#).expect("Extract code contains null bytes");

            py.run(extract_code.as_c_str(), Some(&locals), Some(&locals))?;

            // Get the result
            let result = locals.get_item("schema_data")?.unwrap();
            let result_str = result.str()?;
            let json_str = result_str.to_str()?;

            // Parse JSON to our Rust types
            let schema_data: XsdSchema = serde_json::from_str(json_str)?;

            Ok(schema_data)
        })
    }

    /// Get all element names (useful for quick inspection)
    pub fn element_names(&self) -> Vec<&str> {
        self.root_elements
            .iter()
            .map(|e| e.name.as_str())
            .collect()
    }

    /// Get all complex type names
    pub fn complex_type_names(&self) -> Vec<&str> {
        self.complex_types
            .iter()
            .map(|t| t.name.as_str())
            .collect()
    }
}

impl Cardinality {
    pub fn is_unbounded(&self) -> bool {
        matches!(self, Cardinality::Unbounded)
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Cardinality::Number(0))
    }

    pub fn is_single(&self) -> bool {
        matches!(self, Cardinality::Number(1))
    }
}

impl XsdAttribute {
    pub fn is_required(&self) -> bool {
        self.use_type == "required"
    }

    pub fn is_optional(&self) -> bool {
        self.use_type == "optional"
    }
}

impl ChildElement {
    pub fn is_required(&self) -> bool {
        self.min_occurs.unwrap_or(1) > 0
    }

    pub fn is_multiple(&self) -> bool {
        match &self.max_occurs {
            Some(Cardinality::Unbounded) => true,
            Some(Cardinality::Number(n)) => *n > 1,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_schema() {
        let json = r#"{
            "target_namespace": "http://example.com/test",
            "schema_location": "file:///test.xsd",
            "element_form_default": "qualified",
            "root_elements": [],
            "complex_types": [],
            "simple_types": []
        }"#;

        let schema: XsdSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.target_namespace, Some("http://example.com/test".to_string()));
    }

    #[test]
    fn test_cardinality() {
        let unbounded = Cardinality::Unbounded;
        assert!(unbounded.is_unbounded());

        let single = Cardinality::Number(1);
        assert!(single.is_single());

        let optional = Cardinality::Number(0);
        assert!(optional.is_optional());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_parse_complete_schema() {
        // Sample JSON output from xmlschema
        let json = r#"{
            "target_namespace": "http://example.com/book",
            "schema_location": "file:///tmp/test.xsd",
            "element_form_default": "qualified",
            "root_elements": [
                {
                    "name": "{http://example.com/book}book",
                    "qualified_name": "{http://example.com/book}book",
                    "type": {
                        "name": null,
                        "qualified_name": null,
                        "category": "XsdComplexType",
                        "is_complex": true,
                        "is_simple": false,
                        "content_model": "XsdGroup",
                        "attributes": [
                            {
                                "name": "isbn",
                                "type": "{http://www.w3.org/2001/XMLSchema}string",
                                "use": "required",
                                "default": null
                            }
                        ],
                        "child_elements": [
                            {
                                "name": "{http://example.com/book}title",
                                "type": "{http://www.w3.org/2001/XMLSchema}string",
                                "min_occurs": 1,
                                "max_occurs": 1
                            },
                            {
                                "name": "{http://example.com/book}author",
                                "type": "{http://example.com/book}personType",
                                "min_occurs": 1,
                                "max_occurs": null
                            }
                        ]
                    },
                    "min_occurs": 1,
                    "max_occurs": 1,
                    "nillable": false,
                    "default": null
                }
            ],
            "complex_types": [
                {
                    "name": "{http://example.com/book}personType",
                    "qualified_name": "{http://example.com/book}personType",
                    "category": "XsdComplexType",
                    "is_complex": true,
                    "is_simple": false,
                    "content_model": "XsdGroup",
                    "child_elements": [
                        {
                            "name": "{http://example.com/book}firstName",
                            "type": "{http://www.w3.org/2001/XMLSchema}string",
                            "min_occurs": 1,
                            "max_occurs": 1
                        }
                    ]
                }
            ],
            "simple_types": []
        }"#;

        let schema: XsdSchema = serde_json::from_str(json).unwrap();

        // Verify schema
        assert_eq!(schema.target_namespace, Some("http://example.com/book".to_string()));
        assert_eq!(schema.root_elements.len(), 1);
        assert_eq!(schema.complex_types.len(), 1);

        // Verify root element
        let book_elem = &schema.root_elements[0];
        assert!(book_elem.name.contains("book"));

        let type_info = book_elem.type_info.as_ref().unwrap();
        assert!(type_info.is_complex);

        // Verify attributes
        let attrs = type_info.attributes.as_ref().unwrap();
        assert_eq!(attrs.len(), 1);
        assert!(attrs[0].is_required());

        // Verify child elements
        let children = type_info.child_elements.as_ref().unwrap();
        assert_eq!(children.len(), 2);

        // Check unbounded author
        let author = &children[1];
        assert!(author.is_multiple());
    }

    #[test]
    fn test_cardinality_unbounded() {
        // Test null as unbounded
        let json = r#"null"#;
        let card: Cardinality = serde_json::from_str(json).unwrap();
        assert!(card.is_unbounded());
    }
}

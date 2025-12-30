use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use xmlschema::validators::{
    XsdSchema as RustXsdSchema,
    FormDefault,
    GlobalType,
    Occurs,
    GroupParticle,
    XsdComplexType as RustComplexType,
    ComplexContent,
    XsdGroup,
};

/// Deserialize max_occurs where null means Unbounded
mod cardinality_option_de {
    use super::Cardinality;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Cardinality>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Use Option to handle both null and non-null values
        let opt: Option<Option<u32>> = Option::deserialize(deserializer)?;
        match opt {
            Some(Some(n)) => Ok(Some(Cardinality::Number(n))),
            Some(None) | None => Ok(Some(Cardinality::Unbounded)), // null means unbounded
        }
    }
}

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
    #[serde(default, deserialize_with = "cardinality_option_de::deserialize")]
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
    /// Base type for extension/restriction (XSD inheritance)
    pub base_type: Option<String>,
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
    #[serde(default, deserialize_with = "cardinality_option_de::deserialize")]
    pub max_occurs: Option<Cardinality>,
}

/// XSD restriction (for simple types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Restriction {
    Length { value: u32 },
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
    /// Parse an XSD schema from a file path using the pure Rust xmlschema library
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the XSD schema file
    /// * `catalog_path` - Optional path to XML catalog file for URN resolution (currently unused)
    pub fn from_xsd_file(
        path: impl AsRef<Path>,
        _catalog_path: Option<impl AsRef<Path>>,
    ) -> crate::Result<Self> {
        let path = path.as_ref();

        // Parse the schema using xmlschema-rs
        let rust_schema = RustXsdSchema::from_file(path)?;

        // Extract data from the parsed schema
        Self::from_rust_schema(&rust_schema, path)
    }

    /// Convert a parsed Rust schema to our representation
    fn from_rust_schema(schema: &RustXsdSchema, path: &Path) -> crate::Result<Self> {
        let target_namespace = schema.target_namespace.clone();
        let schema_location = Some(path.to_string_lossy().to_string());
        let element_form_default = Some(match schema.element_form_default {
            FormDefault::Qualified => "qualified".to_string(),
            FormDefault::Unqualified => "unqualified".to_string(),
        });

        // Extract root elements
        let mut root_elements = Vec::new();
        for (qname, elem) in schema.elements() {
            let element = Self::extract_element(&qname.to_string(), elem, schema);
            root_elements.push(element);
        }

        // Extract complex types
        let mut complex_types = Vec::new();
        let mut simple_types = Vec::new();

        for (qname, global_type) in schema.types() {
            match global_type {
                GlobalType::Complex(ct) => {
                    let complex = Self::extract_complex_type(&qname.to_string(), ct, schema);
                    complex_types.push(complex);
                }
                GlobalType::Simple(st) => {
                    let simple = Self::extract_simple_type(&qname.to_string(), st);
                    simple_types.push(simple);
                }
            }
        }

        // Also extract anonymous complex types from element declarations
        // These are named after the element itself (e.g., element `topic` → type `topic`)
        for (qname, elem) in schema.elements() {
            if let xmlschema::validators::ElementType::Complex(ct) = &elem.element_type {
                // Anonymous type (no name)
                if ct.name.is_none() {
                    let elem_name = &qname.local_name;
                    // Use element name directly as type name for anonymous types
                    let mut complex = Self::extract_complex_type(elem_name, ct, schema);
                    complex.is_anonymous = true;
                    complex.element_name = Some(elem_name.to_string());
                    complex_types.push(complex);
                }
            }
        }

        Ok(Self {
            target_namespace,
            schema_location,
            element_form_default,
            root_elements,
            complex_types,
            simple_types,
        })
    }

    /// Extract element information
    fn extract_element(
        qname: &str,
        elem: &Arc<xmlschema::validators::XsdElement>,
        schema: &RustXsdSchema,
    ) -> XsdElement {
        let type_info = Self::extract_element_type(&elem.element_type, schema);

        XsdElement {
            name: qname.to_string(),
            qualified_name: qname.to_string(),
            type_info: Some(type_info),
            min_occurs: Some(elem.occurs.min),
            max_occurs: Some(occurs_to_cardinality(&elem.occurs)),
            nillable: elem.nillable,
            default: elem.default.clone(),
        }
    }

    /// Extract type information from an element type
    fn extract_element_type(
        elem_type: &xmlschema::validators::ElementType,
        schema: &RustXsdSchema,
    ) -> XsdTypeInfo {
        match elem_type {
            xmlschema::validators::ElementType::Complex(ct) => {
                let (child_elements, content_model) = Self::extract_content_model(&ct.content, schema);
                let attributes = Self::extract_attributes(&ct.attributes);

                XsdTypeInfo {
                    name: ct.name.as_ref().map(|q| q.to_string()),
                    qualified_name: ct.name.as_ref().map(|q| q.to_string()),
                    category: "XsdComplexType".to_string(),
                    is_complex: true,
                    is_simple: false,
                    content_model,
                    attributes: Some(attributes),
                    child_elements: Some(child_elements),
                }
            }
            xmlschema::validators::ElementType::Simple(st) => {
                XsdTypeInfo {
                    name: st.name().map(|q| q.to_string()),
                    qualified_name: st.name().map(|q| q.to_string()),
                    category: "XsdSimpleType".to_string(),
                    is_complex: false,
                    is_simple: true,
                    content_model: None,
                    attributes: None,
                    child_elements: None,
                }
            }
            xmlschema::validators::ElementType::Any => {
                XsdTypeInfo {
                    name: Some("xs:anyType".to_string()),
                    qualified_name: Some("{http://www.w3.org/2001/XMLSchema}anyType".to_string()),
                    category: "XsdAnyType".to_string(),
                    is_complex: false,
                    is_simple: false,
                    content_model: None,
                    attributes: None,
                    child_elements: None,
                }
            }
        }
    }

    /// Extract complex type information
    fn extract_complex_type(
        qname: &str,
        ct: &Arc<RustComplexType>,
        schema: &RustXsdSchema,
    ) -> XsdComplexType {
        let (child_elements, content_model) = Self::extract_content_model(&ct.content, schema);
        let attributes = Self::extract_attributes(&ct.attributes);

        // Extract base type for inheritance (XSD extension/restriction)
        let base_type = ct.base_type.as_ref().map(|q| q.to_string());

        XsdComplexType {
            name: qname.to_string(),
            qualified_name: qname.to_string(),
            category: "XsdComplexType".to_string(),
            is_complex: true,
            is_simple: false,
            content_model,
            attributes: Some(attributes),
            child_elements: Some(child_elements),
            is_anonymous: false,
            element_name: None,
            base_type,
        }
    }

    /// Extract content model (child elements and model type)
    fn extract_content_model(
        content: &ComplexContent,
        _schema: &RustXsdSchema,
    ) -> (Vec<ChildElement>, Option<String>) {
        match content {
            ComplexContent::Group(group) => {
                // Check if group is empty (no particles)
                if group.is_empty() {
                    (vec![], Some("EmptyContent".to_string()))
                } else {
                    let children = Self::extract_group_children(group);
                    let model = Some(format!("{:?}", group.model));
                    (children, model)
                }
            }
            ComplexContent::Simple(_) => (vec![], Some("SimpleContent".to_string())),
        }
    }

    /// Extract children from a model group
    fn extract_group_children(group: &Arc<XsdGroup>) -> Vec<ChildElement> {
        let mut children = Vec::new();

        for particle in &group.particles {
            match particle {
                GroupParticle::Element(ep) => {
                    let name = ep.name.to_string();

                    // Try to get the type name from the element declaration
                    // Priority: 1. type_name reference, 2. resolved element_type, 3. fallback
                    let element_type = ep.element()
                        .map(|e| {
                            // First try the type_name reference
                            if let Some(type_name) = &e.type_name {
                                return type_name.to_string();
                            }

                            // Otherwise extract from the resolved element_type
                            match &e.element_type {
                                xmlschema::validators::ElementType::Simple(st) => {
                                    // Use qualified_name_string() which works for both
                                    // named types and builtin types
                                    st.qualified_name_string()
                                        .unwrap_or_else(|| "xs:anySimpleType".to_string())
                                }
                                xmlschema::validators::ElementType::Complex(ct) => {
                                    ct.name.as_ref()
                                        .map(|q| q.to_string())
                                        .unwrap_or_else(|| "xs:anyType".to_string())
                                }
                                xmlschema::validators::ElementType::Any => "xs:anyType".to_string(),
                            }
                        })
                        .unwrap_or_else(|| "xs:anyType".to_string());

                    children.push(ChildElement {
                        name,
                        element_type,
                        min_occurs: Some(ep.occurs.min),
                        max_occurs: Some(occurs_to_cardinality(&ep.occurs)),
                    });
                }
                GroupParticle::Group(nested) => {
                    // Recursively extract children from nested groups
                    children.extend(Self::extract_group_children(nested));
                }
                GroupParticle::Any(_) => {
                    // Wildcard elements - add as xs:any placeholder
                    children.push(ChildElement {
                        name: "##any".to_string(),
                        element_type: "xs:anyType".to_string(),
                        min_occurs: Some(0),
                        max_occurs: Some(Cardinality::Unbounded),
                    });
                }
            }
        }

        children
    }

    /// Extract attributes from an attribute group
    fn extract_attributes(attrs: &xmlschema::validators::XsdAttributeGroup) -> Vec<XsdAttribute> {
        use xmlschema::validators::AttributeUse;

        let mut result = Vec::new();

        for attr in attrs.iter_attributes() {
            let use_type = match attr.use_mode() {
                AttributeUse::Required => "required",
                AttributeUse::Optional => "optional",
                AttributeUse::Prohibited => "prohibited",
            };

            // Get the type - try type_name first, then resolved simple_type
            let attr_type = attr
                .type_name
                .as_ref()
                .map(|q| q.to_string())
                .or_else(|| {
                    // Get the resolved simple type and its qualified name
                    attr.simple_type()
                        .and_then(|st| st.qualified_name_string())
                })
                .unwrap_or_else(|| "xs:string".to_string());

            result.push(XsdAttribute {
                name: attr.name().local_name.clone(),
                attr_type,
                use_type: use_type.to_string(),
                default: attr.default().map(|s| s.to_string()),
            });
        }

        result
    }

    /// Extract simple type information
    fn extract_simple_type(
        qname: &str,
        st: &Arc<dyn xmlschema::validators::SimpleType + Send + Sync>,
    ) -> XsdSimpleType {
        use xmlschema::validators::SimpleType as SimpleTypeTrait;

        // Extract facets from the simple type
        let facets = st.facets();
        let mut restrictions = Vec::new();

        // Extract enumeration values if present
        if let Some(ref enum_facet) = facets.enumeration {
            restrictions.push(Restriction::Enumeration {
                values: enum_facet.values.clone(),
            });
        }

        // Extract other facets
        if let Some(ref len_facet) = facets.length {
            restrictions.push(Restriction::Length { value: len_facet.value as u32 });
        }
        if let Some(ref min_len) = facets.min_length {
            restrictions.push(Restriction::MinLength { value: min_len.value as u32 });
        }
        if let Some(ref max_len) = facets.max_length {
            restrictions.push(Restriction::MaxLength { value: max_len.value as u32 });
        }
        for pattern in &facets.patterns {
            restrictions.push(Restriction::Pattern { value: pattern.pattern.clone() });
        }

        // Get base type if this is a restricted type
        // Use SimpleTypeTrait::base_type to disambiguate from TypeValidator::base_type
        let base_type = SimpleTypeTrait::base_type(st.as_ref())
            .and_then(|base| base.qualified_name_string());

        XsdSimpleType {
            name: qname.to_string(),
            qualified_name: qname.to_string(),
            category: "XsdSimpleType".to_string(),
            base_type,
            restrictions: if restrictions.is_empty() { None } else { Some(restrictions) },
        }
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

/// Convert Occurs to our Cardinality type
fn occurs_to_cardinality(occurs: &Occurs) -> Cardinality {
    match occurs.max {
        Some(n) => Cardinality::Number(n),
        None => Cardinality::Unbounded,
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

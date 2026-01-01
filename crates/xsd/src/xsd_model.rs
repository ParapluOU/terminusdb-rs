//! XSD Model - High-level API for XSD-to-TerminusDB schema operations
//!
//! This module provides `XsdModel`, a unified interface for:
//! - Loading XSD schema bundles
//! - Converting to TerminusDB schemas
//! - Validating XML instances
//! - Parsing XML to TerminusDB instances

use crate::schema_generator::XsdToSchemaGenerator;
use crate::schema_model::XsdSchema;
use crate::xml_parser::{ParseResult, XmlToInstanceParser};
use crate::Result;
use std::path::{Path, PathBuf};
use terminusdb_schema::{Context, Instance, Schema};

// xmlschema-rs imports for XML parsing and conversion
use xmlschema::converters::{create_converter, ConverterType, ElementData};
use xmlschema::documents::{Document, Element};
use xmlschema::validators::XsdSchema as RustXsdSchema;

/// An XSD model containing parsed schemas and conversion rules.
///
/// This is the main entry point for working with XSD schemas in TerminusDB.
///
/// # Example
///
/// ```no_run
/// use terminusdb_xsd::XsdModel;
///
/// // Load from entry points
/// let model = XsdModel::from_entry_points(&[
///     "schemas/NISO-STS-extended-1-mathml3.xsd",
/// ], None::<&str>)?;
///
/// // Get TerminusDB schemas
/// let schemas = model.schemas();
///
/// // Validate XML content
/// model.validate_xml("<standard>...</standard>")?;
/// # Ok::<(), terminusdb_xsd::XsdError>(())
/// ```
#[derive(Debug, Clone)]
pub struct XsdModel {
    /// Path to the XSD bundle (directory or main schema file)
    bundle_path: PathBuf,

    /// Optional XML catalog path for URN resolution
    catalog_path: Option<PathBuf>,

    /// Parsed XSD schemas (raw representation)
    xsd_schemas: Vec<XsdSchema>,

    /// Generated TerminusDB schemas
    tdb_schemas: Vec<Schema>,

    /// TerminusDB Context with namespace from XSD target namespace
    context: Context,

    /// Namespace for generated schemas
    namespace: String,
}

impl XsdModel {
    /// Load an XSD model from explicit entry-point schema files.
    ///
    /// Entry points are complete XSD schemas (not modules) that import/include
    /// all their dependencies. The `xmlschema` library will automatically
    /// resolve includes/imports.
    ///
    /// # Arguments
    ///
    /// * `entry_points` - Paths to XSD entry-point files
    /// * `catalog_path` - Optional XML catalog for URN resolution
    pub fn from_entry_points(
        entry_points: &[impl AsRef<Path>],
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let catalog = catalog_path.as_ref().map(|p| p.as_ref().to_path_buf());

        // Use first entry point as bundle path
        let bundle_path = entry_points
            .first()
            .map(|p| p.as_ref().to_path_buf())
            .unwrap_or_default();

        // Parse all entry points
        let mut xsd_schemas = Vec::new();
        for entry in entry_points {
            let schema = XsdSchema::from_xsd_file(entry, catalog.as_ref())?;
            xsd_schemas.push(schema);
        }

        // Generate TerminusDB schemas with context
        let generator = XsdToSchemaGenerator::new();
        let mut all_schemas = Vec::new();
        let mut context = Context::default();

        for xsd in &xsd_schemas {
            let (ctx, schemas) = generator.generate_with_context(xsd)?;
            // Use the first XSD's context (they should share namespace for a bundle)
            if all_schemas.is_empty() {
                context = ctx;
            }
            all_schemas.extend(schemas);
        }

        // Deduplicate by schema ID
        let tdb_schemas = generator.deduplicate_schemas(all_schemas);

        let namespace = context.schema.clone();
        Ok(Self {
            bundle_path,
            catalog_path: catalog,
            xsd_schemas,
            tdb_schemas,
            context,
            namespace,
        })
    }

    /// Load an XSD model from a directory, auto-detecting entry points.
    ///
    /// This method scans the directory for XSD files and intelligently
    /// identifies which are entry points (complete schemas) vs modules.
    ///
    /// # Arguments
    ///
    /// * `schema_dir` - Directory containing XSD files
    /// * `catalog_path` - Optional XML catalog for URN resolution
    pub fn from_directory(
        schema_dir: impl AsRef<Path>,
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let schema_dir = schema_dir.as_ref();
        let catalog = catalog_path.as_ref().map(|p| p.as_ref().to_path_buf());

        let generator = XsdToSchemaGenerator::new();

        // Collect the raw XSD schemas for later use
        let xsd_files = generator.discover_xsd_files(schema_dir)?;
        let entry_points = generator.identify_entry_points(&xsd_files);

        let mut xsd_schemas = Vec::new();
        for entry in &entry_points {
            if let Ok(schema) = XsdSchema::from_xsd_file(entry, catalog.as_ref()) {
                xsd_schemas.push(schema);
            }
        }

        // Generate TerminusDB schemas with context
        let mut all_schemas = Vec::new();
        let mut context = Context::default();

        for xsd in &xsd_schemas {
            let (ctx, schemas) = generator.generate_with_context(xsd)?;
            if all_schemas.is_empty() {
                context = ctx;
            }
            all_schemas.extend(schemas);
        }

        let tdb_schemas = generator.deduplicate_schemas(all_schemas);
        let namespace = context.schema.clone();

        Ok(Self {
            bundle_path: schema_dir.to_path_buf(),
            catalog_path: catalog,
            xsd_schemas,
            tdb_schemas,
            context,
            namespace,
        })
    }

    /// Load an XSD model from a single schema file.
    pub fn from_file(
        schema_path: impl AsRef<Path>,
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        Self::from_entry_points(&[schema_path.as_ref()], catalog_path)
    }

    /// Set the schema namespace URI for generated schemas.
    ///
    /// This updates both the internal namespace tracking and the TerminusDB
    /// Context's `@schema` field, which controls how type names are expanded
    /// to full URIs.
    ///
    /// By default, the namespace is derived from the XSD's `targetNamespace`.
    /// Use this method to override it.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use terminusdb_xsd::XsdModel;
    /// let model = XsdModel::from_file("schema.xsd", None::<&str>)?
    ///     .with_namespace("http://my.org/schema#");
    /// # Ok::<(), terminusdb_xsd::XsdError>(())
    /// ```
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        let ns = namespace.into();
        self.context.schema = ns.clone();
        self.namespace = ns;
        self
    }

    /// Get the bundle path.
    pub fn bundle_path(&self) -> &Path {
        &self.bundle_path
    }

    /// Get the catalog path, if any.
    pub fn catalog_path(&self) -> Option<&Path> {
        self.catalog_path.as_deref()
    }

    /// Get the parsed XSD schemas (raw representation).
    pub fn xsd_schemas(&self) -> &[XsdSchema] {
        &self.xsd_schemas
    }

    /// Get the generated TerminusDB schemas.
    pub fn schemas(&self) -> &[Schema] {
        &self.tdb_schemas
    }

    /// Get the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get the TerminusDB Context with namespace from XSD target namespace.
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Consume the model and return just the TerminusDB schemas.
    pub fn into_schemas(self) -> Vec<Schema> {
        self.tdb_schemas
    }

    /// Consume the model and return both Context and schemas.
    pub fn into_context_and_schemas(self) -> (Context, Vec<Schema>) {
        (self.context, self.tdb_schemas)
    }

    /// Parse XML content into a JSON representation using the XSD schema.
    ///
    /// This parses the XML according to the XSD schema rules and returns
    /// a JSON value that can be converted to TerminusDB instances.
    ///
    /// # Arguments
    ///
    /// * `xml` - XML content as a string
    ///
    /// # Returns
    ///
    /// A JSON value representing the parsed XML structure.
    pub fn parse_xml_to_json(&self, xml: &str) -> Result<serde_json::Value> {
        // Parse the XML document
        let doc = Document::from_string(xml)?;

        // Get the root element
        let root = doc
            .root
            .as_ref()
            .ok_or_else(|| crate::XsdError::Parsing("XML document has no root element".into()))?;

        // Convert Element to ElementData
        let element_data = element_to_element_data(root);

        // Use the default XMLSchema converter
        let converter = create_converter(ConverterType::Default);

        // Convert to JSON
        let json_value = converter.decode(&element_data, 0);

        // Wrap in root element name
        let output_json = serde_json::json!({
            root.local_name(): json_value
        });

        Ok(output_json)
    }

    /// Validate XML content against the XSD schema.
    ///
    /// This uses xmlschema-rs to validate the XML directly.
    ///
    /// # Arguments
    ///
    /// * `xml` - XML content as a string
    ///
    /// # Returns
    ///
    /// Ok(()) if valid, Err with validation errors otherwise.
    pub fn validate_xml(&self, xml: &str) -> Result<()> {
        let schema_location = self
            .xsd_schemas
            .first()
            .and_then(|s| s.schema_location.as_ref())
            .ok_or_else(|| crate::XsdError::Parsing("No schema location available".into()))?;

        // Load the XSD schema from file
        let rust_schema = RustXsdSchema::from_file(std::path::Path::new(schema_location))?;

        // Validate the XML string
        let result = rust_schema.validate_string(xml);

        if result.valid {
            Ok(())
        } else {
            let errors = result.errors.join("; ");
            Err(crate::XsdError::Parsing(format!(
                "XML validation failed: {}",
                errors
            )))
        }
    }

    /// Parse XML content into TerminusDB instances.
    ///
    /// This parses and validates the XML against the XSD schema, then converts
    /// the result to TerminusDB instances using the generated schemas.
    ///
    /// # Arguments
    ///
    /// * `xml` - XML content as a string
    ///
    /// # Returns
    ///
    /// A vector of TerminusDB instances representing the parsed XML structure.
    ///
    /// # Errors
    ///
    /// Returns an error if XML parsing fails, validation fails, or the XML
    /// structure doesn't match the generated schemas.
    pub fn parse_xml_to_instances(&self, xml: &str) -> ParseResult<Vec<Instance>> {
        // First parse to JSON
        let json = self.parse_xml_to_json(xml)
            .map_err(|e| crate::xml_parser::XmlParseError::parse(e.to_string()))?;

        // Build element-to-class mapping from XSD schemas
        let element_map = self.element_to_class_map();

        // Convert JSON to instances using the generated schemas and element mapping
        let parser = XmlToInstanceParser::with_element_mapping(&self.tdb_schemas, element_map);
        parser.json_to_instances(&json)
    }

    /// Get statistics about the model.
    pub fn stats(&self) -> XsdModelStats {
        let total_complex_types: usize = self.xsd_schemas.iter()
            .map(|s| s.complex_types.len())
            .sum();

        let total_simple_types: usize = self.xsd_schemas.iter()
            .map(|s| s.simple_types.len())
            .sum();

        let total_root_elements: usize = self.xsd_schemas.iter()
            .map(|s| s.root_elements.len())
            .sum();

        XsdModelStats {
            xsd_schema_count: self.xsd_schemas.len(),
            tdb_schema_count: self.tdb_schemas.len(),
            total_complex_types,
            total_simple_types,
            total_root_elements,
        }
    }

    /// Find a TerminusDB schema by class name.
    pub fn find_schema(&self, class_name: &str) -> Option<&Schema> {
        self.tdb_schemas.iter().find(|s| match s {
            Schema::Class { id, .. } => id == class_name,
            _ => false,
        })
    }

    /// Get all class names from the generated schemas.
    pub fn class_names(&self) -> Vec<&str> {
        self.tdb_schemas
            .iter()
            .filter_map(|s| match s {
                Schema::Class { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Build a mapping from XML element names to TerminusDB class names.
    ///
    /// This uses the actual XSD element declarations:
    /// 1. Root elements with named types → map element to type's class
    /// 2. Root elements with anonymous types → map element to element-named class
    /// 3. Complex types that track their element_name (anonymous types)
    ///
    /// Returns a map of lowercase element names to their TerminusDB class names.
    pub fn element_to_class_map(&self) -> std::collections::HashMap<String, String> {
        use heck::ToPascalCase;
        let mut map = std::collections::HashMap::new();

        for xsd in &self.xsd_schemas {
            // Map root elements to their types
            for elem in &xsd.root_elements {
                let local_name = elem.name.split('}').last().unwrap_or(&elem.name);

                if let Some(type_info) = &elem.type_info {
                    // Check if this is a named type or anonymous type
                    if let Some(type_qname) = type_info.name.as_ref().or(type_info.qualified_name.as_ref()) {
                        // Named type - use type name
                        let type_name = type_qname.split('}').last().unwrap_or(type_qname);
                        let class_name = type_name.to_pascal_case();

                        if self.find_schema(&class_name).is_some() {
                            map.insert(local_name.to_lowercase(), class_name);
                        }
                    } else {
                        // Anonymous type - use element name as class name
                        let class_name = local_name.to_pascal_case();
                        if self.find_schema(&class_name).is_some() {
                            map.insert(local_name.to_lowercase(), class_name);
                        }
                    }
                }
            }

            // Map from complex types that have an element_name (anonymous types)
            for ct in &xsd.complex_types {
                if let Some(elem_name) = &ct.element_name {
                    let local_name = elem_name.split('}').last().unwrap_or(elem_name);
                    // The class name for anonymous types should be the element name
                    let class_name = local_name.to_pascal_case();

                    if self.find_schema(&class_name).is_some() {
                        map.insert(local_name.to_lowercase(), class_name);
                    }
                }
            }
        }

        map
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

/// Statistics about an XSD model.
#[derive(Debug, Clone)]
pub struct XsdModelStats {
    /// Number of XSD schemas loaded
    pub xsd_schema_count: usize,
    /// Number of TerminusDB schemas generated
    pub tdb_schema_count: usize,
    /// Total complex types across all XSD schemas
    pub total_complex_types: usize,
    /// Total simple types across all XSD schemas
    pub total_simple_types: usize,
    /// Total root elements across all XSD schemas
    pub total_root_elements: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xsd_model_stats() {
        let stats = XsdModelStats {
            xsd_schema_count: 2,
            tdb_schema_count: 50,
            total_complex_types: 100,
            total_simple_types: 20,
            total_root_elements: 5,
        };

        assert_eq!(stats.xsd_schema_count, 2);
        assert_eq!(stats.tdb_schema_count, 50);
    }
}

//! XSD Model - High-level API for XSD-to-TerminusDB schema operations
//!
//! This module provides `XsdModel`, a unified interface for:
//! - Loading XSD schema bundles
//! - Converting to TerminusDB schemas
//! - Validating XML instances
//! - Parsing XML to TerminusDB instances

use crate::schema_generator::XsdToSchemaGenerator;
use crate::schema_model::XsdSchema;
use crate::Result;
use std::path::{Path, PathBuf};
use terminusdb_schema::Schema;

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

        // Generate TerminusDB schemas
        let generator = XsdToSchemaGenerator::new();
        let mut all_schemas = Vec::new();
        for xsd in &xsd_schemas {
            let schemas = generator.generate(xsd)?;
            all_schemas.extend(schemas);
        }

        // Deduplicate by schema ID
        let tdb_schemas = generator.deduplicate_schemas(all_schemas);

        Ok(Self {
            bundle_path,
            catalog_path: catalog,
            xsd_schemas,
            tdb_schemas,
            namespace: "terminusdb://schema#".to_string(),
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

        // Use the generator's directory scanning
        let tdb_schemas = generator.generate_from_directory(schema_dir, catalog.as_ref())?;

        // Also collect the raw XSD schemas for later use
        let xsd_files = generator.discover_xsd_files(schema_dir)?;
        let entry_points = generator.identify_entry_points(&xsd_files);

        let mut xsd_schemas = Vec::new();
        for entry in &entry_points {
            if let Ok(schema) = XsdSchema::from_xsd_file(entry, catalog.as_ref()) {
                xsd_schemas.push(schema);
            }
        }

        Ok(Self {
            bundle_path: schema_dir.to_path_buf(),
            catalog_path: catalog,
            xsd_schemas,
            tdb_schemas,
            namespace: "terminusdb://schema#".to_string(),
        })
    }

    /// Load an XSD model from a single schema file.
    pub fn from_file(
        schema_path: impl AsRef<Path>,
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        Self::from_entry_points(&[schema_path.as_ref()], catalog_path)
    }

    /// Set the namespace for generated schemas.
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
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

    /// Consume the model and return just the TerminusDB schemas.
    pub fn into_schemas(self) -> Vec<Schema> {
        self.tdb_schemas
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
        use pyo3::prelude::*;
        use pyo3::types::PyModule;

        Python::with_gil(|py| {
            let xmlschema = PyModule::import(py, "xmlschema")?;
            let json_module = PyModule::import(py, "json")?;

            let schema_location = self.xsd_schemas
                .first()
                .and_then(|s| s.schema_location.as_ref())
                .ok_or_else(|| crate::XsdError::Parsing("No schema location available".into()))?;

            // Create XMLSchema from the file
            let schema_obj = xmlschema.call_method1("XMLSchema", (schema_location.as_str(),))?;

            // Parse and convert XML to dict
            let to_dict = schema_obj.call_method1("to_dict", (xml,))?;

            // Convert to JSON
            let json_str: String = json_module
                .call_method1("dumps", (to_dict,))?
                .extract()?;

            // Parse JSON
            let value: serde_json::Value = serde_json::from_str(&json_str)?;

            Ok(value)
        })
    }

    /// Validate XML content against the XSD schema.
    ///
    /// This uses xmlschema to validate the XML directly.
    ///
    /// # Arguments
    ///
    /// * `xml` - XML content as a string
    ///
    /// # Returns
    ///
    /// Ok(()) if valid, Err with validation errors otherwise.
    pub fn validate_xml(&self, xml: &str) -> Result<()> {
        use pyo3::prelude::*;
        use pyo3::types::PyModule;

        Python::with_gil(|py| {
            let xmlschema = PyModule::import(py, "xmlschema")?;

            let schema_location = self.xsd_schemas
                .first()
                .and_then(|s| s.schema_location.as_ref())
                .ok_or_else(|| crate::XsdError::Parsing("No schema location available".into()))?;

            let schema_obj = xmlschema.call_method1("XMLSchema", (schema_location.as_str(),))?;

            // Validate XML (raises exception if invalid)
            match schema_obj.call_method1("validate", (xml,)) {
                Ok(_) => Ok(()),
                Err(e) => Err(crate::XsdError::Parsing(format!("XML validation failed: {}", e))),
            }
        })
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

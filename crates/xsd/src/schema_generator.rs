//! Runtime TerminusDB Schema Generator from XSD

use crate::schema_model::{Cardinality, ChildElement, Restriction, XsdAttribute, XsdComplexType, XsdSchema, XsdSimpleType};
use crate::Result;
use heck::ToPascalCase;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use terminusdb_schema::{Key, Property, Schema, SetCardinality, TypeFamily};

/// Entry point candidate with scoring information.
#[derive(Debug, Clone)]
pub struct EntryPointCandidate {
    pub path: PathBuf,
    pub score: EntryPointScore,
}

/// Scoring breakdown for entry point detection.
#[derive(Debug, Clone)]
pub struct EntryPointScore {
    pub depth_score: i32,
    pub include_count_score: i32,
    pub naming_score: i32,
    pub total_score: i32,
    pub include_count: usize,
    pub depth: usize,
    pub reasons: Vec<String>,
}

pub struct XsdToSchemaGenerator {
    pub namespace: String,
}

impl XsdToSchemaGenerator {
    pub fn new() -> Self {
        Self {
            namespace: "terminusdb://schema#".to_string(),
        }
    }

    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }

    pub fn generate(&self, xsd_schema: &XsdSchema) -> Result<Vec<Schema>> {
        let mut schemas = Vec::new();

        // Generate schemas for complex types
        for complex_type in &xsd_schema.complex_types {
            let schema = self.generate_class_from_complex_type(complex_type)?;
            schemas.push(schema);
        }

        // Generate schemas for simple types (enums, type aliases)
        for simple_type in &xsd_schema.simple_types {
            if let Some(schema) = self.generate_from_simple_type(simple_type)? {
                schemas.push(schema);
            }
        }

        Ok(schemas)
    }

    /// Generate TerminusDB schemas from explicit entry-point XSD files.
    ///
    /// Use this when you know which schemas are entry points (complete schemas
    /// with all dependencies). xmlschema will automatically resolve includes/imports.
    ///
    /// # Arguments
    ///
    /// * `entry_points` - List of entry-point XSD file paths
    /// * `catalog_path` - Optional path to XML catalog for URN resolution
    ///
    /// # Returns
    ///
    /// Consolidated list of all TerminusDB schemas from the type trees,
    /// with automatic deduplication.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
    ///
    /// let generator = XsdToSchemaGenerator::new();
    /// let entry_points = vec![
    ///     "schemas/NISO-STS-extended-1-mathml3.xsd",
    ///     "schemas/NISO-STS-interchange-1-mathml3.xsd",
    /// ];
    /// let schemas = generator.generate_from_entry_points(&entry_points, None::<&str>)?;
    /// # Ok::<(), terminusdb_xsd::XsdError>(())
    /// ```
    pub fn generate_from_entry_points(
        &self,
        entry_points: &[impl AsRef<Path>],
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Vec<Schema>> {
        let entry_paths: Vec<PathBuf> = entry_points.iter().map(|p| p.as_ref().to_path_buf()).collect();

        println!("🎯 Processing {} explicit entry point(s):", entry_paths.len());
        for entry in &entry_paths {
            println!("   • {:?}", entry.file_name().unwrap_or_default());
        }
        println!("\n   xmlschema will automatically resolve includes/imports");

        let mut all_schemas = Vec::new();
        let mut errors = Vec::new();

        for xsd_file in &entry_paths {
            print!("\n📖 Parsing {:?}...\n", xsd_file.file_name().unwrap_or_default());

            match XsdSchema::from_xsd_file(xsd_file, catalog_path.as_ref()) {
                Ok(xsd_schema) => {
                    println!("   ✓ Loaded (includes all dependencies)");
                    println!("   Found {} complex types in type tree", xsd_schema.complex_types.len());

                    match self.generate(&xsd_schema) {
                        Ok(schemas) => {
                            println!("   ✓ Generated {} schemas", schemas.len());
                            all_schemas.extend(schemas);
                        }
                        Err(e) => {
                            println!("   ✗ Generation error");
                            errors.push((xsd_file.clone(), format!("Generation error: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    println!("   ✗ Parse error: {}", e);
                    errors.push((xsd_file.clone(), format!("Parse error: {}", e)));
                }
            }
        }

        if !errors.is_empty() {
            println!("\n⚠️  {} entry point(s) had errors:", errors.len());
            for (file, error) in &errors {
                println!("   {:?}: {}", file.file_name().unwrap_or_default(), error);
            }
        }

        let deduplicated = self.deduplicate_schemas(all_schemas);

        println!("\n✅ Generated {} unique schemas from {} entry point(s)",
                 deduplicated.len(), entry_paths.len());

        Ok(deduplicated)
    }

    /// Generate TerminusDB schemas from all XSD files in a directory.
    ///
    /// Discovers XSD files and intelligently parses them. By default, only parses
    /// entry-point schemas (files likely to be complete schemas, not modules),
    /// letting xmlschema follow includes/imports to get the full type tree.
    ///
    /// # Arguments
    ///
    /// * `schema_dir` - Directory containing XSD schema files
    /// * `catalog_path` - Optional path to XML catalog for URN resolution
    ///
    /// # Returns
    ///
    /// Consolidated list of all TerminusDB schemas from the XSD type tree,
    /// with automatic deduplication.
    pub fn generate_from_directory(
        &self,
        schema_dir: impl AsRef<Path>,
        catalog_path: Option<impl AsRef<Path>>,
    ) -> Result<Vec<Schema>> {
        let schema_dir = schema_dir.as_ref();

        // Discover all XSD files
        let xsd_files = self.discover_xsd_files(schema_dir)?;

        if xsd_files.is_empty() {
            return Ok(Vec::new());
        }

        println!("📂 Discovered {} XSD files in {:?}", xsd_files.len(), schema_dir);

        // Identify entry points (complete schemas, not modules)
        let entry_points = self.identify_entry_points(&xsd_files);

        if entry_points.is_empty() {
            // Check if this is a flat architecture (all files score similarly)
            if let Ok(candidates) = self.analyze_entry_point_candidates(schema_dir) {
                let scores: Vec<_> = candidates.iter().map(|c| c.score.total_score).collect();

                if !scores.is_empty() {
                    let min_score = scores.iter().min().unwrap_or(&0);
                    let max_score = scores.iter().max().unwrap_or(&0);
                    let score_range = max_score - min_score;

                    // Flat architecture: all scores within 20 points and in "GOOD" range (60-80)
                    if score_range <= 20 && *min_score >= 50 && *max_score <= 90 {
                        println!("📋 Flat architecture detected (all {} files score {}-{} pts)",
                                 xsd_files.len(), min_score, max_score);
                        println!("   Each schema is independent - parsing all and deduplicating");
                        return self.parse_all_files(&xsd_files, catalog_path.as_ref());
                    }
                }
            }

            println!("⚠️  No entry points found, falling back to parsing all files");
            return self.parse_all_files(&xsd_files, catalog_path.as_ref());
        }

        println!("🎯 Identified {} entry point(s):", entry_points.len());
        for entry in &entry_points {
            println!("   • {:?}", entry.file_name().unwrap_or_default());
        }
        println!("\n   xmlschema will automatically resolve includes/imports");

        // Parse only entry points - xmlschema handles dependencies automatically
        let mut all_schemas = Vec::new();
        let mut errors = Vec::new();

        for xsd_file in &entry_points {
            print!("\n📖 Parsing {:?}...\n", xsd_file.file_name().unwrap_or_default());

            match XsdSchema::from_xsd_file(xsd_file, catalog_path.as_ref()) {
                Ok(xsd_schema) => {
                    println!("   ✓ Loaded (includes all dependencies)");
                    println!("   Found {} complex types in type tree", xsd_schema.complex_types.len());

                    match self.generate(&xsd_schema) {
                        Ok(schemas) => {
                            println!("   ✓ Generated {} schemas", schemas.len());
                            all_schemas.extend(schemas);
                        }
                        Err(e) => {
                            println!("   ✗ Generation error");
                            errors.push((xsd_file.clone(), format!("Generation error: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    println!("   ✗ Parse error: {}", e);
                    errors.push((xsd_file.clone(), format!("Parse error: {}", e)));
                }
            }
        }

        // Report errors
        if !errors.is_empty() {
            println!("\n⚠️  {} entry point(s) had errors:", errors.len());
            for (file, error) in &errors {
                println!("   {:?}: {}", file.file_name().unwrap_or_default(), error);
            }
        }

        // Deduplicate schemas by class ID
        let deduplicated = self.deduplicate_schemas(all_schemas);

        println!("\n✅ Generated {} unique schemas from {} entry point(s)",
                 deduplicated.len(), entry_points.len());

        Ok(deduplicated)
    }

    /// Analyze XSD files to find likely entry points.
    ///
    /// Returns a ranked list of entry point candidates based on:
    /// - Directory depth (root files more likely than nested modules)
    /// - Number of include/import directives (entry points include many modules)
    /// - File size (complete schemas tend to be larger or coordinate many includes)
    /// - Naming patterns (avoiding "*Mod", "*elements", etc.)
    ///
    /// This allows presenting customers with a dropdown of possibilities.
    pub fn analyze_entry_point_candidates(
        &self,
        schema_dir: impl AsRef<Path>,
    ) -> Result<Vec<EntryPointCandidate>> {
        let schema_dir = schema_dir.as_ref();
        let xsd_files = self.discover_xsd_files(schema_dir)?;

        if xsd_files.is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();

        for file in &xsd_files {
            let score = self.score_entry_point_candidate(file, schema_dir)?;

            if score.total_score > 0 {
                candidates.push(EntryPointCandidate {
                    path: file.clone(),
                    score,
                });
            }
        }

        // Sort by total score (descending)
        candidates.sort_by(|a, b| b.score.total_score.cmp(&a.score.total_score));

        Ok(candidates)
    }

    /// Score a file as a potential entry point.
    fn score_entry_point_candidate(
        &self,
        file: &Path,
        schema_dir: &Path,
    ) -> Result<EntryPointScore> {
        let file_name = file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let path_str = file.to_str().unwrap_or("");

        let mut score = EntryPointScore {
            depth_score: 0,
            include_count_score: 0,
            naming_score: 0,
            total_score: 0,
            include_count: 0,
            depth: 0,
            reasons: Vec::new(),
        };

        // Calculate directory depth (shallower = more likely entry point)
        let relative = file.strip_prefix(schema_dir).unwrap_or(file);
        score.depth = relative.components().count().saturating_sub(1);
        score.depth_score = match score.depth {
            0 => 50,  // Root directory - very likely
            1 => 20,  // One level down - possible
            _ => 0,   // Deep nesting - unlikely
        };

        if score.depth == 0 {
            score.reasons.push("Top-level file".to_string());
        }

        // Parse file to count includes/imports
        if let Ok(content) = std::fs::read_to_string(file) {
            let includes = content.matches("<xs:include").count();
            let imports = content.matches("<xs:import").count();
            score.include_count = includes + imports;

            score.include_count_score = match score.include_count {
                5.. => 40,   // Many includes - likely entry point
                2..=4 => 20, // Some includes - possible
                _ => 0,
            };

            if score.include_count >= 5 {
                score.reasons.push(format!("{} include/import directives", score.include_count));
            }

            // Check for entry point indicators in comments
            if content.contains("entry point") ||
               content.contains("main schema") ||
               content.contains("complete schema") {
                score.naming_score += 30;
                score.reasons.push("Contains entry point annotation".to_string());
            }
        }

        // Naming patterns
        // Positive indicators
        if file_name.starts_with("base") {
            score.naming_score += 20;
            score.reasons.push("Starts with 'base'".to_string());
        } else if file_name.starts_with("NISO-STS") && !file_name.contains("-elements") {
            score.naming_score += 25;
            score.reasons.push("NISO-STS main schema".to_string());
        } else if (file_name.contains("topic") || file_name.contains("map")) &&
                  !file_name.contains("Mod") {
            score.naming_score += 15;
            score.reasons.push("Document type schema".to_string());
        }

        // Negative indicators
        if file_name.contains("Mod") ||
           file_name.contains("Grp") ||
           file_name.contains("Domain") {
            score.naming_score -= 50;
            score.reasons.push("Module naming pattern".to_string());
        }

        if file_name.contains("-elements") ||
           file_name.starts_with("module-") ||
           file_name == "xml.xsd" ||
           file_name.starts_with("mathml") {
            score.naming_score -= 50;
            score.reasons.push("Support module".to_string());
        }

        if path_str.contains("standard-modules/") ||
           path_str.contains("/modules/") {
            score.naming_score -= 40;
            score.reasons.push("In modules directory".to_string());
        }

        // Calculate total score
        score.total_score = score.depth_score + score.include_count_score + score.naming_score;

        Ok(score)
    }

    /// Identify entry-point schemas (complete schemas, not modules).
    ///
    /// Entry points are typically schemas that:
    /// - DITA: Don't have "Mod", "Grp", or "Domain" in their names
    /// - NISO-STS: Top-level schema files (not in subdirectories like standard-modules/)
    /// - Start with "base", "NISO-STS", or are named after document types
    pub fn identify_entry_points(&self, xsd_files: &[PathBuf]) -> Vec<PathBuf> {
        let mut entry_points = Vec::new();

        for file in xsd_files {
            let file_name = file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let path_str = file.to_str().unwrap_or("");

            // Skip common module/library files
            if file_name == "xml.xsd" ||
               file_name == "ditaarch.xsd" ||
               file_name.starts_with("mathml") ||
               file_name.starts_with("module-") ||
               file_name == "xlink.xsd" {
                continue;
            }

            // Skip files in subdirectories that are typically modules
            if path_str.contains("standard-modules/") ||
               path_str.contains("/modules/") {
                continue;
            }

            // Skip DITA module files
            if file_name.contains("Mod") ||
               file_name.contains("Grp") ||
               file_name.contains("Domain") {
                continue;
            }

            // Include files that look like complete schemas
            // DITA: base*, topic, map
            if file_name.starts_with("base") ||
               (file_name.contains("topic") && !file_name.contains("Mod")) ||
               (file_name.contains("map") && !file_name.contains("Mod")) {
                entry_points.push(file.clone());
                continue;
            }

            // NISO-STS: Top-level NISO-STS files (not -elements.xsd which are modules)
            if file_name.starts_with("NISO-STS") && !file_name.contains("-elements") {
                entry_points.push(file.clone());
                continue;
            }
        }

        entry_points
    }

    /// Fallback: Parse all files individually (less efficient).
    fn parse_all_files(
        &self,
        xsd_files: &[PathBuf],
        catalog_path: Option<&impl AsRef<Path>>,
    ) -> Result<Vec<Schema>> {
        let mut all_schemas = Vec::new();
        let mut errors = Vec::new();

        for xsd_file in xsd_files {
            print!("   Parsing {:?}... ", xsd_file.file_name().unwrap_or_default());

            match XsdSchema::from_xsd_file(xsd_file, catalog_path) {
                Ok(xsd_schema) => {
                    match self.generate(&xsd_schema) {
                        Ok(schemas) => {
                            println!("✓ ({} types)", schemas.len());
                            all_schemas.extend(schemas);
                        }
                        Err(e) => {
                            println!("✗ (generation error)");
                            errors.push((xsd_file.clone(), format!("Generation error: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    println!("✗ (parse error)");
                    errors.push((xsd_file.clone(), format!("Parse error: {}", e)));
                }
            }
        }

        if !errors.is_empty() {
            println!("\n⚠️  {} file(s) had errors:", errors.len());
            for (file, error) in &errors {
                println!("   {:?}: {}", file.file_name().unwrap_or_default(), error);
            }
        }

        let deduplicated = self.deduplicate_schemas(all_schemas);
        println!("\n✅ Generated {} unique schemas from {} files", deduplicated.len(), xsd_files.len());

        Ok(deduplicated)
    }

    /// Recursively discover all XSD files in a directory.
    pub fn discover_xsd_files(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut xsd_files = Vec::new();

        if !dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Not a directory: {:?}", dir)
            ).into());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories
                xsd_files.extend(self.discover_xsd_files(&path)?);
            } else if path.extension().and_then(|s| s.to_str()) == Some("xsd") {
                xsd_files.push(path);
            }
        }

        Ok(xsd_files)
    }

    /// Deduplicate schemas by class ID (namespace + name).
    ///
    /// When multiple schemas define the same class, keep the first occurrence.
    pub fn deduplicate_schemas(&self, schemas: Vec<Schema>) -> Vec<Schema> {
        let mut seen = HashMap::new();
        let mut deduplicated = Vec::new();

        for schema in schemas {
            if let Schema::Class { ref id, ref base, .. } = schema {
                // Create unique key from namespace + id
                let key = match base {
                    Some(ns) => format!("{}#{}", ns, id),
                    None => id.clone(),
                };

                if !seen.contains_key(&key) {
                    seen.insert(key, true);
                    deduplicated.push(schema);
                }
            } else {
                // Non-class schemas (enums, etc.) - always include
                deduplicated.push(schema);
            }
        }

        deduplicated
    }

    fn generate_class_from_complex_type(&self, complex_type: &XsdComplexType) -> Result<Schema> {
        // Extract namespace and local name from Clark notation: {namespace}localName
        let (namespace, local_name) = self.parse_clark_notation(
            if complex_type.is_anonymous {
                complex_type
                    .element_name
                    .as_ref()
                    .unwrap_or(&complex_type.name)
            } else {
                &complex_type.name
            }
        );

        // Convert to PascalCase for TerminusDB class naming convention
        let class_id = local_name.to_pascal_case();

        let mut properties = Vec::new();

        if let Some(ref attributes) = complex_type.attributes {
            for attr in attributes {
                properties.push(self.attribute_to_property(attr)?);
            }
        }

        if let Some(ref child_elements) = complex_type.child_elements {
            for element in child_elements {
                properties.push(self.child_element_to_property(element)?);
            }
        }

        // Always use ValueHash for content-based addressing
        // XML instance tracking is handled separately via Chunk models
        let key = Key::ValueHash;

        let subdocument = complex_type.is_anonymous;

        Ok(Schema::Class {
            id: class_id,
            base: namespace,  // Use TerminusDB @base for namespace preservation
            key,
            documentation: None,
            subdocument,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties,
        })
    }

    /// Generate a TerminusDB schema from an XSD simple type.
    ///
    /// Simple types with enumeration restrictions become Schema::Enum.
    /// Other simple types (type aliases, pattern restrictions) are skipped
    /// because they map to primitive xsd: types or string patterns.
    ///
    /// Returns None if the simple type doesn't need a schema definition.
    fn generate_from_simple_type(&self, simple_type: &XsdSimpleType) -> Result<Option<Schema>> {
        // Extract namespace and local name from Clark notation
        let (_namespace, local_name) = self.parse_clark_notation(&simple_type.name);

        // Convert to PascalCase for TerminusDB naming
        let enum_id = local_name.to_pascal_case();

        // Check if this simple type has enumeration restrictions
        if let Some(ref restrictions) = simple_type.restrictions {
            for restriction in restrictions {
                if let Restriction::Enumeration { values } = restriction {
                    // Generate Schema::Enum for enumeration types
                    // Note: Schema::Enum doesn't support base/namespace yet (TODO in schema crate)
                    return Ok(Some(Schema::Enum {
                        id: enum_id,
                        documentation: None,
                        values: values.clone(),
                    }));
                }
            }
        }

        // Simple types without enumeration don't need separate schema definitions
        // They map to xsd: primitives (string, integer, etc.) or are just restricted
        // string types that TerminusDB represents as xsd:string
        Ok(None)
    }

    /// Parse Clark notation to extract namespace and local name.
    ///
    /// Clark notation format: `{http://example.com/ns}localName`
    /// Returns: (Some(namespace), localName) or (None, fullName)
    fn parse_clark_notation(&self, name: &str) -> (Option<String>, String) {
        if let Some(start) = name.find('{') {
            if let Some(end) = name.find('}') {
                let namespace = name[start + 1..end].to_string();
                let local_name = name[end + 1..].to_string();
                return (Some(namespace), local_name);
            }
        }
        // No Clark notation found, return name as-is
        (None, name.to_string())
    }

    fn attribute_to_property(&self, attr: &XsdAttribute) -> Result<Property> {
        let class = self.map_xsd_type_to_tdb_class(&attr.attr_type)?;

        let r#type = match attr.use_type.as_str() {
            "required" => None,
            _ => Some(TypeFamily::Optional),
        };

        // Extract local name from Clark notation for property name
        let (_, local_name) = self.parse_clark_notation(&attr.name);

        Ok(Property {
            name: local_name,
            r#type,
            class,
        })
    }

    fn child_element_to_property(&self, element: &ChildElement) -> Result<Property> {
        let class = self.map_xsd_type_to_tdb_class(&element.element_type)?;

        let min = element.min_occurs.unwrap_or(1);
        let max = element.max_occurs.clone();

        let r#type = match (min, &max) {
            (0, Some(Cardinality::Unbounded)) => Some(TypeFamily::Optional),
            (0, Some(Cardinality::Number(1))) | (0, None) => Some(TypeFamily::Optional),
            (1, Some(Cardinality::Unbounded)) => Some(TypeFamily::Set(SetCardinality::None)),
            (1, Some(Cardinality::Number(1))) | (1, None) => None,
            _ => match max {
                Some(Cardinality::Unbounded) => Some(TypeFamily::Set(SetCardinality::None)),
                Some(Cardinality::Number(n)) if n > 1 => Some(TypeFamily::Set(SetCardinality::None)),
                _ => None,
            },
        };

        // Extract local name from Clark notation for property name
        let (_, local_name) = self.parse_clark_notation(&element.name);

        Ok(Property {
            name: local_name,
            r#type,
            class,
        })
    }

    fn map_xsd_type_to_tdb_class(&self, xsd_type: &str) -> Result<String> {
        // Extract local name from Clark notation if present
        let type_name = if xsd_type.contains('}') {
            xsd_type.split('}').nth(1).unwrap_or(xsd_type)
        } else {
            xsd_type
        };

        // Map XSD built-in types to TerminusDB types
        let mapped = match type_name {
            "string" | "xs:string" | "xsd:string" => "xsd:string",
            "normalizedString" | "xs:normalizedString" | "xsd:normalizedString" => "xsd:string",
            "token" | "xs:token" | "xsd:token" => "xsd:string",
            "NMTOKEN" | "xs:NMTOKEN" | "xsd:NMTOKEN" => "xsd:string",
            "NCName" | "xs:NCName" | "xsd:NCName" => "xsd:string",
            "ID" | "xs:ID" | "xsd:ID" => "xsd:string",
            "IDREF" | "xs:IDREF" | "xsd:IDREF" => "xsd:string",
            "anyURI" | "xs:anyURI" | "xsd:anyURI" => "xsd:string",
            "integer" | "xs:integer" | "xsd:integer" => "xsd:integer",
            "int" | "xs:int" | "xsd:int" => "xsd:integer",
            "long" | "xs:long" | "xsd:long" => "xsd:integer",
            "short" | "xs:short" | "xsd:short" => "xsd:integer",
            "byte" | "xs:byte" | "xsd:byte" => "xsd:integer",
            "nonNegativeInteger" | "xs:nonNegativeInteger" | "xsd:nonNegativeInteger" => "xsd:integer",
            "positiveInteger" | "xs:positiveInteger" | "xsd:positiveInteger" => "xsd:integer",
            "unsignedInt" | "xs:unsignedInt" | "xsd:unsignedInt" => "xsd:integer",
            "decimal" | "xs:decimal" | "xsd:decimal" => "xsd:decimal",
            "float" | "xs:float" | "xsd:float" => "xsd:decimal",
            "double" | "xs:double" | "xsd:double" => "xsd:decimal",
            "boolean" | "xs:boolean" | "xsd:boolean" => "xsd:boolean",
            "dateTime" | "xs:dateTime" | "xsd:dateTime" => "xsd:dateTime",
            "date" | "xs:date" | "xsd:date" => "xsd:date",
            "time" | "xs:time" | "xsd:time" => "xsd:dateTime",
            "gYear" | "xs:gYear" | "xsd:gYear" => "xsd:gYear",
            "gYearMonth" | "xs:gYearMonth" | "xsd:gYearMonth" => "xsd:gYearMonth",
            "base64Binary" | "xs:base64Binary" | "xsd:base64Binary" => "xsd:string",
            "hexBinary" | "xs:hexBinary" | "xsd:hexBinary" => "xsd:string",
            // User-defined type - convert to PascalCase using heck
            other => return Ok(other.to_pascal_case()),
        };

        Ok(mapped.to_string())
    }
}

impl Default for XsdToSchemaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

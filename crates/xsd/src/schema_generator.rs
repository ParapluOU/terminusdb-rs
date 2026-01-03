//! Runtime TerminusDB Schema Generator from XSD

use crate::schema_model::{Cardinality, ChildElement, Restriction, SimpleTypeVariety, XsdAttribute, XsdComplexType, XsdSchema, XsdSimpleType};
use crate::Result;
use heck::ToPascalCase;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use terminusdb_schema::{Context, Key, Property, Schema, SetCardinality, TypeFamily};

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

        // Use entry point elements (inferred from file name) to determine document roots.
        // Only these should be non-subdocuments. Other global elements are just reusable
        // building blocks that should be embedded as subdocuments.
        //
        // IMPORTANT: We need to use the TYPE name of the element, not the element name.
        // For example: <xs:element name="payment" type="ch:paymentType"/>
        // - Element name: "payment" -> would give "Payment"
        // - Type name: "paymentType" -> gives "PaymentType" (correct!)
        let document_root_types: std::collections::HashSet<String> = {
            use heck::ToPascalCase;

            // First, try to find matches from entry_point_elements
            let matched_types: std::collections::HashSet<String> = xsd_schema.entry_point_elements
                .iter()
                .filter_map(|elem_name| {
                    // Find the root element by name (case-insensitive match)
                    let elem_name_lower = elem_name.to_lowercase();
                    xsd_schema.root_elements.iter().find(|e| {
                        // Match on local name part (after } if Clark notation)
                        let local = e.name.split('}').last().unwrap_or(&e.name);
                        local.to_lowercase() == elem_name_lower
                    }).and_then(|elem| {
                        // Get the type name from the element's type_info
                        elem.type_info.as_ref().and_then(|ti| {
                            // Use qualified_name or name if available
                            ti.qualified_name.as_ref().or(ti.name.as_ref()).map(|type_name| {
                                // Extract local part and convert to PascalCase
                                let local = type_name.split('}').last().unwrap_or(type_name);
                                local.to_pascal_case()
                            })
                        })
                    })
                })
                .collect();

            // If no entry_point_elements matched any root elements, fall back to treating
            // ALL root element types as document roots. This handles custom schemas where
            // the file name doesn't match any element name (e.g., choice_types.xsd with
            // elements like "document", "payment", etc.)
            if matched_types.is_empty() && !xsd_schema.root_elements.is_empty() {
                xsd_schema.root_elements.iter()
                    .filter_map(|elem| {
                        elem.type_info.as_ref().and_then(|ti| {
                            ti.qualified_name.as_ref().or(ti.name.as_ref()).map(|type_name| {
                                let local = type_name.split('}').last().unwrap_or(type_name);
                                local.to_pascal_case()
                            })
                        }).or_else(|| {
                            // For anonymous types, use element name as type name
                            let local = elem.name.split('}').last().unwrap_or(&elem.name);
                            Some(local.to_pascal_case())
                        })
                    })
                    .collect()
            } else {
                matched_types
            }
        };

        // Build a map of base type -> derived types for inheritance tracking.
        // In TerminusDB, @subdocument status is inherited, so if a child is a document root,
        // its parent cannot be a subdocument.
        let mut base_to_derived: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for complex_type in &xsd_schema.complex_types {
            if let Some(ref base_type) = complex_type.base_type {
                let (_, base_local_name) = self.parse_clark_notation(base_type);
                let base_class = base_local_name.to_pascal_case();

                let (_, derived_local_name) = self.parse_clark_notation(
                    if complex_type.is_anonymous {
                        complex_type.element_name.as_ref().unwrap_or(&complex_type.name)
                    } else {
                        &complex_type.name
                    }
                );
                let derived_class = derived_local_name.to_pascal_case();

                base_to_derived.entry(base_class).or_default().push(derived_class);
            }
        }

        // Find all types that have document roots in their inheritance tree (direct or transitive).
        // These types cannot be subdocuments because @subdocument is inherited.
        let mut non_subdocument_types: std::collections::HashSet<String> = document_root_types.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for (base, derived_list) in &base_to_derived {
                // If any derived type is non-subdocument, the base must also be non-subdocument
                if derived_list.iter().any(|d| non_subdocument_types.contains(d)) {
                    if non_subdocument_types.insert(base.clone()) {
                        changed = true;
                    }
                }
            }
        }

        // Generate schemas for complex types
        for complex_type in &xsd_schema.complex_types {
            let type_schemas = self.generate_class_from_complex_type_with_context(
                complex_type,
                &non_subdocument_types,
                &xsd_schema.simple_types,
                &xsd_schema.complex_types,
            )?;
            schemas.extend(type_schemas);
        }

        // Generate schemas for simple types (enums, type aliases)
        for simple_type in &xsd_schema.simple_types {
            if let Some(schema) = self.generate_from_simple_type(simple_type)? {
                schemas.push(schema);
            }
        }

        Ok(schemas)
    }

    /// Generate TerminusDB Context and Schemas from XSD.
    ///
    /// This captures the XSD target namespace in the TerminusDB Context,
    /// allowing proper namespace resolution. Class names remain unprefixed;
    /// TerminusDB resolves them using the `@schema` namespace.
    ///
    /// # Returns
    ///
    /// A tuple of (Context, Vec<Schema>) where:
    /// - Context has `@schema` derived from XSD target namespace
    /// - Schemas have unprefixed class names (e.g., "TopicClass")
    pub fn generate_with_context(&self, xsd_schema: &XsdSchema) -> Result<(Context, Vec<Schema>)> {
        let schemas = self.generate(xsd_schema)?;

        // Derive TerminusDB schema namespace from XSD target namespace
        let schema_ns = match &xsd_schema.target_namespace {
            Some(ns) => {
                // Ensure namespace ends with # or / for proper IRI formation
                if ns.ends_with('#') || ns.ends_with('/') {
                    ns.clone()
                } else {
                    format!("{}#", ns)
                }
            }
            None => self.namespace.clone(),
        };

        // Derive base namespace for instance data
        let base_ns = schema_ns
            .replace("#", "/data/")
            .replace("/schema/", "/data/");

        let context = Context {
            schema: schema_ns,
            base: base_ns,
            xsd: Some("http://www.w3.org/2001/XMLSchema#".to_string()),
            documentation: None,
        };

        Ok((context, schemas))
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

    /// Generate TerminusDB schema(s) from an XSD complex type.
    ///
    /// For mixed content types (text + elements interleaved), this generates:
    /// - The main class with a `content` property pointing to MixedContent
    /// - A `{TypeName}Inline` TaggedUnion for allowed inline element types
    /// - A `MixedContent{TypeName}` class with `text` and `subs: List` properties
    ///
    /// For non-mixed types, returns a single schema with individual child properties.
    fn generate_class_from_complex_type_with_context(
        &self,
        complex_type: &XsdComplexType,
        root_element_types: &std::collections::HashSet<String>,
        simple_types: &[XsdSimpleType],
        complex_types: &[XsdComplexType],
    ) -> Result<Vec<Schema>> {
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

        let mut schemas = Vec::new();
        let mut properties = Vec::new();

        // Add attribute properties (common to both mixed and non-mixed)
        if let Some(ref attributes) = complex_type.attributes {
            for attr in attributes {
                properties.push(self.attribute_to_property(attr)?);
            }
        }

        // Check if this is a mixed content type with child elements
        let use_mixed_content = self.should_use_mixed_content(complex_type);

        // Check if base type also has mixed content - in that case, we inherit `content` property
        // rather than defining our own (which would cause diamond property violation in TerminusDB)
        let base_has_mixed_content = if let Some(ref base_type) = complex_type.base_type {
            let (_, base_local_name) = self.parse_clark_notation(base_type);

            // First, check if the base type is actually in our complex types and is mixed
            let base_is_mixed = complex_types.iter().any(|ct| {
                // Match by name (with or without namespace)
                let (_, ct_local) = self.parse_clark_notation(&ct.name);
                (ct_local == base_local_name || ct.name == *base_type) && ct.mixed
            });

            // Fallback: also check name patterns for DITA compatibility
            base_is_mixed || base_type.ends_with(".class") || base_type.ends_with("Class")
        } else {
            false
        };

        if use_mixed_content && !base_has_mixed_content {
            // Mixed content ON BASE CLASS: generate MixedContent structure
            let child_elements = complex_type.child_elements.as_ref().unwrap();

            let (inline_union, mixed_content_class, mixed_content_name) =
                self.generate_mixed_content_schemas(
                    &class_id,
                    namespace.clone(),
                    child_elements,
                    simple_types,
                )?;

            // Add the supporting schemas
            schemas.push(inline_union);
            schemas.push(mixed_content_class);

            // Add a single `content` property instead of individual child properties
            properties.push(Property {
                name: "content".to_string(),
                r#type: Some(TypeFamily::Optional), // May be empty element
                class: mixed_content_name,
            });
        } else if use_mixed_content && base_has_mixed_content {
            // Mixed content ON DERIVED CLASS: inherit `content` from base, don't add property
            // This avoids diamond property violation in TerminusDB
        } else {
            // Non-mixed: generate individual properties for child elements
            if let Some(ref child_elements) = complex_type.child_elements {
                for element in child_elements {
                    properties.push(self.child_element_to_property(element, simple_types)?);
                }
            }

            // Add _text property for simple content or mixed content without child elements
            // - has_simple_content: explicitly simple content (text only)
            // - mixed with no children: can contain text but no inline elements
            let needs_text_property = complex_type.has_simple_content
                || (complex_type.mixed && complex_type.child_elements.as_ref().map_or(true, |c| c.is_empty()));

            if needs_text_property {
                // Use the base type's underlying XSD type if it's a simple type
                let text_type = if let Some(ref base_type) = complex_type.base_type {
                    let (_, base_local_name) = self.parse_clark_notation(base_type);
                    let base_class = base_local_name.to_pascal_case();

                    // Find the simple type and get its base XSD type
                    simple_types
                        .iter()
                        .find(|st| {
                            let st_local = self.parse_clark_notation(&st.qualified_name).1;
                            st_local.to_pascal_case() == base_class
                        })
                        .and_then(|st| st.base_type.clone())
                        .and_then(|bt| self.map_xsd_type_to_tdb_class(&bt).ok())
                        .unwrap_or_else(|| "xsd:string".to_string())
                } else {
                    "xsd:string".to_string()
                };

                properties.push(Property {
                    name: "_text".to_string(),
                    r#type: Some(TypeFamily::Optional),
                    class: text_type,
                });
            }
        }

        // Determine subdocument status:
        // - Document roots (entry points like "Topic") are NOT subdocuments - they're top-level documents
        // - All other types ARE subdocuments - they're embedded within their parent documents
        //
        // This is critical for TerminusDB property typing:
        // - If a property type is a non-subdocument, TerminusDB expects a REFERENCE (ID string)
        // - If a property type is a subdocument, TerminusDB expects an EMBEDDED object
        //
        // In XSD, named types (like title.class) define structure but are never instantiated directly.
        // Their anonymous extensions (like the title element type) are what actually get used.
        // Since properties are typed as the base types (TitleClass), those base types must also
        // be subdocuments to allow embedding instances of derived types (Title).
        let subdocument = !root_element_types.contains(&class_id);

        // Key strategy:
        // - Documents (non-subdocuments) use ValueHash for content-based addressing
        // - Subdocuments should NOT use ValueHash as it causes TerminusDB to fail
        //   on instance insertion (returns "Unexpected failure in request handler")
        let key = if subdocument {
            Key::Random
        } else {
            Key::ValueHash
        };

        // Extract inheritance from XSD base_type (for extension/restriction)
        // IMPORTANT: When a complex type has simpleContent extending a simple type,
        // we should NOT create class inheritance because simple types don't become
        // TerminusDB classes. Instead, the _text property captures the value.
        let inherits = if let Some(ref base_type) = complex_type.base_type {
            let (base_ns, base_local_name) = self.parse_clark_notation(base_type);
            let base_class = base_local_name.to_pascal_case();

            // Check if base is an XSD built-in primitive type (xs:string, xs:integer, etc.)
            // Note: parse_clark_notation adds a # suffix, so check for both variants
            let base_is_xsd_primitive = base_ns
                .as_ref()
                .map(|ns| {
                    ns == "http://www.w3.org/2001/XMLSchema"
                        || ns == "http://www.w3.org/2001/XMLSchema#"
                })
                .unwrap_or(false);

            // Check if this is simpleContent extending a simple type
            let base_is_simple_type = complex_type.has_simple_content
                && (base_is_xsd_primitive
                    || simple_types.iter().any(|st| {
                        let st_local = self.parse_clark_notation(&st.qualified_name).1;
                        st_local.to_pascal_case() == base_class
                    }));

            if base_is_simple_type {
                // Simple type bases don't become class inheritance
                vec![]
            } else {
                vec![base_class]
            }
        } else {
            vec![]
        };

        // Add the main class schema
        // Note: We set `base` from XSD namespace to enable multi-namespace support.
        // When the namespace is set, Instance serialization produces fully-qualified
        // @type: "http://xsd-namespace#ClassName" which matches the expanded schema URI
        // when the schema is inserted with Context { schema: "http://xsd-namespace#" }.
        // This allows multiple XSD schemas with the same class names to coexist.
        schemas.push(Schema::Class {
            id: class_id,
            base: namespace.clone(),
            key,
            documentation: None,
            subdocument,
            r#abstract: false,
            inherits,
            unfoldable: false,
            properties,
        });

        Ok(schemas)
    }

    /// Generate a TerminusDB schema from an XSD simple type.
    ///
    /// - Simple types with enumeration restrictions become Schema::Enum.
    /// - Union types (xs:union) become Schema::TaggedUnion.
    /// - Other simple types (type aliases, pattern restrictions) are skipped
    ///   because they map to primitive xsd: types or string patterns.
    ///
    /// Returns None if the simple type doesn't need a schema definition.
    fn generate_from_simple_type(&self, simple_type: &XsdSimpleType) -> Result<Option<Schema>> {
        // Extract namespace and local name from Clark notation
        let (namespace, local_name) = self.parse_clark_notation(&simple_type.name);

        // Convert to PascalCase for TerminusDB naming
        let type_id = local_name.to_pascal_case();

        // Check for union types (xs:union memberTypes="...")
        if simple_type.variety == Some(SimpleTypeVariety::Union) {
            if let Some(ref member_types) = simple_type.member_types {
                let properties: Vec<Property> = member_types
                    .iter()
                    .map(|member_type| {
                        // Map the member type to a TerminusDB class
                        let class = self.map_xsd_type_to_tdb_class(member_type)
                            .unwrap_or_else(|_| "xsd:anySimpleType".to_string());

                        // Create tag name from the class (lowercase, strip xsd: prefix)
                        let tag_name = if class.starts_with("xsd:") {
                            class.strip_prefix("xsd:").unwrap().to_string()
                        } else {
                            class.chars().next().unwrap().to_lowercase().to_string()
                                + &class[1..]
                        };

                        Property {
                            name: tag_name,
                            r#type: None, // TaggedUnion variants are mutually exclusive, not optional
                            class,
                        }
                    })
                    .collect();

                // TaggedUnion for XSD union - subdocument with Random key
                return Ok(Some(Schema::TaggedUnion {
                    id: type_id,
                    base: namespace.clone(), // Use XSD namespace for multi-namespace support
                    key: Key::Random,
                    r#abstract: false,
                    documentation: None,
                    subdocument: true, // Union values are embedded
                    properties,
                    unfoldable: false,
                }));
            }
        }

        // Check if this simple type has enumeration restrictions
        if let Some(ref restrictions) = simple_type.restrictions {
            for restriction in restrictions {
                if let Restriction::Enumeration { values } = restriction {
                    // Generate Schema::Enum for enumeration types
                    // Note: Schema::Enum doesn't support base/namespace yet (TODO in schema crate)
                    return Ok(Some(Schema::Enum {
                        id: type_id,
                        documentation: None,
                        values: values.clone(),
                    }));
                }
            }
        }

        // Simple types without enumeration or union don't need separate schema definitions
        // They map to xsd: primitives (string, integer, etc.) or are just restricted
        // string types that TerminusDB represents as xsd:string
        Ok(None)
    }

    /// Parse Clark notation to extract namespace and local name.
    ///
    /// Clark notation format: `{http://example.com/ns}localName`
    /// Returns: (Some(namespace_with_hash), localName) or (None, fullName)
    ///
    /// The namespace is returned with a `#` suffix for TerminusDB URI compatibility.
    /// For example: `{http://example.com/book}Doc` returns `Some("http://example.com/book#")`.
    fn parse_clark_notation(&self, name: &str) -> (Option<String>, String) {
        if let Some(start) = name.find('{') {
            if let Some(end) = name.find('}') {
                let mut namespace = name[start + 1..end].to_string();
                // Add # suffix for TerminusDB URI format if not already present
                if !namespace.ends_with('#') && !namespace.ends_with('/') {
                    namespace.push('#');
                }
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

    fn child_element_to_property(
        &self,
        element: &ChildElement,
        simple_types: &[XsdSimpleType],
    ) -> Result<Property> {
        // Check if the element's type is a list simple type
        let is_list_type = self.is_list_simple_type(&element.element_type, simple_types);

        // For list types, use TypeFamily::List regardless of cardinality
        // xs:list types represent space-separated values in a single element,
        // not multiple XML elements (which would use Set based on maxOccurs)
        let r#type = if is_list_type {
            Some(TypeFamily::List)
        } else {
            let min = element.min_occurs.unwrap_or(1);
            let max = element.max_occurs.clone();

            match (min, &max) {
                // minOccurs=0, maxOccurs=unbounded -> Set (can have 0 or more)
                (0, Some(Cardinality::Unbounded)) => Some(TypeFamily::Set(SetCardinality::None)),
                // minOccurs=0, maxOccurs=1 -> Optional (can have 0 or 1)
                (0, Some(Cardinality::Number(1))) | (0, None) => Some(TypeFamily::Optional),
                // minOccurs=1, maxOccurs=unbounded -> Set (must have at least 1)
                (1, Some(Cardinality::Unbounded)) => Some(TypeFamily::Set(SetCardinality::None)),
                // minOccurs=1, maxOccurs=1 -> Treat as Optional
                // This is a pragmatic choice because XSD content models often have complex
                // optionality from choice/sequence combinations that we don't fully analyze.
                // For example, an element in a choice branch is effectively optional even
                // if it has minOccurs=1, because another branch can be taken instead.
                (1, Some(Cardinality::Number(1))) | (1, None) => Some(TypeFamily::Optional),
                // Other cases with max > 1 -> Set
                _ => match max {
                    Some(Cardinality::Unbounded) => Some(TypeFamily::Set(SetCardinality::None)),
                    Some(Cardinality::Number(n)) if n > 1 => Some(TypeFamily::Set(SetCardinality::None)),
                    _ => Some(TypeFamily::Optional),
                },
            }
        };

        // For list types, we need the item type as the class
        // TODO: Once item_type is extracted from xmlschema-rs, use that here
        // For now, if it's a list type without item_type, we fall back to the type name
        let class = if is_list_type {
            // Try to get item_type from the simple type
            if let Some(st) = simple_types.iter().find(|st| {
                st.name == element.element_type || st.qualified_name == element.element_type
            }) {
                st.item_type.clone().unwrap_or_else(|| {
                    // Fallback: use base_type if available, otherwise xsd:anySimpleType
                    st.base_type.clone()
                        .map(|bt| self.map_xsd_type_to_tdb_class(&bt).unwrap_or_else(|_| "xsd:anySimpleType".to_string()))
                        .unwrap_or_else(|| "xsd:anySimpleType".to_string())
                })
            } else {
                self.map_xsd_type_to_tdb_class(&element.element_type)?
            }
        } else {
            // For non-list types, first check if it's a simple type and map to its primitive base
            let (_, type_name) = self.parse_clark_notation(&element.element_type);
            if let Some(st) = simple_types.iter().find(|st| {
                st.name == type_name || st.qualified_name == element.element_type
                || st.name == element.element_type
            }) {
                // This is a simple type - map to its base XSD type
                st.base_type.clone()
                    .map(|bt| self.map_xsd_type_to_tdb_class(&bt).unwrap_or_else(|_| "xsd:string".to_string()))
                    .unwrap_or_else(|| "xsd:string".to_string())
            } else {
                self.map_xsd_type_to_tdb_class(&element.element_type)?
            }
        };

        // Extract local name from Clark notation for property name
        let (_, local_name) = self.parse_clark_notation(&element.name);

        Ok(Property {
            name: local_name,
            r#type,
            class,
        })
    }

    /// Check if a type name refers to a list simple type
    fn is_list_simple_type(&self, type_name: &str, simple_types: &[XsdSimpleType]) -> bool {
        simple_types.iter().any(|st| {
            (st.name == type_name || st.qualified_name == type_name)
                && st.variety == Some(SimpleTypeVariety::List)
        })
    }

    fn map_xsd_type_to_tdb_class(&self, xsd_type: &str) -> Result<String> {
        // Check for unresolved elements - these are errors, not xs:anyType
        if xsd_type.starts_with("UNRESOLVED:") {
            let element_name = xsd_type.strip_prefix("UNRESOLVED:").unwrap_or(xsd_type);
            return Err(crate::XsdError::Parsing(format!(
                "Element '{}' has unresolved type - element declaration not found in schema",
                element_name
            )));
        }

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
            // xs:anyType allows arbitrary content - map to sys:JSON (unconstrained JSON subdocument)
            "anyType" | "xs:anyType" | "xsd:anyType" => "sys:JSON",
            // User-defined type - convert to PascalCase using heck
            other => return Ok(other.to_pascal_case()),
        };

        Ok(mapped.to_string())
    }

    // ========================================================================
    // Mixed Content Support
    // ========================================================================

    /// Generate schemas for mixed content: a TaggedUnion for inline elements and a MixedContent class.
    ///
    /// Mixed content is when an XML element can contain both text and child elements interleaved:
    /// ```xml
    /// <p>The <term>first term</term> and <term>second term</term> are important.</p>
    /// ```
    ///
    /// We model this as:
    /// - `{TypeName}Inline` - TaggedUnion of all allowed child element types
    /// - `MixedContent{TypeName}` - Class with `text: String` and `subs: List({TypeName}Inline)`
    ///
    /// Returns: (inline_union_schema, mixed_content_schema, inline_union_name)
    fn generate_mixed_content_schemas(
        &self,
        type_name: &str,
        namespace: Option<String>,
        child_elements: &[ChildElement],
        _simple_types: &[XsdSimpleType],
    ) -> Result<(Schema, Schema, String)> {
        let inline_union_name = format!("{}Inline", type_name);
        let mixed_content_name = format!("MixedContent{}", type_name);

        // Generate TaggedUnion properties from child elements
        let union_properties: Vec<Property> = child_elements
            .iter()
            .filter_map(|element| {
                // Extract local name from Clark notation
                let (_, local_name) = self.parse_clark_notation(&element.name);

                // Get the class for this element type
                let class = self.map_xsd_type_to_tdb_class(&element.element_type).ok()?;

                Some(Property {
                    name: local_name,
                    r#type: None, // TaggedUnion variants are mutually exclusive
                    class,
                })
            })
            .collect();

        // Create the inline union TaggedUnion
        let inline_union = Schema::TaggedUnion {
            id: inline_union_name.clone(),
            base: namespace.clone(), // Use XSD namespace for multi-namespace support
            key: Key::Random,
            r#abstract: false,
            documentation: None,
            subdocument: true,
            properties: union_properties,
            unfoldable: false,
        };

        // Create the MixedContent class with text and subs
        let mixed_content = Schema::Class {
            id: mixed_content_name.clone(),
            base: namespace.clone(), // Use XSD namespace for multi-namespace support
            key: Key::Random,
            documentation: None,
            subdocument: true,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "text".to_string(),
                    r#type: None, // Required
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "subs".to_string(),
                    r#type: Some(TypeFamily::List), // List preserves order
                    class: inline_union_name.clone(),
                },
            ],
        };

        Ok((inline_union, mixed_content, mixed_content_name))
    }

    /// Check if a complex type should use mixed content handling.
    ///
    /// Mixed content is used when:
    /// 1. The type is marked as mixed (complex_type.mixed == true)
    /// 2. The type has child elements
    fn should_use_mixed_content(&self, complex_type: &XsdComplexType) -> bool {
        complex_type.mixed
            && complex_type.child_elements.as_ref().map_or(false, |children| !children.is_empty())
    }
}

impl Default for XsdToSchemaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

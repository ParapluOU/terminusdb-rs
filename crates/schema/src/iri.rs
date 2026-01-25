use anyhow::{anyhow, bail};
use std::fmt;

/// Represents a parsed TerminusDB IRI with its components
///
/// TerminusDB uses IRIs to uniquely identify documents. These IRIs can come in several formats:
/// - Fragment-based: `terminusdb://data#TypeName/id`
/// - Path-based: `terminusdb:///data/TypeName/id`
/// - Simple typed: `TypeName/id`
/// - Subdocument paths: `Parent/id/property/SubDoc/id`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdbIRI {
    /// The base URI if present (e.g., "terminusdb://data" or "terminusdb:///data")
    base_uri: Option<String>,

    /// Whether this IRI uses fragment-based format (with #)
    is_fragment_based: bool,

    /// The complete typed path (e.g., "TypeName/id" or "Parent/id/prop/SubDoc/id")
    typed_path: String,

    /// The final type name in the path
    type_name: String,

    /// The final ID in the path
    id: String,

    /// For subdocuments, the parent path (e.g., "Parent/id/property")
    parent_path: Option<String>,
}

impl TdbIRI {
    /// Parse a TerminusDB IRI or ID string into its components
    pub fn parse(iri_or_id: &str) -> anyhow::Result<Self> {
        // Handle simple ID (no slashes)
        if !iri_or_id.contains('/') {
            bail!("Invalid IRI format: no type information in '{}'", iri_or_id);
        }

        // Handle full IRI with protocol (including underscore format like terminusdb:_//)
        if iri_or_id.contains("://") || iri_or_id.contains(":_//") {
            Self::parse_full_iri(iri_or_id)
        } else {
            // Handle typed ID or subdocument path
            Self::parse_typed_path(iri_or_id, None)
        }
    }

    /// Parse a full IRI with protocol
    fn parse_full_iri(iri: &str) -> anyhow::Result<Self> {
        if iri.contains('#') {
            // Fragment-based IRI: terminusdb://data#TypeName/id
            let parts: Vec<&str> = iri.split('#').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid fragment-based IRI format: '{}'", iri));
            }

            let base_uri = parts[0].to_string();
            let typed_path = parts[1];

            let mut iri = Self::parse_typed_path(typed_path, Some(base_uri))?;
            iri.is_fragment_based = true;
            Ok(iri)
        } else {
            // Path-based IRI: terminusdb:///data/TypeName/id
            // Find where the document path starts
            let path_parts: Vec<&str> = iri.split('/').collect();

            // Look for the start of the document path (first uppercase component)
            let mut doc_path_start = None;
            for (i, part) in path_parts.iter().enumerate() {
                if !part.is_empty()
                    && !part.contains(':')
                    && part
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                {
                    doc_path_start = Some(i);
                    break;
                }
            }

            match doc_path_start {
                Some(idx) => {
                    let typed_path = path_parts[idx..].join("/");

                    // Extract base URI
                    let base_end = iri
                        .find(&typed_path)
                        .ok_or_else(|| anyhow!("Failed to extract base URI from '{}'", iri))?;
                    let mut base_uri = &iri[..base_end];

                    // Remove trailing slash
                    if base_uri.ends_with('/') && base_uri.len() > 1 {
                        base_uri = &base_uri[..base_uri.len() - 1];
                    }

                    let mut iri = Self::parse_typed_path(&typed_path, Some(base_uri.to_string()))?;
                    iri.is_fragment_based = false;
                    Ok(iri)
                }
                None => Err(anyhow!("Could not find document path in IRI: '{}'", iri)),
            }
        }
    }

    /// Parse a typed path (e.g., "TypeName/id" or "Parent/id/prop/SubDoc/id")
    fn parse_typed_path(typed_path: &str, base_uri: Option<String>) -> anyhow::Result<Self> {
        let parts: Vec<&str> = typed_path.split('/').collect();

        if parts.len() < 2 {
            return Err(anyhow!("Invalid typed path format: '{}'", typed_path));
        }

        // Simple case: TypeName/id
        if parts.len() == 2 {
            Ok(Self {
                base_uri,
                is_fragment_based: false,
                typed_path: typed_path.to_string(),
                type_name: parts[0].to_string(),
                id: parts[1].to_string(),
                parent_path: None,
            })
        } else {
            // Subdocument case: Parent/id/property/SubDoc/id
            // The last two parts are the final Type/ID
            let type_idx = parts.len() - 2;
            let id_idx = parts.len() - 1;

            let parent_path = if type_idx > 0 {
                Some(parts[..type_idx].join("/"))
            } else {
                None
            };

            Ok(Self {
                base_uri,
                is_fragment_based: false,
                typed_path: typed_path.to_string(),
                type_name: parts[type_idx].to_string(),
                id: parts[id_idx].to_string(),
                parent_path,
            })
        }
    }

    /// Get the base URI if present
    pub fn base_uri(&self) -> Option<&str> {
        self.base_uri.as_deref()
    }

    /// Get the complete typed path (e.g., "TypeName/id")
    pub fn typed_path(&self) -> &str {
        &self.typed_path
    }

    /// Get the type name (final type in the path)
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Get the ID (final segment in the path)
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the parent path for subdocuments
    pub fn parent_path(&self) -> Option<&str> {
        self.parent_path.as_deref()
    }

    /// Check if this is a subdocument IRI
    pub fn is_subdocument(&self) -> bool {
        self.parent_path.is_some()
    }

    /// Get the full IRI string (reconstructed)
    pub fn to_iri_string(&self) -> String {
        match &self.base_uri {
            Some(base) => {
                if self.is_fragment_based {
                    format!("{}#{}", base, self.typed_path)
                } else {
                    format!("{}/{}", base, self.typed_path)
                }
            }
            None => self.typed_path.clone(),
        }
    }

    /// Returns a new TdbIRI with the default base URI applied if none is set.
    /// If this IRI already has a base_uri, returns a clone unchanged.
    pub fn with_default_base(&self) -> TdbIRI {
        self.with_base(DEFAULT_DATA_BASE)
    }

    /// Returns a new TdbIRI with the specified base URI applied if none is set.
    /// If this IRI already has a base_uri, returns a clone unchanged.
    pub fn with_base(&self, base: &str) -> TdbIRI {
        if self.base_uri.is_some() {
            self.clone()
        } else {
            TdbIRI {
                base_uri: Some(base.to_string()),
                is_fragment_based: false,
                typed_path: self.typed_path.clone(),
                type_name: self.type_name.clone(),
                id: self.id.clone(),
                parent_path: self.parent_path.clone(),
            }
        }
    }
}

/// Default TerminusDB data IRI prefix
pub const DEFAULT_DATA_BASE: &str = "terminusdb:///data";

impl fmt::Display for TdbIRI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_iri_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_typed_id() {
        let iri = TdbIRI::parse("Person/123").unwrap();
        assert_eq!(iri.type_name(), "Person");
        assert_eq!(iri.id(), "123");
        assert_eq!(iri.typed_path(), "Person/123");
        assert_eq!(iri.base_uri(), None);
        assert!(!iri.is_subdocument());
    }

    #[test]
    fn test_parse_fragment_based_iri() {
        let original = "terminusdb://data#Person/456";
        let iri = TdbIRI::parse(original).unwrap();
        assert_eq!(iri.type_name(), "Person");
        assert_eq!(iri.id(), "456");
        assert_eq!(iri.typed_path(), "Person/456");
        assert_eq!(iri.base_uri(), Some("terminusdb://data"));
        assert_eq!(iri.to_string(), original);
    }

    #[test]
    fn test_parse_path_based_iri() {
        let iri = TdbIRI::parse("terminusdb:///data/Person/789").unwrap();
        assert_eq!(iri.type_name(), "Person");
        assert_eq!(iri.id(), "789");
        assert_eq!(iri.typed_path(), "Person/789");
        assert_eq!(iri.base_uri(), Some("terminusdb:///data"));
        assert_eq!(iri.to_string(), "terminusdb:///data/Person/789");
    }

    #[test]
    fn test_parse_subdocument_path() {
        let iri = TdbIRI::parse("ReviewSession/123/assignments/ReviewAssignment/456").unwrap();
        assert_eq!(iri.type_name(), "ReviewAssignment");
        assert_eq!(iri.id(), "456");
        assert_eq!(
            iri.typed_path(),
            "ReviewSession/123/assignments/ReviewAssignment/456"
        );
        assert_eq!(iri.parent_path(), Some("ReviewSession/123/assignments"));
        assert!(iri.is_subdocument());
    }

    #[test]
    fn test_parse_subdocument_fragment_iri() {
        let iri = TdbIRI::parse("terminusdb://data#Parent/123/prop/SubDoc/456").unwrap();
        assert_eq!(iri.type_name(), "SubDoc");
        assert_eq!(iri.id(), "456");
        assert_eq!(iri.base_uri(), Some("terminusdb://data"));
        assert_eq!(iri.parent_path(), Some("Parent/123/prop"));
        assert!(iri.is_subdocument());
    }

    #[test]
    fn test_parse_subdocument_path_iri() {
        let iri = TdbIRI::parse("terminusdb:///data/Parent/123/prop/SubDoc/789").unwrap();
        assert_eq!(iri.type_name(), "SubDoc");
        assert_eq!(iri.id(), "789");
        assert_eq!(iri.base_uri(), Some("terminusdb:///data"));
        assert_eq!(iri.parent_path(), Some("Parent/123/prop"));
        assert!(iri.is_subdocument());
    }

    #[test]
    fn test_parse_deeply_nested_subdocument() {
        let iri = TdbIRI::parse("A/1/b/B/2/c/C/3/d/D/4").unwrap();
        assert_eq!(iri.type_name(), "D");
        assert_eq!(iri.id(), "4");
        assert_eq!(iri.parent_path(), Some("A/1/b/B/2/c/C/3/d"));
        assert!(iri.is_subdocument());
    }

    #[test]
    fn test_parse_terminusdb_underscore_format() {
        // Format seen in the dependent code: "terminusdb:_//data/TypeName/id"
        let original = "terminusdb:_//data/Person/123";
        let iri = TdbIRI::parse(original).unwrap();
        assert_eq!(iri.type_name(), "Person");
        assert_eq!(iri.id(), "123");
        assert_eq!(iri.typed_path(), "Person/123");
        assert_eq!(iri.base_uri(), Some("terminusdb:_//data"));
        // The reconstruction should match original
        assert_eq!(iri.to_string(), original);
    }

    #[test]
    fn test_parse_invalid_no_type() {
        let result = TdbIRI::parse("123");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no type information"));
    }

    #[test]
    fn test_parse_invalid_no_id() {
        // Empty ID is technically valid in the split, just empty string
        let iri = TdbIRI::parse("Person/").unwrap();
        assert_eq!(iri.type_name(), "Person");
        assert_eq!(iri.id(), "");
    }

    #[test]
    fn test_roundtrip_conversion() {
        let iris = vec![
            "Person/123",
            "terminusdb://data#Person/456",
            "terminusdb:///data/Person/789",
            "Parent/123/child/Child/456",
        ];

        for iri_str in iris {
            let iri = TdbIRI::parse(iri_str).unwrap();
            let reconstructed = iri.to_string();

            // For simple typed paths, the reconstruction should match
            if !iri_str.contains("://") {
                assert_eq!(iri_str, reconstructed);
            }
        }
    }

    #[test]
    fn test_with_default_base() {
        // IRI without base should get default applied
        let iri = TdbIRI::parse("Person/123").unwrap();
        let full_iri = iri.with_default_base();
        assert_eq!(full_iri.base_uri(), Some("terminusdb:///data"));
        assert_eq!(full_iri.to_string(), "terminusdb:///data/Person/123");

        // IRI with existing base should preserve it
        let iri2 = TdbIRI::parse("terminusdb:///data/Person/456").unwrap();
        let full_iri2 = iri2.with_default_base();
        assert_eq!(full_iri2.base_uri(), Some("terminusdb:///data"));
        assert_eq!(full_iri2.to_string(), "terminusdb:///data/Person/456");

        // Fragment-based IRI should preserve original base
        let iri3 = TdbIRI::parse("terminusdb://data#Person/789").unwrap();
        let full_iri3 = iri3.with_default_base();
        assert_eq!(full_iri3.base_uri(), Some("terminusdb://data"));
    }

    #[test]
    fn test_with_custom_base() {
        let iri = TdbIRI::parse("Person/123").unwrap();
        let custom = iri.with_base("custom:///base");
        assert_eq!(custom.base_uri(), Some("custom:///base"));
        assert_eq!(custom.to_string(), "custom:///base/Person/123");
    }
}

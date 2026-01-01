use serde::{Deserialize, Serialize};
use terminusdb_schema::{Schema, ToTDBSchema, ToTDBInstance};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Test case: TaggedUnion marked as subdocument
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true, key = "value_hash")]
pub enum SubdocumentTaggedUnion {
    Simple(String),
    Complex {
        field1: String,
        field2: i32,
    },
    Unit,
}

// Test case: TaggedUnion NOT marked as subdocument
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
pub enum RegularTaggedUnion {
    Simple(String),
    Complex {
        field1: String,
        field2: i32,
    },
    Unit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdocument_propagation_to_variant_structs() {
        // Test subdocument TaggedUnion
        let subdoc_schemas = <SubdocumentTaggedUnion as ToTDBSchema>::to_schema_tree();
        
        // Debug: Print all schemas
        println!("Subdocument schemas:");
        for schema in &subdoc_schemas {
            match schema {
                Schema::Class { id, subdocument, .. } => {
                    println!("  Class: {}, subdocument: {}", id, subdocument);
                }
                Schema::TaggedUnion { id, .. } => {
                    println!("  TaggedUnion: {}", id);
                }
                _ => {}
            }
        }
        
        // Find the generated Complex variant struct
        let subdoc_complex_schema = subdoc_schemas
            .iter()
            .find(|s| matches!(s, Schema::Class { id, .. } if id == "SubdocumentTaggedUnionComplex"))
            .expect("Should find SubdocumentTaggedUnionComplex schema");

        // Verify that the variant struct is marked as subdocument
        if let Schema::Class { subdocument, .. } = subdoc_complex_schema {
            assert!(subdocument, "SubdocumentTaggedUnionComplex should be marked as subdocument");
        } else {
            panic!("Expected Schema::Class for SubdocumentTaggedUnionComplex");
        }

        // Test regular TaggedUnion
        let regular_schemas = <RegularTaggedUnion as ToTDBSchema>::to_schema_tree();
        
        // Find the generated Complex variant struct
        let regular_complex_schema = regular_schemas
            .iter()
            .find(|s| matches!(s, Schema::Class { id, .. } if id == "RegularTaggedUnionComplex"))
            .expect("Should find RegularTaggedUnionComplex schema");

        // Verify that the variant struct is NOT marked as subdocument
        if let Schema::Class { subdocument, .. } = regular_complex_schema {
            assert!(!subdocument, "RegularTaggedUnionComplex should NOT be marked as subdocument");
        } else {
            panic!("Expected Schema::Class for RegularTaggedUnionComplex");
        }
    }

    #[test]
    fn test_all_variant_structs_inherit_subdocument() {
        let schemas = <SubdocumentTaggedUnion as ToTDBSchema>::to_schema_tree();
        
        // Check that the main TaggedUnion is present and marked as subdocument
        let union_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == "SubdocumentTaggedUnion"))
            .expect("Should find SubdocumentTaggedUnion schema");
        
        // Verify the TaggedUnion itself is marked as subdocument
        assert!(union_schema.is_subdocument(), "TaggedUnion itself should be marked as subdocument");

        // All generated variant classes should be subdocuments
        for schema in &schemas {
            match schema {
                Schema::Class { id, subdocument, .. } => {
                    // Skip checking non-variant structs
                    if id.starts_with("SubdocumentTaggedUnion") && id != "SubdocumentTaggedUnion" {
                        assert!(
                            *subdocument,
                            "Variant struct {} should be marked as subdocument",
                            id
                        );
                    }
                }
                _ => {} // Skip non-class schemas
            }
        }
    }
}
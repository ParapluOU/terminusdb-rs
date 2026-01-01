use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_schema::{ClassDocumentation, Key, Schema, ToTDBInstance, ToTDBSchema, TypeFamily};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use terminusdb_schema::json::InstanceFromJson;

    // Basic struct test
    #[derive(TerminusDBModel, Debug, Clone, PartialEq)]
    struct BasicPerson {
        name: String,
        age: i32,
        is_active: bool,
    }

    // Struct with collection types
    #[derive(TerminusDBModel, Debug, Clone)]
    struct PersonWithCollections {
        name: String,
        tags: Vec<String>,
        references: Vec<i32>,
    }

    // Struct with custom attributes
    #[derive(TerminusDBModel, Debug, Clone)]
    #[tdb(
        class_name = "CustomPerson",
        base = "http://example.org/",
        key = "hash",
        doc = "A person with custom attributes"
    )]
    struct PersonWithAttributes {
        #[tdb(name = "fullName", doc = "The person's full name")]
        name: String,
        age: i32,
        #[tdb(name = "emailAddress")]
        email: Option<String>,
    }

    // Simple struct for nesting
    #[derive(TerminusDBModel, Debug, Clone, PartialEq)]
    struct Address {
        street: String,
        city: String,
        country: String,
    }

    // Struct with nested struct (defaults to Link)
    #[derive(TerminusDBModel, Debug, Clone)]
    struct PersonWithLinkAddress {
        name: String,
        #[tdb(subdocument)]
        address: Address,
    }

    // Struct with nested subdocument
    #[derive(TerminusDBModel, Debug, Clone)]
    struct PersonWithSubDocAddress {
        name: String,
        #[tdb(subdocument)]
        address: Address,
    }

    // Struct with Vec<SubDocument>
    #[derive(TerminusDBModel, Debug, Clone)]
    struct Building {
        name: String,
        #[tdb(subdocument)]
        offices: Vec<Address>,
    }

    // Struct with Vec<Link>
    #[derive(TerminusDBModel, Debug, Clone)]
    struct Department {
        name: String,
        locations: Vec<Address>,
    }

    // Test basic ToTDBSchema implementation
    #[test]
    fn test_basic_struct_schema() {
        let schema = <BasicPerson as ToTDBSchema>::to_schema();
        match schema {
            Schema::Class {
                id,
                properties,
                documentation,
                ..
            } => {
                assert_eq!(id, "BasicPerson");
                assert_eq!(properties.len(), 3);

                // Check that the properties match
                let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
                assert_eq!(name_prop.class, "xsd:string");

                let age_prop = properties.iter().find(|p| p.name == "age").unwrap();
                assert_eq!(age_prop.class, "xsd:integer");

                let active_prop = properties.iter().find(|p| p.name == "is_active").unwrap();
                assert_eq!(active_prop.class, "xsd:boolean");

                // Documentation should be None by default
                assert_eq!(documentation, None);
            }
            _ => panic!("Expected a Class schema"),
        }
    }

    #[test]
    fn test_collection_property_types() {
        let schema = <PersonWithCollections as ToTDBSchema>::to_schema();

        if let Schema::Class { properties, .. } = schema {
            // Check collection property types
            let tags_prop = properties.iter().find(|p| p.name == "tags").unwrap();
            assert_eq!(tags_prop.class, "xsd:string");
            assert_eq!(tags_prop.r#type, Some(TypeFamily::List));

            let refs_prop = properties.iter().find(|p| p.name == "references").unwrap();
            assert_eq!(refs_prop.class, "xsd:integer");
            assert_eq!(refs_prop.r#type, Some(TypeFamily::List));
        } else {
            panic!("Expected Schema::Class");
        }
    }

    #[test]
    fn test_struct_with_attributes() {
        let schema = <PersonWithAttributes as ToTDBSchema>::to_schema();

        if let Schema::Class {
            id,
            properties,
            key,
            base,
            documentation,
            subdocument,
            unfoldable,
            ..
        } = schema
        {
            // Check custom class name
            assert_eq!(id, "CustomPerson");

            // Check base URI
            assert_eq!(base, Some("http://example.org/".to_string()));

            // Check key
            assert!(matches!(key, Key::Hash(_)));

            // Check subdocument
            // assert_eq!(subdocument, true);

            // Check documentation
            assert!(documentation.is_some());
            if let Some(doc) = documentation {
                assert_eq!(doc.comment, "A person with custom attributes");
            }

            // Get properties
            // Check custom property names
            let name_prop = properties.iter().find(|p| p.name == "fullName").unwrap();
            assert_eq!(name_prop.class, "xsd:string");

            let email_prop = properties
                .iter()
                .find(|p| p.name == "emailAddress")
                .unwrap();
            assert_eq!(email_prop.class, "xsd:string");

            // Default unfoldable is false
            assert_eq!(unfoldable, false);
        } else {
            panic!("Expected Schema::Class");
        }
    }

    #[test]
    fn test_nested_struct_schema_tree() {
        let schemas = <PersonWithLinkAddress as ToTDBSchema>::to_schema_tree();

        // Should include schemas for PersonWithLinkAddress and Address
        assert_eq!(schemas.len(), 2);

        // Find the main class schema
        let person_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "PersonWithLinkAddress"
                } else {
                    false
                }
            })
            .unwrap();

        // Find the address schema
        let address_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "Address"
                } else {
                    false
                }
            })
            .unwrap();

        // Check that the address property references the Address class
        if let Schema::Class { properties, .. } = person_schema {
            let address_prop = properties.iter().find(|p| p.name == "address").unwrap();
            assert_eq!(address_prop.class, "Address");
        } else {
            panic!("Expected Schema::Class for PersonWithLinkAddress");
        }

        // Check the address schema has the right properties
        if let Schema::Class { properties, .. } = address_schema {
            assert_eq!(properties.len(), 3);

            let prop_names: Vec<&str> = properties.iter().map(|p| p.name.as_str()).collect();
            assert!(prop_names.contains(&"street"));
            assert!(prop_names.contains(&"city"));
            assert!(prop_names.contains(&"country"));
        } else {
            panic!("Expected Schema::Class for Address");
        }
    }
}

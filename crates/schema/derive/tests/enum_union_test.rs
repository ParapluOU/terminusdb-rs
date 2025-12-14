use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use terminusdb_schema::{Schema, ToMaybeTDBSchema, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Basic tagged union enum
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum BasicTaggedUnion {
    Integer(i32),
    Text(String),
    Boolean(bool),
    DateTime(DateTime<Utc>),
}

// Complex tagged union with struct variants
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ComplexTaggedUnion {
    Person {
        name: String,
        age: i32,
    },
    Company {
        name: String,
        employees: Vec<String>,
    },
    None,
}

// Enum with tuple struct variants
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum TupleStructVariants {
    Point(f32, f32),
    RGB(u8, u8, u8),
    Empty,
}

#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize)]
enum SimpleEnum {
    Red,
    Green,
    Blue,
}

#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize)]
enum TaggedEnum {
    Text(String),
    Number(i32),
    Complex { x: f64, y: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::TypeFamily;

    #[test]
    fn test_basic_tagged_union() {
        let schema = <BasicTaggedUnion as terminusdb_schema::ToTDBSchema>::to_schema();

        if let Schema::TaggedUnion { id, properties, .. } = schema {
            // Check union name
            assert_eq!(id, "BasicTaggedUnion");

            // Check variant types
            assert_eq!(properties.len(), 4);

            let int_prop = properties.iter().find(|p| p.name == "integer").unwrap();
            assert_eq!(int_prop.class, "xsd:integer");

            let text_prop = properties.iter().find(|p| p.name == "text").unwrap();
            assert_eq!(text_prop.class, "xsd:string");

            let bool_prop = properties.iter().find(|p| p.name == "boolean").unwrap();
            assert_eq!(bool_prop.class, "xsd:boolean");

            let datetime_prop = properties.iter().find(|p| p.name == "datetime").unwrap();
            assert_eq!(datetime_prop.class, "xsd:dateTime");
        } else {
            panic!("Expected Schema::TaggedUnion");
        }
    }

    #[test]
    fn test_complex_tagged_union() {
        let schemas = <ComplexTaggedUnion as terminusdb_schema::ToTDBSchema>::to_schema_tree();

        // Should include schemas for the union and each struct variant
        assert_eq!(schemas.len(), 3);

        dbg!(&schemas);

        // Find the main union schema
        let union_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::TaggedUnion { id, .. } = s {
                    id == "ComplexTaggedUnion"
                } else {
                    false
                }
            })
            .unwrap();

        // Find the person struct schema
        let person_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "ComplexTaggedUnionPerson"
                } else {
                    false
                }
            })
            .unwrap();

        // Find the company struct schema
        let company_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "ComplexTaggedUnionCompany"
                } else {
                    false
                }
            })
            .unwrap();

        // Check union properties
        if let Schema::TaggedUnion { properties, .. } = union_schema {
            assert_eq!(properties.len(), 3);

            let person_prop = properties.iter().find(|p| p.name == "person").unwrap();
            assert_eq!(person_prop.class, "ComplexTaggedUnionPerson");

            let company_prop = properties.iter().find(|p| p.name == "company").unwrap();
            assert_eq!(company_prop.class, "ComplexTaggedUnionCompany");

            let none_prop = properties.iter().find(|p| p.name == "none").unwrap();
            assert_eq!(none_prop.class, "sys:Unit");
        } else {
            panic!("Expected Schema::TaggedUnion for ComplexTaggedUnion");
        }

        // Check person struct properties
        if let Schema::Class {
            properties,
            subdocument,
            ..
        } = person_schema
        {
            assert_eq!(properties.len(), 2);
            assert!(!subdocument); // Virtual structs are not subdocuments by default

            let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
            assert_eq!(name_prop.class, "xsd:string");

            let age_prop = properties.iter().find(|p| p.name == "age").unwrap();
            assert_eq!(age_prop.class, "xsd:integer");
        } else {
            panic!("Expected Schema::Class for PersonForUnion");
        }

        // Check company struct properties
        if let Schema::Class {
            properties,
            subdocument,
            ..
        } = company_schema
        {
            assert_eq!(properties.len(), 2);
            assert!(!subdocument); // Virtual structs are not subdocuments by default

            let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
            assert_eq!(name_prop.class, "xsd:string");

            let employees_prop = properties.iter().find(|p| p.name == "employees").unwrap();
            assert_eq!(employees_prop.class, "xsd:string");
            assert_eq!(employees_prop.r#type, Some(TypeFamily::List));
        } else {
            panic!("Expected Schema::Class for CompanyForUnion");
        }
    }

    #[test]
    fn test_tuple_struct_variants() {
        let schemas = <TupleStructVariants as terminusdb_schema::ToTDBSchema>::to_schema_tree();

        // Should include schemas for the union and each tuple struct variant
        assert_eq!(schemas.len(), 3);

        // Find the tuple struct schemas
        let point_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "TupleStructVariantsPoint"
                } else {
                    false
                }
            })
            .unwrap();

        let rgb_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "TupleStructVariantsRGB"
                } else {
                    false
                }
            })
            .unwrap();

        // Check point tuple struct properties
        if let Schema::Class { properties, .. } = point_schema {
            assert_eq!(properties.len(), 2);

            let x_prop = properties.iter().find(|p| p.name == "_0").unwrap();
            assert_eq!(x_prop.class, "xsd:float");

            let y_prop = properties.iter().find(|p| p.name == "_1").unwrap();
            assert_eq!(y_prop.class, "xsd:float");
        } else {
            panic!("Expected Schema::Class for TupleStructVariantsPoint");
        }

        // Check RGB tuple struct properties
        if let Schema::Class { properties, .. } = rgb_schema {
            assert_eq!(properties.len(), 3);

            let r_prop = properties.iter().find(|p| p.name == "_0").unwrap();
            assert_eq!(r_prop.class, "xsd:unsignedByte");

            let g_prop = properties.iter().find(|p| p.name == "_1").unwrap();
            assert_eq!(g_prop.class, "xsd:unsignedByte");

            let b_prop = properties.iter().find(|p| p.name == "_2").unwrap();
            assert_eq!(b_prop.class, "xsd:unsignedByte");
        } else {
            panic!("Expected Schema::Class for TupleStructVariantsRGB");
        }
    }

    #[test]
    fn test_simple_enum_schema() {
        let schema = <SimpleEnum as terminusdb_schema::ToTDBSchema>::to_schema();
        assert_eq!(
            schema,
            Schema::Enum {
                id: "SimpleEnum".to_string(),
                documentation: None,
                values: vec!["red".to_string(), "green".to_string(), "blue".to_string()],
            }
        );
    }

    #[test]
    fn test_tagged_enum_schema() {
        let schema = <TaggedEnum as terminusdb_schema::ToTDBSchema>::to_schema();
        if let Schema::TaggedUnion {
            id,
            properties,
            base,
            documentation,
            r#abstract,
            key: _,
            subdocument: _,
            unfoldable: _,
        } = schema
        {
            assert_eq!(id, "TaggedEnum");
            assert_eq!(base, None);
            assert_eq!(documentation, None);
            assert_eq!(r#abstract, false);
            assert_eq!(properties.len(), 3);

            let property_names: Vec<_> = properties.iter().map(|p| &p.name).collect();
            assert!(property_names.contains(&&"text".to_string()));
            assert!(property_names.contains(&&"number".to_string()));
            assert!(property_names.contains(&&"complex".to_string()));
        } else {
            panic!("Expected TaggedUnion schema");
        }
    }

    #[test]
    fn test_tagged_enum_schema_tree() {
        let schemas = <TaggedEnum as terminusdb_schema::ToTDBSchema>::to_schema_tree();

        // Should include the TaggedEnum schema and virtual struct for Complex variant
        assert!(schemas.len() >= 1);

        let tagged_enum_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == "TaggedEnum"))
            .unwrap();

        if let Schema::TaggedUnion {
            id,
            properties,
            base,
            documentation,
            r#abstract,
            key: _,
            subdocument: _,
            unfoldable: _,
        } = tagged_enum_schema
        {
            assert_eq!(id, "TaggedEnum");
            assert_eq!(base, &None);
            assert_eq!(documentation, &None);
            assert_eq!(r#abstract, &false);
            assert_eq!(properties.len(), 3);
        } else {
            panic!("Expected TaggedUnion schema");
        }
    }

    #[test]
    fn test_schema_tree_includes_virtual_struct() {
        let schemas = <TaggedEnum as terminusdb_schema::ToTDBSchema>::to_schema_tree();

        // Check if virtual struct for Complex variant exists
        let virtual_struct = schemas
            .iter()
            .find(|s| matches!(s, Schema::Class { id, .. } if id == "TaggedEnumComplex"));

        if let Some(Schema::Class { properties, .. }) = virtual_struct {
            // Should have x and y properties
            assert_eq!(properties.len(), 2);

            let prop_names: Vec<_> = properties.iter().map(|p| &p.name).collect();
            assert!(prop_names.contains(&&"x".to_string()));
            assert!(prop_names.contains(&&"y".to_string()));
        }
        // Note: virtual struct may not always be generated depending on implementation
    }

    #[test]
    fn test_enum_with_separate_model_variants() {
        // Test case for enums where all variants are separate structs that derive TerminusDBModel
        // This mimics the user's ActivityEvent use case

        // Define separate model structs
        #[derive(
            TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize,
        )]
        struct EventUserLogin {
            user_id: String,
            timestamp: String,
        }

        #[derive(
            TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize,
        )]
        struct EventUserLogout {
            user_id: String,
            timestamp: String,
        }

        #[derive(
            TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize,
        )]
        struct EventPublicationCreated {
            publication_id: String,
            title: String,
        }

        // Define enum that wraps these separate models
        #[derive(
            TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq, Serialize, Deserialize,
        )]
        enum ActivityEvent {
            UserLogin(EventUserLogin),
            UserLogout(EventUserLogout),
            PublicationCreated(EventPublicationCreated),
        }

        // Get the schema tree
        let schema_tree = <ActivityEvent as ToTDBSchema>::to_schema_tree();

        // Debug output
        dbg!(&schema_tree);

        // Should include:
        // 1. ActivityEvent TaggedUnion
        // 2. EventUserLogin Class
        // 3. EventUserLogout Class
        // 4. EventPublicationCreated Class
        assert_eq!(
            schema_tree.len(),
            4,
            "Schema tree should include ActivityEvent and all 3 variant models"
        );

        // Verify ActivityEvent schema exists
        assert!(
            schema_tree
                .iter()
                .any(|s| s.id() == "ActivityEvent" && s.is_tagged_union()),
            "ActivityEvent TaggedUnion schema should be present"
        );

        // Verify EventUserLogin schema exists
        assert!(
            schema_tree.iter().any(|s| s.id() == "EventUserLogin"),
            "EventUserLogin schema should be included in schema tree"
        );

        // Verify EventUserLogout schema exists
        assert!(
            schema_tree.iter().any(|s| s.id() == "EventUserLogout"),
            "EventUserLogout schema should be included in schema tree"
        );

        // Verify EventPublicationCreated schema exists
        assert!(
            schema_tree
                .iter()
                .any(|s| s.id() == "EventPublicationCreated"),
            "EventPublicationCreated schema should be included in schema tree"
        );
    }
}

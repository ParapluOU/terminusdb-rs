//! Conversion from terminusdb-schema Schema types to terminusdb-community AllFrames.
//!
//! This module provides the bridge between our Rust schema definitions and
//! TerminusDB's internal frame representation used for GraphQL generation.

use serde_json::{json, Map, Value};
use terminusdb_schema::{Schema, ToTDBSchemas};

use terminusdb_community::graphql::frame::{AllFrames, UncleanAllFrames};

/// Convert schemas from ToTDBSchemas to TerminusDB's AllFrames structure.
///
/// This function takes our Rust schema definitions and converts them to the
/// AllFrames format that terminusdb-community uses for GraphQL schema generation.
///
/// # Example
///
/// ```ignore
/// use terminusdb_gql::schemas_to_allframes;
///
/// // Convert a tuple of model types to AllFrames
/// let frames = schemas_to_allframes::<(Project, Ticket, Milestone)>();
/// ```
pub fn schemas_to_allframes<T: ToTDBSchemas>() -> AllFrames {
    let schemas = T::to_schemas();
    schemas_vec_to_allframes(&schemas)
}

/// Convert a vector of Schema definitions to AllFrames.
pub fn schemas_vec_to_allframes(schemas: &[Schema]) -> AllFrames {
    // Build the JSON structure that UncleanAllFrames expects
    let json_value = schemas_to_frames_json(schemas);

    // Deserialize as UncleanAllFrames
    let unclean: UncleanAllFrames = serde_json::from_value(json_value)
        .expect("Failed to deserialize schemas as UncleanAllFrames");

    // Finalize to get AllFrames with all the computed indices
    unclean.finalize()
}

/// Convert schemas to the JSON format expected by UncleanAllFrames.
///
/// The format is:
/// ```json
/// {
///   "@context": {
///     "@type": "Context",
///     "@base": "terminusdb:///data/",
///     "@schema": "terminusdb:///schema#"
///   },
///   "ClassName": {
///     "@type": "Class",
///     "fieldName": "xsd:string",
///     ...
///   },
///   "EnumName": {
///     "@type": "Enum",
///     "@values": ["Value1", "Value2"]
///   }
/// }
/// ```
fn schemas_to_frames_json(schemas: &[Schema]) -> Value {
    let mut root = Map::new();

    // Build the @context
    let context = json!({
        "@type": "Context",
        "@base": "terminusdb:///data/",
        "@schema": "terminusdb:///schema#"
    });
    root.insert("@context".to_string(), context);

    // Add each schema as a type definition
    for schema in schemas {
        let class_name = schema.class_name().clone();
        let type_def = schema_to_type_definition(schema);
        root.insert(class_name, type_def);
    }

    Value::Object(root)
}

/// Convert a single Schema to the type definition format expected by UncleanAllFrames.
fn schema_to_type_definition(schema: &Schema) -> Value {
    match schema {
        Schema::Class {
            key,
            documentation,
            subdocument,
            r#abstract,
            inherits,
            properties,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("@type".to_string(), json!("Class"));

            // Add key if not Random (default)
            if let Some(key_json) = key_to_json(key) {
                obj.insert("@key".to_string(), key_json);
            }

            // Add subdocument flag
            if *subdocument {
                obj.insert("@subdocument".to_string(), json!([]));
            }

            // Add abstract flag
            if *r#abstract {
                obj.insert("@abstract".to_string(), json!([]));
            }

            // Add inherits
            if !inherits.is_empty() {
                obj.insert("@inherits".to_string(), json!(inherits));
            }

            // Add documentation
            if let Some(doc) = documentation {
                obj.insert("@documentation".to_string(), documentation_to_json(doc));
            }

            // Add properties
            for prop in properties {
                obj.insert(prop.name.clone(), property_to_json(prop));
            }

            Value::Object(obj)
        }

        Schema::Enum {
            values,
            documentation,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("@type".to_string(), json!("Enum"));
            obj.insert("@values".to_string(), json!(values));

            if let Some(doc) = documentation {
                obj.insert("@documentation".to_string(), documentation_to_json(doc));
            }

            Value::Object(obj)
        }

        Schema::TaggedUnion {
            key,
            documentation,
            subdocument,
            properties,
            r#abstract,
            ..
        } => {
            // TaggedUnion is represented as a Class with @oneOf in TerminusDB frames
            let mut obj = Map::new();
            obj.insert("@type".to_string(), json!("Class"));

            if let Some(key_json) = key_to_json(key) {
                obj.insert("@key".to_string(), key_json);
            }

            if *subdocument {
                obj.insert("@subdocument".to_string(), json!([]));
            }

            if *r#abstract {
                obj.insert("@abstract".to_string(), json!([]));
            }

            if let Some(doc) = documentation {
                obj.insert("@documentation".to_string(), documentation_to_json(doc));
            }

            // For TaggedUnion, all properties are part of a @oneOf
            let mut one_of_obj = Map::new();
            for prop in properties {
                one_of_obj.insert(prop.name.clone(), property_to_json(prop));
            }
            obj.insert("@oneOf".to_string(), json!([one_of_obj]));

            Value::Object(obj)
        }

        Schema::OneOfClass {
            documentation,
            subdocument,
            r#abstract,
            inherits,
            classes,
            properties,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("@type".to_string(), json!("Class"));

            if *subdocument {
                obj.insert("@subdocument".to_string(), json!([]));
            }

            if *r#abstract {
                obj.insert("@abstract".to_string(), json!([]));
            }

            if !inherits.is_empty() {
                obj.insert("@inherits".to_string(), json!(inherits));
            }

            if let Some(doc) = documentation {
                obj.insert("@documentation".to_string(), documentation_to_json(doc));
            }

            // Add regular properties
            for prop in properties {
                obj.insert(prop.name.clone(), property_to_json(prop));
            }

            // Add @oneOf with the class choices
            let one_of_array: Vec<Value> = classes
                .iter()
                .map(|prop_set| {
                    let mut choice = Map::new();
                    for prop in prop_set {
                        choice.insert(prop.name.clone(), property_to_json(prop));
                    }
                    Value::Object(choice)
                })
                .collect();
            obj.insert("@oneOf".to_string(), Value::Array(one_of_array));

            Value::Object(obj)
        }
    }
}

/// Convert a Key to JSON format.
fn key_to_json(key: &terminusdb_schema::Key) -> Option<Value> {
    use terminusdb_schema::Key;

    match key {
        Key::Random => Some(json!({"@type": "Random"})),
        Key::Lexical(fields) => Some(json!({
            "@type": "Lexical",
            "@fields": fields
        })),
        Key::Hash(fields) => Some(json!({
            "@type": "Hash",
            "@fields": fields
        })),
        Key::ValueHash => Some(json!({"@type": "ValueHash"})),
    }
}

/// Convert ClassDocumentation to JSON format.
fn documentation_to_json(doc: &terminusdb_schema::ClassDocumentation) -> Value {
    let mut obj = Map::new();

    if !doc.comment.is_empty() {
        obj.insert("@comment".to_string(), json!(doc.comment));
    }

    // Add property documentation if present
    if !doc.properties_or_values.is_empty() {
        let props: Map<String, Value> = doc
            .properties_or_values
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        obj.insert("@properties".to_string(), Value::Object(props));
    }

    Value::Object(obj)
}

/// Convert a Property to JSON format.
fn property_to_json(prop: &terminusdb_schema::Property) -> Value {
    use terminusdb_schema::TypeFamily;

    match &prop.r#type {
        None => {
            // Required field - just the class name
            json!(prop.class)
        }
        Some(TypeFamily::Optional) => {
            json!({
                "@type": "Optional",
                "@class": prop.class
            })
        }
        Some(TypeFamily::Set(_)) => {
            json!({
                "@type": "Set",
                "@class": prop.class
            })
        }
        Some(TypeFamily::List) => {
            json!({
                "@type": "List",
                "@class": prop.class
            })
        }
        Some(TypeFamily::Array(dims)) => {
            json!({
                "@type": "Array",
                "@class": prop.class,
                "dimensions": dims
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schemas_to_frames_json_basic() {
        let schemas = vec![
            Schema::Class {
                id: "Person".to_string(),
                base: None,
                key: terminusdb_schema::Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: false,
                properties: vec![terminusdb_schema::Property {
                    name: "name".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                }],
            },
            Schema::Enum {
                id: "Status".to_string(),
                base: None,
                values: vec!["Active".to_string(), "Inactive".to_string()],
                documentation: None,
            },
        ];

        let json = schemas_to_frames_json(&schemas);

        // Verify structure
        assert!(json.get("@context").is_some());
        assert!(json.get("Person").is_some());
        assert!(json.get("Status").is_some());

        // Verify Person class
        let person = json.get("Person").unwrap();
        assert_eq!(person.get("@type").unwrap(), "Class");
        assert_eq!(person.get("name").unwrap(), "xsd:string");

        // Verify Status enum
        let status = json.get("Status").unwrap();
        assert_eq!(status.get("@type").unwrap(), "Enum");
    }

    #[test]
    fn test_schemas_vec_to_allframes() {
        let schemas = vec![
            Schema::Class {
                id: "Project".to_string(),
                base: None,
                key: terminusdb_schema::Key::Lexical(vec!["name".to_string()]),
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: false,
                properties: vec![
                    terminusdb_schema::Property {
                        name: "name".to_string(),
                        r#type: None,
                        class: "xsd:string".to_string(),
                    },
                    terminusdb_schema::Property {
                        name: "description".to_string(),
                        r#type: Some(terminusdb_schema::TypeFamily::Optional),
                        class: "xsd:string".to_string(),
                    },
                ],
            },
            Schema::Class {
                id: "Ticket".to_string(),
                base: None,
                key: terminusdb_schema::Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: false,
                properties: vec![
                    terminusdb_schema::Property {
                        name: "title".to_string(),
                        r#type: None,
                        class: "xsd:string".to_string(),
                    },
                    terminusdb_schema::Property {
                        name: "project".to_string(),
                        r#type: None,
                        class: "Project".to_string(),
                    },
                    terminusdb_schema::Property {
                        name: "labels".to_string(),
                        r#type: Some(terminusdb_schema::TypeFamily::Set(
                            terminusdb_schema::SetCardinality::None,
                        )),
                        class: "xsd:string".to_string(),
                    },
                ],
            },
            Schema::Enum {
                id: "Priority".to_string(),
                base: None,
                values: vec!["Low".to_string(), "Medium".to_string(), "High".to_string()],
                documentation: None,
            },
        ];

        let allframes = schemas_vec_to_allframes(&schemas);

        // Verify that frames were created
        assert!(
            allframes.frames.len() >= 3,
            "Expected at least 3 frames, got {}",
            allframes.frames.len()
        );

        // Verify context prefixes
        assert!(!allframes.context.base.is_empty());
        assert!(!allframes.context.schema.is_empty());
    }

    #[test]
    fn test_property_types() {
        use terminusdb_schema::{Property, SetCardinality, TypeFamily};

        // Test required field
        let required = Property {
            name: "title".to_string(),
            r#type: None,
            class: "xsd:string".to_string(),
        };
        assert_eq!(property_to_json(&required), json!("xsd:string"));

        // Test optional field
        let optional = Property {
            name: "description".to_string(),
            r#type: Some(TypeFamily::Optional),
            class: "xsd:string".to_string(),
        };
        assert_eq!(
            property_to_json(&optional),
            json!({"@type": "Optional", "@class": "xsd:string"})
        );

        // Test set field
        let set = Property {
            name: "tags".to_string(),
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: "xsd:string".to_string(),
        };
        assert_eq!(
            property_to_json(&set),
            json!({"@type": "Set", "@class": "xsd:string"})
        );

        // Test list field
        let list = Property {
            name: "items".to_string(),
            r#type: Some(TypeFamily::List),
            class: "Item".to_string(),
        };
        assert_eq!(
            property_to_json(&list),
            json!({"@type": "List", "@class": "Item"})
        );
    }
}

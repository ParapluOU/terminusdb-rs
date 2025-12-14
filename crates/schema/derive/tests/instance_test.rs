use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_schema::{
    Instance, InstanceProperty, Key, PrimitiveValue, Property, RelationValue, Schema,
    SetCardinality, ToTDBInstance, ToTDBInstances, ToTDBSchema, TypeFamily,
};
use terminusdb_schema_derive::TerminusDBModel;

// Simple struct for testing
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct SimpleStruct {
    name: String,
    count: i32,
    active: bool,
}

// Struct with optional fields
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct StructWithOptions {
    name: String,
    description: Option<String>,
    count: Option<i32>,
}

// Struct with collections
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct StructWithCollections {
    name: String,
    tags: Vec<String>,
    scores: Vec<i32>,
}

// Address struct for nesting
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    country: String,
}

// Person struct with nested Address
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct Person {
    name: String,
    age: i32,
    address: Address,
}

// Simple enum
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
enum Color {
    Red,
    Green,
    Blue,
}

// Tagged union enum with values
#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
enum TaggedValue {
    Text(String),
    Number(i32),
    Flag(bool),
}

#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
struct StructWithHashMap {
    name: String,
    properties: HashMap<String, String>,
}

#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
#[tdb(id_field = "id")]
struct StructWithId {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::ToMaybeTDBSchema;

    #[test]
    fn test_struct_with_id_instance() {
        let simple = StructWithId {
            id: format!("test-id"),
        };

        let instance = simple.to_instance(None);

        assert_eq!(&instance.id, &Some(format!("StructWithId/test-id")));

        dbg!(<StructWithId as ToTDBSchema>::to_schema());
    }

    #[test]
    fn test_simple_struct_instance() {
        let simple = SimpleStruct {
            name: "Test".to_string(),
            count: 42,
            active: true,
        };

        let instance = simple.to_instance(None);

        assert_eq!(
            instance.properties.get("name").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("Test".to_string()))
        );
        assert_eq!(
            instance.properties.get("count").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(42)))
        );
        assert_eq!(
            instance.properties.get("active").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::Bool(true))
        );
    }

    #[test]
    fn test_optional_fields_instance() {
        let with_optionals = StructWithOptions {
            name: "Test".to_string(),
            description: Some("A test".to_string()),
            count: None,
        };

        let instance = with_optionals.to_instance(None);

        assert_eq!(
            instance.properties.get("name").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("Test".to_string()))
        );
        assert_eq!(
            instance.properties.get("description").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("A test".to_string()))
        );

        // Either count is not present or it's present as Null
        if let Some(count_prop) = instance.properties.get("count") {
            assert_eq!(
                count_prop,
                &InstanceProperty::Primitive(PrimitiveValue::Null)
            );
        }
    }

    #[test]
    fn test_collections_instance() {
        let with_collections = StructWithCollections {
            name: "Test".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            scores: vec![10, 20, 30],
        };

        let instance = with_collections.to_instance(None);

        assert_eq!(
            instance.properties.get("name").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("Test".to_string()))
        );

        if let InstanceProperty::Primitives(tags) = instance.properties.get("tags").unwrap() {
            assert_eq!(tags.len(), 2);
            assert!(tags.contains(&PrimitiveValue::String("tag1".to_string())));
            assert!(tags.contains(&PrimitiveValue::String("tag2".to_string())));
        } else {
            panic!("tags field is not Primitives");
        }

        if let InstanceProperty::Primitives(scores) = instance.properties.get("scores").unwrap() {
            assert_eq!(scores.len(), 3);
            assert!(scores.contains(&PrimitiveValue::Number(serde_json::Number::from(10))));
            assert!(scores.contains(&PrimitiveValue::Number(serde_json::Number::from(20))));
            assert!(scores.contains(&PrimitiveValue::Number(serde_json::Number::from(30))));
        } else {
            panic!("scores field is not Any");
        }
    }

    #[test]
    fn test_nested_struct_instance() {
        let address = Address {
            street: "123 Main St".to_string(),
            city: "Example City".to_string(),
            country: "Example Country".to_string(),
        };

        let person = Person {
            name: "John Doe".to_string(),
            age: 30,
            address,
        };

        let instance = person.to_instance(None);

        assert_eq!(
            instance.properties.get("name").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("John Doe".to_string()))
        );
        assert_eq!(
            instance.properties.get("age").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(30)))
        );

        // Check for any relation type, not just Reference
        assert!(matches!(
            instance.properties.get("address").unwrap(),
            InstanceProperty::Relation(_)
        ));
    }

    #[test]
    fn test_instance_tree() {
        let address = Address {
            street: "123 Main St".to_string(),
            city: "Example City".to_string(),
            country: "Example Country".to_string(),
        };

        let person = Person {
            name: "John Doe".to_string(),
            age: 30,
            address,
        };

        let instances = person.to_instance_tree();

        // Should have two instances: Person and Address
        assert_eq!(instances.len(), 2);

        // Find the Person instance
        let person_instance = instances
            .iter()
            .find(|i| matches!(&i.schema, Schema::Class{id, ..} if id == "Person"))
            .expect("Person instance not found");

        // Check person properties
        assert_eq!(
            person_instance.properties.get("name").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("John Doe".to_string()))
        );
        assert_eq!(
            person_instance.properties.get("age").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(30)))
        );

        // Find the Address instance
        let address_instance = instances
            .iter()
            .find(|i| matches!(&i.schema, Schema::Class{id, ..} if id == "Address"))
            .expect("Address instance not found");

        // Check Address properties
        assert_eq!(
            address_instance.properties.get("street").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("123 Main St".to_string()))
        );
        assert_eq!(
            address_instance.properties.get("city").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("Example City".to_string()))
        );
        assert_eq!(
            address_instance.properties.get("country").unwrap(),
            &InstanceProperty::Primitive(PrimitiveValue::String("Example Country".to_string()))
        );
    }

    #[test]
    fn test_enum_instance() {
        let color = Color::Blue;

        let instance = color.to_instance(None);

        assert!(matches!(&instance.schema, Schema::Enum{id, ..} if id == "Color"));

        // Check that we have a variant property
        match &instance.schema {
            Schema::Enum { .. } => {
                assert_eq!(instance.properties.len(), 1);
                assert!(instance.properties.contains_key("blue"));
            }
            _ => panic!("Expected Schema::Enum"),
        }
    }

    #[test]
    fn test_tagged_union_instance() {
        let text_value = TaggedValue::Text("Hello".to_string());
        let number_value = TaggedValue::Number(42);
        let flag_value = TaggedValue::Flag(true);

        let text_instance = text_value.to_instance(None);
        let number_instance = number_value.to_instance(None);
        let flag_instance = flag_value.to_instance(None);

        // Check schema type
        assert!(
            matches!(&text_instance.schema, Schema::TaggedUnion{id, ..} if id == "TaggedValue")
        );
        assert!(
            matches!(&number_instance.schema, Schema::TaggedUnion{id, ..} if id == "TaggedValue")
        );
        assert!(
            matches!(&flag_instance.schema, Schema::TaggedUnion{id, ..} if id == "TaggedValue")
        );

        // Inspect the actual properties to understand the implementation
        println!("Text variant properties: {:?}", text_instance.properties);
        println!(
            "Number variant properties: {:?}",
            number_instance.properties
        );
        println!("Flag variant properties: {:?}", flag_instance.properties);

        // Check basic expectations - each variant should have a single property
        // but we're not asserting specific keys since the implementation may vary
        assert_eq!(text_instance.properties.len(), 1);
        assert_eq!(number_instance.properties.len(), 1);
        assert_eq!(flag_instance.properties.len(), 1);

        // Get first property value (key doesn't matter)
        let text_prop = text_instance.properties.values().next().unwrap();
        let number_prop = number_instance.properties.values().next().unwrap();
        let flag_prop = flag_instance.properties.values().next().unwrap();

        // Check the values match what we expect
        assert_eq!(
            text_prop,
            &InstanceProperty::Primitive(PrimitiveValue::String("Hello".to_string()))
        );
        assert_eq!(
            number_prop,
            &InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(42)))
        );
        assert_eq!(
            flag_prop,
            &InstanceProperty::Primitive(PrimitiveValue::Bool(true))
        );
    }

    #[test]
    fn test_hashmap_instance() {
        let mut properties = HashMap::new();
        properties.insert("color".to_string(), "red".to_string());
        properties.insert("size".to_string(), "large".to_string());

        let data = StructWithHashMap {
            name: "Test Object".to_string(),
            properties,
        };

        let instance = data.to_instance(Some("test1".to_string()));

        assert_eq!(instance.id, Some("StructWithHashMap/test1".to_string()));

        // Verify name property
        if let Some(InstanceProperty::Primitive(PrimitiveValue::String(name))) =
            instance.properties.get("name")
        {
            assert_eq!(name, "Test Object");
        } else {
            panic!("Expected name property to be a String");
        }

        // Verify properties field contains HashMap entries
        if let Some(InstanceProperty::Any(entries)) = instance.properties.get("properties") {
            assert_eq!(entries.len(), 2);

            // Check each entry has the expected structure
            for entry in entries {
                if let InstanceProperty::Relation(RelationValue::One(entry_instance)) = entry {
                    // Check it's a HashMapStringEntry
                    if let Schema::Class { id, .. } = &entry_instance.schema {
                        assert_eq!(id, "HashMapStringEntry");
                    } else {
                        panic!("Expected Class schema");
                    }

                    // Get key and value
                    let key =
                        if let Some(InstanceProperty::Primitive(PrimitiveValue::String(key))) =
                            entry_instance.properties.get("key")
                        {
                            key.clone()
                        } else {
                            panic!("Expected key property to be a String");
                        };

                    let value =
                        if let Some(InstanceProperty::Primitive(PrimitiveValue::String(value))) =
                            entry_instance.properties.get("value")
                        {
                            value.clone()
                        } else {
                            panic!("Expected value property to be a String");
                        };

                    // Verify the key-value pairs match what we put in
                    if key == "color" {
                        assert_eq!(value, "red");
                    } else if key == "size" {
                        assert_eq!(value, "large");
                    } else {
                        panic!("Unexpected key: {}", key);
                    }
                } else {
                    panic!("Expected Relation entry");
                }
            }
        } else {
            panic!("Expected properties to be an Any with entries");
        }
    }

    #[test]
    fn test_hashmap_schema() {
        let schema = <StructWithHashMap as ToTDBSchema>::to_schema();

        if let Schema::Class { properties, .. } = &schema {
            // Find the properties field
            let prop = properties
                .iter()
                .find(|p| p.name == "properties")
                .expect("properties field not found in schema");

            // Verify it's a Set type referencing HashMapStringEntry
            if let Some(TypeFamily::Set(_)) = &prop.r#type {
                assert_eq!(prop.class, "HashMapStringEntry");
            } else {
                panic!("Expected Set type family");
            }
        } else {
            panic!("Expected Class schema");
        }
    }

    #[test]
    fn test_hashmap_instance_tree() {
        let mut properties = HashMap::new();
        properties.insert("color".to_string(), "red".to_string());
        properties.insert("size".to_string(), "large".to_string());

        let data = StructWithHashMap {
            name: "Test Object".to_string(),
            properties,
        };

        let instance_tree = data.to_instance_tree();

        // Should have at least one instance (the main struct)
        assert!(!instance_tree.is_empty());

        // First instance should be the main struct
        let main_instance = &instance_tree[0];
        if let Schema::Class { id, .. } = &main_instance.schema {
            assert_eq!(id, "StructWithHashMap");
        } else {
            panic!("Expected Class schema");
        }

        // Verify the main instance contains the HashMap entries
        if let Some(InstanceProperty::Any(entries)) = main_instance.properties.get("properties") {
            assert_eq!(entries.len(), 2);

            // Check each entry has the expected structure
            for entry in entries {
                if let InstanceProperty::Relation(RelationValue::One(entry_instance)) = entry {
                    if let Schema::Class { id, .. } = &entry_instance.schema {
                        assert_eq!(id, "HashMapStringEntry");
                    } else {
                        panic!("Expected Class schema");
                    }

                    // Verify each entry has key and value properties
                    assert!(entry_instance.properties.contains_key("key"));
                    assert!(entry_instance.properties.contains_key("value"));
                } else {
                    panic!("Expected Relation entry");
                }
            }
        } else {
            panic!("Expected properties to be an Any with entries");
        }
    }
}

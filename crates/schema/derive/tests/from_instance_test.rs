use anyhow::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use terminusdb_schema::{
    build_instance_tree, FromInstanceProperty, FromTDBInstance, Instance, InstanceProperty, Key,
    PrimitiveValue, Schema, ToTDBInstance, ToTDBInstances, ToTDBSchema,
};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Simple struct for basic instance tests
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
struct SimpleStruct {
    name: String,
    count: i32,
    active: bool,
}

// Struct with optional fields
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
struct StructWithOptions {
    name: String,
    description: Option<String>,
    count: Option<i32>,
}

// Address struct for nesting
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
struct Address {
    street: String,
    city: String,
    country: String,
}

// Person struct with nested Address
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
struct Person {
    name: String,
    age: i32,
    address: Address,
}

// Simple enum
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
enum Color {
    Red,
    Green,
    Blue,
}

// Tagged union enum with values
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
enum TaggedValue {
    Text(String),
    Number(i32),
    Flag(bool),
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel)]
pub struct TestStruct {
    name: String,
    description: Option<String>,
    count: Option<i32>,
}

// Manual implementations removed - now handled by TerminusDBModel derive

impl TestStruct {
    pub fn test_instance_tree() -> BTreeMap<String, Instance> {
        let mut instances = BTreeMap::new();

        // Base TestStruct instance
        let test_instance = Instance {
            schema: Schema::Class {
                id: "TestStruct".to_string(),
                base: None,
                key: Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: true,
                properties: vec![],
            },
            id: Some("test1".to_string()),
            capture: false,
            ref_props: false,
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String("Test Item".to_string())),
                );
                props.insert(
                    "description".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "A test item description".to_string(),
                    )),
                );
                props.insert(
                    "count".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(
                        42,
                    ))),
                );
                props
            },
        };

        instances.insert("test1".to_string(), test_instance);
        instances
    }

    pub fn deserialize_test() -> Result<Self, anyhow::Error> {
        let instances = Self::test_instance_tree();
        let instance = instances.get("test1").unwrap();

        Self::from_instance(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::trace;

    #[test]
    fn test_simple_struct_instance() {
        // Create an original instance
        let original = SimpleStruct {
            name: "Test".to_string(),
            count: 42,
            active: true,
        };

        // Convert to TDB instance
        let instance = original.to_instance(None);

        // Convert back using FromTDBInstance
        let deserialized = SimpleStruct::from_instance(&instance).unwrap();

        // Verify
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_optional_fields_instance() {
        // Create an original instance with some fields present and others not
        let original = StructWithOptions {
            name: "Test".to_string(),
            description: Some("A test".to_string()),
            count: None,
        };

        // Convert to TDB instance
        let instance = original.to_instance(None);

        // Convert back using FromTDBInstance
        let deserialized = StructWithOptions::from_instance(&instance).unwrap();

        // Verify
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_nested_struct_instance() {
        // Create a nested structure
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

        // Convert to TDB instance
        let instance = person.to_instance(None);

        // Convert back using FromTDBInstance
        let deserialized = Person::from_instance(&instance).unwrap();

        // Verify
        assert_eq!(deserialized.name, person.name);
        assert_eq!(deserialized.age, person.age);
        assert_eq!(deserialized.address.street, person.address.street);
        assert_eq!(deserialized.address.city, person.address.city);
        assert_eq!(deserialized.address.country, person.address.country);
    }

    #[test]
    fn test_instance_tree() {
        // Create a nested structure
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

        // Convert to TDB instance tree
        let instances = person.to_instance_tree();

        // Convert back using FromTDBInstance
        let deserialized = Person::from_instance_tree(&instances).unwrap();

        // Verify
        assert_eq!(deserialized.name, person.name);
        assert_eq!(deserialized.age, person.age);
        assert_eq!(deserialized.address.street, person.address.street);
        assert_eq!(deserialized.address.city, person.address.city);
        assert_eq!(deserialized.address.country, person.address.country);
    }

    #[test]
    fn test_enum_instance() {
        // Create an original enum value
        let original = Color::Blue;

        // Convert to TDB instance
        let instance = original.to_instance(None);

        // Convert back using FromTDBInstance
        let deserialized = Color::from_instance(&instance).unwrap();

        // Verify
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_tagged_union_instance() {
        // Test each variant
        let text_value = TaggedValue::Text("Hello".to_string());
        let number_value = TaggedValue::Number(42);
        let flag_value = TaggedValue::Flag(true);

        // Convert to TDB instances
        let text_instance = text_value.to_instance(None);
        let number_instance = number_value.to_instance(None);
        let flag_instance = flag_value.to_instance(None);

        // Debug print the instances
        trace!("Text instance: {:#?}", text_instance);
        trace!("Number instance: {:#?}", number_instance);
        trace!("Flag instance: {:#?}", flag_instance);

        // Convert back using FromTDBInstance
        let deserialized_text = TaggedValue::from_instance(&text_instance).unwrap();
        let deserialized_number = TaggedValue::from_instance(&number_instance).unwrap();
        let deserialized_flag = TaggedValue::from_instance(&flag_instance).unwrap();

        // Verify
        assert_eq!(deserialized_text, text_value);
        assert_eq!(deserialized_number, number_value);
        assert_eq!(deserialized_flag, flag_value);
    }

    #[test]
    fn test_manual_instance_creation() {
        // Create an instance manually
        let instance = Instance {
            schema: Schema::Class {
                id: "SimpleStruct".to_string(),
                base: None,
                key: Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: true,
                properties: vec![],
            },
            id: None,
            capture: false,
            ref_props: false,
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String("Test".to_string())),
                );
                props.insert(
                    "count".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::Number(serde_json::Number::from(
                        42,
                    ))),
                );
                props.insert(
                    "active".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::Bool(true)),
                );
                props
            },
        };

        // Convert to Rust struct using FromTDBInstance
        let deserialized = SimpleStruct::from_instance(&instance).unwrap();

        // Verify
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.count, 42);
        assert_eq!(deserialized.active, true);
    }

    #[test]
    fn test_deserialize_test_struct() {
        let result = TestStruct::deserialize_test();
        assert!(result.is_ok());

        let test_struct = result.unwrap();
        assert_eq!(test_struct.name, "Test Item");
        assert_eq!(
            test_struct.description,
            Some("A test item description".to_string())
        );
        assert_eq!(test_struct.count, Some(42));
    }

    #[test]
    fn test_updated_fromtdbinstance_impl() {
        // Create a simple struct instance with different types
        let original = SimpleStruct {
            name: "Test Updated".to_string(),
            count: 100,
            active: true,
        };

        // Convert to TDB instance
        let instance = original.to_instance(None);

        // Test that we can access fields directly using FromInstanceProperty
        let name = String::from_property(instance.get_property("name").unwrap()).unwrap();
        let count = i32::from_property(instance.get_property("count").unwrap()).unwrap();
        let active = bool::from_property(instance.get_property("active").unwrap()).unwrap();

        assert_eq!(name, "Test Updated");
        assert_eq!(count, 100);
        assert_eq!(active, true);

        // Now test that our derived FromTDBInstance implementation works
        let deserialized = SimpleStruct::from_instance(&instance).unwrap();
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_vec_deserialization() {
        // Create an instance with a vector field
        let original = StructWithCollections {
            name: "Test Collections".to_string(),
            items: vec![1, 2, 3, 4, 5],
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        };

        // Convert to TDB instance
        let instance = original.to_instance(None);

        let items_prop = instance.get_property("items").unwrap();

        assert!(matches!(&items_prop, InstanceProperty::Primitives(_)));

        // Test direct FromInstanceProperty for Vec types
        let items = Vec::<i32>::from_property(items_prop).unwrap();
        let tags = Vec::<String>::from_property(instance.get_property("tags").unwrap()).unwrap();

        assert_eq!(items, vec![1, 2, 3, 4, 5]);
        assert_eq!(tags, vec!["tag1".to_string(), "tag2".to_string()]);

        // Test that our derived FromTDBInstance implementation works
        let deserialized = StructWithCollections::from_instance(&instance).unwrap();
        assert_eq!(deserialized, original);
    }
}

// Add a new struct with collections to test Vec deserialization
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
struct StructWithCollections {
    name: String,
    items: Vec<i32>,
    tags: Vec<String>,
}

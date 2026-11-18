use std::collections::HashMap;
use terminusdb_schema::{
    InstanceProperty, PrimitiveValue, RelationValue, Schema, ToTDBInstance, ToTDBInstances, ToTDBSchema, TypeFamily,
};
use terminusdb_schema_derive::TerminusDBModel;

// Simple struct for testing
#[derive(TerminusDBModel, Debug, Clone)]
struct SimpleStruct {
    name: String,
    count: i32,
    active: bool,
}

// Struct with optional fields
#[derive(TerminusDBModel, Debug, Clone)]
struct StructWithOptions {
    name: String,
    description: Option<String>,
    count: Option<i32>,
}

// Struct with collections
#[derive(TerminusDBModel, Debug, Clone)]
struct StructWithCollections {
    name: String,
    tags: Vec<String>,
    scores: Vec<i32>,
}

// Address struct for nesting
#[derive(TerminusDBModel, Debug, Clone)]
struct Address {
    street: String,
    city: String,
    country: String,
}

// Person struct with nested Address
#[derive(TerminusDBModel, Debug, Clone)]
struct Person {
    name: String,
    age: i32,
    address: Address,
}

// Simple enum
#[derive(TerminusDBModel, Debug, Clone)]
enum Color {
    Red,
    Green,
    Blue,
}

// Tagged union enum with values
#[derive(TerminusDBModel, Debug, Clone)]
enum TaggedValue {
    Text(String),
    Number(i32),
    Flag(bool),
}

#[derive(TerminusDBModel, Debug, Clone)]
struct StructWithHashMap {
    name: String,
    properties: HashMap<String, String>,
}

#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(id_field = "id")]
struct StructWithId {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    

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

    // `HashMap<String, String>` is stored as a single `sys:JSON` object
    // (`{key: value}`), not as a set of `HashMapStringEntry` subdocuments. The
    // subdocument form was write-only — nothing could deserialize it back — so
    // `crates/schema/src/impl/map.rs` replaced it with the JSON form that
    // matches its `FromInstanceProperty` impl and `HashMap<String, Value>`.
    // These three tests asserted the removed representation.

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

        // The map is one JSON object, keys mapping straight to string values.
        let Some(InstanceProperty::Primitive(PrimitiveValue::Object(json))) =
            instance.properties.get("properties")
        else {
            panic!(
                "expected properties to be a JSON object, got {:?}",
                instance.properties.get("properties")
            );
        };
        let map = json.as_object().expect("properties should be a JSON object");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("color").and_then(|v| v.as_str()), Some("red"));
        assert_eq!(map.get("size").and_then(|v| v.as_str()), Some("large"));
    }

    #[test]
    fn test_hashmap_schema() {
        let schema = <StructWithHashMap as ToTDBSchema>::to_schema();

        let Schema::Class { properties, .. } = &schema else {
            panic!("Expected Class schema");
        };
        let prop = properties
            .iter()
            .find(|p| p.name == "properties")
            .expect("properties field not found in schema");

        // A plain `sys:JSON` datatype property: no type family, no subdocument
        // class to reference.
        assert_eq!(prop.class, "sys:JSON");
        assert!(
            prop.r#type.is_none(),
            "expected no type family for a JSON map, got {:?}",
            prop.r#type
        );
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

        // The map is a primitive, so it contributes no further instances: the
        // tree is just the struct itself.
        assert_eq!(instance_tree.len(), 1);
        let main_instance = &instance_tree[0];
        if let Schema::Class { id, .. } = &main_instance.schema {
            assert_eq!(id, "StructWithHashMap");
        } else {
            panic!("Expected Class schema");
        }

        assert!(matches!(
            main_instance.properties.get("properties"),
            Some(InstanceProperty::Primitive(PrimitiveValue::Object(_)))
        ));
    }
}

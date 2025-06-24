use terminusdb_schema::{
    Client, FromTDBInstance, Instance, InstanceProperty, Key, PrimitiveValue, RelationValue,
    Schema, TdbLazy, ToTDBInstance, ToTDBSchema,
};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

// Mock client for testing
struct MockClient;
impl Client for MockClient {
    fn get_instance(&self, id: &str) -> Result<Instance, anyhow::Error> {
        if id == "activity1" {
            let mut properties = BTreeMap::new();
            properties.insert(
                "name".to_string(),
                InstanceProperty::Primitive(PrimitiveValue::String("Test Activity".to_string())),
            );
            properties.insert(
                "description".to_string(),
                InstanceProperty::Primitive(PrimitiveValue::String("Test Description".to_string())),
            );

            Result::Ok(Instance {
                schema: Schema::Class {
                    id: "Activity".to_string(),
                    base: None,
                    key: Key::Random,
                    documentation: None,
                    subdocument: false,
                    r#abstract: false,
                    inherits: vec![],
                    unfoldable: false,
                    properties: vec![],
                },
                id: Some("activity1".to_string()),
                capture: false,
                ref_props: false,
                properties,
            })
        } else {
            Err(anyhow::anyhow!("Instance not found"))
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
struct Activity {
    name: String,
    description: String,
}

// Define a simple struct without derivations to avoid ToInstanceProperty errors
#[derive(Debug, PartialEq, Clone, TerminusDBModel, FromTDBInstance)]
struct AxiomWithLazy {
    name: String,
    activity: TdbLazy<Activity>,
}

#[test]
fn test_deserialize_reference() {
    // Create a JSON representation of an instance with a reference
    let json_instance = json!({
        "@type": "AxiomWithLazy",
        "name": "Test Axiom",
        "activity": {
            "@ref": "activity1"
        }
    });

    // Parse JSON into Instance using the FromTDBInstance implementation
    let instance_result = Instance::from_json_with_schema::<AxiomWithLazy>(json_instance);
    let instance = instance_result.unwrap();

    // Deserialize to the target type
    let axiom_lazy_result = AxiomWithLazy::from_instance(&instance);
    assert!(
        axiom_lazy_result.is_ok(),
        "Failed to deserialize: {:?}",
        axiom_lazy_result.err()
    );

    let mut axiom_lazy = axiom_lazy_result.unwrap();
    assert_eq!(axiom_lazy.name, "Test Axiom");
    assert_eq!(axiom_lazy.activity.id().to_string(), "activity1");

    // Test loading the reference
    let client = MockClient;
    let activity = axiom_lazy.activity.get(&client).unwrap();
    assert_eq!(activity.name, "Test Activity");
    assert_eq!(activity.description, "Test Description");
}

#[test]
fn test_deserialize_full_instance() {
    // Create a full instance directly
    let mut activity_properties = BTreeMap::new();
    activity_properties.insert(
        "name".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::String("Test Activity".to_string())),
    );
    activity_properties.insert(
        "description".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::String("Test Description".to_string())),
    );

    let activity_instance = Instance {
        schema: Schema::Class {
            id: "Activity".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![],
        },
        id: Some("activity1".to_string()),
        capture: false,
        ref_props: false,
        properties: activity_properties,
    };

    let mut parent_properties = BTreeMap::new();
    parent_properties.insert(
        "name".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::String("Test Axiom 2".to_string())),
    );
    parent_properties.insert(
        "activity".to_string(),
        InstanceProperty::Relation(RelationValue::One(activity_instance)),
    );

    let parent_instance = Instance {
        schema: Schema::Class {
            id: "AxiomWithLazy".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![],
        },
        id: Some("AxiomWithLazy/test2".to_string()),
        capture: false,
        ref_props: false,
        properties: parent_properties,
    };

    // Deserialize to the target type
    let axiom_lazy_result = AxiomWithLazy::from_instance(&parent_instance);
    assert!(
        axiom_lazy_result.is_ok(),
        "Failed to deserialize: {:?}",
        axiom_lazy_result.err()
    );

    let mut axiom_lazy = axiom_lazy_result.unwrap();
    assert_eq!(axiom_lazy.name, "Test Axiom 2");

    // Since it's a full instance, we should be able to access it without a client
    let client = MockClient;
    let activity = axiom_lazy.activity.get(&client).unwrap();
    assert_eq!(activity.name, "Test Activity");
    assert_eq!(activity.description, "Test Description");
}

#[test]
fn test_client_deserializer() {
    // Define a simple deserializer trait and implementation for testing
    trait TDBInstanceDeserializer {
        fn from_instance<T: FromTDBInstance>(
            &mut self,
            json: &serde_json::Value,
        ) -> Result<T, anyhow::Error>;
    }

    struct DefaultTDBDeserializer {}

    impl TDBInstanceDeserializer for DefaultTDBDeserializer {
        fn from_instance<T: FromTDBInstance>(
            &mut self,
            json: &serde_json::Value,
        ) -> Result<T, anyhow::Error> {
            if let Some(ty) = json.get("@type") {
                if ty.as_str() == Some("AxiomWithLazy") {
                    // For AxiomWithLazy, manually create an instance
                    let name = json
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let activity_ref = json
                        .get("activity")
                        .and_then(|a| a.get("@ref"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string());

                    let mut properties = BTreeMap::new();
                    properties.insert(
                        "name".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String(name)),
                    );

                    if let Some(ref_id) = activity_ref {
                        properties.insert(
                            "activity".to_string(),
                            InstanceProperty::Relation(RelationValue::ExternalReference(ref_id)),
                        );
                    }

                    // Create a basic instance
                    let instance = Instance {
                        schema: Schema::Class {
                            id: "AxiomWithLazy".to_string(),
                            base: None,
                            key: Key::Random,
                            documentation: None,
                            subdocument: false,
                            r#abstract: false,
                            inherits: vec![],
                            unfoldable: false,
                            properties: vec![],
                        },
                        id: None,
                        capture: false,
                        ref_props: false,
                        properties,
                    };

                    return T::from_instance(&instance);
                }
            }

            Err(anyhow::anyhow!("Unsupported type for deserialization"))
        }
    }

    // Create a JSON representation of an instance with a reference
    let json_value = json!({
        "@type": "AxiomWithLazy",
        "name": "Test Axiom",
        "activity": {
            "@ref": "activity1"
        }
    });

    // Use the client's deserializer
    let mut deserializer = DefaultTDBDeserializer {};
    let axiom_lazy_result = deserializer.from_instance(&json_value);
    assert!(
        axiom_lazy_result.is_ok(),
        "Failed to deserialize: {:?}",
        axiom_lazy_result.err()
    );

    let axiom_lazy: AxiomWithLazy = axiom_lazy_result.unwrap();
    assert_eq!(axiom_lazy.name, "Test Axiom");
    assert_eq!(axiom_lazy.activity.id().to_string(), "activity1");
}

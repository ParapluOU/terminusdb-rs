use crate as terminusdb_schema;
use crate::*;
use pretty_assertions::{assert_eq, assert_ne};
use serde;
use std::collections::BTreeMap;
use terminusdb_schema_derive::*;

#[derive(Clone, TerminusDBModel)]
pub struct Model {
    // primitives
    u32: u32,
    usize: usize,
    isize: isize,
    float: f64,
    bool: bool,
    string: String,

    // primitives in relationships
    u32_opt: Option<u32>,
    usize_opt: Option<usize>,
    isize_opt: Option<isize>,
    float_opt: Option<f64>,
    bool_opt: Option<bool>,
    string_opt: Option<String>,

    // primitive arrays
    u32_arr: Vec<u32>,
    usize_arr: Vec<usize>,
    isize_arr: Vec<isize>,
    float_arr: Vec<f64>,
    bool_arr: Vec<bool>,
    string_arrt: Vec<String>,

    // class properties
    submodel: SubModel,
}

#[derive(Clone, TerminusDBModel)]
pub struct SubModel {
    u32: u32,
}

#[derive(Clone, TerminusDBModel)]
struct DummyModel {
    id: String,
    name: Option<String>,
    count: Option<i32>,
}

#[derive(Clone, TerminusDBModel)]
struct SimpleClass {
    id: String,
}

#[test]
fn test_schema_class_tree_generate() {
    let schemas = <Model as ToTDBSchema>::to_schema_tree();
    assert_eq!(schemas.len(), 2);
}

#[test]
fn test_tuple_schemas_generate() {
    // Test single tuple - should return all schemas from Model's tree
    let schemas1 = <(Model,)>::to_schemas();
    assert_eq!(schemas1.len(), 2); // Model and SubModel

    // Test double tuple - should return all schemas from both types
    let schemas2 = <(SimpleClass, DummyModel)>::to_schemas();
    assert_eq!(schemas2.len(), 2); // SimpleClass and DummyModel (no nested types)

    // Test triple tuple with nested types
    let schemas3 = <(Model, SimpleClass, DummyModel)>::to_schemas();
    assert_eq!(schemas3.len(), 4); // Model, SubModel, SimpleClass, DummyModel

    // Verify the schema names are correct
    let schema_names: Vec<String> = schemas3
        .iter()
        .map(|s| s.class_name().to_string())
        .collect();
    assert!(schema_names.contains(&"Model".to_string()));
    assert!(schema_names.contains(&"SubModel".to_string()));
    assert!(schema_names.contains(&"SimpleClass".to_string()));
    assert!(schema_names.contains(&"DummyModel".to_string()));
}

#[test]
fn test_schema_class_generate() {
    let schema = <Model as ToTDBSchema>::to_schema();

    assert_eq!(
        schema,
        Schema::Class {
            id: "Model".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            properties: vec![
                <u32 as ToSchemaProperty<()>>::to_property("u32"),
                <usize as ToSchemaProperty<()>>::to_property("usize"),
                <isize as ToSchemaProperty<()>>::to_property("isize"),
                <f64 as ToSchemaProperty<()>>::to_property("float"),
                <bool as ToSchemaProperty<()>>::to_property("bool"),
                <String as ToSchemaProperty<()>>::to_property("string"),
                // Optional properties
                <Option<u32> as ToSchemaProperty<()>>::to_property("u32_opt"),
                <Option<usize> as ToSchemaProperty<()>>::to_property("usize_opt"),
                <Option<isize> as ToSchemaProperty<()>>::to_property("isize_opt"),
                <Option<f64> as ToSchemaProperty<()>>::to_property("float_opt"),
                <Option<bool> as ToSchemaProperty<()>>::to_property("bool_opt"),
                <Option<String> as ToSchemaProperty<()>>::to_property("string_opt"),
                // Array properties
                <Vec<u32> as ToSchemaProperty<()>>::to_property("u32_arr"),
                <Vec<usize> as ToSchemaProperty<()>>::to_property("usize_arr"),
                <Vec<isize> as ToSchemaProperty<()>>::to_property("isize_arr"),
                <Vec<f64> as ToSchemaProperty<()>>::to_property("float_arr"),
                <Vec<bool> as ToSchemaProperty<()>>::to_property("bool_arr"),
                <Vec<String> as ToSchemaProperty<()>>::to_property("string_arrt"),
                // Class property
                <SubModel as ToSchemaProperty<()>>::to_property("submodel"),
            ],
            unfoldable: false,
        }
    )
}

fn create_test_instance() -> Instance {
    let model = Model {
        u32: 0,
        usize: 0,
        isize: 0,
        float: 0.0,
        bool: false,
        string: "test".to_string(),
        u32_opt: Some(0),
        usize_opt: Some(0),
        isize_opt: Some(0),
        float_opt: Some(0.0),
        bool_opt: Some(true),
        string_opt: None,
        u32_arr: vec![0, 1, 2],
        usize_arr: vec![0, 1, 2],
        isize_arr: vec![0, 1, 2],
        float_arr: vec![0.0, 1.1, 2.2],
        bool_arr: vec![true, false, true],
        string_arrt: vec!["test".to_string(), "test".to_string(), "test".to_string()],
        submodel: SubModel { u32: 0 },
    };

    model.to_instance(None)
}

#[test]
fn test_instance_generate() {
    let instance = create_test_instance();

    let mut expected_properties = BTreeMap::new();
    expected_properties.insert(
        "u32".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "usize".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "isize".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "float".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(
            serde_json::Number::from_f64(0.0).unwrap(),
        )),
    );
    expected_properties.insert(
        "bool".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Bool(false)),
    );
    expected_properties.insert(
        "string".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::String("test".to_string())),
    );

    expected_properties.insert(
        "u32_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "usize_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "isize_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "float_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(
            serde_json::Number::from_f64(0.0).unwrap(),
        )),
    );
    expected_properties.insert(
        "bool_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Bool(true)),
    );
    expected_properties.insert(
        "string_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Null),
    );

    // Array properties - using Primitives instead of Any
    expected_properties.insert(
        "u32_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "usize_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "isize_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "float_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(serde_json::Number::from_f64(0.0).unwrap()),
            PrimitiveValue::Number(serde_json::Number::from_f64(1.1).unwrap()),
            PrimitiveValue::Number(serde_json::Number::from_f64(2.2).unwrap()),
        ]),
    );
    expected_properties.insert(
        "bool_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Bool(true),
            PrimitiveValue::Bool(false),
            PrimitiveValue::Bool(true),
        ]),
    );
    expected_properties.insert(
        "string_arrt".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::String("test".to_string()),
            PrimitiveValue::String("test".to_string()),
            PrimitiveValue::String("test".to_string()),
        ]),
    );

    // Nested model
    let submodel_instance = SubModel { u32: 0 }.to_instance(None);
    expected_properties.insert(
        "submodel".to_string(),
        InstanceProperty::Relation(RelationValue::One(submodel_instance)),
    );

    assert_eq!(
        instance,
        Instance {
            schema: <Model as ToTDBSchema>::to_schema(),
            id: None,
            capture: false,
            properties: expected_properties,
            ref_props: true,
        }
    )
}

#[test]
fn test_instance_json_generate() {
    let instance = create_test_instance();

    // Create the expected instance properties
    let mut expected_properties = BTreeMap::new();
    expected_properties.insert(
        "u32".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "usize".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "isize".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "float".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(
            serde_json::Number::from_f64(0.0).unwrap(),
        )),
    );
    expected_properties.insert(
        "bool".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Bool(false)),
    );
    expected_properties.insert(
        "string".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::String("test".to_string())),
    );

    expected_properties.insert(
        "u32_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "usize_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "isize_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(0.into())),
    );
    expected_properties.insert(
        "float_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Number(
            serde_json::Number::from_f64(0.0).unwrap(),
        )),
    );
    expected_properties.insert(
        "bool_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Bool(true)),
    );
    expected_properties.insert(
        "string_opt".to_string(),
        InstanceProperty::Primitive(PrimitiveValue::Null),
    );

    // Array properties - using Primitives instead of Any
    expected_properties.insert(
        "u32_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "usize_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "isize_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(0.into()),
            PrimitiveValue::Number(1.into()),
            PrimitiveValue::Number(2.into()),
        ]),
    );
    expected_properties.insert(
        "float_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Number(serde_json::Number::from_f64(0.0).unwrap()),
            PrimitiveValue::Number(serde_json::Number::from_f64(1.1).unwrap()),
            PrimitiveValue::Number(serde_json::Number::from_f64(2.2).unwrap()),
        ]),
    );
    expected_properties.insert(
        "bool_arr".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::Bool(true),
            PrimitiveValue::Bool(false),
            PrimitiveValue::Bool(true),
        ]),
    );
    expected_properties.insert(
        "string_arrt".to_string(),
        InstanceProperty::Primitives(vec![
            PrimitiveValue::String("test".to_string()),
            PrimitiveValue::String("test".to_string()),
            PrimitiveValue::String("test".to_string()),
        ]),
    );

    // Nested model
    let submodel_instance = SubModel { u32: 0 }.to_instance(None);
    expected_properties.insert(
        "submodel".to_string(),
        InstanceProperty::Relation(RelationValue::One(submodel_instance)),
    );

    let spec = Instance {
        schema: <Model as ToTDBSchema>::to_schema(),
        id: None,
        capture: false,
        properties: expected_properties,
        ref_props: true,
    };

    assert_eq!(instance.to_json_string(), spec.to_json_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as terminusdb_schema;
    use crate::Instance;
    use crate::InstanceProperty;
    use crate::PrimitiveValue;
    use crate::Property;
    use crate::RelationValue;
    use crate::Schema;
    use crate::TypeFamily;
    use crate::{Client, FromTDBInstance, TdbLazy, ToSchemaClass, ToSchemaProperty, ToTDBSchema};
    use pretty_assertions::{assert_eq, assert_ne};
    use std::collections::BTreeMap;

    // Mock client for testing
    struct MockClient;
    impl Client for MockClient {
        fn get_instance(&self, id: &str) -> Result<Instance, anyhow::Error> {
            if id == "Activity/activity1" || id == "activity1" {
                Ok(create_test_instance(
                    Some("activity1".to_string()),
                    Some("Activity".to_string()),
                    vec![
                        (
                            "name".to_string(),
                            InstanceProperty::Primitive(PrimitiveValue::String(
                                "Test Activity".to_string(),
                            )),
                        ),
                        (
                            "description".to_string(),
                            InstanceProperty::Primitive(PrimitiveValue::String(
                                "Test Description".to_string(),
                            )),
                        ),
                    ],
                ))
            } else {
                Err(anyhow::anyhow!("Instance not found"))
            }
        }
    }

    #[derive(Debug, PartialEq, Clone, TerminusDBModel, FromTDBInstance)]
    struct Activity {
        name: String,
        description: String,
    }

    #[derive(Debug, PartialEq, Clone, TerminusDBModel, FromTDBInstance)]
    struct Axiom {
        name: String,
        activity: Activity,
    }

    #[derive(Debug, PartialEq, Clone, TerminusDBModel, FromTDBInstance)]
    struct AxiomLazy {
        pub name: String,
        pub activity: TdbLazy<Activity>,
    }

    fn create_test_instance(
        id: Option<String>,
        r#type: Option<String>,
        props: Vec<(String, InstanceProperty)>,
    ) -> Instance {
        let mut properties = BTreeMap::new();

        for (key, value) in props {
            properties.insert(key, value);
        }

        Instance {
            id,
            schema: Schema::Class {
                id: r#type.unwrap_or_default(),
                base: None,
                key: Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                properties: vec![],
                unfoldable: true,
            },
            capture: false,
            properties,
            ref_props: false,
        }
    }

    #[test]
    fn test_nested_deserialization() {
        // First create the activity instance
        let activity_instance = create_test_instance(
            Some("activity1".to_string()),
            Some("Activity".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Activity".to_string(),
                    )),
                ),
                (
                    "description".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Description".to_string(),
                    )),
                ),
            ],
        );

        // Then create the axiom instance with the activity as a relation
        let instance = create_test_instance(
            Some("axiom1".to_string()),
            Some("Axiom".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Activity".to_string(),
                    )),
                ),
                (
                    "activity".to_string(),
                    InstanceProperty::Relation(RelationValue::One(activity_instance)),
                ),
            ],
        );

        let axiom = Axiom::from_instance(&instance).unwrap();
        assert_eq!(axiom.name, "Test Activity");
        assert_eq!(axiom.activity.name, "Test Activity");
        assert_eq!(axiom.activity.description, "Test Description");
    }

    #[test]
    fn test_referenced_deserialization() {
        let instance = create_test_instance(
            Some("axiom1".to_string()),
            Some("AxiomLazy".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String("Test Axiom".to_string())),
                ),
                (
                    "activity".to_string(),
                    InstanceProperty::Relation(RelationValue::ExternalReference(
                        "activity1".to_string(),
                    )),
                ),
            ],
        );

        let axiom_lazy = AxiomLazy::from_instance(&instance).unwrap();
        assert_eq!(axiom_lazy.name, "Test Axiom");
        assert_eq!(axiom_lazy.activity.id().id(), "activity1");

        // Now test with a full instance to ensure it's loaded immediately
        let activity_instance = create_test_instance(
            Some("activity1".to_string()),
            Some("Activity".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Activity".to_string(),
                    )),
                ),
                (
                    "description".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Description".to_string(),
                    )),
                ),
            ],
        );

        // Create an axiom with a full activity instance
        let instance_with_full_activity = create_test_instance(
            Some("axiom2".to_string()),
            Some("AxiomLazy".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String("Test Axiom 2".to_string())),
                ),
                (
                    "activity".to_string(),
                    InstanceProperty::Relation(RelationValue::One(activity_instance)),
                ),
            ],
        );

        let client = MockClient::new();
        let mut axiom_with_full = AxiomLazy::from_instance(&instance_with_full_activity).unwrap();
        assert_eq!(axiom_with_full.name, "Test Axiom 2");

        // Since it's fully loaded, we should be able to access it without a client
        let activity = axiom_with_full.activity.get(&client).unwrap();
        assert_eq!(activity.name, "Test Activity");
        assert_eq!(activity.description, "Test Description");
    }

    #[test]
    fn test_lazy_with_full_instance() {
        // Test case where we have a full instance in the reference
        let instance = create_test_instance(
            Some("activity1".to_string()),
            Some("Activity".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Activity".to_string(),
                    )),
                ),
                (
                    "description".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Description".to_string(),
                    )),
                ),
            ],
        );

        let mut lazy = TdbLazy::<Activity>::from_instance(&instance).unwrap();
        assert_eq!(lazy.id().id(), "activity1");

        // Data should be immediately available since it was a full instance
        let client = MockClient;
        let activity = lazy.get(&client).unwrap();
        assert_eq!(activity.name, "Test Activity");
        assert_eq!(activity.description, "Test Description");
    }

    #[test]
    fn test_lazy_with_reference_only() {
        // Create a TdbLazy directly with just an ID for testing reference behavior
        let mut lazy = TdbLazy::<Activity>::new(Some(EntityIDFor::new("activity1").unwrap()), None);
        assert_eq!(lazy.id().id(), "activity1");

        // Data should be loaded on demand
        let client = MockClient;
        let activity = lazy.get(&client).unwrap();
        assert_eq!(activity.name, "Test Activity");
        assert_eq!(activity.description, "Test Description");
    }

    #[test]
    fn test_lazy_error_handling() {
        // Test with an invalid instance (no ID)
        let instance = create_test_instance(None, Some("Activity".to_string()), vec![]);

        // Test creating a TdbLazy directly with empty string ID (should be invalid)
        let mut lazy = TdbLazy::<Activity>::new(Some(EntityIDFor::new("").unwrap()), None);
        let client = MockClient;

        // This should fail when we try to get the activity because the ID is empty
        let result = lazy.get(&client);
        assert!(result.is_err(), "get() should fail with an empty ID");

        // Test with invalid instance reference
        let non_existent_id = "non_existent_activity_id";
        let mut lazy =
            TdbLazy::<Activity>::new(Some(EntityIDFor::new(non_existent_id).unwrap()), None);

        // Try to load a non-existent activity
        let result = lazy.get(&client);
        assert!(result.is_err(), "get() should fail with a non-existent ID");
    }

    #[test]
    fn test_lazy_caching() {
        let instance = create_test_instance(
            Some("activity1".to_string()),
            Some("Activity".to_string()),
            vec![
                (
                    "name".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Activity".to_string(),
                    )),
                ),
                (
                    "description".to_string(),
                    InstanceProperty::Primitive(PrimitiveValue::String(
                        "Test Description".to_string(),
                    )),
                ),
            ],
        );

        let mut lazy = TdbLazy::<Activity>::from_instance(&instance).unwrap();

        // First call should load from client
        let client = MockClient;
        let activity1 = lazy.get(&client).unwrap();

        // Second call should use cached data - store the pointer before we call get again
        let ptr1 = activity1 as *const _;

        // Get again from the same lazy object
        let activity2 = lazy.get(&client).unwrap();
        let ptr2 = activity2 as *const _;

        // Should be the same reference
        assert!(ptr1 == ptr2);
    }

    impl MockClient {
        pub fn new() -> Self {
            MockClient {}
        }

        pub fn fetch_instance(&self, id: &str) -> Result<Instance, anyhow::Error> {
            let instance = create_test_instance(
                Some(id.to_string()),
                Some("Activity".to_string()),
                vec![
                    (
                        "@type".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String("Activity".to_string())),
                    ),
                    (
                        "name".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String(
                            "Test Activity".to_string(),
                        )),
                    ),
                    (
                        "description".to_string(),
                        InstanceProperty::Primitive(PrimitiveValue::String(
                            "Test Description".to_string(),
                        )),
                    ),
                ],
            );
            Ok(instance)
        }
    }
}

// Tests for Option<EntityIDFor<T>> and related functionality
// These are in a separate cfg(test) module to avoid workspace compilation issues
#[cfg(test)]
mod optional_entity_id_tests {
    use crate as terminusdb_schema;
    use crate::*;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Clone, Debug, TerminusDBModel, PartialEq)]
    #[tdb(key = "random", id_field = "id")]
    pub struct ModelWithOptionalEntityID {
        pub id: Option<EntityIDFor<Self>>,
        pub name: String,
    }

    #[derive(Clone, Debug, TerminusDBModel, PartialEq)]
    #[tdb(key = "lexical", key_fields = "email", id_field = "id")]
    pub struct UserWithOptionalEntityID {
        pub id: ServerIDFor<Self>,
        pub email: String,
        pub username: String,
    }

    #[test]
    fn test_optional_entity_id_compiles_and_works() {
        // Simple test - just verify compilation works
        let _model = ModelWithOptionalEntityID {
            id: Some(EntityIDFor::new("test-123").unwrap()),
            name: "Test Model".to_string(),
        };

        let _model2 = ModelWithOptionalEntityID {
            id: None,
            name: "Another Model".to_string(),
        };

        // Test that the type implements ToSchemaProperty
        let prop_name = "id";
        let prop = <Option<EntityIDFor<ModelWithOptionalEntityID>> as ToSchemaProperty<
            ModelWithOptionalEntityID,
        >>::to_property(prop_name);
        assert_eq!(prop.name, "id");
        assert_eq!(prop.r#type, Some(TypeFamily::Optional));
        assert_eq!(prop.class, STRING);

        // Test to_instance works without stack overflow
        let instance1 = _model.to_instance(None);
        assert!(
            instance1.id.is_some(),
            "Model with Some(EntityIDFor) should have ID"
        );
        assert_eq!(
            instance1.id.as_ref().unwrap(),
            "ModelWithOptionalEntityID/test-123"
        );

        let instance2 = _model2.to_instance(None);
        assert!(
            instance2.id.is_none(),
            "Model with None EntityIDFor should not have ID"
        );

        // Test lexical key model with ServerIDFor
        let _user_with_id = UserWithOptionalEntityID {
            id: ServerIDFor::from_entity_id(EntityIDFor::new("user-456").unwrap()),
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
        };
        let instance_with_id = _user_with_id.to_instance(None);
        assert!(instance_with_id.id.is_some());

        // This should work fine with empty ServerIDFor
        let user_without_id = UserWithOptionalEntityID {
            id: ServerIDFor::new(),
            email: "test2@example.com".to_string(),
            username: "testuser2".to_string(),
        };

        let user_instance = user_without_id.to_instance(None);
        assert!(user_instance.id.is_none());

        // Test schema generation doesn't stack overflow
        let schema = <ModelWithOptionalEntityID as ToTDBSchema>::to_schema();
        match &schema {
            Schema::Class { properties, .. } => {
                let id_prop = properties
                    .iter()
                    .find(|p| p.name == "id")
                    .expect("id property not found");
                assert_eq!(id_prop.r#type, Some(TypeFamily::Optional));
                assert_eq!(id_prop.class, STRING);
            }
            _ => panic!("Expected Schema::Class"),
        }

        assert!(
            true,
            "Option<EntityIDFor<Self>> works without stack overflow!"
        );
    }

    #[test]
    fn test_tdb_lazy_with_lexical_key() {
        #[derive(Clone, Debug, TerminusDBModel, PartialEq)]
        #[tdb(key = "lexical", key_fields = "email")]
        pub struct LexicalUser {
            pub email: String,
            pub name: String,
        }

        let user = LexicalUser {
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };

        // Create TdbLazy from data - should work without ID
        let lazy_result = TdbLazy::new_data(user.clone());
        assert!(
            lazy_result.is_ok(),
            "Should be able to create TdbLazy without ID for lexical key model"
        );

        let mut lazy = lazy_result.unwrap();
        assert_eq!(lazy.get_expect().email, "test@example.com");
        assert_eq!(lazy.get_expect().name, "Test User");

        // Create another lazy for testing id() panic
        let lazy2 = TdbLazy::new_data(user.clone()).unwrap();

        // Accessing id() should panic since there's no ID yet
        let id_result = std::panic::catch_unwind(|| {
            lazy2.id();
        });
        assert!(id_result.is_err(), "id() should panic when ID is None");

        // Test ToInstanceProperty - should include full instance when data is loaded
        let schema = Schema::Class {
            id: "TestParent".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            properties: vec![],
            unfoldable: false,
        };

        let lazy3 = TdbLazy::new_data(user.clone()).unwrap();
        let prop = <TdbLazy<LexicalUser> as ToInstanceProperty<Schema>>::to_property(
            lazy3, "user", &schema,
        );
        match prop {
            InstanceProperty::Relation(RelationValue::One(instance)) => {
                assert!(
                    instance.id.is_none(),
                    "Instance should have no ID for lexical key without ID"
                );
            }
            _ => panic!("Expected Relation::One for loaded TdbLazy"),
        }

        // Test creating TdbLazy with just an ID (for existing entities)
        let lazy_with_id = TdbLazy::<LexicalUser>::new(
            Some(EntityIDFor::new("LexicalUser/test@example.com").unwrap()),
            None,
        );
        assert_eq!(
            lazy_with_id.id().to_string(),
            "LexicalUser/test@example.com"
        );

        // Test FromTDBInstance with no ID
        let instance_no_id = user.to_instance(None);
        assert!(instance_no_id.id.is_none());

        let lazy_from_instance = TdbLazy::<LexicalUser>::from_instance(&instance_no_id).unwrap();

        // Should panic when accessing ID
        let id_result2 = std::panic::catch_unwind(|| {
            lazy_from_instance.id();
        });
        assert!(
            id_result2.is_err(),
            "id() should panic for instance without ID"
        );

        assert_eq!(lazy_from_instance.get_expect().email, "test@example.com");
    }
}

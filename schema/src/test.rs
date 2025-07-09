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

#[derive(Clone, TerminusDBModel, serde::Serialize, serde::Deserialize)]
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

    #[derive(
        Debug,
        PartialEq,
        Clone,
        TerminusDBModel,
        FromTDBInstance,
        serde::Serialize,
        serde::Deserialize,
    )]
    struct Activity {
        name: String,
        description: String,
    }

    impl<Parent> ToSchemaProperty<Parent> for Activity {
        fn to_property(name: &str) -> Property {
            Property {
                name: name.to_string(),
                r#type: None,
                class: "Activity".to_string(),
            }
        }
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
        let mut lazy = TdbLazy::<Activity>::new(EntityIDFor::new("activity1").unwrap(), None);
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
        let mut lazy = TdbLazy::<Activity>::new(EntityIDFor::new("").unwrap(), None);
        let client = MockClient;

        // This should fail when we try to get the activity because the ID is empty
        let result = lazy.get(&client);
        assert!(result.is_err(), "get() should fail with an empty ID");

        // Test with invalid instance reference
        let non_existent_id = "non_existent_activity_id";
        let mut lazy = TdbLazy::<Activity>::new(EntityIDFor::new(non_existent_id).unwrap(), None);

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

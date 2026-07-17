use crate::*;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

/// A struct representing a key-value entry in a HashMap<String, String>
#[derive(Debug, Clone)]
pub struct HashMapStringEntry {
    pub key: String,
    pub value: String,
}

impl ToTDBSchema for HashMapStringEntry {
    fn to_schema() -> Schema {
        Schema::Class {
            id: "HashMapStringEntry".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: true,
            r#abstract: false,
            inherits: Vec::new(),
            unfoldable: true,
            properties: vec![
                Property {
                    name: "key".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "value".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
            ],
        }
    }

    fn to_schema_tree() -> Vec<Schema> {
        vec![<Self as ToTDBSchema>::to_schema()]
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        let schema = <Self as ToTDBSchema>::to_schema();

        // Only add the schema if it's not already in the collection
        if !collection
            .iter()
            .any(|s| s.class_name() == schema.class_name())
        {
            collection.insert(schema);

            // This is for a concrete type (HashMapStringEntry) so no need to process generic types
        }
    }
}

impl ToTDBInstances for HashMapStringEntry {
    fn to_instance_tree(&self) -> Vec<Instance> {
        vec![self.to_instance(None)]
    }
}

impl ToTDBInstance for HashMapStringEntry {
    fn to_instance(&self, id: Option<String>) -> crate::Instance {
        let mut instance = crate::Instance {
            schema: <Self as ToTDBSchema>::to_schema(),
            id,
            capture: false,
            ref_props: false,
            properties: Default::default(),
        };

        instance.properties.insert(
            "key".to_string(),
            InstanceProperty::Primitive(PrimitiveValue::String(self.key.clone())),
        );

        instance.properties.insert(
            "value".to_string(),
            InstanceProperty::Primitive(PrimitiveValue::String(self.value.clone())),
        );

        instance
    }
}

impl ToSchemaClass for HashMap<String, String> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

// A HashMap<String, String> is stored as a single `sys:JSON` object `{key: value}`
// — consistent with its FromInstanceProperty deserializer (impl/hashmap.rs) and
// with HashMap<String, Value>. (The earlier `HashMapStringEntry` subdocument form
// was write-only: it could not be deserialized back, so the two sides disagreed.)
impl Primitive for HashMap<String, String> {}

impl ToMaybeTDBSchema for HashMap<String, String> {}

impl From<HashMap<String, String>> for PrimitiveValue {
    fn from(map: HashMap<String, String>) -> Self {
        let json_map: serde_json::Map<String, serde_json::Value> = map
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();
        Self::Object(serde_json::Value::Object(json_map))
    }
}

impl From<HashMap<String, String>> for InstanceProperty {
    fn from(map: HashMap<String, String>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<String, String> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

// Add additional implementations for different HashMap types if needed

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
    fn to_class() -> &'static str {
        JSON
    }
}

impl<Parent> ToSchemaProperty<Parent> for HashMap<String, String> {
    fn to_property(field_name: &str) -> Property {
        Property {
            name: field_name.to_string(),
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: "HashMapStringEntry".to_string(),
        }
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<String, String> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        let entries: Vec<InstanceProperty> = self
            .into_iter()
            .map(|(k, v)| {
                let entry = HashMapStringEntry { key: k, value: v };
                InstanceProperty::Relation(RelationValue::One(entry.to_instance(None)))
            })
            .collect();

        InstanceProperty::Any(entries)
    }
}

// Add additional implementations for different HashMap types if needed

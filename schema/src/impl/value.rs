use crate::{
    json::{InstancePropertyFromJson, ToJson},
    ToSchemaClass, JSON,
};
use crate::{
    FromInstanceProperty, InstanceProperty, Property, Schema, ToInstanceProperty, ToSchemaProperty,
};
use serde_json::{Map, Value};

impl ToJson for serde_json::Value {
    fn to_map(&self) -> Map<String, Value> {
        match self.clone() {
            Value::Object(map) => map,
            _ => panic!("Value is not an object"),
        }
    }
}

impl<Parent> ToSchemaProperty<Parent> for Option<serde_json::Value> {
    fn to_property(prop_name: &str) -> Property {
        todo!()
    }
}

// impl<Parent> ToSchemaProperty<Parent> for serde_json::Value {
//     fn to_property(prop_name: &str) -> Property {
//         Property {
//             name: prop_name.to_string(),
//             r#type: None,
//             class: JSON.to_string(),
//         }
//     }
// }

impl ToSchemaClass for serde_json::Value {
    fn to_class() -> &'static str {
        JSON
    }
}

impl<Parent> ToInstanceProperty<Parent> for Option<serde_json::Value> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        todo!()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for Option<serde_json::Value> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}

impl FromInstanceProperty for Option<serde_json::Value> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        todo!()
    }
}

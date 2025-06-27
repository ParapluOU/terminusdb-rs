use crate::{
    json::{InstancePropertyFromJson, ToJson},
    InstanceFromJson, PrimitiveValue, ToSchemaClass, JSON,
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

impl<Parent> ToInstanceProperty<Parent> for serde_json::Value {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::Object(self))
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for serde_json::Value {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl FromInstanceProperty for serde_json::Value {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Object(json)) => Ok(json.clone()),
            _ => anyhow::bail!("Expected object, got {:?}", prop),
        }
    }
}

impl ToSchemaClass for serde_json::Value {
    fn to_class() -> &'static str {
        JSON
    }
}

use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::{FromInstanceProperty, InstanceProperty, PrimitiveValue, Property, Schema, ToInstanceProperty, ToSchemaProperty, STRING};
use crate::json::InstancePropertyFromJson;

const GRAPH_TYPE_SCHEMA: &str = "schema";
const GRAPH_TYPE_INSTANCE: &str = "instance";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum GraphType {
    Schema,
    #[default]
    Instance,
}


impl GraphType {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Schema => GRAPH_TYPE_SCHEMA,
            Self::Instance => GRAPH_TYPE_INSTANCE,
        }
    }
}

impl ToString for GraphType {
    fn to_string(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }
}

impl<Parent> ToSchemaProperty<Parent> for GraphType {
    fn to_property(prop_name: &str) -> Property {
        Property {
            name: prop_name.to_string(),
            r#type: None,
            class: STRING.to_string(),
        }
    }
}

impl<Parent> ToInstanceProperty<Parent> for GraphType {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::String(self.to_string()))
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for GraphType {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}

impl FromInstanceProperty for GraphType {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        todo!()
    }
}
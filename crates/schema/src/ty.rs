use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, PrimitiveValue, Property, Schema, ToInstanceProperty,
    ToSchemaProperty, STRING,
};
use serde_json::Value;

const GRAPH_TYPE_SCHEMA: &str = "schema";
const GRAPH_TYPE_INSTANCE: &str = "instance";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
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

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            GRAPH_TYPE_SCHEMA => Ok(Self::Schema),
            GRAPH_TYPE_INSTANCE => Ok(Self::Instance),
            _ => Err(anyhow::anyhow!(
                "Expected graph type \"schema\" or \"instance\", got {:?}",
                value
            )),
        }
    }
}

impl ToString for GraphType {
    fn to_string(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }
}

impl<Parent> ToSchemaProperty<Parent> for GraphType {
    fn to_schema_property(prop_name: &str) -> Property {
        Property {
            name: prop_name.to_string(),
            r#type: None,
            class: STRING.to_string(),
        }
    }
}

impl<Parent> ToInstanceProperty<Parent> for GraphType {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::String(self.to_string()))
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for GraphType {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            Value::String(value) => {
                Self::parse(&value)?;
                Ok(InstanceProperty::Primitive(PrimitiveValue::String(value)))
            }
            _ => Err(anyhow::anyhow!(
                "Expected graph type string, got {:?}",
                json
            )),
        }
    }
}

impl FromInstanceProperty for GraphType {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(value)) => Self::parse(value),
            _ => Err(anyhow::anyhow!(
                "Expected graph type string primitive, got {:?}",
                prop
            )),
        }
    }
}

impl crate::ToSchemaClass for GraphType {
    fn to_class() -> String {
        STRING.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_type_json_is_exact_and_fail_closed() {
        let schema = <GraphType as InstancePropertyFromJson<()>>::property_from_json(
            Value::String("schema".into()),
        )
        .unwrap();
        let instance = <GraphType as InstancePropertyFromJson<()>>::property_from_json(
            Value::String("instance".into()),
        )
        .unwrap();

        assert_eq!(
            GraphType::from_property(&schema).unwrap(),
            GraphType::Schema
        );
        assert_eq!(
            GraphType::from_property(&instance).unwrap(),
            GraphType::Instance
        );
        assert!(
            <GraphType as InstancePropertyFromJson<()>>::property_from_json(Value::String(
                "Schema".into()
            ))
            .is_err()
        );
        assert!(
            <GraphType as InstancePropertyFromJson<()>>::property_from_json(Value::Null).is_err()
        );
        assert!(
            GraphType::from_property(&InstanceProperty::Primitive(PrimitiveValue::Bool(true)))
                .is_err()
        );
    }
}

use serde_json::Value;
use crate::{InstanceProperty, Primitive, PrimitiveValue, Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass, STRING, FromInstanceProperty};
use uuid::Uuid;
use crate::json::InstancePropertyFromJson;

// Implement ToSchemaClass for Uuid
impl ToSchemaClass for Uuid {
    fn to_class() -> &'static str {
        STRING
    }
}

// Mark Uuid as a primitive type
impl Primitive for Uuid {}

// Implement ToMaybeTDBSchema for Uuid (default impl is fine)
impl ToMaybeTDBSchema for Uuid {}

// Implement conversion from Uuid to PrimitiveValue
impl From<Uuid> for PrimitiveValue {
    fn from(id: Uuid) -> Self {
        Self::String(id.to_string())
    }
}

// Implement conversion from Uuid to InstanceProperty
impl From<Uuid> for InstanceProperty {
    fn from(id: Uuid) -> Self {
        Self::Primitive(id.into())
    }
}

// Implement ToInstanceProperty for Uuid
impl<Parent> ToInstanceProperty<Parent> for Uuid {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for Uuid {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}

impl FromInstanceProperty for Uuid {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => {
                Ok(Uuid::parse_str(s)?)
            }
            _ => Err(anyhow::anyhow!("Expected String primitive")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Property;
    use crate::ToSchemaProperty;

    #[test]
    fn test_uuid_schema_property() {
        let property = <Uuid as ToSchemaProperty<()>>::to_property("id");
        assert_eq!(property.name, "id");
        assert_eq!(property.class, STRING);
    }

    #[test]
    fn test_uuid_instance_property() {
        let uuid = Uuid::new_v4();
        let property = <Uuid as ToInstanceProperty<()>>::to_property(
            uuid,
            "id",
            &Schema::empty_class("Test"),
        );
        match property {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => {
                assert_eq!(s, uuid.to_string());
            }
            _ => panic!("Expected String primitive value"),
        }
    }
}

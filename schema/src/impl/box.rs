use crate::json::InstancePropertyFromJson;
use crate::{
    FromTDBInstance, Instance, InstanceFromJson, PrimitiveValue, RelationValue, Schema, TerminusDBModel,
    ToTDBInstance, ToTDBInstances, ToTDBSchema,
};
use crate::{InstanceProperty, ToInstanceProperty};

// impl<T: ToTDBInstance> ToTDBSchema for Box<T> {
//     fn to_schema_tree() -> Vec<Schema> {
//         T::to_schema_tree()
//     }
// }

impl<T> !InstanceFromJson for Box<T> {}

// Implementation for Box<T> where T implements ToTDBInstance
impl<T: ToTDBInstance> ToTDBInstance for Box<T> {
    fn to_instance(&self, id: Option<String>) -> Instance {
        (**self).to_instance(id)
    }
}

// Implementation of ToTDBInstances for Box<T>
impl<T: ToTDBInstances> ToTDBInstances for Box<T> {
    fn to_instance_tree(&self) -> Vec<Instance> {
        (**self).to_instance_tree()
    }
}

// Implementation of ToInstanceProperty for Option<Box<T>> where T implements ToTDBInstance
impl<T: ToTDBInstance, S> ToInstanceProperty<S> for Option<Box<T>> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        match self {
            Some(boxed) => InstanceProperty::Relation(RelationValue::One(boxed.to_instance(None))),
            None => InstanceProperty::Primitive(PrimitiveValue::Null),
        }
    }
}

// Implementation for Box<T> - delegates to T
impl<T: FromTDBInstance + TerminusDBModel> FromTDBInstance for Box<T> {
    default fn from_instance(instance: &Instance) -> Result<Self, anyhow::Error> {
        T::from_instance(instance).map(Box::new)
    }
}

impl<Parent, T: InstancePropertyFromJson<Parent> + ToTDBInstance> InstancePropertyFromJson<Parent>
    for Box<T>
{
    default fn property_from_json(json: serde_json::Value) -> anyhow::Result<InstanceProperty> {
        T::property_from_json(json)
    }
}

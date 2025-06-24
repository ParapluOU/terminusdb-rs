use crate::{
    FromTDBInstance, Instance, InstanceProperty, Primitive, PrimitiveValue, Property,
    RelationValue, Schema, ToInstanceProperty, ToSchemaClass, ToSchemaProperty, ToTDBInstance,
    ToTDBInstances, ToTDBSchema, TypeFamily,
};
use std::collections::HashSet;

impl<T: ToTDBSchema> ToTDBSchema for Option<T> {
    fn to_schema() -> Schema {
        T::to_schema()
    }

    fn to_schema_tree() -> Vec<Schema> {
        T::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        T::to_schema_tree_mut(collection);
    }
}

// Implement ToInstanceProperty for Option<T> where T implements ToTDBInstance
impl<T: ToTDBInstance, S> ToInstanceProperty<S> for Option<T> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        match self {
            Some(value) => {
                if T::to_schema().is_enum() {
                    return InstanceProperty::Primitive(PrimitiveValue::String(
                        value.to_instance(None).enum_value().unwrap(),
                    ));
                }
                InstanceProperty::Relation(RelationValue::One(value.to_instance(None)))
            }
            None => InstanceProperty::Primitive(PrimitiveValue::Null),
        }
    }
}

impl<T: Primitive, S> ToInstanceProperty<S> for Option<Vec<T>> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitives({
            match self {
                Some(values) => values.into_iter().map(Into::into).collect(),
                None => vec![],
            }
        })
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for Option<Vec<T>> {
    fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            r#type: Some(TypeFamily::Array(1)),
            class: T::to_class().to_string(),
        }
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for Option<T> {
    default fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            r#type: Some(TypeFamily::Optional),
            class: T::to_class().to_string(),
        }
    }
}

// Implement for Option<T>
// impl<T: FromTDBInstance> FromTDBInstance for Option<T> {
//     fn from_instance(instance: &Instance) -> Result<Self, anyhow::Error> {
//         // Otherwise, try to deserialize the value
//         match T::from_instance(instance) {
//             Ok(value) => Ok(Some(value)),
//             Err(e) => Err(e),
//         }
//     }
//
//     fn from_instance_tree(instances: &[Instance]) -> Result<Self, anyhow::Error> {
//         if instances.is_empty() {
//             return Ok(None);
//         }
//
//         match T::from_instance_tree(instances) {
//             Ok(value) => Ok(Some(value)),
//             Err(e) => Err(e),
//         }
//     }
// }

impl<T: ToSchemaClass> ToSchemaClass for Option<T> {
    fn to_class() -> &'static str {
        T::to_class()
    }
}

impl<I: ToTDBInstance> ToTDBInstances for Option<I> {
    default fn to_instance_tree(&self) -> Vec<Instance> {
        self.as_ref()
            .map(|v| v.to_instance_tree())
            .unwrap_or_default()
    }
}

impl<T: FromTDBInstance> FromTDBInstance for Option<T> {
    fn from_instance(instance: &Instance) -> anyhow::Result<Self> {
        T::from_instance(instance).map(Some)
    }
}

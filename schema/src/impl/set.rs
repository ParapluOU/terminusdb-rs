use std::collections::{BTreeSet, HashSet};
use serde_json::Value;
use crate::{FromInstanceProperty, InstanceProperty, Property, Schema, SetCardinality, ToInstanceProperty, ToSchemaClass, ToSchemaProperty, TypeFamily};
use crate::json::InstancePropertyFromJson;

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for HashSet<T> {
    fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            // todo: retrieve set cardinality as macro derive attr?
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: T::to_class().to_string(),
        }
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for BTreeSet<T> {
    fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            // todo: retrieve set cardinality as macro derive attr?
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: T::to_class().to_string(),
        }
    }
}

impl<Parent, T> ToInstanceProperty<Parent> for HashSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        todo!()
    }
}

// impl<Parent, T> ToInstanceProperty<Parent> for BTreeSet<T> {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         todo!()
//     }
// }

impl<T> FromInstanceProperty for BTreeSet<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        todo!()
    }
}

impl<Parent, T> InstancePropertyFromJson<Parent> for BTreeSet<T> where Self: ToInstanceProperty<Parent> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}
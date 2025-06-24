use crate::json::{InstancePropertyFromJson, NestedRef};
use crate::{
    EntityIDFor, FromInstanceProperty, FromTDBInstance, Instance, InstanceFromJson,
    InstanceProperty, Key, PrimitiveValue, Property, RelationValue, Schema, TdbModel,
    ToInstanceProperty, ToSchemaClass, ToSchemaProperty, ToTDBInstance, ToTDBInstances,
    ToTDBSchema, URI,
};
use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryInto;

/// Lazy loading container for TerminusDB instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdbLazy<T: TdbModel> {
    id: EntityIDFor<T>,
    data: Option<Box<T>>,
}

impl<T: TdbModel> TdbLazy<T> {
    pub fn new(id: EntityIDFor<T>, data: Option<T>) -> Self {
        Self {
            id,
            data: data.map(Box::new),
        }
    }

    pub fn new_id(id: &str) -> anyhow::Result<Self> {
        Ok(Self {
            id: EntityIDFor::new(&id)?,
            data: None,
        })
    }

    pub fn new_id_unchecked(id: &str) -> Self {
        (Self {
            id: EntityIDFor::new(&id).unwrap(),
            data: None,
        })
    }

    pub fn new_data(data: T) -> anyhow::Result<Self> {
        data.instance_id()
            .ok_or(anyhow!("could not derive ID"))
            .and_then(|id| Ok(Self::new(id, Some(data))))
    }

    pub fn get(&mut self, client: &impl Client) -> Result<&T, anyhow::Error> {
        if self.data.is_none() {
            // Fetch the data from the store
            let instance = client.get_instance(&self.id.typed())?;
            let data = T::from_instance(&instance)?;
            self.data = Some(Box::new(data));
        }

        // Now we can safely return a reference
        Ok(self.data.as_ref().unwrap())
    }

    pub fn id(&self) -> &EntityIDFor<T> {
        &self.id
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }
}

impl<T: TdbModel> From<T> for TdbLazy<T> {
    fn from(value: T) -> Self {
        Self::new_data(value).unwrap()
    }
}

impl<T: TdbModel> From<EntityIDFor<T>> for TdbLazy<T> {
    fn from(id: EntityIDFor<T>) -> Self {
        Self::new(id, None)
    }
}

impl<T: TdbModel + ToSchemaClass> ToSchemaClass for TdbLazy<T> {
    fn to_class() -> &'static str {
        T::to_class()
    }
}

impl<T: TdbModel> ToTDBSchema for TdbLazy<T> {
    type Type = T::Type;

    fn id() -> Option<String> {
        T::id()
    }

    fn key() -> Key {
        T::key()
    }

    fn to_schema_tree() -> Vec<Schema> {
        T::to_schema_tree()
    }

    // Change to_schema_tree_mut to be a static method
    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        T::to_schema_tree_mut(collection);
    }

    fn properties() -> Option<Vec<Property>> {
        T::properties()
    }

    fn values() -> Option<Vec<URI>> {
        T::values()
    }
}

// impl<Parent, T: ToSchemaProperty<Parent>+TdbModel> ToSchemaProperty<Parent> for TdbLazy<T> {
//     fn to_property(prop_name: &str) -> Property {
//         T::to_property(prop_name)
//     }
// }

// impl<T: TdbModel> FromTDBInstance for TdbLazy<T> {
//     fn from_instance(instance: &Instance) -> Result<Self, anyhow::Error> {
//         if let Some(id) = instance.id.as_ref() {
//             // If this is a full instance (not just a reference), parse it immediately
//             if instance.properties.len() > 2 {
//                 // More than just @type and @id
//                 let data = T::from_instance(instance)?;
//                 Ok(Self {
//                     id: id.clone(),
//                     data: Some(Box::new(data)),
//                 })
//             } else {
//                 // Just a reference, store the id for later
//                 Ok(Self::new(id.clone(), None))
//             }
//         } else {
//             Err(anyhow::anyhow!("Instance has no ID"))
//         }
//     }
//
//     fn from_instance_tree(instances: &[Instance]) -> Result<Self, anyhow::Error> {
//         if instances.is_empty() {
//             return Err(anyhow::anyhow!("Empty instance tree"));
//         }
//         Self::from_instance(&instances[0])
//     }
// }

// todo: somehow be able to determine whether the entity already exists
// so that we dont needlessly nest Instances?
impl<Parent, T: TdbModel> ToInstanceProperty<Parent> for TdbLazy<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        if self.is_loaded() {
            InstanceProperty::Relation(RelationValue::One(
                self.data
                    .as_ref()
                    .unwrap()
                    .to_instance(Some((*self.id).clone())),
            ))
        } else {
            InstanceProperty::Relation(RelationValue::ExternalReference(self.id.to_string()))
        }
    }
}

impl<Parent, T: TdbModel + ToSchemaClass + InstanceFromJson> InstancePropertyFromJson<Parent>
    for TdbLazy<T>
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            // todo: validate?
            Value::String(id) => {
                return Ok(InstanceProperty::Relation(
                    RelationValue::ExternalReference(id),
                ))
            }
            Value::Object(ref map) => {
                if let Ok(NestedRef::<T> {
                    type_name,
                    reference,
                    ..
                }) = serde_json::from_value(json.clone())
                {
                    let target_cls = <T as ToTDBSchema>::schema_name();

                    if !type_name.starts_with(&target_cls) {
                        bail!("Expected type name to start with {}", &target_cls);
                    }

                    return Ok(InstanceProperty::Relation(
                        RelationValue::ExternalReference(reference),
                    ));
                } else if let Ok(instance) =
                    <T as InstanceFromJson>::instance_from_json(json.clone())
                {
                    return Ok(InstanceProperty::Relation(RelationValue::One(instance)));
                }
            }
            _ => {}
        }

        bail!("Expected a string or object, got: {}", json)
    }
}

// impl<T: TdbModel, Parent> ToInstanceProperty<Parent> for TdbLazy<T> {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         if self.is_loaded() {
//             InstanceProperty::Relation(RelationValue::One(self.data.as_ref().unwrap().to_instance(Some(self.id.clone()))))
//         }
//         else {
//             InstanceProperty::Relation(RelationValue::ExternalReference(self.id.clone()))
//         }
//     }
// }

impl<T: TdbModel> FromInstanceProperty for TdbLazy<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(id)) => {
                Ok(Self::new(id.try_into()?, None))
            }
            InstanceProperty::Relation(RelationValue::One(one)) => {
                Ok(Self::new_data(T::from_instance(one)?)?)
            }
            InstanceProperty::Relation(RelationValue::ExternalReference(r)) => {
                Ok(Self::new(r.try_into()?, None))
            }
            _ => {
                bail!(
                    "Expected RelationValue::One or RelationValue::ExternalReference, got: {:#?}",
                    prop
                )
            }
        }
    }
}

impl<T: TdbModel> ToTDBInstances for TdbLazy<T> {
    fn to_instance_tree(&self) -> Vec<Instance> {
        todo!()
    }
}

impl<T: TdbModel> ToTDBInstance for TdbLazy<T> {
    fn to_instance(&self, id: Option<String>) -> Instance {
        if self.is_loaded() {
            self.data.as_ref().unwrap().to_instance(id)
        } else {
            Instance::new_reference::<T>(&self.id)
        }
    }
}

impl<T: TdbModel> PartialEq for TdbLazy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: TdbModel> FromTDBInstance for TdbLazy<T> {
    fn from_instance(instance: &Instance) -> anyhow::Result<Self> {
        if instance.is_reference() {
            return Ok(Self::new(instance.id().unwrap().try_into()?, None));
        }
        let inst = T::from_instance(instance)?;
        let mut lazy = Self::new_data(inst)?;
        if let Some(id) = instance.id() {
            lazy.id = id.try_into()?;
        }
        Ok(lazy)
    }
}

/// Client trait for retrieving instances
pub trait Client {
    fn get_instance(&self, id: &str) -> Result<Instance, anyhow::Error>;
}

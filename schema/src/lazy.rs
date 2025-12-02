use crate::json::{InstancePropertyFromJson, NestedRef};
use crate::{
    EntityIDFor, FromInstanceProperty, FromTDBInstance, Instance, InstanceFromJson,
    InstanceProperty, Key, PrimitiveValue, Property, RelationValue, Schema, TerminusDBModel,
    ToInstanceProperty, ToSchemaClass, ToSchemaProperty, ToTDBInstance, ToTDBInstances,
    ToTDBSchema, URI,
};
use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryInto;

/// Lazy loading container for TerminusDB instances
#[derive(Debug, Clone)]
pub struct TdbLazy<T: TerminusDBModel> {
    id: Option<EntityIDFor<T>>,
    data: Option<Box<T>>,
}

impl<T: TerminusDBModel> TdbLazy<T> {
    pub fn new(id: Option<EntityIDFor<T>>, data: Option<T>) -> Self {
        Self {
            id,
            data: data.map(Box::new),
        }
    }

    pub fn new_id(id: &str) -> anyhow::Result<Self> {
        Ok(Self {
            id: Some(EntityIDFor::new_unchecked(&id)?),
            data: None,
        })
    }

    pub fn new_id_unchecked(id: &str) -> Self {
        Self {
            id: Some(EntityIDFor::new_unchecked(&id).unwrap()),
            data: None,
        }
    }

    pub fn new_data(data: T) -> anyhow::Result<Self> {
        // Try to get the ID, but it's ok if it's not available (e.g., for lexical keys)
        let id = data.instance_id();
        Ok(Self::new(id, Some(data)))
    }

    pub fn get(&mut self, client: &impl Client) -> Result<&T, anyhow::Error> {
        if self.data.is_none() {
            // We need an ID to fetch data
            let id = self.id.as_ref()
                .ok_or_else(|| anyhow!("Cannot fetch data: TdbLazy has neither data nor ID"))?;
            
            // Fetch the data from the store
            let instance = client.get_instance(&id.typed())?;
            let data = T::from_instance(&instance)?;
            self.data = Some(Box::new(data));
        }

        // Now we can safely return a reference
        Ok(self.data.as_ref().unwrap())
    }

    pub fn id(&self) -> &EntityIDFor<T> {
        self.id.as_ref()
            .expect("TdbLazy ID is None - this typically happens with lexical key models before they are saved")
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    /// Get a reference to the inner data, panicking if not loaded.
    ///
    /// # Panics
    /// Panics if the data has not been loaded yet.
    pub fn get_expect(&self) -> &T {
        self.data
            .as_ref()
            .expect("TdbLazy data not loaded")
            .as_ref()
    }

    /// Take ownership of the inner data, panicking if not loaded.
    ///
    /// # Panics
    /// Panics if the data has not been loaded yet.
    pub fn take_expect(self) -> T {
        *self.data.expect("TdbLazy data not loaded")
    }

    /// Convert this TdbLazy to a reference-only variant, discarding loaded data.
    /// This is a no-op if the data is not loaded.
    ///
    /// Useful to prevent nested models from being re-saved in a transaction
    /// when they already exist in the database.
    ///
    /// # Panics
    /// Panics if neither ID nor data is present (cannot create a reference without an ID).
    pub fn make_ref(&mut self) {
        if self.data.is_some() {
            // Ensure we have an ID before discarding data
            if self.id.is_none() {
                // Try to get ID from the data before discarding
                if let Some(id) = self.data.as_ref().and_then(|d| d.instance_id()) {
                    self.id = Some(id);
                } else {
                    panic!("Cannot make_ref: TdbLazy has data but no ID (lexical key not yet computed?)");
                }
            }
            self.data = None;
        }
    }
}

impl<T: TerminusDBModel> From<T> for TdbLazy<T> {
    fn from(value: T) -> Self {
        Self::new_data(value).unwrap()
    }
}

impl<T: TerminusDBModel> From<EntityIDFor<T>> for TdbLazy<T> {
    fn from(id: EntityIDFor<T>) -> Self {
        Self::new(Some(id), None)
    }
}

impl<T: TerminusDBModel + Serialize> Serialize for TdbLazy<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.is_loaded() {
            // When data is loaded, serialize it transparently
            self.data.as_ref().unwrap().serialize(serializer)
        } else {
            // When only ID is present, serialize the ID
            match &self.id {
                Some(id) => id.to_string().serialize(serializer),
                None => serializer.serialize_none()
            }
        }
    }
}

impl<'de, T: TerminusDBModel + DeserializeOwned> Deserialize<'de> for TdbLazy<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize to a generic JSON value
        let value = Value::deserialize(deserializer)?;
        
        match value {
            // If it's a string, treat it as an ID
            Value::String(id_str) => {
                EntityIDFor::<T>::new_unchecked(&id_str)
                    .map(|id| Self::new(Some(id), None))
                    .map_err(serde::de::Error::custom)
            }
            // Otherwise, try to deserialize it as the full data type
            _ => {
                let data: T = serde_json::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Self::new_data(data)
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

impl<T: TerminusDBModel + ToSchemaClass> ToSchemaClass for TdbLazy<T> {
    fn to_class() -> String {
        T::to_class()
    }
}

impl<T: TerminusDBModel> ToTDBSchema for TdbLazy<T> {
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

// impl<Parent, T: ToSchemaProperty<Parent>+TerminusDBModel> ToSchemaProperty<Parent> for TdbLazy<T> {
//     fn to_property(prop_name: &str) -> Property {
//         T::to_property(prop_name)
//     }
// }

// impl<T: TerminusDBModel> FromTDBInstance for TdbLazy<T> {
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
impl<Parent, T: TerminusDBModel> ToInstanceProperty<Parent> for TdbLazy<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        if self.is_loaded() {
            // When loaded, pass the ID if available (it might be None for lexical keys)
            let id = self.id.as_ref().map(|id| id.to_string());
            InstanceProperty::Relation(RelationValue::One(
                self.data
                    .as_ref()
                    .unwrap()
                    .to_instance(id),
            ))
        } else {
            // When not loaded, we need an ID to reference
            match self.id.as_ref() {
                Some(id) => InstanceProperty::Relation(RelationValue::ExternalReference(id.to_string())),
                None => panic!("Cannot convert TdbLazy to property: has neither data nor ID")
            }
        }
    }
}

impl<Parent, T: TerminusDBModel + ToSchemaClass + InstanceFromJson> InstancePropertyFromJson<Parent>
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

// impl<T: TerminusDBModel, Parent> ToInstanceProperty<Parent> for TdbLazy<T> {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         if self.is_loaded() {
//             InstanceProperty::Relation(RelationValue::One(self.data.as_ref().unwrap().to_instance(Some(self.id.clone()))))
//         }
//         else {
//             InstanceProperty::Relation(RelationValue::ExternalReference(self.id.clone()))
//         }
//     }
// }

impl<T: TerminusDBModel> FromInstanceProperty for TdbLazy<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(id)) => {
                Ok(Self::new(Some(id.try_into()?), None))
            }
            InstanceProperty::Relation(RelationValue::One(one)) => {
                Ok(Self::new_data(T::from_instance(one)?)?)
            }
            InstanceProperty::Relation(RelationValue::ExternalReference(r)) => {
                Ok(Self::new(Some(r.try_into()?), None))
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

impl<T: TerminusDBModel> ToTDBInstances for TdbLazy<T> {
    fn to_instance_tree(&self) -> Vec<Instance> {
        todo!()
    }
}

impl<T: TerminusDBModel> ToTDBInstance for TdbLazy<T> {
    fn to_instance(&self, id: Option<String>) -> Instance {
        if self.is_loaded() {
            self.data.as_ref().unwrap().to_instance(id)
        } else {
            // When not loaded, we need an ID to create a reference
            match self.id.as_ref() {
                Some(ref_id) => Instance::new_reference::<T>(&ref_id.to_string()),
                None => panic!("Cannot create instance reference: TdbLazy has neither data nor ID")
            }
        }
    }
}

impl<T: TerminusDBModel> PartialEq for TdbLazy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: TerminusDBModel> Eq for TdbLazy<T> {}

impl<T: TerminusDBModel> std::hash::Hash for TdbLazy<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: TerminusDBModel> FromTDBInstance for TdbLazy<T> {
    fn from_instance(instance: &Instance) -> anyhow::Result<Self> {
        if instance.is_reference() {
            // For references, we need an ID
            match instance.id() {
                Some(id) => Ok(Self::new(Some(id.try_into()?), None)),
                None => bail!("Cannot create TdbLazy from reference without ID")
            }
        } else {
            // For full instances, create from data
            let inst = T::from_instance(instance)?;
            let mut lazy = Self::new_data(inst)?;
            // Update the ID if the instance has one
            if let Some(id) = instance.id() {
                lazy.id = Some(id.try_into()?);
            }
            Ok(lazy)
        }
    }
}

/// Client trait for retrieving instances
pub trait Client {
    fn get_instance(&self, id: &str) -> Result<Instance, anyhow::Error>;
}

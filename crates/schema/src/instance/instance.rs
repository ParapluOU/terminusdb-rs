use crate::{
    deserialize_property, json::InstanceFromJson, InstanceProperty, Key, RelationValue, ToTDBSchema,
};
use crate::{json::ToJson, Property, Schema};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash};
use std::sync::Arc;
use uuid::Uuid;

/// Trait for deserializing a TDBInstance back into a Rust type
pub trait FromTDBInstance: Sized {
    /// Convert a TDBInstance into a Rust type
    fn from_instance(instance: &Instance) -> anyhow::Result<Self>;

    /// Convert a tree of TDBInstances into a Rust type, following references
    /// between instances
    fn from_instance_tree(instances: &[Instance]) -> anyhow::Result<Self> {
        if instances.is_empty() {
            return Err(anyhow::anyhow!("Empty instance tree"));
        }
        Self::from_instance(&instances[0])
    }

    /// Define from_json as a default method relying on InstanceFromJson being implemented separately (by TerminusDBModel derive)
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self>
    where
        Self: InstanceFromJson, // Add bound here to ensure instance_from_json exists
    {
        Self::from_instance(&Self::instance_from_json(json)?)
    }
}

//
// INSTANCE
//

pub trait ToTDBInstances: Send {
    /// Returns a tree of instances that need to be saved, including nested instances
    fn to_instance_tree(&self) -> Vec<Instance>;

    /// Returns a tree of instances that need to be saved, including nested instances,
    /// but with nested relations flattened to references. This is useful when saving
    /// instances to TerminusDB to avoid duplicate saves of nested instances that are
    /// already in the database.
    fn to_instance_tree_flatten(&self, for_transaction: bool) -> Vec<Instance> {
        let mut instances = self.to_instance_tree();
        for instance in &mut instances {
            instance.flatten(for_transaction);
        }
        // Filter out subdocuments and TaggedUnions with subdocument variants
        // - they should remain nested within their parent instances
        instances
            .into_iter()
            .filter(|inst| !inst.should_remain_embedded())
            .collect()
    }

    /// make into trait object so that we can add different model types to a Vec
    /// and insert in a single query for performance
    fn boxed(self) -> Box<dyn ToTDBInstances>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

pub trait IntoBoxedTDBInstances {
    fn into_boxed(self) -> Box<dyn ToTDBInstances>;
}

impl<T: ToTDBInstances + 'static> IntoBoxedTDBInstances for T {
    fn into_boxed(self) -> Box<dyn ToTDBInstances> {
        self.boxed()
    }
}

/// Helper function to build an instance tree from an Instance
/// This extracts all related instances recursively from a given instance
///
/// We use this helper function instead of a blanket implementation for several reasons:
///
/// 1. **Type Safety**: A blanket implementation would require complex trait bounds to ensure
///    the type implements both ToTDBInstance and ToTDBInstances, which could lead to
///    implementation conflicts.
///
/// 2. **Flexibility**: Different types may need custom tree building logic. By using a
///    helper function, we allow types to override the default behavior while still
///    having access to the common implementation.
///
/// 3. **Implementation Conflicts**: A blanket implementation would conflict with existing
///    implementations for common types like Option<T>, Vec<T>, etc. The helper function
///    approach avoids these conflicts by allowing each type to implement ToTDBInstances
///    independently.
///
/// 4. **Maintainability**: The tree building logic is complex and needs to handle various
///    edge cases. Having it in one place makes it easier to maintain and update.
pub fn build_instance_tree(instance: &Instance) -> Vec<Instance> {
    let mut instances = vec![instance.clone()];
    let mut additional_instances = Vec::new();

    // For each field that's a complex type (implements ToTDBInstances),
    // collect its instances and add them to the result
    for (_, prop) in &instance.properties {
        if let InstanceProperty::Relation(relation) = prop {
            match relation {
                RelationValue::One(instance) => {
                    additional_instances.push(instance.clone());
                }
                RelationValue::More(instances_vec) => {
                    additional_instances.extend(instances_vec.clone());
                }
                _ => {}
            }
        } else if let InstanceProperty::Relations(relations) = prop {
            for relation in relations {
                if let RelationValue::One(instance) = relation {
                    additional_instances.push(instance.clone());
                } else if let RelationValue::More(instances_vec) = relation {
                    additional_instances.extend(instances_vec.clone());
                }
            }
        }
    }

    // do call recursively for nested instances
    additional_instances = additional_instances
        .iter()
        .map(|i| build_instance_tree(i))
        .flatten()
        .collect();

    // include the self
    instances.extend(additional_instances);
    instances
}

impl ToTDBInstances for Instance {
    fn to_instance_tree(&self) -> Vec<Instance> {
        build_instance_tree(self)
    }
}

pub trait ToTDBInstance: ToTDBSchema + ToTDBInstances {
    fn to_instance(&self, id: Option<String>) -> Instance;

    fn to_json_tree(&self) -> Vec<serde_json::Value> {
        self.to_instance(None).to_json_tree()
    }

    fn to_json(&self) -> serde_json::Value {
        self.to_instance(None).to_json()
    }

    fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.to_json())
            .expect("Failed to serialize instance to JSON string")
    }

    // fn from_value(
    //     instance: serde_json::Value,
    // ) -> anyhow::Result<<<Self as ToRelational>::Relational as Relational<Model = Self>> + Cacheable,
    // {
    //     let (target_id, mut flattened) = flatten_json(&instance);
    //
    //     let target_rel: <Self as ToRelational>::Relational =
    //         serde_json::from_value(flattened.remove(&target_id).unwrap()).unwrap();
    //
    //     // dbg!(&target_id);
    //
    //     let cache = SimpleCache::new();
    //
    //     Ok(target_rel
    //         .to_model_with_map(cache.clone(), flattened)
    //         .unwrap())
    // }

    fn hash_key_id(&self) -> String
    where
        Self: Serialize + Debug + Sized,
    {
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        (serde_json::to_string(self).unwrap()).hash(&mut hasher);
        let hash = hasher.finish().to_string();

        <Self as ToTDBSchema>::to_schema().format_id(&hash)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Instance {
    /// the @type key has to be derived from the schema
    pub schema: Schema,
    /// whether a static ID for this instance should be included
    pub id: Option<String>,
    /// whether this instance is referenced by another instance in the same transaction
    pub capture: bool,
    /// whether the instance should include @ref references for referenced documents
    /// meaning that a {@ref: *captured sub document id*} is used instead of the
    /// URI direct as property.
    pub ref_props: bool,
    /// the propertiy fields that should be defined on the instance
    /// without the @ prefix
    pub properties: BTreeMap<String, InstanceProperty>,
}

impl Instance {
    pub fn new_reference<T: ToTDBSchema>(id: &str) -> Self {
        Self {
            schema: T::to_schema(),
            id: Some(id.to_string()),
            capture: false,
            ref_props: false,
            properties: BTreeMap::new(),
        }
    }

    pub fn from_json_with_schema<T: crate::ToTDBSchema>(
        json: serde_json::Value,
    ) -> anyhow::Result<Self> {
        // First deserialize to a partial instance without schema
        #[derive(Deserialize)]
        struct PartialInstance {
            #[serde(rename = "@id")]
            id: Option<String>,
            #[serde(flatten)]
            properties: BTreeMap<String, Value>,
        }

        let partial: PartialInstance = serde_json::from_value(json)?;

        // Convert properties to InstanceProperty
        let mut instance_props = BTreeMap::new();
        for (key, value) in partial.properties {
            if !key.starts_with('@') {
                // Skip metadata properties
                instance_props.insert(key, deserialize_property(value)?);
            }
        }

        Ok(Self {
            schema: T::to_schema(),
            id: partial.id,
            capture: false,
            ref_props: false,
            properties: instance_props,
        })
    }

    /// make sure the schema class prefix is set when key type is random
    pub fn set_random_key_prefix(&mut self) {
        // Don't set IDs for subdocuments
        if self.schema.is_subdocument() {
            return;
        }

        let class = self.schema.class_name();
        // if self.schema.is_key_random()
        //     && let Some(id) = self.id.as_ref()
        //     && !id.starts_with(class)
        // {
        //     self.id = Some(format!("{}/{}", class, self.id.as_ref().unwrap()));
        // }

        if self.schema.is_key_random()
            && self
                .id
                .as_ref()
                .map(|i| !i.starts_with(class))
                .unwrap_or_default()
        {
            self.id = Some(format!("{}/{}", class, self.id.as_ref().unwrap()));
        }
    }

    pub fn is_reference(&self) -> bool {
        self.id.is_some() && self.properties.is_empty()
    }

    pub fn is_enum(&self) -> bool {
        self.schema.is_enum()
    }

    pub fn is_tagged_union(&self) -> bool {
        self.schema.is_tagged_union()
    }

    // todo: fix name as that actually returns the property name, NOT the value
    pub fn enum_value(&self) -> Option<String> {
        for prop in self.properties.keys() {
            return prop.clone().into();
        }
        None
    }

    pub fn get_property(&self, key: &str) -> Option<&InstanceProperty> {
        self.properties.get(key)
    }

    pub fn has_property(&self, name: &str) -> bool {
        self.get_property(name).is_some()
    }

    pub fn set_id_capture(mut self, v: bool) -> Self {
        self.capture = v;
        self
    }

    pub fn set_relations_reffed(mut self, v: bool) -> Self {
        self.ref_props = v;
        self
    }

    pub fn id(&self) -> Option<&String> {
        self.id.as_ref()
    }

    pub fn gen_id(&self) -> Option<String> {
        if self.has_id() {
            return self.id.clone();
        }

        // TaggedUnion themselves cant have unique ID's
        if self.is_tagged_union() {
            // TaggedUnions have exactly one property containing the active variant
            for (_, prop) in &self.properties {
                if let InstanceProperty::Relation(RelationValue::One(variant_inst)) = prop {
                    // todo: use helper
                    let gid = variant_inst.gen_id()?;
                    return Some(if !gid.contains("/") {
                        (format!("{}/{}", self.schema.class_name(), gid))
                    } else {
                        gid
                    });
                }
            }
        }

        match self.schema.key() {
            Some(Key::Random) => Some(Uuid::new_v4().to_string()),
            Some(Key::Hash(fields)) => {
                // For hash keys with specific fields, ID is server-generated based on a hash of specified field values
                None
            }
            Some(Key::Lexical(_)) => {
                // For lexical keys, ID is server-generated based on key fields
                None
            }
            Some(Key::ValueHash) => {
                // For value_hash keys, ID is server-generated based on a hash of all field values
                None
            }
            _ => None,
        }
    }

    pub fn has_id(&self) -> bool {
        self.id.is_some()
    }

    pub fn id_contains(&self, s: &str) -> bool {
        self.id.as_ref().map(|i| i.contains(s)).unwrap_or_default()
    }

    pub fn is_of_type<T: ToTDBInstance>(&self) -> bool {
        self.schema.is_of_type::<T>()
    }

    /// Checks if this instance should remain embedded (not be flattened to a reference).
    /// This includes regular subdocuments and TaggedUnions containing subdocument variants.
    pub fn should_remain_embedded(&self) -> bool {
        // Check if this instance is a subdocument
        if self.schema.is_subdocument() {
            return true;
        }

        // For TaggedUnions, check if the active variant is a subdocument
        if self.schema.is_tagged_union() {
            // TaggedUnions have exactly one property containing the active variant
            for (_, prop) in &self.properties {
                if let InstanceProperty::Relation(RelationValue::One(variant_inst)) = prop {
                    if variant_inst.schema.is_subdocument() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Flattens all nested relations to references by extracting their IDs.
    /// This is useful when saving instances to TerminusDB to avoid duplicate saves
    /// of nested instances that are already in the database.
    /// Returns a Vec of all instances that were removed from nesting.
    pub fn flatten(&mut self, for_transaction: bool) -> Vec<Instance> {
        let mut removed = Vec::new();
        for (_, prop) in self.properties.iter_mut() {
            removed.extend(prop.flatten(for_transaction));
        }
        removed
    }

    pub fn to_json_tree(&self) -> Vec<serde_json::Value> {
        // self.to_instance_tree_flatten().into_iter().map(|i| i.to_json()).collect()
        self.to_instance_tree()
            .into_iter()
            .map(|i| i.to_json())
            .collect::<Vec<_>>()
    }
}

impl ToJson for Instance {
    fn to_json(&self) -> serde_json::Value {
        if self.is_enum() {
            return self.enum_value().expect("should not happen; enum instances should always have a proprty with the actual variant name").into();
        }

        if self.is_reference() {
            return Value::String(self.id().cloned().unwrap());
        }

        // default
        serde_json::Value::Object(self.to_map())
    }

    fn to_map(&self) -> Map<String, Value> {
        assert!(!self.is_enum());

        let mut map = serde_json::Map::new();

        // Use SHORT names everywhere - Context @schema will expand them
        // This allows Context to handle all namespace resolution

        // class type name - use short name, Context will expand
        map.insert(
            "@type".to_string(),
            self.schema.class_name().clone().into(),
        );

        // insert id if we have one
        if let Some(id) = self.id.clone() {
            map.insert("@id".to_string(), id.clone().into());

            // allow hardcoded referencing if we are using Rust-hash predefined keys
            if self.schema.is_key_random() && self.capture {
                map.insert("@capture".to_string(), id.into());
            }
        }

        // Use SHORT property names - TerminusDB applies context expansion
        // The @type uses full URI but property names use short names that
        // the context will expand to match the schema
        for (propkey, propval) in &self.properties {
            map.insert(propkey.clone(), propval.clone().into());
        }

        map
    }
}

impl Instance {
    /// Convert instance to JSON with fully-qualified type URI for multi-namespace support.
    ///
    /// This method produces JSON where the `@type` field uses the fully-qualified
    /// class URI (e.g., `http://example.com/book#DocumentType`) rather than the
    /// short name. This allows instances from different namespaces with the same
    /// class names to coexist in the same TerminusDB database.
    ///
    /// Property names remain short as TerminusDB validates them against the schema.
    pub fn to_namespaced_json(&self) -> serde_json::Value {
        if self.is_enum() {
            return self
                .enum_value()
                .expect("enum instances should always have a property with the variant name")
                .into();
        }

        if self.is_reference() {
            return Value::String(self.id().cloned().unwrap());
        }

        let mut map = serde_json::Map::new();
        let namespace = self.schema.namespace_base();

        // Use fully-qualified type if namespace is present
        let type_name = if let Some(ns) = namespace {
            format!("{}{}", ns, self.schema.class_name())
        } else {
            self.schema.class_name().clone()
        };
        map.insert("@type".to_string(), type_name.into());

        // Insert id if we have one
        if let Some(id) = self.id.clone() {
            map.insert("@id".to_string(), id.clone().into());

            if self.schema.is_key_random() && self.capture {
                map.insert("@capture".to_string(), id.into());
            }
        }

        // Property names remain short - TerminusDB validates against schema
        for (propkey, propval) in &self.properties {
            map.insert(propkey.clone(), propval.clone().into());
        }

        serde_json::Value::Object(map)
    }
}

impl Serialize for Instance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_json().serialize(serializer)
    }
}

//! Implementation of TerminusDB schema traits for HashMap<Uuid, T>
//!
//! This module provides support for using HashMap with Uuid keys in TerminusDB models.
//! The HashMap is serialized as a JSON object with UUID string keys.

use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, Primitive, PrimitiveValue, Schema, ToInstanceProperty,
    ToMaybeTDBSchema, ToSchemaClass, JSON,
};
use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// === HashMap<Uuid, serde_json::Value> ===

impl ToSchemaClass for HashMap<Uuid, Value> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl Primitive for HashMap<Uuid, Value> {}
impl ToMaybeTDBSchema for HashMap<Uuid, Value> {}

impl From<HashMap<Uuid, Value>> for PrimitiveValue {
    fn from(map: HashMap<Uuid, Value>) -> Self {
        let json_value = Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        );
        Self::Object(json_value)
    }
}

impl From<HashMap<Uuid, Value>> for InstanceProperty {
    fn from(map: HashMap<Uuid, Value>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<Uuid, Value> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<Uuid, Value> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl FromInstanceProperty for HashMap<Uuid, Value> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let uuid = Uuid::parse_str(key_str)
                    .map_err(|e| anyhow!("Invalid UUID key '{}': {}", key_str, e))?;
                result.insert(uuid, value.clone());
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "Expected Object primitive, got {:?}",
                prop
            ))
        }
    }
}

// === HashMap<Uuid, String> ===

impl ToSchemaClass for HashMap<Uuid, String> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl Primitive for HashMap<Uuid, String> {}
impl ToMaybeTDBSchema for HashMap<Uuid, String> {}

impl From<HashMap<Uuid, String>> for PrimitiveValue {
    fn from(map: HashMap<Uuid, String>) -> Self {
        let json_value = Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_string(), Value::String(v)))
                .collect(),
        );
        Self::Object(json_value)
    }
}

impl From<HashMap<Uuid, String>> for InstanceProperty {
    fn from(map: HashMap<Uuid, String>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<Uuid, String> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<Uuid, String> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl FromInstanceProperty for HashMap<Uuid, String> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let uuid = Uuid::parse_str(key_str)
                    .map_err(|e| anyhow!("Invalid UUID key '{}': {}", key_str, e))?;
                let value_str = value.as_str().ok_or(anyhow!(
                    "Expected string value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;
                result.insert(uuid, value_str.to_string());
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "Expected Object primitive, got {:?}",
                prop
            ))
        }
    }
}

// === HashMap<Uuid, i32> ===

impl ToSchemaClass for HashMap<Uuid, i32> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl Primitive for HashMap<Uuid, i32> {}
impl ToMaybeTDBSchema for HashMap<Uuid, i32> {}

impl From<HashMap<Uuid, i32>> for PrimitiveValue {
    fn from(map: HashMap<Uuid, i32>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(key.to_string(), Value::Number(value.into()));
        }
        Self::Object(Value::Object(json_map))
    }
}

impl From<HashMap<Uuid, i32>> for InstanceProperty {
    fn from(map: HashMap<Uuid, i32>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<Uuid, i32> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<Uuid, i32> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl FromInstanceProperty for HashMap<Uuid, i32> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let uuid = Uuid::parse_str(key_str)
                    .map_err(|e| anyhow!("Invalid UUID key '{}': {}", key_str, e))?;
                let value_i32 = value.as_i64().ok_or(anyhow!(
                    "Expected integer value for key '{}', got {:?}",
                    key_str,
                    value
                ))? as i32;
                result.insert(uuid, value_i32);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "Expected Object primitive, got {:?}",
                prop
            ))
        }
    }
}

// === HashMap<Uuid, bool> ===

impl ToSchemaClass for HashMap<Uuid, bool> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl Primitive for HashMap<Uuid, bool> {}
impl ToMaybeTDBSchema for HashMap<Uuid, bool> {}

impl From<HashMap<Uuid, bool>> for PrimitiveValue {
    fn from(map: HashMap<Uuid, bool>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(key.to_string(), Value::Bool(value));
        }
        Self::Object(Value::Object(json_map))
    }
}

impl From<HashMap<Uuid, bool>> for InstanceProperty {
    fn from(map: HashMap<Uuid, bool>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for HashMap<Uuid, bool> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<Uuid, bool> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl FromInstanceProperty for HashMap<Uuid, bool> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let uuid = Uuid::parse_str(key_str)
                    .map_err(|e| anyhow!("Invalid UUID key '{}': {}", key_str, e))?;
                let value_bool = value.as_bool().ok_or(anyhow!(
                    "Expected boolean value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;
                result.insert(uuid, value_bool);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "Expected Object primitive, got {:?}",
                prop
            ))
        }
    }
}

// === HashMap<Uuid, T> where T: Serialize + DeserializeOwned (generic impl for custom types) ===
// This provides support for HashMap<Uuid, CustomStruct> where the struct can be serialized to JSON

/// A marker wrapper type that allows using HashMap<Uuid, T> with serializable types.
/// Use this when your value type T implements Serialize and DeserializeOwned.
#[derive(Debug, Clone, PartialEq)]
pub struct HashMapUuid<T>(pub HashMap<Uuid, T>);

impl<T> From<HashMap<Uuid, T>> for HashMapUuid<T> {
    fn from(map: HashMap<Uuid, T>) -> Self {
        HashMapUuid(map)
    }
}

impl<T> From<HashMapUuid<T>> for HashMap<Uuid, T> {
    fn from(wrapper: HashMapUuid<T>) -> Self {
        wrapper.0
    }
}

impl<T: Serialize + DeserializeOwned> ToSchemaClass for HashMapUuid<T> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: Serialize + DeserializeOwned> Primitive for HashMapUuid<T> {}
impl<T: Serialize + DeserializeOwned> ToMaybeTDBSchema for HashMapUuid<T> {}

impl<T: Serialize + DeserializeOwned> From<HashMapUuid<T>> for PrimitiveValue {
    fn from(wrapper: HashMapUuid<T>) -> Self {
        let json_value = Value::Object(
            wrapper.0
                .into_iter()
                .map(|(k, v)| (k.to_string(), serde_json::to_value(v).unwrap_or(Value::Null)))
                .collect(),
        );
        Self::Object(json_value)
    }
}

impl<T: Serialize + DeserializeOwned> From<HashMapUuid<T>> for InstanceProperty {
    fn from(wrapper: HashMapUuid<T>) -> Self {
        Self::Primitive(wrapper.into())
    }
}

impl<Parent, T: Serialize + DeserializeOwned> ToInstanceProperty<Parent> for HashMapUuid<T> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent, T: Serialize + DeserializeOwned> InstancePropertyFromJson<Parent> for HashMapUuid<T> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl<T: Serialize + DeserializeOwned> FromInstanceProperty for HashMapUuid<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let uuid = Uuid::parse_str(key_str)
                    .map_err(|e| anyhow!("Invalid UUID key '{}': {}", key_str, e))?;
                let typed_value: T = serde_json::from_value(value.clone())
                    .map_err(|e| anyhow!("Failed to deserialize value for key '{}': {}", key_str, e))?;
                result.insert(uuid, typed_value);
            }

            Ok(HashMapUuid(result))
        } else {
            Err(anyhow::anyhow!(
                "Expected Object primitive, got {:?}",
                prop
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as terminusdb_schema;
    use crate::ToSchemaProperty;

    #[test]
    fn test_hashmap_uuid_string_schema_property() {
        let property = <HashMap<Uuid, String> as ToSchemaProperty<()>>::to_property("uuid_map");
        assert_eq!(property.name, "uuid_map");
        assert_eq!(property.class, JSON);
    }

    #[test]
    fn test_hashmap_uuid_string_round_trip() {
        let mut map = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        map.insert(uuid1, "value1".to_string());
        map.insert(uuid2, "value2".to_string());

        // Convert to InstanceProperty
        let property = <HashMap<Uuid, String> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "uuid_map",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<Uuid, String>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&uuid1), Some(&"value1".to_string()));
        assert_eq!(restored_map.get(&uuid2), Some(&"value2".to_string()));
    }

    #[test]
    fn test_hashmap_uuid_value_round_trip() {
        let mut map = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        map.insert(uuid1, Value::String("value1".to_string()));
        map.insert(uuid2, Value::Number(42.into()));

        // Convert to InstanceProperty
        let property = <HashMap<Uuid, Value> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "uuid_map",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<Uuid, Value>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&uuid1), Some(&Value::String("value1".to_string())));
        assert_eq!(restored_map.get(&uuid2), Some(&Value::Number(42.into())));
    }

    #[test]
    fn test_hashmap_uuid_i32_round_trip() {
        let mut map = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        map.insert(uuid1, 42);
        map.insert(uuid2, 100);

        // Convert to InstanceProperty
        let property = <HashMap<Uuid, i32> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "scores",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<Uuid, i32>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&uuid1), Some(&42));
        assert_eq!(restored_map.get(&uuid2), Some(&100));
    }

    #[test]
    fn test_hashmap_uuid_bool_round_trip() {
        let mut map = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        map.insert(uuid1, true);
        map.insert(uuid2, false);

        // Convert to InstanceProperty
        let property = <HashMap<Uuid, bool> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "flags",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<Uuid, bool>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&uuid1), Some(&true));
        assert_eq!(restored_map.get(&uuid2), Some(&false));
    }
}

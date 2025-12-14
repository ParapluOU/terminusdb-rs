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

// === Generic HashMap<Uuid, T> implementation for any serializable type ===

impl<T> ToSchemaClass for HashMap<Uuid, T> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: Serialize + DeserializeOwned> Primitive for HashMap<Uuid, T> {}

impl<T: Serialize + DeserializeOwned> From<HashMap<Uuid, T>> for PrimitiveValue {
    fn from(map: HashMap<Uuid, T>) -> Self {
        let json_value = Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_string(), serde_json::to_value(v).unwrap_or(Value::Null)))
                .collect(),
        );
        Self::Object(json_value)
    }
}

impl<T: Serialize + DeserializeOwned> From<HashMap<Uuid, T>> for InstanceProperty {
    fn from(map: HashMap<Uuid, T>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<Parent, T: Serialize + DeserializeOwned> ToInstanceProperty<Parent> for HashMap<Uuid, T> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent, T: Serialize + DeserializeOwned> InstancePropertyFromJson<Parent> for HashMap<Uuid, T> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let _map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

impl<T: Serialize + DeserializeOwned> FromInstanceProperty for HashMap<Uuid, T> {
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

            Ok(result)
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
    use serde::{Deserialize, Serialize};

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

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct CustomStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn test_hashmap_uuid_custom_struct_round_trip() {
        let mut map = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        map.insert(uuid1, CustomStruct { name: "first".to_string(), value: 1 });
        map.insert(uuid2, CustomStruct { name: "second".to_string(), value: 2 });

        // Convert to InstanceProperty
        let property = <HashMap<Uuid, CustomStruct> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "structs",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<Uuid, CustomStruct>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&uuid1), Some(&CustomStruct { name: "first".to_string(), value: 1 }));
        assert_eq!(restored_map.get(&uuid2), Some(&CustomStruct { name: "second".to_string(), value: 2 }));
    }
}

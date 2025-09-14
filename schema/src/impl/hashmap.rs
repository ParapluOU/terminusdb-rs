use crate::json::InstancePropertyFromJson;
use crate::{
    EntityIDFor, FromInstanceProperty, FromTDBInstance, Instance, InstanceProperty, Primitive,
    PrimitiveValue, Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass, ToTDBInstance,
    ToTDBSchema, JSON,
};
use anyhow::anyhow;
use chrono::{DateTime, NaiveTime, Utc};
use serde;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

// todo: this impl is redundant with the Vec<_> and BTreeMap ones.
// what we _should_ do is implement Container1<T> for these as a marker
// and do a blanket impl on that
impl<K, T: ToTDBSchema> ToTDBSchema for HashMap<K, T> {
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

// Implement ToSchemaClass for HashMap<String, Value>
impl ToSchemaClass for HashMap<String, Value> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

// Mark HashMap<String, Value> as a primitive type
impl Primitive for HashMap<String, Value> {}

// Implement ToMaybeTDBSchema for HashMap<String, Value> (default impl is fine)
impl ToMaybeTDBSchema for HashMap<String, Value> {}

// Implement conversion from HashMap<String, Value> to PrimitiveValue
impl From<HashMap<String, Value>> for PrimitiveValue {
    fn from(map: HashMap<String, Value>) -> Self {
        let json_map: serde_json::Map<String, Value> = map.into_iter().collect();
        Self::Object(Value::Object(json_map))
    }
}

// Implement conversion from HashMap<String, Value> to InstanceProperty
impl From<HashMap<String, Value>> for InstanceProperty {
    fn from(map: HashMap<String, Value>) -> Self {
        Self::Primitive(map.into())
    }
}

// Implement ToInstanceProperty for HashMap<String, Value>
impl<Parent> ToInstanceProperty<Parent> for HashMap<String, Value> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<String, Value> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            Value::Object(map) => Ok(InstanceProperty::Primitive(PrimitiveValue::Object(
                Value::Object(map),
            ))),
            _ => Err(anyhow!("Expected JSON object")),
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for HashMap<String, String> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            Value::Object(map) => {
                // Validate that all values are strings
                for (key, value) in &map {
                    if !value.is_string() {
                        return Err(anyhow!(
                            "Expected string value for key '{}', got {:?}",
                            key,
                            value
                        ));
                    }
                }
                Ok(InstanceProperty::Primitive(PrimitiveValue::Object(
                    Value::Object(map),
                )))
            }
            _ => Err(anyhow!("Expected JSON object")),
        }
    }
}

// HashMap<String, serde_json::Value>
// impl FromTDBInstance for std::collections::HashMap<String, serde_json::Value> {
//     fn from_instance(instance: &Instance) -> Result<Self, anyhow::Error> {
//         let mut map = HashMap::new();
//
//         // Process each property and convert it to a serde_json::Value
//         for (key, prop) in &instance.properties {
//             if key.starts_with('@') {
//                 continue; // Skip metadata properties
//             }
//
//             // Convert the property to a serde_json::Value
//             let value = match prop {
//                 InstanceProperty::Primitive(prim) => match prim {
//                     PrimitiveValue::String(s) => serde_json::Value::String(s.clone()),
//                     PrimitiveValue::Number(n) => serde_json::Value::Number(n.clone()),
//                     PrimitiveValue::Bool(b) => serde_json::Value::Bool(*b),
//                     PrimitiveValue::Object(v) => v.clone(),
//                     PrimitiveValue::Unit => serde_json::Value::Array(vec![]),
//                     PrimitiveValue::Null => serde_json::Value::Null,
//                 },
//                 InstanceProperty::Primitives(prims) => {
//                     let values: Vec<serde_json::Value> = prims
//                         .iter()
//                         .map(|p| match p {
//                             PrimitiveValue::String(s) => serde_json::Value::String(s.clone()),
//                             PrimitiveValue::Number(n) => serde_json::Value::Number(n.clone()),
//                             PrimitiveValue::Bool(b) => serde_json::Value::Bool(*b),
//                             PrimitiveValue::Object(v) => v.clone(),
//                             PrimitiveValue::Unit => serde_json::Value::Array(vec![]),
//                             PrimitiveValue::Null => serde_json::Value::Null,
//                         })
//                         .collect();
//                     serde_json::Value::Array(values)
//                 }
//                 _ => {
//                     // For complex properties, try to convert them to JSON
//                     let json_value: serde_json::Value = prop.clone().into();
//                     json_value
//                 }
//             };
//
//             map.insert(key.clone(), value);
//         }
//
//         Ok(map)
//     }
// }

impl FromInstanceProperty for HashMap<String, serde_json::Value> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            Ok(obj
                .as_object()
                .ok_or(anyhow!("Expected JSON object"))?
                .clone()
                .into_iter()
                .collect())
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl FromInstanceProperty for HashMap<String, String> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            obj.as_object()
                .ok_or(anyhow!("Expected JSON object"))?
                .clone()
                .into_iter()
                .map(|(k, v)| {
                    v.as_str()
                        .ok_or(anyhow!("expected string but got: {:?}", v))
                        .map(|s| (k, s.to_string()))
                })
                .collect()
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

// === EntityIDFor<T> as HashMap Keys ===

// Specific implementations for HashMap<EntityIDFor<T>, String>
impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, String> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, String> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, String> {}

// Specific implementations for HashMap<EntityIDFor<T>, serde_json::Value>
impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, serde_json::Value> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, serde_json::Value> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, serde_json::Value> {}

// Implement conversion from HashMap<EntityIDFor<T>, String> to PrimitiveValue
impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, String>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, String>) -> Self {
        // Convert HashMap to a JSON object, using EntityIDFor's string representation as keys
        let json_map: serde_json::Map<String, serde_json::Value> = map
            .into_iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v)))
            .collect();
        let json_value = Value::Object(json_map);
        Self::Object(json_value)
    }
}

// Implement conversion from HashMap<EntityIDFor<T>, serde_json::Value> to PrimitiveValue
impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, serde_json::Value>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, serde_json::Value>) -> Self {
        // Convert HashMap to a JSON object, using EntityIDFor's string representation as keys
        let json_map: serde_json::Map<String, serde_json::Value> =
            map.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let json_value = Value::Object(json_map);
        Self::Object(json_value)
    }
}

// Implement conversion from HashMap<EntityIDFor<T>, String> to InstanceProperty
impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, String>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, String>) -> Self {
        Self::Primitive(map.into())
    }
}

// Implement conversion from HashMap<EntityIDFor<T>, serde_json::Value> to InstanceProperty
impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, serde_json::Value>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, serde_json::Value>) -> Self {
        Self::Primitive(map.into())
    }
}

// Implement ToInstanceProperty for HashMap<EntityIDFor<T>, String>
impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, String> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

// Implement ToInstanceProperty for HashMap<EntityIDFor<T>, serde_json::Value>
impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent>
    for HashMap<EntityIDFor<T>, serde_json::Value>
{
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

// Implement FromInstanceProperty for HashMap<EntityIDFor<T>, String>
impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, String> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                let value_str = value.as_str().ok_or(anyhow!(
                    "Expected string value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;
                result.insert(entity_id, value_str.to_string());
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

// Implement FromInstanceProperty for HashMap<EntityIDFor<T>, serde_json::Value>
impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, serde_json::Value> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                result.insert(entity_id, value.clone());
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

// Implement InstancePropertyFromJson for HashMap<EntityIDFor<T>, String>
impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent> for HashMap<EntityIDFor<T>, String> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// Implement InstancePropertyFromJson for HashMap<EntityIDFor<T>, serde_json::Value>
impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent>
    for HashMap<EntityIDFor<T>, serde_json::Value>
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// Specific implementations for EntityIDFor<T> with common value types

// HashMap<EntityIDFor<T>, i32>

impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, i32> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, i32> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, i32> {}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, i32>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, i32>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(key.to_string(), Value::Number(value.into()));
        }
        Self::Object(Value::Object(json_map))
    }
}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, i32>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, i32>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, i32> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, i32> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                let value_i32 = value.as_i64().ok_or(anyhow!(
                    "Expected integer value for key '{}', got {:?}",
                    key_str,
                    value
                ))? as i32;
                result.insert(entity_id, value_i32);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent> for HashMap<EntityIDFor<T>, i32> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// HashMap<EntityIDFor<T>, bool>

impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, bool> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, bool> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, bool> {}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, bool>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, bool>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(key.to_string(), Value::Bool(value));
        }
        Self::Object(Value::Object(json_map))
    }
}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, bool>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, bool>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, bool> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, bool> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                let value_bool = value.as_bool().ok_or(anyhow!(
                    "Expected boolean value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;
                result.insert(entity_id, value_bool);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent> for HashMap<EntityIDFor<T>, bool> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// HashMap<EntityIDFor<T>, DateTime<Utc>>

impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, DateTime<Utc>> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, DateTime<Utc>> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, DateTime<Utc>> {}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, DateTime<Utc>>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, DateTime<Utc>>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(key.to_string(), Value::String(value.to_rfc3339()));
        }
        Self::Object(Value::Object(json_map))
    }
}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, DateTime<Utc>>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, DateTime<Utc>>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, DateTime<Utc>> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, DateTime<Utc>> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                let value_str = value.as_str().ok_or(anyhow!(
                    "Expected string value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;
                let datetime: DateTime<Utc> = value_str.parse().map_err(|e| {
                    anyhow!("Failed to parse datetime for key '{}': {}", key_str, e)
                })?;
                result.insert(entity_id, datetime);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent>
    for HashMap<EntityIDFor<T>, DateTime<Utc>>
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// HashMap<EntityIDFor<T>, NaiveTime>

impl<T: ToTDBSchema> ToSchemaClass for HashMap<EntityIDFor<T>, NaiveTime> {
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<T: ToTDBSchema> Primitive for HashMap<EntityIDFor<T>, NaiveTime> {}
impl<T: ToTDBSchema> ToMaybeTDBSchema for HashMap<EntityIDFor<T>, NaiveTime> {}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, NaiveTime>> for PrimitiveValue {
    fn from(map: HashMap<EntityIDFor<T>, NaiveTime>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            json_map.insert(
                key.to_string(),
                Value::String(value.format("%H:%M:%S%.f").to_string()),
            );
        }
        Self::Object(Value::Object(json_map))
    }
}

impl<T: ToTDBSchema> From<HashMap<EntityIDFor<T>, NaiveTime>> for InstanceProperty {
    fn from(map: HashMap<EntityIDFor<T>, NaiveTime>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, NaiveTime> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<T: ToTDBSchema> FromInstanceProperty for HashMap<EntityIDFor<T>, NaiveTime> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;
                let value_str = value.as_str().ok_or(anyhow!(
                    "Expected string value for key '{}', got {:?}",
                    key_str,
                    value
                ))?;

                // Try parsing with different formats
                let naive_time =
                    if let Ok(time) = NaiveTime::parse_from_str(value_str, "%H:%M:%S%.f") {
                        time
                    } else if let Ok(time) = NaiveTime::parse_from_str(value_str, "%H:%M:%S") {
                        time
                    } else if let Ok(time) = NaiveTime::parse_from_str(value_str, "%H:%M") {
                        time
                    } else {
                        return Err(anyhow!(
                            "Failed to parse time string for key '{}': {}",
                            key_str,
                            value_str
                        ));
                    };

                result.insert(entity_id, naive_time);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent>
    for HashMap<EntityIDFor<T>, NaiveTime>
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

// HashMap<EntityIDFor<T>, V> where V implements ToTDBInstance (for custom structs)

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > ToSchemaClass for HashMap<EntityIDFor<T>, V>
{
    fn to_class() -> String {
        JSON.to_string()
    }
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > Primitive for HashMap<EntityIDFor<T>, V>
{
}
impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > ToMaybeTDBSchema for HashMap<EntityIDFor<T>, V>
{
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > From<HashMap<EntityIDFor<T>, V>> for PrimitiveValue
{
    fn from(map: HashMap<EntityIDFor<T>, V>) -> Self {
        let mut json_map = serde_json::Map::new();
        for (key, value) in map {
            // Serialize the struct to JSON
            if let Ok(json_value) = serde_json::to_value(&value) {
                json_map.insert(key.to_string(), json_value);
            }
        }
        Self::Object(Value::Object(json_map))
    }
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > From<HashMap<EntityIDFor<T>, V>> for InstanceProperty
{
    fn from(map: HashMap<EntityIDFor<T>, V>) -> Self {
        Self::Primitive(map.into())
    }
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
        Parent,
    > ToInstanceProperty<Parent> for HashMap<EntityIDFor<T>, V>
{
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
    > FromInstanceProperty for HashMap<EntityIDFor<T>, V>
{
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::Object(obj)) = prop {
            let map = obj.as_object().ok_or(anyhow!("Expected JSON object"))?;
            let mut result = HashMap::new();

            for (key_str, value_json) in map.iter() {
                let entity_id = EntityIDFor::<T>::new(key_str)
                    .map_err(|e| anyhow!("Invalid EntityIDFor key '{}': {}", key_str, e))?;

                // Deserialize the JSON value to the struct type V
                let value: V = serde_json::from_value(value_json.clone()).map_err(|e| {
                    anyhow!("Failed to deserialize value for key '{}': {}", key_str, e)
                })?;

                result.insert(entity_id, value);
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Expected Object primitive, got {:?}", prop))
        }
    }
}

impl<
        T: ToTDBSchema,
        V: ToTDBSchema
            + ToTDBInstance
            + FromTDBInstance
            + serde::Serialize
            + for<'de> serde::Deserialize<'de>,
        Parent,
    > InstancePropertyFromJson<Parent> for HashMap<EntityIDFor<T>, V>
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let map = json.as_object().ok_or(anyhow!("Expected JSON object"))?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::Object(json)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Property;
    use crate::ToSchemaProperty;
    use chrono::{DateTime, NaiveTime, TimeZone, Utc};

    // Test struct for EntityIDFor testing
    #[derive(Debug, Clone)]
    struct TestEntity {
        name: String,
    }

    impl ToTDBSchema for TestEntity {
        fn schema_name() -> String {
            "TestEntity".to_string()
        }

        fn to_schema_tree() -> Vec<crate::Schema> {
            vec![]
        }
    }

    #[test]
    fn test_hashmap_schema_property() {
        let property = <HashMap<String, Value> as ToSchemaProperty<()>>::to_property("metadata");
        assert_eq!(property.name, "metadata");
        assert_eq!(property.class, JSON);
    }

    #[test]
    fn test_hashmap_instance_property() {
        let mut map = HashMap::new();
        map.insert("key1".to_string(), Value::String("value1".to_string()));
        map.insert("key2".to_string(), Value::Number(42.into()));

        let property =
            <std::collections::HashMap<String, serde_json::Value> as ToInstanceProperty<
                (),
            >>::to_property(map, "metadata", &Schema::empty_class("Test"));
        match property {
            InstanceProperty::Primitive(PrimitiveValue::Object(json_value)) => {
                if let Value::Object(obj) = json_value {
                    assert_eq!(obj.len(), 2);
                    assert_eq!(obj.get("key1"), Some(&Value::String("value1".to_string())));
                    assert_eq!(obj.get("key2"), Some(&Value::Number(42.into())));
                } else {
                    panic!("Expected JSON Object");
                }
            }
            _ => panic!("Expected Object primitive value"),
        }
    }

    #[test]
    fn test_entityid_hashmap_string_schema_property() {
        let property =
            <HashMap<EntityIDFor<TestEntity>, String> as ToSchemaProperty<()>>::to_property(
                "entity_map",
            );
        assert_eq!(property.name, "entity_map");
        assert_eq!(property.class, JSON);
    }

    #[test]
    fn test_entityid_hashmap_string_instance_property() {
        let mut map = HashMap::new();
        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        map.insert(entity_id1, "value1".to_string());
        map.insert(entity_id2, "value2".to_string());

        let property =
            <HashMap<EntityIDFor<TestEntity>, String> as ToInstanceProperty<()>>::to_property(
                map,
                "entity_map",
                &Schema::empty_class("Test"),
            );

        match property {
            InstanceProperty::Primitive(PrimitiveValue::Object(json_value)) => {
                if let Value::Object(obj) = json_value {
                    assert_eq!(obj.len(), 2);
                    assert_eq!(
                        obj.get("TestEntity/123"),
                        Some(&Value::String("value1".to_string()))
                    );
                    assert_eq!(
                        obj.get("TestEntity/456"),
                        Some(&Value::String("value2".to_string()))
                    );
                } else {
                    panic!("Expected JSON Object");
                }
            }
            _ => panic!("Expected Object primitive value"),
        }
    }

    #[test]
    fn test_entityid_hashmap_string_from_instance_property() {
        // Create a JSON object that represents a HashMap<EntityIDFor<TestEntity>, String>
        let json_obj = serde_json::json!({
            "TestEntity/123": "value1",
            "TestEntity/456": "value2"
        });

        let instance_prop = InstanceProperty::Primitive(PrimitiveValue::Object(json_obj));

        let result = HashMap::<EntityIDFor<TestEntity>, String>::from_property(&instance_prop);
        assert!(result.is_ok());

        let map = result.unwrap();
        assert_eq!(map.len(), 2);

        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        assert_eq!(map.get(&entity_id1), Some(&"value1".to_string()));
        assert_eq!(map.get(&entity_id2), Some(&"value2".to_string()));
    }

    #[test]
    fn test_entityid_hashmap_i32_round_trip() {
        let mut map = HashMap::new();
        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        map.insert(entity_id1.clone(), 42);
        map.insert(entity_id2.clone(), 100);

        // Convert to InstanceProperty
        let property =
            <HashMap<EntityIDFor<TestEntity>, i32> as ToInstanceProperty<()>>::to_property(
                map.clone(),
                "scores",
                &Schema::empty_class("Test"),
            );

        // Convert back from InstanceProperty
        let result = HashMap::<EntityIDFor<TestEntity>, i32>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&entity_id1), Some(&42));
        assert_eq!(restored_map.get(&entity_id2), Some(&100));
    }

    #[test]
    fn test_entityid_hashmap_bool_round_trip() {
        let mut map = HashMap::new();
        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        map.insert(entity_id1.clone(), true);
        map.insert(entity_id2.clone(), false);

        // Convert to InstanceProperty
        let property =
            <HashMap<EntityIDFor<TestEntity>, bool> as ToInstanceProperty<()>>::to_property(
                map.clone(),
                "flags",
                &Schema::empty_class("Test"),
            );

        // Convert back from InstanceProperty
        let result = HashMap::<EntityIDFor<TestEntity>, bool>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&entity_id1), Some(&true));
        assert_eq!(restored_map.get(&entity_id2), Some(&false));
    }

    #[test]
    fn test_entityid_hashmap_datetime_round_trip() {
        let mut map = HashMap::new();
        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        let dt1 = Utc.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2023, 6, 15, 18, 30, 0).unwrap();

        map.insert(entity_id1.clone(), dt1);
        map.insert(entity_id2.clone(), dt2);

        // Convert to InstanceProperty
        let property = <HashMap<EntityIDFor<TestEntity>, DateTime<Utc>> as ToInstanceProperty<()>>::to_property(
            map.clone(),
            "timestamps",
            &Schema::empty_class("Test"),
        );

        // Convert back from InstanceProperty
        let result = HashMap::<EntityIDFor<TestEntity>, DateTime<Utc>>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&entity_id1), Some(&dt1));
        assert_eq!(restored_map.get(&entity_id2), Some(&dt2));
    }

    #[test]
    fn test_entityid_hashmap_naivetime_round_trip() {
        let mut map = HashMap::new();
        let entity_id1 = EntityIDFor::<TestEntity>::new("TestEntity/123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("TestEntity/456").unwrap();

        let time1 = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let time2 = NaiveTime::from_hms_opt(17, 45, 30).unwrap();

        map.insert(entity_id1.clone(), time1);
        map.insert(entity_id2.clone(), time2);

        // Convert to InstanceProperty
        let property =
            <HashMap<EntityIDFor<TestEntity>, NaiveTime> as ToInstanceProperty<()>>::to_property(
                map.clone(),
                "times",
                &Schema::empty_class("Test"),
            );

        // Convert back from InstanceProperty
        let result = HashMap::<EntityIDFor<TestEntity>, NaiveTime>::from_property(&property);
        assert!(result.is_ok());

        let restored_map = result.unwrap();
        assert_eq!(restored_map.len(), 2);
        assert_eq!(restored_map.get(&entity_id1), Some(&time1));
        assert_eq!(restored_map.get(&entity_id2), Some(&time2));
    }
}

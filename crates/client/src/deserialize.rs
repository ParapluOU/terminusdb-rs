use serde_json::Value;
use terminusdb_schema::*;

pub trait TDBInstanceDeserializer<T: ToTDBInstance>: Sized + Clone {
    fn from_instance(&mut self, instance: serde_json::Value) -> anyhow::Result<T>;
}

pub fn strip_tdb_meta(value: &mut Value) {
    if let Value::Object(map) = value {
        map.remove("@id");
        map.remove("@type");
    }
}

/// A default deserializer that converts serde_json::Value to Instance and then to the target type T
#[derive(Clone)]
pub struct DefaultTDBDeserializer;

impl<T: ToTDBInstance + FromTDBInstance + InstanceFromJson> TDBInstanceDeserializer<T>
    for DefaultTDBDeserializer
{
    fn from_instance(&mut self, json: serde_json::Value) -> anyhow::Result<T> {
        T::from_json(json)
    }
}

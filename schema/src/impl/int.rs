use serde_json::Value;
use crate::InstanceProperty;
use crate::json::InstancePropertyFromJson;

impl<Parent> InstancePropertyFromJson<Parent> for u64 {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}
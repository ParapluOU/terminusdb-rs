use crate::json::InstancePropertyFromJson;
use crate::InstanceProperty;
use serde_json::Value;

impl<Parent> InstancePropertyFromJson<Parent> for u64 {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}

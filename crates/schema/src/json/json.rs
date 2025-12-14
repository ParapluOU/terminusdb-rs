use crate::{Instance, InstanceProperty, ToInstanceProperty, ToTDBInstance};

pub trait ToJson {
    fn to_map(&self) -> serde_json::Map<String, serde_json::Value>;

    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Object(self.to_map())
    }

    fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_json()).expect("TerminusDB JSON serialization")
    }

    fn to_json_string_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).unwrap()
    }
}

/// trait to be derived to instantiate a Instance from
/// a JSON Value
pub trait InstanceFromJson: ToTDBInstance + Sized {
    fn instance_from_json(json: serde_json::Value) -> anyhow::Result<Instance>;
}

pub trait InstancePropertyFromJson<Parent>: ToInstanceProperty<Parent> + Sized {
    fn property_from_json(json: serde_json::Value) -> anyhow::Result<InstanceProperty>;

    /// for when we deserialize from NULL or non-existent values.
    /// this would specifically be implemented by Option to still yield None
    fn property_from_maybe_json(
        json: Option<serde_json::Value>,
    ) -> anyhow::Result<InstanceProperty> {
        if let Some(v) = json {
            return Self::property_from_json(v);
        }

        // For non-Option types, return an error when the value is missing
        Err(anyhow::anyhow!(
            "Required property not found for {}",
            std::any::type_name::<Self>()
        ))
    }
}

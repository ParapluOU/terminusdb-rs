use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum PrimitiveValue {
    // or enum value
    String(String),
    Number(serde_json::Number),
    Bool(bool),
    Object(serde_json::Value),
    // (), or empty array
    Unit,
    Null,
}

impl Into<serde_json::Value> for PrimitiveValue {
    fn into(self) -> Value {
        match self {
            PrimitiveValue::String(s) => s.into(),
            PrimitiveValue::Number(n) => n.into(),
            PrimitiveValue::Bool(b) => b.into(),
            PrimitiveValue::Null => serde_json::Value::Null,
            PrimitiveValue::Unit => serde_json::Value::Array(vec![]),
            PrimitiveValue::Object(json) => json,
        }
    }
}

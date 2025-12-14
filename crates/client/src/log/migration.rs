use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
pub struct Migration {
    #[serde(rename = "@type")]
    pub ty: String,
    pub class_document: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
}

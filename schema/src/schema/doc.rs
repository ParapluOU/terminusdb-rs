use crate::json::ToJson;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

// todo: the derive should take this from the docstring
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ContextDocumentation {
    pub(crate) title: String,
    pub(crate) authors: Vec<String>,
    pub(crate) description: String,
}

impl ToJson for ContextDocumentation {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("@title".to_string(), self.title.to_string().into());
        map.insert("@authors".to_string(), self.authors.clone().into());
        map.insert("@description".to_string(), self.description.clone().into());
        map
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ClassDocumentation {
    pub comment: String,
    /// The keywords of the @documentation object are @comment and either @properties or @values for standard classes or Enums respectively
    pub properties_or_values: BTreeMap<String, String>,
}

impl ToJson for ClassDocumentation {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("@comment".to_string(), self.comment.to_string().into());
        let props: serde_json::Map<_, _> = self
            .properties_or_values
            .clone()
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect::<serde_json::Map<_, _>>();
        // TODO: only use this in class context
        map.insert("@properties".to_string(), props.clone().into());
        // TODO: only use this in Enum context
        map.insert("@values".to_string(), props.clone().into());
        map
    }
}

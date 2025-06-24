use crate::{json::ToJson, SetCardinality, TypeFamily};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub trait ToSchemaPropertyName {
    fn to_property_name(&self) -> String;
}

pub trait ToSchemaPropertyJsonValue {
    fn to_property_value(&self) -> serde_json::Value;
}

pub trait ToSchemaProperty<Parent> {
    fn to_property(prop_name: &str) -> crate::schema::Property;
}

#[derive(Eq, PartialEq, Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Property {
    /// graph edge name
    pub name: String,
    /// type family should only be given for relations that represent
    /// multiplicities or optionality
    pub r#type: Option<TypeFamily>,
    pub class: String,
}

impl Ord for Property {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare by name
        match self.name.cmp(&other.name) {
            std::cmp::Ordering::Equal => {
                // If names are equal, compare by class
                self.class.cmp(&other.class)
            }
            ordering => ordering,
        }
    }
}

impl PartialOrd for Property {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Property {
    pub fn is_relation(&self) -> bool {
        !crate::primitive::is_primitive(&self.class)
    }

    pub fn field_name(&self) -> &String {
        &self.name
    }
}

impl ToSchemaPropertyJsonValue for Property {
    fn to_property_value(&self) -> Value {
        if self.r#type.is_none() {
            return self.class.clone().into();
        }

        let mut map = serde_json::Map::new();

        // the relation type
        if let Some(t) = self.r#type {
            map.insert("@type".to_string(), t.to_string().into());
            map.append(&mut t.to_map()); // todo: isnt this redundant?
        }

        // the class of object targeted by the relation
        map.insert("@class".to_string(), self.class.clone().into());

        map.into()
    }
}

// create {propname: propvalue}
// impl ToJson for Property {
//     fn to_map(&self) -> Map<String, Value> {
//         let mut map = serde_json::Map::new();

//         map.insert(self.name.clone(), self.to_property_value());

//         map
//     }
// }

impl ToSchemaPropertyName for Property {
    fn to_property_name(&self) -> String {
        self.name.clone()
    }
}

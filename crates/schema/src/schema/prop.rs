use crate::{json::ToJson, TypeFamily};
use serde_json::Value;

pub trait ToSchemaPropertyName {
    fn to_property_name(&self) -> String;
}

pub trait ToSchemaPropertyJsonValue {
    fn to_property_value(&self) -> serde_json::Value;
}

pub trait ToSchemaProperty<Parent> {
    fn to_schema_property(prop_name: &str) -> crate::schema::Property;
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
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
    /// Whether this property's range is a stored value or a graph link.
    pub fn kind(&self) -> terminusdb_format::PropertyKind {
        terminusdb_format::PropertyKind::of(&self.class)
    }

    /// True when this property is a graph link (object property) rather than a
    /// datatype value. Replaces the old `is_relation`; unlike it, `sys:*` ranges
    /// (`sys:JSON`, `sys:Unit`) are correctly treated as values, not links.
    /// (Distinct from [`crate::InstanceProperty::is_relation`], which is about an
    /// instance *value* being a relation.)
    pub fn is_link(&self) -> bool {
        self.kind().is_link()
    }

    pub fn field_name(&self) -> &String {
        &self.name
    }
}

impl ToSchemaPropertyJsonValue for Property {
    fn to_property_value(&self) -> Value {
        use terminusdb_format::keyword;
        if self.r#type.is_none() {
            return self.class.clone().into();
        }

        let mut map = serde_json::Map::new();

        // the type family (List/Set/Array/Optional)
        if let Some(t) = self.r#type {
            map.insert(keyword::TYPE.to_string(), t.family_name().into());
            map.append(&mut t.to_map()); // todo: isnt this redundant?
        }

        // the class of object targeted by the relation
        map.insert(keyword::CLASS.to_string(), self.class.clone().into());

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

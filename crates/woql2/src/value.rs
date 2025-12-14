use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use crate::prelude::*;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use terminusdb_schema::{FromInstanceProperty, InstanceProperty, Property, Schema, ToInstanceProperty, ToSchemaProperty, ToTDBInstance};
use terminusdb_schema::{FromTDBInstance, XSDAnySimpleType};
use terminusdb_schema::json::{InstancePropertyFromJson, ToJson};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Helper struct for DictionaryTemplate
// todo: make key type 'Random'
/// A representation of a JSON style dictionary, but with free variables. It is similar to an interpolated string in that it is a template with quoted data and substituted values.
#[derive(
    TerminusDBModel,
    FromTDBInstance,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
)]

pub struct FieldValuePair {
    /// The field or key of a dictionary value pair
    pub field: String,
    /// The value of a dictionary value pair.
    pub value: self::Value,
}

// Helper struct for Value::Dictionary
// todo: make key type 'random'
/// A representation of a JSON style dictionary, but with free variables. It is similar to an interpolated string in that it is a template with quoted data and substituted values.
#[derive(
    TerminusDBModel,
    FromTDBInstance,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
)]

pub struct DictionaryTemplate {
    /// Pairs of Key-Values to be constructed into a dictionary
    pub data: BTreeSet<FieldValuePair>,
}

// Represents TaggedUnion "Value"
/// A variable, node or data point.
#[derive(
    TerminusDBModel,
    FromTDBInstance,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
)]
#[tdb(class_name = "Value")]
#[tdb(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum WoqlValue {
    /// An xsd data type value.
    Data(XSDAnySimpleType),
    /// A dictionary.
    Dictionary(DictionaryTemplate),
    /// A list of datavalues
    List(Vec<WoqlValue>),
    /// A URI representing a resource.
    Node(String),
    /// A variable.
    Variable(String),
}

pub type Value = WoqlValue;

// Represents TaggedUnion "NodeValue"
/// A variable or node.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[tdb(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NodeValue {
    /// A URI representing a resource.
    Node(String),
    /// A variable.
    Variable(String),
}

// Represents TaggedUnion "DataValue"
/// A variable or node.
#[derive(TerminusDBModel, FromTDBInstance, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[tdb(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DataValue {
    /// An xsd data type value.
    Data(XSDAnySimpleType),
    /// A list of datavalues
    List(Vec<DataValue>),
    /// A variable.
    Variable(String),
}


/// Represents either a list of values or a variable that will resolve to a list at runtime.
/// Used in operations like Concatenate and Join that expect list inputs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// #[tdb(abstract_class = true)]
pub enum ListOrVariable {
    /// A concrete list of data values
    List(Vec<DataValue>),
    /// A variable that will resolve to a list at runtime
    Variable(DataValue),
}

impl ::std::convert::From<DataValue> for ListOrVariable {
    fn from(value: DataValue) -> Self {
        match value {
            DataValue::List(items) => ListOrVariable::List(items),
            // Variables and other data values go to the Variable variant
            _ => ListOrVariable::Variable(value),
        }
    }
}

impl<Parent> ToInstanceProperty<Parent> for ListOrVariable {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        // Convert ListOrVariable to a JSON value that can be stored as a primitive
        let json_value = match self {
            ListOrVariable::List(items) => {
                // Convert each DataValue to JSON using to_instance().to_json()
                let json_items: Vec<serde_json::Value> = items
                    .into_iter()
                    .map(|item| item.to_instance(None).to_json())
                    .collect();
                serde_json::Value::Array(json_items)
            }
            ListOrVariable::Variable(var) => {
                // Convert the variable using to_instance().to_json()
                var.to_instance(None).to_json()
            }
        };
        
        // Use PrimitiveValue::Object to store the JSON directly
        InstanceProperty::Primitive(terminusdb_schema::PrimitiveValue::Object(json_value))
    }
}

impl<Parent> ToSchemaProperty<Parent> for ListOrVariable {
    fn to_property(prop_name: &str) -> Property {
        // ListOrVariable is stored as a JSON string
        Property {
            name: prop_name.to_string(),
            r#type: None, // No type family needed for primitives
            class: "xsd:string".to_string(), // Store as string
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for ListOrVariable {
    fn property_from_json(json: serde_json::Value) -> anyhow::Result<InstanceProperty> {
        todo!()
    }
}

impl FromInstanceProperty for ListOrVariable {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        todo!()
    }
}
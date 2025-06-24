use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::prelude::*;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::{FromTDBInstance, XSDAnySimpleType};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

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
pub enum WoqlValue {
    /// An xsd data type value.
    Data(XSDAnySimpleType),
    /// A dictionary.
    Dictionary(DictionaryTemplate),
    /// A list of datavalues
    List(Vec<Self>),
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
pub enum NodeValue {
    /// A URI representing a resource.
    Node(String),
    /// A variable.
    Variable(String),
}

// Represents TaggedUnion "DataValue"
/// A variable or node.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[tdb(rename_all = "lowercase")]
pub enum DataValue {
    /// An xsd data type value.
    Data(XSDAnySimpleType),
    /// A list of datavalues
    List(Vec<DataValue>),
    /// A variable.
    Variable(String),
}

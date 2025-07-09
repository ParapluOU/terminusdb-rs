use crate::prelude::*;
use crate::value::WoqlValue;
use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// True whenever 'left' is the same as 'right'. Performs unification.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Equals {
    /// A URI, data value or variable.
    pub left: WoqlValue,
    /// A URI, data value or variable.
    pub right: WoqlValue,
}

/// Predicate determining if one thing is less than another according to natural ordering.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Less {
    /// The lesser element.
    pub left: DataValue,
    /// The greater element.
    pub right: DataValue,
}

/// Predicate determining if one thing is greater than another according to natural ordering.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Greater {
    /// The greater element.
    pub left: DataValue,
    /// The lesser element.
    pub right: DataValue,
}

/// Provides class subsumption (the inheritance model) according to the schema design. True whenver 'child' is a child of 'parent'. Can be used as a generator or a check.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Subsumption {
    /// The child class as a URI or variable.
    pub child: NodeValue,
    /// The parent class as a URI or variable
    pub parent: NodeValue,
}

/// Test (or generate) the type of an element.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct IsA {
    /// The element to test.
    pub element: NodeValue,
    /// The type of the element.
    #[tdb(name = "type")]
    pub type_of: NodeValue,
}

/// TypeOf gives the type of an element.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct TypeOf {
    /// The value of which to obtain the type.
    pub value: self::Value, // Element to get type of
    /// The URI which that specifies the type.
    #[tdb(name = "type")]
    pub type_uri: NodeValue, // Resulting type URI
}

/// Casts one type as another if possible.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Typecast {
    /// The value to cast.
    pub value: self::Value, // Value to cast
    /// The type to which to cast.
    #[tdb(name = "type")]
    pub type_uri: NodeValue,
    /// The resulting value after cast.
    #[tdb(name = "result")]
    pub result_value: self::Value,
}

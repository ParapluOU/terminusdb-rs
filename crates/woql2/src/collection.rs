use crate::prelude::*;
use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Generate or test every element of a list.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Member {
    /// The element to test for membership or to supply as generated.
    pub member: DataValue,
    /// The list of elements against which to generate or test.
    pub list: DataValue, // Should be List<DataValue>
}

/// Sum a list of strings.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Sum {
    /// The list of numbers to sum.
    pub list: DataValue, // Should be List<DataValue>
    /// The result of the sum as a number.
    pub result: DataValue,
}

/// The length of a list.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Length {
    /// The list of which to find the length.
    pub list: DataValue, // Should be List<DataValue>
    /// The length of the list.
    pub length: DataValue,
}

/// Extract the value of a key in a bound document.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Dot {
    /// Document which is being accessed.
    pub document: DataValue,
    /// The field from which the document which is being accessed.
    pub field: DataValue,
    /// The value for the document and field.
    pub value: DataValue,
}

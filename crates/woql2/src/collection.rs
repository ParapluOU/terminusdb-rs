use crate::prelude::*;
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

/// Extracts a contiguous subsequence from a list. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Slice {
    /// The input list to slice.
    pub list: DataValue,
    /// The start index (0-based).
    pub start: DataValue,
    /// The end index (exclusive, optional).
    pub end: Option<DataValue>,
    /// The resulting sliced list.
    pub result: DataValue,
}

/// Convert a list to a set, removing duplicates. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct ListToSet {
    /// The input list.
    pub list: DataValue,
    /// The resulting set (deduplicated list).
    pub set: DataValue,
}

/// The set union of two lists. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct SetUnion {
    /// The first list.
    pub list_a: DataValue,
    /// The second list.
    pub list_b: DataValue,
    /// The union of both lists as a set.
    pub result: DataValue,
}

/// The set intersection of two lists. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct SetIntersection {
    /// The first list.
    pub list_a: DataValue,
    /// The second list.
    pub list_b: DataValue,
    /// The intersection of both lists as a set.
    pub result: DataValue,
}

/// The set difference of two lists (elements of `list_a` not in `list_b`). (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct SetDifference {
    /// The list to subtract from.
    pub list_a: DataValue,
    /// The list of elements to remove.
    pub list_b: DataValue,
    /// The difference as a set.
    pub result: DataValue,
}

/// Test or generate membership of a set. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct SetMember {
    /// The element to test for membership or to supply as generated.
    pub element: DataValue,
    /// The set against which to generate or test.
    pub set: DataValue,
}

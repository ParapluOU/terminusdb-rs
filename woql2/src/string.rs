use crate::prelude::*;
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

/// Trims whitespace from 'untrimmed'.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Trim {
    /// The untrimmed string.
    pub untrimmed: DataValue,
    /// The string to be trimmed.
    pub trimmed: DataValue,
}

/// Lowercase a string.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Lower {
    /// The mixed case string.
    pub mixed: DataValue,
    /// The lower case string.
    pub lower: DataValue,
}

/// Uppercase a string.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Upper {
    /// The mixed case string.
    pub mixed: DataValue,
    /// The upper case string.
    pub upper: DataValue,
}

/// Pad a string.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Pad {
    /// The starting string.
    pub string: DataValue,
    /// The padding character.
    pub char: DataValue,
    /// The number of times to repeat the padding character.
    pub times: DataValue,
    /// The result of the padding as a string.
    #[tdb(name = "result")]
    pub result_string: DataValue,
}

/// Split a string.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Split {
    /// The starting string.
    pub string: DataValue,
    /// The splitting pattern.
    pub pattern: DataValue,
    /// The result list of strings.
    pub list: DataValue, // Should be List<DataValue>
}

/// Join a list of strings using 'separator'.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Join {
    /// The list to concatenate.
    pub list: DataValue, // Should be List<DataValue>
    /// The separator between each joined string
    pub separator: DataValue,
    /// The result string.
    #[tdb(name = "result")]
    pub result_string: DataValue,
}

/// Concatenate a list of strings.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Concatenate {
    /// The list to concatenate.
    pub list: DataValue, // Should be List<DataValue>
    /// The result string.
    #[tdb(name = "result")]
    pub result_string: DataValue,
}

/// Finds the boundaries of a substring in a string.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Substring {
    /// The super-string as data or variable.
    pub string: DataValue,
    /// The count of characters before substring as an integer or variable.
    pub before: DataValue,
    /// The length of the string as an integer or variable.
    pub length: DataValue,
    /// The count of characters after substring as an integer or variable.
    pub after: DataValue,
    /// The super-string as data or variable.
    pub substring: DataValue,
}

/// Test a string against a PCRE style regex pattern.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Regexp {
    /// The PCRE style pattern.
    pub pattern: DataValue,
    /// The string to test.
    pub string: DataValue,
    /// An optional result list of matches.
    pub result: Option<DataValue>,
}

/// Distance between strings, similar to a Levenstein distance.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Like {
    /// The first string.
    pub left: DataValue,
    /// The second string.
    pub right: DataValue,
    /// Number between -1 and 1 which gives a scale for similarity.
    pub similarity: DataValue, // Should be float? Schema says DataValue
}

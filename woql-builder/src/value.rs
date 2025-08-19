use std::ops::Deref;
use terminusdb_woql2::value::Value as Woql2Value;
// Import XSDAnySimpleType for literals
use terminusdb_schema::XSDAnySimpleType;
// NodeValue and DataValue are specific enums used elsewhere
use decimal_rs::Decimal; // Import the Decimal type
use std::str::FromStr;
use terminusdb_woql2::prelude::{DataValue, NodeValue}; // Import FromStr for Decimal parsing
use chrono::{DateTime, NaiveDate, NaiveTime, Utc}; // Import chrono types for datetime support

/// Represents an input value for WOQL builder functions.
/// This allows functions to accept variables, IRIs (as strings), or literals easily.
#[derive(Debug, Clone)]
pub enum WoqlInput {
    Variable(Var),
    Node(String),   // Represents an IRI Node
    String(String), // Represents a Data Literal (string)
    Boolean(bool),  // Represents a Data Literal (boolean)
    Integer(i64),   // Represents a Data Literal (integer)
    Decimal(String), // Represents a Data Literal (decimal, stored as string)
    DateTime(String), // Represents a Data Literal (datetime, ISO 8601 format)
    Date(String),    // Represents a Data Literal (date, ISO 8601 format)
    Time(String),    // Represents a Data Literal (time, ISO 8601 format)
    List(Vec<WoqlInput>), // Represents a list of values
}

/// Represents a WOQL variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Var {
    name: String,
}

impl Var {
    /// Creates a new variable reference.
    pub fn new(name: impl Into<String>) -> Self {
        Var { name: name.into() }
    }

    /// Returns the name of the variable (without the "v:" prefix).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the full name of the variable prefixed with "v:".
    pub fn full_name(&self) -> String {
        format!("v:{}", self.name)
    }
}

// --- Input Type Conversions ---

impl From<Var> for WoqlInput {
    fn from(v: Var) -> Self {
        WoqlInput::Variable(v)
    }
}

impl From<&str> for WoqlInput {
    fn from(s: &str) -> Self {
        if s.starts_with("v:") && s.len() > 2 {
            WoqlInput::Variable(Var::new(&s[2..]))
        } else {
            // Default interpretation of &str is a Node IRI.
            // Use explicit functions like `string_literal("text")` for data.
            WoqlInput::Node(s.to_string())
        }
    }
}

impl From<String> for WoqlInput {
    fn from(s: String) -> Self {
        WoqlInput::from(s.as_str())
    }
}

impl From<bool> for WoqlInput {
    fn from(b: bool) -> Self {
        WoqlInput::Boolean(b)
    }
}

impl Deref for Var {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

macro_rules! impl_from_integer {
    ($($t:ty),*) => {
        $(impl From<$t> for WoqlInput {
            fn from(i: $t) -> Self {
                // Convert all integer inputs to i64 for internal representation
                WoqlInput::Integer(i as i64)
            }
        })*
    };
}
impl_from_integer!(i8, u8, i16, u16, i32, u32, i64, u64);

impl<T> From<Vec<T>> for WoqlInput
where
    T: Into<WoqlInput>,
{
    fn from(vec: Vec<T>) -> Self {
        WoqlInput::List(vec.into_iter().map(Into::into).collect())
    }
}

/// Creates multiple `Var` instances from string literals.
///
/// # Example
/// ```
/// // Import Var from the value module and vars! from the crate root
/// # use terminusdb_woql_builder::{value::Var, vars};
/// let (name_var, age_var) = vars!("Name", "Age");
/// assert_eq!(name_var, Var::new("Name"));
/// assert_eq!(age_var, Var::new("Age"));
///
/// let single_var = vars!("OnlyOne");
/// assert_eq!(single_var, Var::new("OnlyOne"));
/// ```
#[macro_export]
macro_rules! vars {
    // Base case: single variable
    ($name:expr) => {
        $crate::value::Var::new($name)
    };
    // Multiple variables: recursive case
    ($($names:expr),+) => {
        (
            $($crate::value::Var::new($names)),+
        )
    };
}

// --- Conversion to woql2 types ---

/// Trait providing methods to convert builder inputs into specific woql2 value types.
/// Made public so it can be used as a bound in public functions like `WoqlBuilder::triple`.
pub trait IntoWoql2 {
    /// Converts the input into a general `woql2::Value`.
    fn into_woql2_value(self) -> Woql2Value;
    /// Converts the input into a `woql2::NodeValue` (Node IRI or Variable).
    /// Panics if the input represents a literal.
    fn into_woql2_node_value(self) -> NodeValue;
    /// Converts the input into a `woql2::DataValue` (Literal or Variable).
    /// Panics if the input represents a Node IRI.
    #[allow(dead_code)]
    fn into_woql2_data_value(self) -> DataValue;
}

impl IntoWoql2 for WoqlInput {
    /// Convert WoqlInput into the main Woql2Value enum.
    fn into_woql2_value(self) -> Woql2Value {
        match self {
            WoqlInput::Variable(var) => Woql2Value::Variable(var.name),
            WoqlInput::Node(iri) => Woql2Value::Node(iri),
            WoqlInput::String(s) => Woql2Value::Data(XSDAnySimpleType::String(s)),
            WoqlInput::Boolean(b) => Woql2Value::Data(XSDAnySimpleType::Boolean(b)),
            WoqlInput::Integer(i) => Woql2Value::Data(XSDAnySimpleType::UnsignedInt(
                i.try_into().expect("Integer input must be non-negative"),
            )),
            WoqlInput::Decimal(d) => Woql2Value::Data(XSDAnySimpleType::Decimal(
                Decimal::from_str(&d).expect("Invalid decimal string format"),
            )),
            WoqlInput::DateTime(dt) => {
                let datetime = DateTime::parse_from_rfc3339(&dt)
                    .expect("Invalid datetime format, expected ISO 8601")
                    .with_timezone(&Utc);
                Woql2Value::Data(XSDAnySimpleType::DateTime(datetime))
            }
            WoqlInput::Date(d) => {
                let date = NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                    .expect("Invalid date format, expected YYYY-MM-DD");
                Woql2Value::Data(XSDAnySimpleType::Date(date))
            }
            WoqlInput::Time(t) => {
                let time = NaiveTime::parse_from_str(&t, "%H:%M:%S")
                    .or_else(|_| NaiveTime::parse_from_str(&t, "%H:%M:%S%.f"))
                    .expect("Invalid time format, expected HH:MM:SS[.fff]");
                Woql2Value::Data(XSDAnySimpleType::Time(time))
            }
            WoqlInput::List(items) => Woql2Value::List(
                items.into_iter().map(|item| item.into_woql2_value()).collect()
            ),
        }
    }

    /// Convert WoqlInput specifically into a NodeValue enum.
    fn into_woql2_node_value(self) -> NodeValue {
        match self {
            WoqlInput::Variable(var) => NodeValue::Variable(var.name),
            WoqlInput::Node(iri) => NodeValue::Node(iri),
            _ => panic!(
                "Attempted to convert a literal input ({:?}) into a NodeValue",
                self
            ),
        }
    }

    /// Convert WoqlInput specifically into a DataValue enum.
    fn into_woql2_data_value(self) -> DataValue {
        match self {
            WoqlInput::Variable(var) => DataValue::Variable(var.name),
            WoqlInput::String(s) => DataValue::Data(XSDAnySimpleType::String(s)),
            WoqlInput::Boolean(b) => DataValue::Data(XSDAnySimpleType::Boolean(b)),
            WoqlInput::Integer(i) => DataValue::Data(XSDAnySimpleType::UnsignedInt(
                i.try_into().expect("Integer input must be non-negative"),
            )),
            WoqlInput::Decimal(d) => DataValue::Data(XSDAnySimpleType::Decimal(
                Decimal::from_str(&d).expect("Invalid decimal string format"),
            )),
            WoqlInput::DateTime(dt) => {
                let datetime = DateTime::parse_from_rfc3339(&dt)
                    .expect("Invalid datetime format, expected ISO 8601")
                    .with_timezone(&Utc);
                DataValue::Data(XSDAnySimpleType::DateTime(datetime))
            }
            WoqlInput::Date(d) => {
                let date = NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                    .expect("Invalid date format, expected YYYY-MM-DD");
                DataValue::Data(XSDAnySimpleType::Date(date))
            }
            WoqlInput::Time(t) => {
                let time = NaiveTime::parse_from_str(&t, "%H:%M:%S")
                    .or_else(|_| NaiveTime::parse_from_str(&t, "%H:%M:%S%.f"))
                    .expect("Invalid time format, expected HH:MM:SS[.fff]");
                DataValue::Data(XSDAnySimpleType::Time(time))
            }
            WoqlInput::List(items) => DataValue::List(
                items.into_iter().map(|item| item.into_woql2_data_value()).collect()
            ),
            _ => panic!(
                "Attempted to convert a Node IRI input ({:?}) into a DataValue",
                self
            ),
        }
    }
}

// Helper function to create an explicit string literal input
pub fn string_literal(s: impl Into<String>) -> WoqlInput {
    WoqlInput::String(s.into())
}

/// Helper function to explicitly create a Node or Variable input.
/// If the string starts with "v:", it's treated as a Variable.
/// Otherwise, it's treated as a Node IRI.
pub fn node(s: impl Into<String>) -> WoqlInput {
    let s_owned = s.into();
    if s_owned.starts_with("v:") && s_owned.len() > 2 {
        WoqlInput::Variable(Var::new(&s_owned[2..]))
    } else {
        WoqlInput::Node(s_owned)
    }
}

/// Helper function to create a list literal.
/// Accepts an iterator of items that can be converted into WoqlInput.
pub fn list<I, T>(items: I) -> WoqlInput 
where
    I: IntoIterator<Item = T>,
    T: Into<WoqlInput>,
{
    WoqlInput::List(items.into_iter().map(Into::into).collect())
}

/// Helper function to create a datetime literal input.
/// Accepts ISO 8601 formatted datetime strings (e.g., "2025-08-19T00:00:00Z").
pub fn datetime_literal(datetime_str: impl Into<String>) -> WoqlInput {
    WoqlInput::DateTime(datetime_str.into())
}

/// Helper function to create a date literal input.
/// Accepts ISO 8601 formatted date strings (e.g., "2025-08-19").
pub fn date_literal(date_str: impl Into<String>) -> WoqlInput {
    WoqlInput::Date(date_str.into())
}

/// Helper function to create a time literal input.
/// Accepts ISO 8601 formatted time strings (e.g., "14:30:00" or "14:30:00.123").
pub fn time_literal(time_str: impl Into<String>) -> WoqlInput {
    WoqlInput::Time(time_str.into())
}

// Blanket implementations for convenience types

macro_rules! impl_into_woql2_for {
    ($($t:ty),*) => {
        $(impl IntoWoql2 for $t {
            fn into_woql2_value(self) -> Woql2Value {
                WoqlInput::from(self).into_woql2_value()
            }
            fn into_woql2_node_value(self) -> NodeValue {
                 WoqlInput::from(self).into_woql2_node_value()
            }
             fn into_woql2_data_value(self) -> DataValue {
                 WoqlInput::from(self).into_woql2_data_value()
            }
        })*
    };
}

impl_into_woql2_for!(Var, &str, String, bool, i8, u8, i16, u16, i32, u32, i64, u64);

// Add IntoWoql2 implementation for Vec<T>
impl<T> IntoWoql2 for Vec<T>
where
    T: Into<WoqlInput>,
{
    fn into_woql2_value(self) -> Woql2Value {
        WoqlInput::from(self).into_woql2_value()
    }
    
    fn into_woql2_node_value(self) -> NodeValue {
        WoqlInput::from(self).into_woql2_node_value()
    }
    
    fn into_woql2_data_value(self) -> DataValue {
        WoqlInput::from(self).into_woql2_data_value()
    }
}

impl IntoWoql2 for f32 {
    fn into_woql2_node_value(self) -> NodeValue {
        panic!("Cannot convert f32 literal {} into a NodeValue", self);
    }
    fn into_woql2_data_value(self) -> DataValue {
        DataValue::Data(XSDAnySimpleType::Float(self as f64)) // Convert to f64 for XSD
    }
    fn into_woql2_value(self) -> Woql2Value {
        Woql2Value::Data(XSDAnySimpleType::Float(self as f64))
    }
}

// Add implementation for f64
impl IntoWoql2 for f64 {
    fn into_woql2_node_value(self) -> NodeValue {
        panic!("Cannot convert f64 literal {} into a NodeValue", self);
    }
    fn into_woql2_data_value(self) -> DataValue {
        DataValue::Data(XSDAnySimpleType::Float(self))
    }
    fn into_woql2_value(self) -> Woql2Value {
        Woql2Value::Data(XSDAnySimpleType::Float(self))
    }
}

impl IntoWoql2 for Decimal {
    fn into_woql2_value(self) -> Woql2Value {
        Woql2Value::Data(XSDAnySimpleType::Decimal(self))
    }
    fn into_woql2_node_value(self) -> NodeValue {
        panic!("Cannot convert Decimal literal {} into a NodeValue", self);
    }
    fn into_woql2_data_value(self) -> DataValue {
        DataValue::Data(XSDAnySimpleType::Decimal(self))
    }
}

// Add implementation for usize
impl IntoWoql2 for usize {
    fn into_woql2_node_value(self) -> NodeValue {
        panic!("Cannot convert usize literal {} into a NodeValue", self);
    }
    fn into_woql2_data_value(self) -> DataValue {
        DataValue::Data(XSDAnySimpleType::UnsignedInt(self))
    }
    fn into_woql2_value(self) -> Woql2Value {
        Woql2Value::Data(XSDAnySimpleType::UnsignedInt(self))
    }
}

// Add implementation for isize
impl IntoWoql2 for isize {
    fn into_woql2_node_value(self) -> NodeValue {
        panic!("Cannot convert isize literal {} into a NodeValue", self);
    }
    fn into_woql2_data_value(self) -> DataValue {
        // Assuming isize maps to xsd:integer (represented as Decimal or String? Let's use Decimal for now)
        // Or should it map to a signed int type if available? Let's assume Decimal is safest.
        DataValue::Data(XSDAnySimpleType::Decimal(Decimal::from(self)))
    }
    fn into_woql2_value(self) -> Woql2Value {
        Woql2Value::Data(XSDAnySimpleType::Decimal(Decimal::from(self)))
    }
}

#[cfg(test)]
mod value_tests {
    use super::*; // Import items from the parent module (value.rs)
    use crate::vars; // Import the macro from the crate root

    #[test]
    fn test_vars_macro() {
        // ... existing test code ...
    }
}

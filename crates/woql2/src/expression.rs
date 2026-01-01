use serde::{Deserialize, Serialize};
// Removed incorrect imports for TdbDataType, TdbDebug, TdbDisplay
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::{FromTDBInstance, XSDAnySimpleType};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Represents TaggedUnion "ArithmeticValue"
/// A variable or data value used within an arithmetic expression.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum ArithmeticValue {
    /// An xsd data type value.
    Data(XSDAnySimpleType), // Use XSDAnySimpleType instead of PrimitiveValue
    /// A variable.
    Variable(String),
}

// Represents the abstract class "ArithmeticExpression"
/// An abstract class specifying the AST super-class of all arithemtic expressions.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum ArithmeticExpression {
    Value(ArithmeticValue),
    Plus(Plus),
    Minus(Minus),
    Times(Times),
    Divide(Divide),
    Div(Div),
    Exp(Exp),
    Floor(Floor),
}

/// Add two numbers.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Plus {
    /// First operand of add.
    pub left: Box<ArithmeticExpression>,
    /// Second operand of add.
    pub right: Box<ArithmeticExpression>,
}

/// Subtract two numbers.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Minus {
    /// First operand of minus.
    pub left: Box<ArithmeticExpression>,
    /// Second operand of minus.
    pub right: Box<ArithmeticExpression>,
}

/// Multiply two numbers.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Times {
    /// First operand of times.
    pub left: Box<ArithmeticExpression>,
    /// Second operand of times.
    pub right: Box<ArithmeticExpression>,
}

/// Divide two numbers.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Divide {
    /// First operand of divide.
    pub left: Box<ArithmeticExpression>,
    /// Second operand of divide.
    pub right: Box<ArithmeticExpression>,
}

/// Integer divide two numbers.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Div {
    /// First operand of div.
    pub left: Box<ArithmeticExpression>,
    /// Second operand of div.
    pub right: Box<ArithmeticExpression>,
}

/// Exponentiate a number.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Exp {
    /// The base.
    pub left: Box<ArithmeticExpression>,
    /// The exponent.
    pub right: Box<ArithmeticExpression>,
}

/// Find the integral part of a number.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Floor {
    /// The number to floor.
    pub argument: Box<ArithmeticExpression>,
}

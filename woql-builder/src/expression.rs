//! Represents WOQL arithmetic expressions for use with the builder.
use crate::value::Var; // Only need Var specifically here?
use decimal_rs::Decimal;
use terminusdb_woql2::expression::{
    ArithmeticExpression as Woql2ArithmeticExpression, ArithmeticValue as Woql2ArithmeticValue,
    Div as Woql2Div, Divide as Woql2Divide, Exp as Woql2Exp, Floor as Woql2Floor,
    Minus as Woql2Minus, Plus as Woql2Plus, Times as Woql2Times,
};

/// Represents a WOQL arithmetic expression tree for the builder.
/// This will be converted to Woql2ArithmeticExpression at the end.
#[derive(Debug, Clone)]
pub enum ArithmeticExpression {
    // Use direct types for literals/vars in the builder's representation
    Var(String),
    String(String),
    Decimal(Decimal),
    Float(f64),
    Boolean(bool),
    UnsignedInt(u64), // Use u64 for consistency?
    // TODO: Add other numeric types if needed (i64, etc.)

    // Operations store boxed versions of this enum
    Plus(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Minus(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Times(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Divide(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Div(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Exp(Box<ArithmeticExpression>, Box<ArithmeticExpression>),
    Floor(Box<ArithmeticExpression>),
}

// Macro to simplify From implementations
#[macro_export]
macro_rules! implement_from_for_arithmetic {
    ($from_type:ty, $variant:ident) => {
        impl From<$from_type> for ArithmeticExpression {
            fn from(value: $from_type) -> Self {
                ArithmeticExpression::$variant(value)
            }
        }
    };
    ($from_type:ty, $variant:ident, $into_type:ty) => {
        impl From<$from_type> for ArithmeticExpression {
            fn from(value: $from_type) -> Self {
                ArithmeticExpression::$variant(value as $into_type)
            }
        }
    };
}

// --- Helper Functions for Building Expressions --- //

/// Creates a '+' expression.
pub fn plus<L, R>(left: L, right: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Plus(Box::new(left.into()), Box::new(right.into()))
}

/// Creates a '-' expression.
pub fn minus<L, R>(left: L, right: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Minus(Box::new(left.into()), Box::new(right.into()))
}

/// Creates a '*' expression.
pub fn times<L, R>(left: L, right: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Times(Box::new(left.into()), Box::new(right.into()))
}

/// Creates a '/' expression (floating point division).
pub fn divide<L, R>(left: L, right: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Divide(Box::new(left.into()), Box::new(right.into()))
}

/// Creates a 'div' expression (integer division).
pub fn div<L, R>(left: L, right: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Div(Box::new(left.into()), Box::new(right.into()))
}

/// Creates an 'exp' expression (exponentiation).
pub fn exp<L, R>(base: L, exponent: R) -> ArithmeticExpression
where
    L: Into<ArithmeticExpression>,
    R: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Exp(Box::new(base.into()), Box::new(exponent.into()))
}

/// Creates a 'floor' expression.
pub fn floor<A>(argument: A) -> ArithmeticExpression
where
    A: Into<ArithmeticExpression>,
{
    ArithmeticExpression::Floor(Box::new(argument.into()))
}

// Implement `Into<ArithmeticExpression>` for basic types

impl From<Var> for ArithmeticExpression {
    fn from(v: Var) -> Self {
        ArithmeticExpression::Var(v.name().to_string())
    }
}

// Use the macro AFTER defining it
implement_from_for_arithmetic!(String, String);
implement_from_for_arithmetic!(Decimal, Decimal);
implement_from_for_arithmetic!(f64, Float);
implement_from_for_arithmetic!(bool, Boolean);
implement_from_for_arithmetic!(u64, UnsignedInt);
// Add more numeric types as needed, ensuring IntoWoql2 exists for them
implement_from_for_arithmetic!(u8, UnsignedInt, u64);
implement_from_for_arithmetic!(u16, UnsignedInt, u64);
implement_from_for_arithmetic!(u32, UnsignedInt, u64);
implement_from_for_arithmetic!(usize, UnsignedInt, u64);
implement_from_for_arithmetic!(f32, Float, f64); // Convert f32 to f64
                                                 // Note: Decide how to handle signed integers (e.g., map to Decimal?)
                                                 // implement_from_for_arithmetic!(i64, ...);
                                                 // implement_from_for_arithmetic!(i32, ...);

// --- Final Conversion Trait --- //

use terminusdb_schema::XSDAnySimpleType;

pub trait FinalizeWoqlExpr {
    fn finalize_expr(self) -> Woql2ArithmeticExpression;
    fn finalize_val(self) -> Woql2ArithmeticValue;
}

impl FinalizeWoqlExpr for ArithmeticExpression {
    fn finalize_expr(self) -> Woql2ArithmeticExpression {
        match self {
            ArithmeticExpression::Var(s) => {
                Woql2ArithmeticExpression::Value(Woql2ArithmeticValue::Variable(s))
            }
            ArithmeticExpression::String(s) => Woql2ArithmeticExpression::Value(
                Woql2ArithmeticValue::Data(XSDAnySimpleType::String(s)),
            ),
            ArithmeticExpression::Decimal(d) => Woql2ArithmeticExpression::Value(
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Decimal(d)),
            ),
            ArithmeticExpression::Float(f) => Woql2ArithmeticExpression::Value(
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Float(f)),
            ),
            ArithmeticExpression::Boolean(b) => Woql2ArithmeticExpression::Value(
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Boolean(b)),
            ),
            ArithmeticExpression::UnsignedInt(u) => Woql2ArithmeticExpression::Value(
                Woql2ArithmeticValue::Data(XSDAnySimpleType::UnsignedInt(u as usize)),
            ), // Convert u64 to usize for XSD
            ArithmeticExpression::Plus(l, r) => Woql2ArithmeticExpression::Plus(Woql2Plus {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Minus(l, r) => Woql2ArithmeticExpression::Minus(Woql2Minus {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Times(l, r) => Woql2ArithmeticExpression::Times(Woql2Times {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Divide(l, r) => Woql2ArithmeticExpression::Divide(Woql2Divide {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Div(l, r) => Woql2ArithmeticExpression::Div(Woql2Div {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Exp(l, r) => Woql2ArithmeticExpression::Exp(Woql2Exp {
                left: Box::new(l.finalize_expr()),
                right: Box::new(r.finalize_expr()),
            }),
            ArithmeticExpression::Floor(arg) => Woql2ArithmeticExpression::Floor(Woql2Floor {
                argument: Box::new(arg.finalize_expr()),
            }),
        }
    }

    fn finalize_val(self) -> Woql2ArithmeticValue {
        match self {
            ArithmeticExpression::Var(s) => Woql2ArithmeticValue::Variable(s),
            ArithmeticExpression::String(s) => {
                Woql2ArithmeticValue::Data(XSDAnySimpleType::String(s))
            }
            ArithmeticExpression::Decimal(d) => {
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Decimal(d))
            }
            ArithmeticExpression::Float(f) => {
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Float(f))
            }
            ArithmeticExpression::Boolean(b) => {
                Woql2ArithmeticValue::Data(XSDAnySimpleType::Boolean(b))
            }
            ArithmeticExpression::UnsignedInt(u) => {
                Woql2ArithmeticValue::Data(XSDAnySimpleType::UnsignedInt(u as usize))
            } // Convert u64 to usize for XSD
            _ => panic!(
                "Cannot finalize complex expression into a simple value for Woql2Eval result."
            ),
        }
    }
}

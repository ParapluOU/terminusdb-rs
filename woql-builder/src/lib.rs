pub mod builder;
mod expression;
mod path;
pub mod value;

// Re-export the vars! macro alongside other key types
#[macro_use]
pub mod prelude {
    // Explicitly list re-exports to avoid ambiguity
    pub use crate::builder::WoqlBuilder;

    // Expression items
    pub use crate::expression::ArithmeticExpression;
    pub use crate::expression::FinalizeWoqlExpr;
    pub use crate::expression::{div, divide, exp, floor, minus, plus, times};

    // Path items
    pub use crate::path::FinalizeWoqlPath;
    pub use crate::path::PathPattern;
    pub use crate::path::{
        inv, or, plus as path_plus, pred, seq, star as path_star, times as path_times,
    };

    // Value items
    pub use crate::value::{list, node, string_literal, IntoWoql2, Var, WoqlInput};

    // Make the vars! macro available via the prelude
    pub use crate::vars;
    // Maybe re-export key woql2 types needed for building?
    // pub use terminusdb_woql2::Query as WoqlQuery;
}

// Declare the tests module, only compiled when running tests
#[cfg(test)]
mod tests;


#![feature(specialization)]

//! High-level relation traits and macros for TerminusDB models

mod traits;


pub use traits::{RelationTo, RelationFrom, RelationField, DefaultField, basic_relation_constraints, generate_relation_constraints};

// Re-export for convenience
pub use terminusdb_woql2::{var, triple, type_, optional, and};
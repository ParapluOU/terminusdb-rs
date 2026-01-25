#![feature(specialization)]

//! High-level relation traits and macros for TerminusDB models

mod traits;

pub use traits::{
    basic_relation_constraints,
    generate_relation_constraints,
    // ORM relation traits
    BelongsTo,
    DefaultField,
    ForwardRelation,
    RelationField,
    RelationFrom,
    RelationTo,
    ReverseRelation,
};

// Re-export for convenience
pub use terminusdb_woql2::{and, optional, triple, type_, var};

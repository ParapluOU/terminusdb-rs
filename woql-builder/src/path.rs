//! Represents WOQL Path patterns for use with the builder.
//!
//! **Note:** Currently, this builder module does not support binding intermediate
//! variables within a path pattern itself (e.g., binding a specific edge traversed
//! during a `pred` step). Only the final resulting path can optionally be bound
//! to a variable in the `WoqlBuilder::path` function.
// Remove unused imports
// use crate::value::{IntoWoql2, Var};
use terminusdb_woql2::{
    path::{
        InversePathPredicate as Woql2InversePathPredicate,
        PathOr as Woql2PathOr,
        // Import woql2 path types
        PathPattern as Woql2PathPattern,
        PathPlus as Woql2PathPlus,
        PathPredicate as Woql2PathPredicate,
        PathSequence as Woql2PathSequence,
        PathStar as Woql2PathStar,
        PathTimes as Woql2PathTimes,
    },
    // prelude::NodeValue, // Unused import - Removed
};

/// Represents a WOQL Path Pattern tree for the builder.
/// This will be converted to Woql2PathPattern at the end.
#[derive(Debug, Clone)]
pub enum PathPattern {
    Predicate(String), // Simple predicate name
    Inverse(String),   // Inverse predicate name
    Sequence(Vec<PathPattern>),
    Or(Vec<PathPattern>),
    Plus(Box<PathPattern>),
    Star(Box<PathPattern>),
    Times(Box<PathPattern>, u64, u64), // Pattern, Min, Max
}

// --- Final Conversion Trait --- //
pub trait FinalizeWoqlPath {
    fn finalize_path(self) -> Woql2PathPattern;
}

impl FinalizeWoqlPath for PathPattern {
    fn finalize_path(self) -> Woql2PathPattern {
        match self {
            PathPattern::Predicate(pred) => Woql2PathPattern::Predicate(Woql2PathPredicate {
                predicate: Some(pred), // Assuming predicate is always Some for this builder variant
            }),
            PathPattern::Inverse(pred) => {
                Woql2PathPattern::InversePredicate(Woql2InversePathPredicate {
                    predicate: Some(pred),
                })
            }
            PathPattern::Sequence(patterns) => Woql2PathPattern::Sequence(Woql2PathSequence {
                sequence: patterns.into_iter().map(|p| p.finalize_path()).collect(),
            }),
            PathPattern::Or(patterns) => Woql2PathPattern::Or(Woql2PathOr {
                or: patterns.into_iter().map(|p| p.finalize_path()).collect(),
            }),
            PathPattern::Plus(pattern) => Woql2PathPattern::Plus(Woql2PathPlus {
                plus: Box::new(pattern.finalize_path()), // Use struct literal syntax
            }),
            PathPattern::Star(pattern) => Woql2PathPattern::Star(Woql2PathStar {
                star: Box::new(pattern.finalize_path()), // Use struct literal syntax
            }),
            PathPattern::Times(pattern, from, to) => Woql2PathPattern::Times(Woql2PathTimes {
                times: Box::new(pattern.finalize_path()), // Use struct literal syntax
                from,
                to,
            }),
        }
    }
}

// --- Helper Functions for Building Path Patterns --- //

/// Creates a simple forward predicate pattern.
pub fn pred(predicate_iri: impl Into<String>) -> PathPattern {
    PathPattern::Predicate(predicate_iri.into())
}

/// Creates an inverse (backward) predicate pattern.
pub fn inv(predicate_iri: impl Into<String>) -> PathPattern {
    PathPattern::Inverse(predicate_iri.into())
}

/// Creates a sequence pattern.
pub fn seq(patterns: impl IntoIterator<Item = PathPattern>) -> PathPattern {
    PathPattern::Sequence(patterns.into_iter().collect())
}

/// Creates an alternative (or) pattern.
pub fn or(patterns: impl IntoIterator<Item = PathPattern>) -> PathPattern {
    PathPattern::Or(patterns.into_iter().collect())
}

/// Creates a one-or-more repetition pattern (`+`).
pub fn plus(pattern: PathPattern) -> PathPattern {
    PathPattern::Plus(Box::new(pattern))
}

/// Creates a zero-or-more repetition pattern (`*`).
pub fn star(pattern: PathPattern) -> PathPattern {
    PathPattern::Star(Box::new(pattern))
}

/// Creates a repetition pattern with min/max bounds (`{from,to}`).
pub fn times(pattern: PathPattern, from: u64, to: u64) -> PathPattern {
    PathPattern::Times(Box::new(pattern), from, to)
}

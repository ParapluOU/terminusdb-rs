use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Represents the abstract class "PathPattern"
/// An abstract class specifying the AST super-class of all path patterns.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum PathPattern {
    Predicate(PathPredicate),
    InversePredicate(InversePathPredicate),
    Sequence(PathSequence),
    Or(PathOr),
    Plus(PathPlus),
    Star(PathStar),
    Times(PathTimes),
}

/// A predicate to traverse.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathPredicate {
    /// The predicate to use in the pattern traversal.
    pub predicate: Option<String>,
}

/// A predicate to traverse *backwards*.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct InversePathPredicate {
    /// The predicate to use in reverse direction in the pattern traversal.
    pub predicate: Option<String>,
}

/// A sequence of patterns in which each of the patterns in the list must result in objects which are subjects of the next pattern in the list.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathSequence {
    /// A sequence of path patterns.
    pub sequence: Vec<PathPattern>,
}

/// A set of patterns in which each of the patterns can result in objects starting from our current subject set.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathOr {
    /// A disjunction of path patterns.
    pub or: Vec<PathPattern>,
}

/// The path pattern specified by 'plus' must hold one or more times in succession.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathPlus {
    /// A path patterns.
    pub plus: Box<PathPattern>,
}

/// The path pattern specified by 'star' may hold zero or more times in succession.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathStar {
    /// A path pattern.
    pub star: Box<PathPattern>,
}

/// The path pattern specified by 'times' may hold 'from' to 'to' times in succession.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct PathTimes {
    /// A path pattern.
    pub times: Box<PathPattern>,
    /// The number of times to start the repetition of the pattern
    pub from: u64,
    /// The number of times after which to end the repeition of the pattern.
    pub to: u64,
}

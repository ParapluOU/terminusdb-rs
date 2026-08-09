use crate::prelude::*;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Select a specific collection for query.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Using {
    /// The resource over which to run the query.
    pub collection: String,
    /// The query which will be run on the selected collection.
    pub query: Box<Query>,
}

/// Change the default read graph (between instance/schema).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct From {
    /// The graph filter: 'schema' or 'instance' or '*'.. The graph filter: 'schema' or 'instance' or '*'..
    pub graph: String,
    /// The subquery with a new default graph.
    pub query: Box<Query>,
}

/// Change the default write graph (between instance/schema).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Into {
    /// The graph filter: schema or instance.
    pub graph: String,
    /// The subquery with a new default write graph.
    pub query: Box<Query>,
}

/// Select specific variables from a query to return.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Select {
    /// The variables to select from the query.
    pub variables: Vec<String>,
    /// The query which will be run prior to selection.
    pub query: Box<Query>,
}

/// Ensure variables listed result in distinct solutions.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Distinct {
    /// The variables which must be distinct from the query.
    pub variables: Vec<String>,
    /// The query which will be run prior to selection.
    pub query: Box<Query>,
}

/// Keep a subquery from being optimized, 'Pin' it in the order given
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Pin {
    /// The query to pin
    pub query: Box<Query>,
}

/// Run the consequent for every success of the condition; succeed unchanged if it fails.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
pub struct When {
    /// A query which provides bindings for the consequent.
    pub query: Box<Query>,
    /// A query run for every solution of `query`.
    pub consequent: Box<Query>,
}

/// Temporary Rust spelling retained while the proof planner's quarantined source
/// still matches `Query::If`; both names encode the authoritative `When` class.
pub type If = When;

// Renamed from Optional due to Rust keyword conflict
/// A query which will succeed (without bindings) even in the case of failure.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
#[tdb(class_name = "Optional")]
pub struct WoqlOptional {
    /// The query to run.
    pub query: Box<Query>,
}

/// Obtains exactly one solution from a query. Simliar to a limit of 1.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Once {
    /// The query from which to obtain a solution.
    pub query: Box<Query>,
}

/// Attempts to perform all side-effecting operations immediately. Can have strange non-backtracking effects but can also increase performance. Use at your own risk.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Immediately {
    /// The query from which to obtain the side-effects.
    pub query: Box<Query>,
}

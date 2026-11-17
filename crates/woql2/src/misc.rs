use crate::prelude::*;
use terminusdb_schema::GraphType;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Start a query at the nth solution specified by 'start'. Allows resumption and paging of queries.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Start {
    /// The numbered solution to start at.
    pub start: u64,
    /// The query to perform.
    pub query: Box<Query>,
}

/// Limit a query to a particular maximum number of solutions specified by 'limit'. Can be used with start to perform paging.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Limit {
    /// Maximum number of solutions.
    pub limit: u64,
    /// The query to perform.
    pub query: Box<Query>,
}

/// Counts the number of solutions of a query.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Count {
    /// The query from which to obtain the count.
    pub query: Box<Query>,
    /// The count of the number of solutions.
    pub count: DataValue,
}

/// Generates a key identical to those generated automatically by 'LexicalKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct LexicalKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// List of data elements required to generate the key.
    pub key_list: Vec<DataValue>,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Generates a key identical to those generated automatically by 'HashKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct HashKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// List of data elements required to generate the key.
    pub key_list: Vec<DataValue>,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Generates a key identical to those generated automatically by 'RandomKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct RandomKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Size of a database in magic units (bytes?).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Size {
    /// The resource to obtain the size of.
    pub resource: String,
    /// The size.
    pub size: DataValue,
}

/// The number of edges in a database, or the number of triples matching a
/// pattern.
///
/// TerminusDB 12.1 serves two forms through the same class:
/// * the *resource* form — `resource` names a collection (e.g. a graph IRI) and
///   `count` is bound to the number of edges in it;
/// * the *pattern* form — any of `subject`/`predicate`/`object`/`graph` restrict
///   the triples counted, answered from the index.
///
/// Use [`TripleCount::for_resource`] or [`TripleCount::pattern`] rather than
/// filling the struct by hand.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct TripleCount {
    /// The resource to obtain the edges from (resource form).
    pub resource: Option<String>,
    /// A URI or variable restricting the subject of the counted triples.
    pub subject: Option<NodeValue>,
    /// A URI or variable restricting the predicate of the counted triples.
    pub predicate: Option<NodeValue>,
    /// A URI, datatype or variable restricting the object of the counted triples.
    pub object: Option<self::Value>,
    /// An optional graph (either 'instance' or 'schema').
    #[tdb(default)]
    pub graph: Option<GraphType>,
    /// The count of edges.
    pub count: DataValue,
}

impl TripleCount {
    /// The resource form: count every edge in `resource`.
    pub fn for_resource(resource: impl std::convert::Into<String>, count: DataValue) -> Self {
        Self {
            resource: Some(resource.into()),
            subject: None,
            predicate: None,
            object: None,
            graph: None,
            count,
        }
    }

    /// The pattern form: count the triples matching the given (partial) pattern.
    /// Pass `None` for any position that should be unrestricted.
    pub fn pattern(
        subject: Option<NodeValue>,
        predicate: Option<NodeValue>,
        object: Option<self::Value>,
        count: DataValue,
    ) -> Self {
        Self {
            resource: None,
            subject,
            predicate,
            object,
            graph: None,
            count,
        }
    }

    /// Restrict this count to a particular graph.
    pub fn with_graph(mut self, graph: GraphType) -> Self {
        self.graph = Some(graph);
        self
    }
}

/// Generates a UUIDv7 key identical to those generated automatically by
/// 'UuidV7' `@key` specifications. (TerminusDB 12.1)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct UuidV7 {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Attach a comment to a query; the query itself (if any) is disabled. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Comment {
    /// The comment text.
    pub comment: DataValue,
    /// The (disabled) query the comment applies to.
    pub query: Option<Box<Query>>,
}

/// Collect all solutions of a sub-query into a list using a template. An empty solution set yields an empty list. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Collect {
    /// The template describing what to collect per solution.
    pub template: self::Value,
    /// The variable receiving the collected list.
    pub into: self::Value,
    /// The sub-query whose solutions are collected.
    pub query: Box<Query>,
}

/// Generate a sequence of numbers from 'start' to 'end' with optional 'step' and 'count'. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Sequence {
    /// The generated value.
    pub value: DataValue,
    /// The start of the sequence.
    pub start: DataValue,
    /// The end of the sequence.
    pub end: DataValue,
    /// The step between elements (optional).
    pub step: Option<DataValue>,
    /// The number of elements (optional).
    pub count: Option<DataValue>,
}

/// Test or generate whether 'value' lies in the half-open range ['start', 'end'). (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct InRange {
    /// The value to test or generate.
    pub value: DataValue,
    /// The inclusive start of the range.
    pub start: DataValue,
    /// The exclusive end of the range.
    pub end: DataValue,
}

/// The minimum element of a list. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct RangeMin {
    /// The list of which to find the minimum.
    pub list: DataValue,
    /// The minimum element.
    pub result: DataValue,
}

/// The maximum element of a list. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct RangeMax {
    /// The list of which to find the maximum.
    pub list: DataValue,
    /// The maximum element.
    pub result: DataValue,
}

use crate::prelude::*;
use serde::{Deserialize, Serialize};
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::{FromTDBInstance, GraphType};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Specify an edge pattern in the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Triple {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// An optional graph (either 'instance' or 'schema'), defaults to 'instance' if not specified
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Specify an edge to add to the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddTriple {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// An optional graph (either 'instance' or 'schema'), defaults to 'instance' if not specified
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Specify an edge pattern which was *added* at *this commit*.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddedTriple {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// An optional graph (either 'instance' or 'schema'), defaults to 'instance' if not specified
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Specify an edge pattern to remove from the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DeleteTriple {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// An optional graph (either 'instance' or 'schema'), defaults to 'instance' if not specified
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Specify an edge pattern which was *deleted* at *this commit*.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DeletedTriple {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// An optional graph (either 'instance' or 'schema'), defaults to 'instance' if not specified
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

// Note: Link/Data types are similar to Triple but restrict the object type.

/// Specify an edge pattern which is not terminal, but a link between objects.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Link {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI or variable which is the target or object of the graph edge.
    pub object: NodeValue, // Object must be a Node
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Specify an edge pattern which is terminal, and provides a data value association.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Data {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A data type or variable which is the target or object of the graph edge.
    pub object: DataValue, // Object must be Data
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Add an edge which links between nodes in the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddLink {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI or variable which is the target or object of the graph edge.
    pub object: NodeValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Specify an edge pattern which links between nodes at *this* commit.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddedLink {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI or variable which is the target or object of the graph edge.
    pub object: NodeValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Add an edge with a data value.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddData {
    /// A URI or variable which is the source or subject of the graph edge. The variable must be bound.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge. The variable must be bound.
    pub predicate: NodeValue,
    /// A data value or variable which is the target or object of the graph edge. The variable must be bound.
    pub object: DataValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Specify an edge pattern with data value which was added in *this* commit*.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct AddedData {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A datatype or variable which is the target or object of the graph edge.
    pub object: DataValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Delete an edge linking nodes.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DeleteLink {
    /// A URI or variable which is the source or subject of the graph edge. The variable must be bound.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge. The variable must be bound.
    pub predicate: NodeValue,
    /// A URI or variable which is the target or object of the graph edge. The variable must be bound.
    pub object: NodeValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// An edge pattern specifying a link beween nodes deleted *at this commit*.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DeletedLink {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI or variable which is the target or object of the graph edge.
    pub object: NodeValue,
    /// An optional graph (either 'instance' or 'schema')
    pub graph: GraphType,
}

/// Ordered range query over triples: bind objects in the half-open range ['low', 'high'). (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct TripleSlice {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// The inclusive lower bound of the object range.
    pub low: self::Value,
    /// The exclusive upper bound of the object range.
    pub high: self::Value,
    /// An optional graph (either 'instance' or 'schema')
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Ordered range query over triples in reverse order. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct TripleSliceRev {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// The inclusive lower bound of the object range.
    pub low: self::Value,
    /// The exclusive upper bound of the object range.
    pub high: self::Value,
    /// An optional graph (either 'instance' or 'schema')
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Bind the next object after 'object' in the triple ordering. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct TripleNext {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// The next object in the ordering.
    pub next: self::Value,
    /// An optional graph (either 'instance' or 'schema')
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

/// Bind the previous object before 'object' in the triple ordering. (TerminusDB 12)
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct TriplePrevious {
    /// A URI or variable which is the source or subject of the graph edge.
    pub subject: NodeValue,
    /// A URI or variable which is the edge-label or predicate of the graph edge.
    pub predicate: NodeValue,
    /// A URI, datatype or variable which is the target or object of the graph edge.
    pub object: self::Value,
    /// The previous object in the ordering.
    pub previous: self::Value,
    /// An optional graph (either 'instance' or 'schema')
    #[tdb(default)]
    pub graph: Option<GraphType>,
}

// Note: DeleteData is not explicitly in the schema, seems DeleteTriple covers it.

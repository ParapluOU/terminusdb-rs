use crate::prelude::*;
use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Read a full document from an identifier.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct ReadDocument {
    /// The URI of the document to load.
    pub identifier: NodeValue,
    /// Variable which will be bound to the document.
    pub document: self::Value,
}

/// Insert a document in the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct InsertDocument {
    /// The document to insert. Must either have an '@id' or have a class specified key.
    pub document: self::Value,
    /// An optional returned identifier specifying the documentation location.
    pub identifier: Option<NodeValue>,
}

/// Update a document in the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct UpdateDocument {
    /// The document to update. Must either have an '@id' or have a class specified key.
    pub document: self::Value,
    /// An optional returned identifier specifying the documentation location.
    pub identifier: Option<NodeValue>,
}

/// Delete a document from the graph.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DeleteDocument {
    /// An identifier specifying the documentation location to delete.
    pub identifier: NodeValue,
}

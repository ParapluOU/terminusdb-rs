use crate::prelude::*;
use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Start a query at the nth solution specified by 'start'. Allows resumption and paging of queries.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Start {
    /// The numbered solution to start at.
    pub start: u64,
    /// The query to perform.
    pub query: Box<Query>,
}

/// Limit a query to a particular maximum number of solutions specified by 'limit'. Can be used with start to perform paging.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Limit {
    /// Maximum number of solutions.
    pub limit: u64,
    /// The query to perform.
    pub query: Box<Query>,
}

/// Counts the number of solutions of a query.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Count {
    /// The query from which to obtain the count.
    pub query: Box<Query>,
    /// The count of the number of solutions.
    pub count: DataValue,
}

/// Generates a key identical to those generated automatically by 'LexicalKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct LexicalKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// List of data elements required to generate the key.
    pub key_list: Vec<DataValue>,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Generates a key identical to those generated automatically by 'HashKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct HashKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// List of data elements required to generate the key.
    pub key_list: Vec<DataValue>,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Generates a key identical to those generated automatically by 'RandomKey' specifications.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct RandomKey {
    /// The URI base to the left of the key.
    pub base: DataValue,
    /// The resulting URI.
    pub uri: NodeValue,
}

/// Size of a database in magic units (bytes?).
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct Size {
    /// The resource to obtain the size of.
    pub resource: String,
    /// The size.
    pub size: DataValue,
}

/// The number of edges in a database.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct TripleCount {
    /// The resource to obtain the edges from.
    pub resource: String,
    /// The count of edges.
    pub count: DataValue,
}

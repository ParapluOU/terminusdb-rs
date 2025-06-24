use crate::prelude::*;
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

// Represents Enum "Order"
/// Specifies the ordering direction (ascending or descending).
#[derive(
    TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "lowercase")]
#[tdb(rename_all = "lowercase")]
pub enum Order {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

// Represents Class "OrderTemplate"
/// The order template, consisting of the variable and ordering direction.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct OrderTemplate {
    /// The variable to order.
    pub variable: String,
    /// An enum either 'asc' or 'desc'.
    pub order: Order,
}

// Represents Class "OrderBy"
/// Orders query results according to an ordering specification.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct OrderBy {
    /// A specification of the ordering of solutions.
    pub ordering: Vec<OrderTemplate>,
    /// The base query giving the solutions to order.
    pub query: Box<Query>,
}

// Represents Class "GroupBy"
/// Group a query into a list with each element of the list specified by 'template' using a given variable set for the group.
#[derive(TerminusDBModel, FromTDBInstance, Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct GroupBy {
    /// The template of elements in the result list.
    pub template: self::Value,
    /// The variables which should be grouped into like solutions.
    pub group_by: Vec<String>,
    /// The final list of templated solutions.
    #[tdb(name = "grouped")]
    pub grouped_value: self::Value,
    /// The subquery providing the solutions for the grouping.
    pub query: Box<Query>,
}

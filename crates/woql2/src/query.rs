use crate::collection::{Dot, Length, Member, Sum};
use crate::compare::{Equals, Greater, IsA, Less, Subsumption, TypeOf, Typecast};
use crate::control::{
    Distinct, From, If, Immediately, Into, Once, Pin, Select, Using, WoqlOptional,
};
use crate::document::{DeleteDocument, InsertDocument, ReadDocument, UpdateDocument};
use crate::expression::{ArithmeticExpression, ArithmeticValue};
use crate::get::Get;
use crate::misc::{Count, HashKey, LexicalKey, Limit, RandomKey, Size, Start, TripleCount};
use crate::order::{GroupBy, OrderBy};
use crate::path::PathPattern;
use crate::prelude::*;
use crate::string::{Concatenate, Join, Like, Lower, Pad, Regexp, Split, Substring, Trim, Upper};
use crate::triple::{
    AddData, AddLink, AddTriple, AddedData, AddedLink, AddedTriple, Data, DeleteLink, DeleteTriple,
    DeletedLink, DeletedTriple, Link, Triple,
};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::{FromTDBInstance, ToJson, ToTDBSchema};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// todo: define key type as lexical on the 'name' field
/// A named query names a specific query for later retrieval and re-use.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct NamedQuery {
    /// The name of the NamedQuery to be retrieved
    pub name: String,
    /// The query AST as WOQL JSON
    pub query: Query,
}

/// A named parametric query which names a specific query for later retrieval and re-use and allows the specification of bindings for a specific set of variables in the query.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct NamedParametricQuery {
    /// The name of the NamedParametricQuery to be retrieved.
    pub name: String,
    /// Variable name list for auxilliary bindings.
    pub parameters: Vec<String>,
    /// The query AST as WOQL JSON.
    pub query: Query,
}

// todo: define key type as lexical on the 'name' field
/// A call of a named parametric query. Variables will be passed to the named query and bound according to the results. Named queries can be (mutually) recursive.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
pub struct Call {
    /// The name of the NamedParametricQuery to be retrieved.
    pub name: String,
    /// The arguments to use when binding formal parameters of the parametric query.
    pub arguments: Vec<self::Value>,
}

// Represents the abstract class "Query"
/// An abstract class which represents an arbitrary query AST.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
#[tdb(abstract_class = true)]
pub enum Query {
    And(And),
    Or(Or),
    Not(Not),
    True(True),
    Triple(Triple),
    AddTriple(AddTriple),
    AddedTriple(AddedTriple),
    DeleteTriple(DeleteTriple),
    DeletedTriple(DeletedTriple),
    Link(Link),
    Data(Data),
    AddLink(AddLink),
    AddedLink(AddedLink),
    AddData(AddData),
    AddedData(AddedData),
    DeleteLink(DeleteLink),
    DeletedLink(DeletedLink),
    Eval(Eval),
    Path(Path),
    ReadDocument(ReadDocument),
    InsertDocument(InsertDocument),
    UpdateDocument(UpdateDocument),
    DeleteDocument(DeleteDocument),
    Equals(Equals),
    Less(Less),
    Greater(Greater),
    Subsumption(Subsumption),
    IsA(IsA),
    TypeOf(TypeOf),
    Typecast(Typecast),
    Trim(Trim),
    Lower(Lower),
    Upper(Upper),
    Pad(Pad),
    Split(Split),
    Join(Join),
    Concatenate(Concatenate),
    Substring(Substring),
    Regexp(Regexp),
    Like(Like),
    Member(Member),
    Sum(Sum),
    Length(Length),
    Dot(Dot),
    Get(Get),
    Using(Using),
    From(From),
    Into(Into),
    Select(Select),
    Distinct(Distinct),
    Pin(Pin),
    If(If),
    WoqlOptional(WoqlOptional),
    Once(Once),
    Immediately(Immediately),
    OrderBy(OrderBy),
    GroupBy(GroupBy),
    Start(Start),
    Limit(Limit),
    Count(Count),
    LexicalKey(LexicalKey),
    HashKey(HashKey),
    RandomKey(RandomKey),
    Size(Size),
    TripleCount(TripleCount),
    Call(Call),
}

impl Query {
    /// Unwraps pagination operations (Start, Limit) to get the underlying query.
    /// This is useful when you need to count all results regardless of pagination.
    pub fn unwrap_pagination(self) -> Query {
        match self {
            Query::Limit(limit) => (*limit.query).unwrap_pagination(),
            Query::Start(start) => (*start.query).unwrap_pagination(),
            _ => self,
        }
    }
}

#[test]
fn test_abstract_query() {
    let query = Query::And(And { and: vec![] });
    let schema = Query::to_schema();
    assert!(schema.is_abstract());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_pagination_removes_limit() {
        let inner = Query::True(True {});
        let with_limit = Query::Limit(Limit {
            limit: 10,
            query: Box::new(inner.clone()),
        });
        
        let unwrapped = with_limit.unwrap_pagination();
        assert_eq!(unwrapped, inner);
    }

    #[test]
    fn test_unwrap_pagination_removes_start() {
        let inner = Query::True(True {});
        let with_start = Query::Start(Start {
            start: 5,
            query: Box::new(inner.clone()),
        });
        
        let unwrapped = with_start.unwrap_pagination();
        assert_eq!(unwrapped, inner);
    }

    #[test]
    fn test_unwrap_pagination_removes_nested_start_limit() {
        let inner = Query::True(True {});
        let with_limit = Query::Limit(Limit {
            limit: 10,
            query: Box::new(inner.clone()),
        });
        let with_start_and_limit = Query::Start(Start {
            start: 5,
            query: Box::new(with_limit),
        });
        
        let unwrapped = with_start_and_limit.unwrap_pagination();
        assert_eq!(unwrapped, inner);
    }

    #[test]
    fn test_unwrap_pagination_removes_nested_limit_start() {
        let inner = Query::True(True {});
        let with_start = Query::Start(Start {
            start: 5,
            query: Box::new(inner.clone()),
        });
        let with_limit_and_start = Query::Limit(Limit {
            limit: 10,
            query: Box::new(with_start),
        });
        
        let unwrapped = with_limit_and_start.unwrap_pagination();
        assert_eq!(unwrapped, inner);
    }

    #[test]
    fn test_unwrap_pagination_leaves_other_queries_unchanged() {
        let and_query = Query::And(And { and: vec![] });
        let unwrapped = and_query.clone().unwrap_pagination();
        assert_eq!(unwrapped, and_query);

        let or_query = Query::Or(Or { or: vec![] });
        let unwrapped = or_query.clone().unwrap_pagination();
        assert_eq!(unwrapped, or_query);
    }
}

/// A conjunction of queries which must all have a solution.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct And {
    /// List of queries which must hold.
    pub and: Vec<Query>,
}

/// A disjunction of queries any of which can provide a solution.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Or {
    /// List of queries which may hold.
    pub or: Vec<Query>,
}

/// The negation of a query. Provides no solution bindings, but will succeed if its sub-query fails.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Not {
    /// The query which must not hold.
    pub query: Box<Query>,
}

/// The query which is always true.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct True {}

/// Evaluate an arithmetic expression to obtain a result.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Eval {
    /// The expression to be evaluated.
    pub expression: ArithmeticExpression,
    /// The numeric result.
    #[tdb(name = "result")]
    pub result_value: ArithmeticValue,
}

/// Find a path through the graph according to 'pattern'. This 'pattern' is a regular graph expression which avoids cycles.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Path {
    /// The starting node.
    pub subject: self::Value,
    /// The pattern which describes how to traverse edges.
    pub pattern: PathPattern,
    /// The ending node.
    pub object: self::Value,
    /// An optional list of edges traversed.
    pub path: Option<self::Value>,
}

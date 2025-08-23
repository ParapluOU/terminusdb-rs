#![allow(warnings)]

//! WOQL AST (Abstract Syntax Tree) types generated from schema.

// resources:
// - https://terminusdb.com/docs/woql-explanation/
// - https://terminusdb.com/docs/woql-basics/
// - https://github.com/terminusdb/terminusdb-docs/tree/main/guides/how-to-guides/query-using-woql
// - https://terminusdb.com/docs/woql-class-reference-guide/
// -

// Add pub mod declarations here as modules are created.
pub mod collection;
pub mod compare;
pub mod control;
pub mod document;
pub mod expression;
pub mod get;
pub mod misc;
pub mod order;
pub mod path;
pub mod query;
pub mod string;
pub mod triple;
pub mod value;
pub mod dsl;
pub mod macros;

pub mod prelude {
    // Re-export core types
    pub use super::collection::{Dot, Length, Member, Sum};
    pub use super::compare::{Equals, Greater, IsA, Less, Subsumption, TypeOf, Typecast};
    pub use super::control::{
        Distinct, From, If, Immediately, Into, Once, Pin, Select, Using, WoqlOptional,
    };
    pub use super::document::{DeleteDocument, InsertDocument, ReadDocument, UpdateDocument};
    pub use super::expression::{
        ArithmeticExpression, ArithmeticValue, Div, Divide, Exp, Floor, Minus, Plus, Times,
    };
    pub use super::get::{Column, FormatType, Get, Indicator, QueryResource, Source};
    pub use super::misc::{Count, HashKey, LexicalKey, Limit, RandomKey, Size, Start, TripleCount};
    pub use super::order::{GroupBy, Order, OrderBy, OrderTemplate};
    pub use super::path::{
        InversePathPredicate, PathOr, PathPattern, PathPlus, PathPredicate, PathSequence, PathStar,
        PathTimes,
    };
    pub use super::query::*;
    pub use super::string::{
        Concatenate, Join, Like, Lower, Pad, Regexp, Split, Substring, Trim, Upper,
    };
    pub use super::triple::{
        AddData, AddLink, AddTriple, AddedData, AddedLink, AddedTriple, Data, DeleteLink,
        DeleteTriple, DeletedLink, DeletedTriple, Link, Triple,
    };
    pub use super::value::{DataValue, DictionaryTemplate, FieldValuePair, NodeValue, Value};

    // Re-export macros
    pub use crate::{
        var, node_var, node, node_value, data, list,
        triple, and, or, not, select, eq, greater, less,
        path, limit, eval, read_doc, insert_doc, update_doc, delete_doc,
        if_then_else,
        // Shortcut macros
        type_, isa, optional, distinct_vars, count_into, cast,
        sum, concat, member, immediately, link, data_triple,
        regex, trim, true_, compare,
        // String operation macros
        starts_with, ends_with, contains,
        // Date/time macros
        today, after, before, in_between, today_in_between
    };

    // Potentially re-export common traits if needed
    // Removed re-exports for non-existent TdbDataType, TdbDebug, TdbDisplay
    pub use terminusdb_schema::FromTDBInstance;
    pub use terminusdb_schema::ToTDBInstance;
    
    // DSL rendering trait
    pub use super::dsl::ToDSL;
}

#[test]
fn it_compiles() {}

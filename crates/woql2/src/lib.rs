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
pub mod dsl;
pub mod expression;
pub mod get;
pub mod macros;
pub mod misc;
pub mod order;
pub mod path;
pub mod query;
pub mod string;
pub mod triple;
pub mod value;
// pub mod macros_refactored;
// pub mod macros_refactored2;
pub mod path_builder;
pub mod query_dsl;

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
    // Export the IntoDataValue trait for ergonomic API usage
    pub use super::macros::{IntoDataValue, IntoOrderTemplate};

    // Re-export macros
    pub use crate::{
        after,
        and,
        before,
        cast,
        compare,
        concat,
        contains,
        count_into,
        data,
        data_triple,
        delete_doc,
        distinct_vars,
        ends_with,
        eq,
        equals,
        eval,
        // Type-safe field access
        field,
        // High-level relation traversal
        from_path,
        greater,
        id,
        if_then_else,
        immediately,
        in_between,
        insert_doc,
        isa,
        less,
        limit,
        link,
        list,
        member,
        node,
        node_value,
        node_var,
        not,
        opt,
        option,
        optional,
        or,
        path,
        prop,
        // Query DSL macros
        query,
        read_doc,
        regex,
        schema_type,
        select,
        // String operation macros
        starts_with,
        sum,
        t,
        // Date/time macros
        today,
        today_in_between,
        trim,
        triple,
        true_,
        // Shortcut macros
        type_,
        typename,
        update_doc,
        v,
        var,
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

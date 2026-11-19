//! Shared test fixtures.
//!
//! Compiled only under `cfg(test)`. Every module's tests need a
//! [`SchemaIndex`], and a hand-rolled one per module would drift, so the
//! canonical fixture lives here: a small but deliberately awkward schema with
//! an abstract root, two levels of inheritance, a subdocument, an enum, a
//! set-valued link, and one property whose range is never declared.

use serde_json::{json, Value};

use crate::ids::{ClassName, DocId, PropName};
use crate::schema_index::SchemaIndex;

/// The fixture schema as authored schema-graph documents, i.e. exactly what
/// `GET /api/document/<path>?graph_type=schema&as_list=true` returns.
///
/// ```text
/// Agent (abstract)            name: xsd:string
///  ├── Person                 age: xsd:integer, height: xsd:double,
///  │                          active: xsd:boolean,
///  │                          nicknames: Set<xsd:string>, favourite: Colour,
///  │                          employer: Optional<Company>, address: Address,
///  │                          friends: Set<Person>
///  │    └── Employee          salary: xsd:decimal,
///  │                          nicknames: Optional<xsd:string> (overrides)
///  └── Company                departments: Set<Department>
/// Department                  head: Person, ghost: Ghost (undeclared!)
/// Address (subdocument)       city: xsd:string
/// Colour (enum)               Red | Green
/// ```
pub fn schema_fixture_docs() -> Vec<Value> {
    vec![
        json!({
            "@type": "@context",
            "@base": "terminusdb:///data/",
            "@schema": "terminusdb:///schema#"
        }),
        json!({
            "@id": "Agent",
            "@type": "Class",
            "@abstract": [],
            "name": "xsd:string"
        }),
        json!({
            "@id": "Person",
            "@type": "Class",
            "@inherits": ["Agent"],
            // An indexable integer, double and boolean, so predicate tests have
            // a verified range of each comparison discipline to work with, plus
            // `Employee.salary` (xsd:decimal) as a deliberately *unindexable*
            // one. See `crate::predicate::INDEXABLE_RANGES`.
            "age": "xsd:integer",
            "height": "xsd:double",
            "active": "xsd:boolean",
            "nicknames": {"@type": "Set", "@class": "xsd:string"},
            "favourite": "Colour",
            "employer": {"@type": "Optional", "@class": "Company"},
            "address": "Address",
            "friends": {"@type": "Set", "@class": "Person"}
        }),
        json!({
            "@id": "Employee",
            "@type": "Class",
            "@inherits": ["Person"],
            "salary": "xsd:decimal",
            // Overrides Person's Set-valued `nicknames`, so inheritance order
            // is observable.
            "nicknames": {"@type": "Optional", "@class": "xsd:string"}
        }),
        json!({
            "@id": "Company",
            "@type": "Class",
            "@inherits": ["Agent"],
            "departments": {"@type": "Set", "@class": "Department"}
        }),
        json!({
            "@id": "Department",
            "@type": "Class",
            "head": "Person",
            "ghost": "Ghost"
        }),
        json!({
            "@id": "Address",
            "@type": "Class",
            "@subdocument": [],
            "@key": {"@type": "Random"},
            "city": "xsd:string"
        }),
        json!({"@id": "Colour", "@type": "Enum", "@value": ["Red", "Green"]}),
    ]
}

/// The fixture schema, parsed.
pub fn schema_fixture() -> SchemaIndex {
    SchemaIndex::from_schema_documents(&schema_fixture_docs())
        .expect("the fixture schema must parse")
}

/// Shorthand for a class name in tests.
pub fn cls(name: &str) -> ClassName {
    ClassName::from(name)
}

/// Shorthand for a property name in tests.
pub fn prop(name: &str) -> PropName {
    PropName::from(name)
}

/// Shorthand for a document id in tests.
pub fn doc(id: &str) -> DocId {
    DocId::parse(id)
}

//! Small `terminusdb-woql2` construction helpers shared across the client.
//!
//! These replace the (now-removed) `terminusdb-woql-builder` shortcuts with
//! direct `terminusdb-woql2` AST construction.

use terminusdb_schema::{GraphType, ToTDBSchema};
use terminusdb_woql2::macros::IntoNodeValue;
use terminusdb_woql2::prelude::{Query, Triple};
use terminusdb_woql2::value::{NodeValue, Value};

/// Build an `rdf:type` triple constraining `subject` to be an instance of `T`.
///
/// This reproduces the old woql-builder `isa2::<T>()` shortcut exactly: a
/// `Triple` on the instance graph whose object is `@schema:{schema_name}`.
pub(crate) fn isa_model<T: ToTDBSchema>(subject: &Value) -> Query {
    Query::Triple(Triple {
        subject: subject.clone().into_node_value(),
        predicate: NodeValue::Node("rdf:type".into()),
        object: Value::Node(format!("@schema:{}", T::schema_name())),
        graph: Some(GraphType::Instance),
    })
}

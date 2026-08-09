//! Shared WOQL AST builders for the authenticated woql2 test suites.

#![allow(dead_code)]

use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::compare::{Equals, Greater, Gte, Less, Lte};
use terminusdb_woql2::query::{And, Query};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{DataValue, NodeValue, Value};

/// The IRI convention shared with the store test layer (`ValueTriple::new_node`).
pub fn iri(s: &str) -> String {
    format!("http://ex/{s}")
}

/// `t(?subj, <iri(pred)>, ?obj)` on the (default) instance graph.
pub fn tv(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.to_string()),
        graph: None,
    })
}

/// A triple whose predicate is a VARIABLE (`t(?subj, ?pred, ?obj)`).
pub fn tv_pred_var(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Variable(pred.to_string()),
        object: Value::Variable(obj.to_string()),
        graph: None,
    })
}

/// A triple whose SUBJECT is a node constant (`t(<iri(subj)>, <iri(pred)>, ?obj)`).
pub fn tv_subj_const(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subj)),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Variable(obj.to_string()),
        graph: None,
    })
}

/// A triple whose OBJECT is a node constant.
pub fn tv_obj_node(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Node(iri(obj)),
        graph: None,
    })
}

/// A triple whose OBJECT is a data literal of a STILL-unsupported datatype (xsd:hexBinary),
/// used to exercise the reject-closed path.
pub fn tv_obj_data(subj: &str, pred: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::HexBinary("deadbeef".to_string())),
        graph: None,
    })
}

/// A triple whose OBJECT is an xsd:boolean literal (`t(?subj, <iri(pred)>, b)`).
pub fn tv_obj_bool(subj: &str, pred: &str, b: bool) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::Boolean(b)),
        graph: None,
    })
}

/// A triple whose OBJECT is an xsd:float/double literal (`t(?subj, <iri(pred)>, f)`).
pub fn tv_obj_float(subj: &str, pred: &str, f: f64) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::Float(f)),
        graph: None,
    })
}

/// A triple whose OBJECT is an xsd:string literal (`t(?subj, <iri(pred)>, "value")`).
pub fn tv_obj_string(subj: &str, pred: &str, value: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::String(value.to_string())),
        graph: None,
    })
}

/// A triple whose OBJECT is an xsd:unsignedInt literal (`t(?subj, <iri(pred)>, value)`).
pub fn tv_obj_uint(subj: &str, pred: &str, value: usize) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::UnsignedInt(value)),
        graph: None,
    })
}

/// A triple whose OBJECT is an xsd:integer literal (`t(?subj, <iri(pred)>, value)`).
pub fn tv_obj_integer(subj: &str, pred: &str, value: i64) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Variable(subj.to_string()),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Data(XSDAnySimpleType::Integer(value)),
        graph: None,
    })
}

/// A fully-ground triple (`t(<iri(subj)>, <iri(pred)>, <iri(obj)>)`) — no variables.
pub fn tv_ground(subj: &str, pred: &str, obj: &str) -> Query {
    Query::Triple(Triple {
        subject: NodeValue::Node(iri(subj)),
        predicate: NodeValue::Node(iri(pred)),
        object: Value::Node(iri(obj)),
        graph: None,
    })
}

/// `And` of a set of conjuncts.
pub fn and_of(qs: Vec<Query>) -> Query {
    Query::And(And { and: qs })
}

pub fn equals(left: Value, right: Value) -> Query {
    Query::Equals(Equals { left, right })
}

pub fn value_var(name: &str) -> Value {
    Value::Variable(name.to_string())
}

pub fn value_node(name: &str) -> Value {
    Value::Node(iri(name))
}

pub fn value_uint(value: usize) -> Value {
    Value::Data(XSDAnySimpleType::UnsignedInt(value))
}

pub fn value_integer(value: i64) -> Value {
    Value::Data(XSDAnySimpleType::Integer(value))
}

/// A `DataValue::Variable`, for building comparison atoms.
pub fn dvar(name: &str) -> DataValue {
    DataValue::Variable(name.to_string())
}

/// A `DataValue::Data(xsd:unsignedInt)` constant — the round-trip-aligned range datatype.
pub fn duint(k: usize) -> DataValue {
    DataValue::Data(XSDAnySimpleType::UnsignedInt(k))
}

/// A `DataValue::Data(xsd:string)` constant (an UNSUPPORTED range datatype).
pub fn dstr(s: &str) -> DataValue {
    DataValue::Data(XSDAnySimpleType::String(s.to_string()))
}

/// A `DataValue::Data(xsd:integer)` constant — the `Int64` order-preserving range datatype.
pub fn dint(k: i64) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Integer(k))
}

/// A `DataValue::Data(xsd:boolean)` constant — the `Boolean` order-preserving range datatype.
pub fn dbool(b: bool) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Boolean(b))
}

/// A `DataValue::Data(xsd:float/double)` constant — the `Float64` order-preserving range datatype.
pub fn dfloat(f: f64) -> DataValue {
    DataValue::Data(XSDAnySimpleType::Float(f))
}

/// A `DataValue::Data(xsd:hexBinary)` constant (an UNSUPPORTED range datatype).
pub fn dhex(s: &str) -> DataValue {
    DataValue::Data(XSDAnySimpleType::HexBinary(s.to_string()))
}

/// A `DataValue::List` (an unsupported range operand).
pub fn dlist(items: Vec<DataValue>) -> DataValue {
    DataValue::List(items)
}

/// `Greater(left, right)` — `left > right`.
pub fn greater(left: DataValue, right: DataValue) -> Query {
    Query::Greater(Greater { left, right })
}

/// `Less(left, right)` — `left < right`.
pub fn less(left: DataValue, right: DataValue) -> Query {
    Query::Less(Less { left, right })
}

/// `Gte(left, right)` — `left >= right`.
pub fn gte(left: DataValue, right: DataValue) -> Query {
    Query::Gte(Gte { left, right })
}

/// `Lte(left, right)` — `left <= right`.
pub fn lte(left: DataValue, right: DataValue) -> Query {
    Query::Lte(Lte { left, right })
}

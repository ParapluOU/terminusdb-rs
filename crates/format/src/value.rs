//! Classification of a WOQL/document result JSON value into a wire term.
//!
//! A value coming back from a WOQL query binding (or embedded in a document)
//! can be one of a few shapes, and both the SQL binding decoder
//! (`runner.rs::decode_value`) and the SPARQL result reader
//! (`tests/spec.rs::binding_scalar`) implemented the same three-way split by
//! hand. [`classify_value`] is the single canonical implementation.

use crate::keyword::{ID, TYPE, VALUE};
use serde_json::Value;

/// A TerminusDB wire value, classified from a `serde_json::Value`.
///
/// The distinction that matters is **node vs literal**: a bare JSON string, or
/// an object `{"@id": …}`, is a *node* reference (a graph edge target / IRI); an
/// object `{"@type": …, "@value": …}` is a *typed literal*. Bare JSON scalars
/// are literals of the obvious kind.
#[derive(Clone, Debug, PartialEq)]
pub enum WireValue {
    /// JSON `null`.
    Null,
    /// A JSON boolean literal.
    Bool(bool),
    /// A bare JSON numeric literal (untyped — no accompanying `@type`).
    Number(serde_json::Number),
    /// A node reference: a bare string IRI, or `{"@id": "<iri>"}`.
    Node(String),
    /// A typed literal `{"@type": <datatype>, "@value": <value>}`. `datatype` is
    /// `None` when a `@value` appears without an accompanying `@type`.
    Literal {
        /// The datatype string (CURIE or IRI), if present.
        datatype: Option<String>,
        /// The raw `@value` payload (may be a JSON string, number, bool, …).
        value: Value,
    },
    /// A value not matching any known shape (e.g. an object with neither `@id`
    /// nor `@value`, or a bare array). Callers usually pass this through as raw
    /// JSON.
    Unknown(Value),
}

/// Classify a WOQL/document result value.
///
/// Rules (matching the previous hand-rolled decoders):
/// - bare string ⇒ [`WireValue::Node`] (WOQL bindings represent node ids as bare strings)
/// - bare bool/number/null ⇒ the corresponding scalar literal
/// - object with `@value` ⇒ [`WireValue::Literal`] (datatype from `@type` if present)
/// - object with `@id` (and no `@value`) ⇒ [`WireValue::Node`]
/// - anything else ⇒ [`WireValue::Unknown`]
pub fn classify_value(v: &Value) -> WireValue {
    match v {
        Value::Null => WireValue::Null,
        Value::Bool(b) => WireValue::Bool(*b),
        Value::Number(n) => WireValue::Number(n.clone()),
        Value::String(s) => WireValue::Node(s.clone()),
        Value::Object(o) => {
            if let Some(value) = o.get(VALUE) {
                WireValue::Literal {
                    datatype: o.get(TYPE).and_then(Value::as_str).map(str::to_string),
                    value: value.clone(),
                }
            } else if let Some(id) = o.get(ID).and_then(Value::as_str) {
                WireValue::Node(id.to_string())
            } else {
                WireValue::Unknown(v.clone())
            }
        }
        Value::Array(_) => WireValue::Unknown(v.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bare_string_is_node() {
        assert_eq!(classify_value(&json!("Person/alice")), WireValue::Node("Person/alice".into()));
    }

    #[test]
    fn typed_literal() {
        let v = json!({"@type": "xsd:integer", "@value": 42});
        assert_eq!(
            classify_value(&v),
            WireValue::Literal { datatype: Some("xsd:integer".into()), value: json!(42) }
        );
    }

    #[test]
    fn id_object_is_node() {
        assert_eq!(classify_value(&json!({"@id": "Person/bob"})), WireValue::Node("Person/bob".into()));
    }

    #[test]
    fn value_without_type() {
        assert_eq!(
            classify_value(&json!({"@value": "hi"})),
            WireValue::Literal { datatype: None, value: json!("hi") }
        );
    }

    #[test]
    fn scalars_and_unknown() {
        assert_eq!(classify_value(&json!(true)), WireValue::Bool(true));
        assert_eq!(classify_value(&Value::Null), WireValue::Null);
        assert!(matches!(classify_value(&json!({"foo": 1})), WireValue::Unknown(_)));
        assert!(matches!(classify_value(&json!([1, 2])), WireValue::Unknown(_)));
    }
}

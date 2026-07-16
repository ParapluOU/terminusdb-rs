//! Executing a compiled query and decoding WOQL bindings back into SQL rows.
//!
//! The emitter produced one WOQL variable per projected column; a result is one
//! binding map per solution. An **absent** variable in a binding is decoded as SQL
//! `NULL` (the datalog-absence ↔ NULL reconciliation described in `emit`).

use std::collections::HashMap;

use serde_json::Value;

use crate::emit::ProjCol;

/// A decoded SQL result set.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResponse {
    /// Output column names, in SELECT order.
    pub columns: Vec<String>,
    /// Rows, each a value per column (same order as `columns`).
    pub rows: Vec<Vec<SqlValue>>,
}

/// A decoded SQL cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    /// Absent binding — a SQL NULL.
    Null,
    /// A node / IRI (an `id` or object-reference value).
    Node(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(String),
    Str(String),
    /// A date / time / dateTime kept in its lexical form.
    Temporal(String),
    /// Anything else, kept as raw JSON.
    Json(Value),
}

impl QueryResponse {
    /// Decode WOQL bindings into rows, in the projection's column order.
    pub(crate) fn decode(bindings: Vec<HashMap<String, Value>>, projection: &[ProjCol]) -> Self {
        let columns = projection.iter().map(|p| p.sql_name.clone()).collect();
        let rows = bindings
            .into_iter()
            .map(|binding| {
                projection
                    .iter()
                    .map(|p| match binding.get(&p.woql_var) {
                        None | Some(Value::Null) => SqlValue::Null,
                        Some(v) => decode_value(v),
                    })
                    .collect()
            })
            .collect();
        QueryResponse { columns, rows }
    }

    /// Render rows as column-name-keyed JSON objects (absent cells → JSON null).
    pub fn to_json_rows(&self) -> Vec<serde_json::Map<String, Value>> {
        self.rows
            .iter()
            .map(|row| {
                self.columns
                    .iter()
                    .zip(row)
                    .map(|(name, val)| (name.clone(), val.to_json()))
                    .collect()
            })
            .collect()
    }
}

impl SqlValue {
    pub fn to_json(&self) -> Value {
        match self {
            SqlValue::Null => Value::Null,
            SqlValue::Node(s) | SqlValue::Str(s) | SqlValue::Decimal(s) | SqlValue::Temporal(s) => {
                Value::String(s.clone())
            }
            SqlValue::Bool(b) => Value::Bool(*b),
            SqlValue::Int(i) => Value::Number((*i).into()),
            SqlValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            SqlValue::Json(v) => v.clone(),
        }
    }
}

/// Decode one WOQL binding value. A bare string is a node IRI; a typed
/// `{"@type","@value"}` object is a literal; bare numbers/bools are literals too.
fn decode_value(v: &Value) -> SqlValue {
    match v {
        Value::String(s) => SqlValue::Node(s.clone()),
        Value::Bool(b) => SqlValue::Bool(*b),
        Value::Number(n) => number_to_sql(n),
        Value::Object(o) => match (o.get("@type").and_then(Value::as_str), o.get("@value")) {
            (Some(ty), Some(val)) => typed_literal(ty, val),
            // A node object like {"@id": "..."}.
            _ => match o.get("@id").and_then(Value::as_str) {
                Some(id) => SqlValue::Node(id.to_string()),
                None => SqlValue::Json(v.clone()),
            },
        },
        Value::Null => SqlValue::Null,
        other => SqlValue::Json(other.clone()),
    }
}

fn typed_literal(ty: &str, val: &Value) -> SqlValue {
    let local = ty.strip_prefix("xsd:").unwrap_or(ty);
    match local {
        "string" | "normalizedString" | "token" | "anyURI" => {
            SqlValue::Str(val.as_str().unwrap_or_default().to_string())
        }
        "boolean" => SqlValue::Bool(as_bool(val)),
        "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
        | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" | "unsignedInt"
        | "unsignedLong" | "unsignedShort" | "unsignedByte" => match val.as_i64() {
            Some(i) => SqlValue::Int(i),
            None => val
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .map(SqlValue::Int)
                .unwrap_or_else(|| SqlValue::Json(val.clone())),
        },
        "decimal" => SqlValue::Decimal(scalar_string(val)),
        "double" | "float" => match val.as_f64() {
            Some(f) => SqlValue::Float(f),
            None => val
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .map(SqlValue::Float)
                .unwrap_or_else(|| SqlValue::Json(val.clone())),
        },
        "date" | "dateTime" | "dateTimeStamp" | "time" => SqlValue::Temporal(scalar_string(val)),
        _ => SqlValue::Str(scalar_string(val)),
    }
}

fn number_to_sql(n: &serde_json::Number) -> SqlValue {
    if let Some(i) = n.as_i64() {
        SqlValue::Int(i)
    } else if let Some(f) = n.as_f64() {
        SqlValue::Float(f)
    } else {
        SqlValue::Json(Value::Number(n.clone()))
    }
}

fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => s == "true",
        _ => false,
    }
}

fn scalar_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

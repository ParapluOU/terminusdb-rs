use crate::*;
use terminusdb_schema::{
    BOOL, DATE, DATETIME, DECIMAL, FLOAT, HEX_BINARY, TIME, UNSIGNED_INT, URI,
};

impl ToCLIQueryAST for XSDAnySimpleType {
    fn to_ast(&self) -> String {
        match self {
            XSDAnySimpleType::String(inner) => inner.clone(),
            XSDAnySimpleType::Decimal(inner) => {
                format!("\"{}\"^^<{}>", inner.to_string(), DECIMAL)
            }
            XSDAnySimpleType::Float(inner) => {
                format!("\"{}\"^^<{}>", inner.to_string(), FLOAT)
            }
            XSDAnySimpleType::Boolean(inner) => {
                format!("\"{}\"^^<{}>", inner.to_string(), BOOL)
            }
            XSDAnySimpleType::HexBinary(inner) => {
                format!("\"{}\"^^<{}>", inner.to_string(), HEX_BINARY)
            }
            XSDAnySimpleType::URI(inner) => {
                format!("\"{}\"^^<{}>", inner, URI)
            }
            XSDAnySimpleType::Date(inner) => format!("\"{}\"^^<{}>", inner.to_string(), DATE),
            XSDAnySimpleType::UnsignedInt(inner) => {
                format!("\"{}\"^^<{}>", inner, UNSIGNED_INT)
            }
            XSDAnySimpleType::DateTime(inner) => {
                format!("\"{}\"^^<{}>", inner.to_rfc3339(), DATETIME)
            }
            XSDAnySimpleType::Time(inner) => {
                format!("\"{}\"^^<{}>", inner.format("%H:%M:%S%.f"), TIME)
            }
        }
    }
}

impl ToRESTQuery for XSDAnySimpleType {
    fn to_rest_query_json(&self) -> serde_json::Value {
        todo!()
    }
}

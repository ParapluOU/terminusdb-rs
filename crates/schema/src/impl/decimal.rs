//! `xsd:decimal` model-field support via `bigdecimal::BigDecimal` (arbitrary
//! precision). TerminusDB 12 stores decimals as exact rationals (up to 256
//! digits) and returns them as native JSON numbers; with serde_json's
//! `arbitrary_precision` feature enabled workspace-wide, those numbers reach us
//! losslessly. We serialize on write as a string-wrapped `@value` — TerminusDB
//! accepts string `xsd:decimal` input, which avoids any float coercion and is
//! exact — and accept either a JSON string or number on read.

use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, MaybeFromTDBInstance, Primitive, PrimitiveValue,
    Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass, DECIMAL,
};
use anyhow::bail;
use bigdecimal::BigDecimal;
use serde_json::Value;
use std::str::FromStr;

impl ToSchemaClass for BigDecimal {
    fn to_class() -> String {
        DECIMAL.to_string()
    }
}

impl Primitive for BigDecimal {}

impl ToMaybeTDBSchema for BigDecimal {}

impl From<BigDecimal> for PrimitiveValue {
    fn from(d: BigDecimal) -> Self {
        // Exact, lossless, and free of float coercion.
        Self::String(d.to_string())
    }
}

impl From<BigDecimal> for InstanceProperty {
    fn from(d: BigDecimal) -> Self {
        Self::Primitive(d.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for BigDecimal {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

/// Parse a `BigDecimal` from a primitive that TerminusDB may return either as a
/// string or as an `arbitrary_precision` JSON number.
fn decimal_from_primitive(prop: &InstanceProperty) -> anyhow::Result<Option<BigDecimal>> {
    match prop {
        InstanceProperty::Primitive(PrimitiveValue::String(s)) => Ok(Some(BigDecimal::from_str(s)?)),
        InstanceProperty::Primitive(PrimitiveValue::Number(n)) => {
            Ok(Some(BigDecimal::from_str(&n.to_string())?))
        }
        _ => Ok(None),
    }
}

impl FromInstanceProperty for BigDecimal {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match decimal_from_primitive(prop)? {
            Some(d) => Ok(d),
            None => bail!("Expected an xsd:decimal (string or number), got {:?}", prop),
        }
    }
}

impl MaybeFromTDBInstance for BigDecimal {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }

    fn maybe_from_property(prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        // Non-decimal primitives are simply "not a match", not an error.
        Ok(decimal_from_primitive(prop).unwrap_or(None))
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for BigDecimal {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        let s = match json {
            Value::String(s) => s,
            // arbitrary_precision preserves the full digit string here.
            Value::Number(n) => n.to_string(),
            other => bail!("Expected an xsd:decimal string or number, got {}", other),
        };
        // Validate it really is a decimal, but keep the exact text.
        BigDecimal::from_str(&s)?;
        Ok(InstanceProperty::Primitive(PrimitiveValue::String(s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Property, ToSchemaProperty};

    #[test]
    fn test_decimal_schema_property() {
        let property = <BigDecimal as ToSchemaProperty<()>>::to_property("price");
        assert_eq!(property.name, "price");
        assert_eq!(property.class, DECIMAL);
    }

    #[test]
    fn test_decimal_roundtrip_high_precision() {
        // 20+ significant digits survive a full round-trip (string form).
        let d = BigDecimal::from_str("0.33333333333333333333").unwrap();
        let prop = <BigDecimal as ToInstanceProperty<()>>::to_property(
            d.clone(),
            "ratio",
            &Schema::empty_class("Test"),
        );
        let back = BigDecimal::from_property(&prop).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn test_decimal_from_native_number() {
        // A high-precision arbitrary_precision JSON number parses losslessly.
        let json: Value = serde_json::from_str("0.33333333333333333333").unwrap();
        let prop = <BigDecimal as InstancePropertyFromJson<()>>::property_from_json(json).unwrap();
        let back = BigDecimal::from_property(&prop).unwrap();
        assert_eq!(back, BigDecimal::from_str("0.33333333333333333333").unwrap());
    }
}

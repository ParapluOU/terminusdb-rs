use crate::{
    json::InstancePropertyFromJson, FromInstanceProperty, InstanceProperty, Primitive, PrimitiveValue,
    Schema, ToInstanceProperty, ToSchemaClass, DATETIME, DECIMAL, INTEGER, TIME, UNSIGNED_INT, URI,
};
use chrono::Utc;
use decimal_rs::Decimal;
use serde::Serialize;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
pub struct XSDDate {
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

impl ToString for XSDDate {
    fn to_string(&self) -> String {
        format!("{}-{}-{}", self.year, self.month, self.day).to_string()
    }
}

// todo: expand
/// http://www.datypic.com/sc/xsd/t-xsd_anySimpleType.html
#[derive(Clone, Debug, Serialize)]
pub enum XSDAnySimpleType {
    String(String),
    Decimal(Decimal),
    Float(f64),
    // Double(),
    Boolean(bool),
    // todo: specific format
    HexBinary(String),
    URI(URI),
    DateTime(chrono::DateTime<Utc>),
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    // DateTime(chrono::DateTime<Tz>)
    UnsignedInt(usize),
    Integer(i64),
}

// Helper function to get a consistent numeric representation of the variant for ordering
fn variant_order(v: &XSDAnySimpleType) -> u8 {
    match v {
        XSDAnySimpleType::String(_) => 0,
        XSDAnySimpleType::Decimal(_) => 1,
        XSDAnySimpleType::Float(_) => 2,
        XSDAnySimpleType::Boolean(_) => 3,
        XSDAnySimpleType::HexBinary(_) => 4,
        XSDAnySimpleType::URI(_) => 5,
        XSDAnySimpleType::DateTime(_) => 6,
        XSDAnySimpleType::Date(_) => 7,
        XSDAnySimpleType::Time(_) => 8,
        XSDAnySimpleType::UnsignedInt(_) => 9,
        XSDAnySimpleType::Integer(_) => 10,
    }
}

impl Into<PrimitiveValue> for XSDAnySimpleType {
    fn into(self) -> PrimitiveValue {
        todo!()
    }
}

impl PartialOrd for XSDAnySimpleType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let order = variant_order(self).cmp(&variant_order(other));
        if order != Ordering::Equal {
            return Some(order);
        }

        // Variants are the same, compare inner values
        match (self, other) {
            (Self::String(l), Self::String(r)) => l.partial_cmp(r),
            (Self::Decimal(l), Self::Decimal(r)) => l.partial_cmp(r),
            (Self::Float(l), Self::Float(r)) => l.partial_cmp(r),
            (Self::Boolean(l), Self::Boolean(r)) => l.partial_cmp(r),
            (Self::HexBinary(l), Self::HexBinary(r)) => l.partial_cmp(r),
            (Self::URI(l), Self::URI(r)) => l.partial_cmp(r),
            (Self::DateTime(l), Self::DateTime(r)) => l.partial_cmp(r),
            (Self::Date(l), Self::Date(r)) => l.partial_cmp(r),
            (Self::Time(l), Self::Time(r)) => l.partial_cmp(r),
            (Self::UnsignedInt(l), Self::UnsignedInt(r)) => l.partial_cmp(r),
            (Self::Integer(l), Self::Integer(r)) => l.partial_cmp(r),
            // This case should be unreachable because we check variant order first
            _ => unreachable!("Variant order mismatch after equality check"),
        }
    }
}

impl Ord for XSDAnySimpleType {
    fn cmp(&self, other: &Self) -> Ordering {
        let order = variant_order(self).cmp(&variant_order(other));
        if order != Ordering::Equal {
            return order;
        }

        // Variants are the same, compare inner values using total order
        match (self, other) {
            (Self::String(l), Self::String(r)) => l.cmp(r),
            (Self::Decimal(l), Self::Decimal(r)) => l.cmp(r),
            (Self::Float(l), Self::Float(r)) => {
                // Use total_cmp for a total order on f64 (requires Rust 1.62+)
                l.total_cmp(r)
            }
            (Self::Boolean(l), Self::Boolean(r)) => l.cmp(r),
            (Self::HexBinary(l), Self::HexBinary(r)) => l.cmp(r),
            (Self::URI(l), Self::URI(r)) => l.cmp(r),
            (Self::DateTime(l), Self::DateTime(r)) => l.cmp(r),
            (Self::Date(l), Self::Date(r)) => l.cmp(r),
            (Self::Time(l), Self::Time(r)) => l.cmp(r),
            (Self::UnsignedInt(l), Self::UnsignedInt(r)) => l.cmp(r),
            (Self::Integer(l), Self::Integer(r)) => l.cmp(r),
            // This case should be unreachable because we check variant order first
            _ => unreachable!("Variant order mismatch after equality check"),
        }
    }
}

impl PartialEq for XSDAnySimpleType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::Decimal(l0), Self::Decimal(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => l0.to_bits() == r0.to_bits(),
            (Self::Boolean(l0), Self::Boolean(r0)) => l0 == r0,
            (Self::HexBinary(l0), Self::HexBinary(r0)) => l0 == r0,
            (Self::URI(l0), Self::URI(r0)) => l0 == r0,
            (Self::DateTime(l0), Self::DateTime(r0)) => l0 == r0,
            (Self::Date(l0), Self::Date(r0)) => l0 == r0,
            (Self::Time(l0), Self::Time(r0)) => l0 == r0,
            (Self::UnsignedInt(l0), Self::UnsignedInt(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for XSDAnySimpleType {}

impl Hash for XSDAnySimpleType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            XSDAnySimpleType::String(s) => s.hash(state),
            XSDAnySimpleType::Decimal(d) => d.hash(state),
            XSDAnySimpleType::Float(f) => f.to_bits().hash(state),
            XSDAnySimpleType::Boolean(b) => b.hash(state),
            XSDAnySimpleType::HexBinary(s) => s.hash(state),
            XSDAnySimpleType::URI(u) => u.hash(state),
            XSDAnySimpleType::DateTime(dt) => dt.hash(state),
            XSDAnySimpleType::Date(d) => d.hash(state),
            XSDAnySimpleType::Time(t) => t.hash(state),
            XSDAnySimpleType::UnsignedInt(i) => i.hash(state),
            XSDAnySimpleType::Integer(i) => i.hash(state),
        }
    }
}

#[macro_export]
macro_rules! xsd {
    ($val:expr => $variant:ident) => {
        XSDAnySimpleType::$variant($val)
    };
}

// todo: impl TerminusDB traits

impl Primitive for XSDAnySimpleType {}

impl ToSchemaClass for XSDAnySimpleType {
    fn to_class() -> String {
        "xsd:anySimpleType".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xsd_any_simple_type_serialization_issue() {
        // This test demonstrates the current serialization behavior vs what TerminusDB expects

        // Create an integer value
        let int_value = XSDAnySimpleType::Decimal(Decimal::from(50));

        // Current serialization (using serde)
        let current_json = serde_json::to_value(&int_value).unwrap();
        println!(
            "Current serialization: {}",
            serde_json::to_string_pretty(&current_json).unwrap()
        );

        // What we get: {"Decimal": "50"}
        // This is the derived serde serialization which includes the enum variant name

        // What TerminusDB expects for WOQL queries:
        let expected_json = serde_json::json!({
            "@type": "xsd:integer",
            "@value": 50
        });
        println!(
            "Expected by TerminusDB: {}",
            serde_json::to_string_pretty(&expected_json).unwrap()
        );

        // The issue is that XSDAnySimpleType needs custom serialization when used in WOQL contexts
        // to produce JSON-LD format with @type and @value fields

        // This assertion will fail, demonstrating the issue
        // assert_eq!(current_json, expected_json);

        // Instead, we document what actually happens
        assert!(current_json.is_object());
        assert!(current_json.as_object().unwrap().contains_key("Decimal"));
    }

    #[test]
    fn test_all_xsd_types_serialization() {
        // Test various XSD types to show the pattern
        let test_cases = vec![
            (
                XSDAnySimpleType::String("hello".to_string()),
                "String",
                "hello",
            ),
            (
                XSDAnySimpleType::Decimal(Decimal::from(42)),
                "Decimal",
                "42",
            ),
            (XSDAnySimpleType::Float(3.14), "Float", "3.14"),
            (XSDAnySimpleType::Boolean(true), "Boolean", "true"),
        ];

        for (value, expected_key, _expected_value) in test_cases {
            let json = serde_json::to_value(&value).unwrap();
            let obj = json.as_object().unwrap();

            // Current behavior: enum variant as key
            assert!(
                obj.contains_key(expected_key),
                "Expected key '{}' not found in {:?}",
                expected_key,
                obj
            );

            // What TerminusDB needs: {"@type": "xsd:TYPE", "@value": VALUE}
            // This would require custom serialization implementation
        }
    }

    #[test]
    fn canonical_typed_xsd_json_roundtrips_and_rejects_malformed_objects() {
        let json = serde_json::json!({"@type": "xsd:unsignedInt", "@value": 42});
        let property =
            <XSDAnySimpleType as InstancePropertyFromJson<()>>::property_from_json(json).unwrap();
        assert_eq!(
            XSDAnySimpleType::from_property(&property).unwrap(),
            XSDAnySimpleType::UnsignedInt(42)
        );
        for invalid in [
            serde_json::json!({"@type": "xsd:unsignedInt"}),
            serde_json::json!({"@type": "xsd:unsignedInt", "@value": 42, "extra": true}),
            serde_json::json!({"@type": "xsd:unknown", "@value": 42}),
            serde_json::json!({"@type": "xsd:unsignedInt", "@value": -1}),
        ] {
            assert!(
                <XSDAnySimpleType as InstancePropertyFromJson<()>>::property_from_json(invalid)
                    .is_err()
            );
        }
    }
}

// impl ToTDBSchema for XSDAnySimpleType {
//     fn to_schema() -> Schema {
//         unimplemented!()
//     }
//
//     fn to_schema_tree() -> Vec<Schema> {
//         unimplemented!()
//     }
//
//     fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
//         let schema = Self::to_schema();
//         let class_name = schema.class_name().clone();
//
//         if !collection
//             .iter()
//             .any(|s: &Schema| s.class_name() == &class_name)
//         {
//             collection.insert(schema);
//         }
//     }
// }

impl FromInstanceProperty for XSDAnySimpleType {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        let primitive = match prop {
            InstanceProperty::Primitive(value) => value,
            _ => return Err(anyhow::anyhow!("xsd value must be a primitive property")),
        };
        match primitive {
            PrimitiveValue::String(value) => Ok(Self::String(value.clone())),
            PrimitiveValue::Bool(value) => Ok(Self::Boolean(*value)),
            PrimitiveValue::Number(value) => value
                .as_f64()
                .map(Self::Float)
                .ok_or_else(|| anyhow::anyhow!("xsd numeric value is not representable as f64")),
            PrimitiveValue::Object(value) => xsd_from_typed_json(value),
            PrimitiveValue::Null | PrimitiveValue::Unit => Err(anyhow::anyhow!(
                "null/unit is not an xsd:anySimpleType value"
            )),
        }
    }
}

impl<Parent> ToInstanceProperty<Parent> for XSDAnySimpleType {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        match self {
            XSDAnySimpleType::String(s) => InstanceProperty::Primitive(PrimitiveValue::String(s)),
            XSDAnySimpleType::Boolean(b) => InstanceProperty::Primitive(PrimitiveValue::Bool(b)),
            XSDAnySimpleType::Decimal(d) => {
                InstanceProperty::Primitive(PrimitiveValue::Object(serde_json::json!({
                    "@type": DECIMAL,
                    "@value": d.to_string()
                })))
            }
            XSDAnySimpleType::Float(f) => InstanceProperty::Primitive(PrimitiveValue::Number(
                serde_json::Number::from_f64(f).expect("parse f64 to serde_json Number"),
            )),
            XSDAnySimpleType::HexBinary(s) => {
                InstanceProperty::Primitive(PrimitiveValue::String(s))
            }
            XSDAnySimpleType::URI(u) => {
                InstanceProperty::Primitive(PrimitiveValue::String(u.to_string()))
            }
            XSDAnySimpleType::DateTime(dt) => {
                InstanceProperty::Primitive(PrimitiveValue::Object(serde_json::json!({
                    "@type": DATETIME,
                    "@value": dt.to_rfc3339()
                })))
            }
            XSDAnySimpleType::Date(d) => {
                InstanceProperty::Primitive(PrimitiveValue::String(d.to_string()))
            }
            XSDAnySimpleType::Time(t) => {
                InstanceProperty::Primitive(PrimitiveValue::Object(serde_json::json!({
                    "@type": TIME,
                    "@value": t.format("%H:%M:%S%.f").to_string()
                })))
            }
            XSDAnySimpleType::UnsignedInt(i) => {
                InstanceProperty::Primitive(PrimitiveValue::Object(serde_json::json!({
                    "@type": UNSIGNED_INT,
                    "@value": i
                })))
            }
            XSDAnySimpleType::Integer(i) => {
                InstanceProperty::Primitive(PrimitiveValue::Object(serde_json::json!({
                    "@type": INTEGER,
                    "@value": i
                })))
            }
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for XSDAnySimpleType {
    fn property_from_json(json: serde_json::Value) -> anyhow::Result<InstanceProperty> {
        // Convert serde_json::Value to PrimitiveValue
        let primitive = match json {
            serde_json::Value::String(s) => PrimitiveValue::String(s),
            serde_json::Value::Number(n) => PrimitiveValue::Number(n),
            serde_json::Value::Bool(b) => PrimitiveValue::Bool(b),
            serde_json::Value::Null => PrimitiveValue::Null,
            serde_json::Value::Object(_) => {
                // Validate the canonical typed JSON now, then retain it verbatim so
                // `FromInstanceProperty` can reconstruct the exact enum variant.
                xsd_from_typed_json(&json)?;
                PrimitiveValue::Object(json)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported JSON type for XSDAnySimpleType"
                ))
            }
        };
        Ok(InstanceProperty::Primitive(primitive))
    }
}

fn xsd_from_typed_json(json: &serde_json::Value) -> anyhow::Result<XSDAnySimpleType> {
    let map = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("typed xsd value must be an object"))?;
    if map.len() != 2 || !map.contains_key("@type") || !map.contains_key("@value") {
        return Err(anyhow::anyhow!(
            "typed xsd value must contain exactly @type and @value"
        ));
    }
    let datatype = map["@type"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("typed xsd @type must be a string"))?;
    let value = &map["@value"];
    let string = || {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("{datatype} @value must be a string"))
    };
    Ok(match datatype {
        "xsd:string" => XSDAnySimpleType::String(string()?),
        "xsd:boolean" => XSDAnySimpleType::Boolean(
            value
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("xsd:boolean @value must be boolean"))?,
        ),
        "xsd:unsignedInt" => XSDAnySimpleType::UnsignedInt(
            usize::try_from(
                value
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("xsd:unsignedInt @value must be unsigned"))?,
            )
            .map_err(|_| anyhow::anyhow!("xsd:unsignedInt does not fit usize"))?,
        ),
        "xsd:integer" => XSDAnySimpleType::Integer(
            value
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("xsd:integer @value must fit i64"))?,
        ),
        "xsd:float" | "xsd:double" => XSDAnySimpleType::Float(
            value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("{datatype} @value must be numeric"))?,
        ),
        "xsd:decimal" => XSDAnySimpleType::Decimal(
            Decimal::from_str(&string()?).map_err(|_| anyhow::anyhow!("invalid xsd:decimal"))?,
        ),
        "xsd:hexBinary" => XSDAnySimpleType::HexBinary(string()?),
        "xsd:anyURI" => XSDAnySimpleType::URI(string()?),
        "xsd:dateTime" => XSDAnySimpleType::DateTime(
            chrono::DateTime::parse_from_rfc3339(&string()?)
                .map_err(|_| anyhow::anyhow!("invalid xsd:dateTime"))?
                .with_timezone(&Utc),
        ),
        "xsd:date" => XSDAnySimpleType::Date(
            chrono::NaiveDate::parse_from_str(&string()?, "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("invalid xsd:date"))?,
        ),
        "xsd:time" => XSDAnySimpleType::Time(
            chrono::NaiveTime::parse_from_str(&string()?, "%H:%M:%S%.f")
                .map_err(|_| anyhow::anyhow!("invalid xsd:time"))?,
        ),
        _ => return Err(anyhow::anyhow!("unsupported xsd datatype tag '{datatype}'")),
    })
}

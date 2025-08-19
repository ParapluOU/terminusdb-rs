use crate::{
    json::InstancePropertyFromJson, FromInstanceProperty, InstanceProperty, Primitive,
    PrimitiveValue, Schema, ToInstanceProperty, ToSchemaClass, ToTDBSchema, URI,
};
use chrono::Utc;
use decimal_rs::Decimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    fn to_class() -> &'static str {
        "xsd:anySimpleType"
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
        todo!()
    }
}

impl<Parent> ToInstanceProperty<Parent> for XSDAnySimpleType {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        match self {
            XSDAnySimpleType::String(s) => InstanceProperty::Primitive(PrimitiveValue::String(s)),
            XSDAnySimpleType::Boolean(b) => InstanceProperty::Primitive(PrimitiveValue::Bool(b)),
            XSDAnySimpleType::Decimal(d) => InstanceProperty::Primitive(PrimitiveValue::String(d.to_string())),
            XSDAnySimpleType::Float(f) => InstanceProperty::Primitive(PrimitiveValue::Number(
                serde_json::Number::from_f64(f).expect("parse f64 to serde_json Number")
            )),
            XSDAnySimpleType::HexBinary(s) => InstanceProperty::Primitive(PrimitiveValue::String(s)),
            XSDAnySimpleType::URI(u) => InstanceProperty::Primitive(PrimitiveValue::String(u.to_string())),
            XSDAnySimpleType::DateTime(dt) => InstanceProperty::Primitive(PrimitiveValue::Object(
                serde_json::json!({
                    "@type": "xsd:dateTime",
                    "@value": dt.to_rfc3339()
                })
            )),
            XSDAnySimpleType::Date(d) => InstanceProperty::Primitive(PrimitiveValue::String(d.to_string())),
            XSDAnySimpleType::Time(t) => InstanceProperty::Primitive(PrimitiveValue::Object(
                serde_json::json!({
                    "@type": "xsd:time",
                    "@value": t.format("%H:%M:%S%.f").to_string()
                })
            )),
            XSDAnySimpleType::UnsignedInt(i) => InstanceProperty::Primitive(PrimitiveValue::Number(
                serde_json::Number::from_u128(i as u128).expect("parse usize to serde_json Number")
            )),
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
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported JSON type for XSDAnySimpleType"
                ))
            }
        };
        Ok(InstanceProperty::Primitive(primitive))
    }
}

// https://github.com/terminusdb/terminusdb-documents-ui/blob/95a8afeb26d563984c9cf9cca5b4036de273319a/src/constants.js
// https://github.com/terminusdb/terminus-dashboard/blob/f578c3004cbea315fc8f41c789cf57f59db01c39/src/html/datatypes/StringEditor.js
// https://github.com/terminusdb/terminusdb/blob/main/src/core/triple/base_type.pl

// todo: parse XSD instead, but where is it?

use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::marker::PhantomData;

use refined::{boundable::signed::Positive, Refinement};

use crate::{
    FromInstanceProperty, Instance, InstanceProperty, Primitive, PrimitiveValue, RelationValue,
    Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass,
};

pub const UNIT: &str = "sys:Unit";

pub const JSON: &str = "sys:JSON";
pub const JSON_DOCUMENT: &str = "sys:JSONDocument";

/**
* base_type(?BaseTypeURI:uri) is nondet.
*
* Predicate which gives the available basetypes.
# Core types
| xsd:string	| Character strings (but not all Unicode character strings) |
| xsd:boolean	| true, false |
| xsd:decimal	| Arbitrary-precision decimal numbers |
| xsd:integer	| Arbitrary-size integer numbers |
# IEEE floating-point numbers
| xsd:double	| 64-bit floating point numbers incl. ±Inf, ±0, NaN |
| xsd:float	| 32-bit floating point numbers incl. ±Inf, ±0, NaN |
# Time and date
| xsd:date	| Dates (yyyy-mm-dd) with or without timezone |
| xsd:time	| Times (hh:mm:ss.sss…) with or without timezone |
| xsd:dateTime	| Date and time with or without timezone |
| xsd:dateTimeStamp	| Date and time with required timezone |
# Recurring and partial dates
| xsd:gYear | 	Gregorian calendar year |
| xsd:gMonth |	Gregorian calendar month |
| xsd:gDay	| Gregorian calendar day of the month |
| xsd:gYearMonth |	Gregorian calendar year and month |
| xsd:gMonthDay |	Gregorian calendar month and day |
| xsd:duration	| Duration of time |
| xsd:yearMonthDuration |	Duration of time (months and years only) |
| xsd:dayTimeDuration |	Duration of time (days, hours, minutes, seconds only) |
# Limited-range integer numbers
| xsd:byte	| -128…+127 (8 bit) |
| xsd:short |	-32768…+32767 (16 bit) |
| xsd:int |	-2147483648…+2147483647 (32 bit) |
| xsd:long |	-9223372036854775808…+9223372036854775807 (64 bit) |
| xsd:unsignedByte |	0…255 (8 bit) |
| xsd:unsignedShort |	0…65535 (16 bit) |
| xsd:unsignedInt |	0…4294967295 (32 bit) |
| xsd:unsignedLong |	0…18446744073709551615 (64 bit) |
# non limited
| xsd:positiveInteger |	Integer numbers >0 |
| xsd:nonNegativeInteger |	Integer numbers ≥0 |
| xsd:negativeInteger |	Integer numbers <0 |
| xsd:nonPositiveInteger |	Integer numbers ≤0 |
# Encoded binary data
| xsd:hexBinary |	Hex-encoded binary data |
| xsd:base64Binary |	Base64-encoded binary data |
# Miscellaneous XSD types
| xsd:anyURI |	Absolute or relative URIs and IRIs |
| xsd:language |	Language tags per [BCP47] |
| xsd:normalizedString |	Whitespace-normalized strings |
| xsd:token |	Tokenized strings |
| xsd:NMTOKEN |	XML NMTOKENs |
| xsd:Name |	XML Names |
| xsd:NCName |	XML NCNames |
 */

macro_rules! primitive {
    ($const_name:ident: $xsd_type:expr) => {
        pub const $const_name : &str = concat!("xsd:", $xsd_type);
    };

    ({$($const_name:ident: $xsd_type:expr),*}) => {
        $(
            primitive!($const_name: $xsd_type);
        )*
    }
}

pub fn is_primitive(cls: &str) -> bool {
    cls.starts_with("xsd:")
}

primitive!({
    // the 'G' stands for 'Gregorian'
    G_YEAR: "gYear",
    G_YEAR_MONTH: "gYearMonth",
    G_YEAR_RANGE: "gYearRange",
    G_MONTH_DAY: "gMonthDay",

    YEAR_MONTH_DURATION: "yearMonthDuration",
    DAY_TIME_DURATION: "dayTimeDuration",
    DURATION: "duration",

    DATE: "date",
    DATETIME: "dateTime",
    DATE_TIMESTAMP: "dateTimeStamp",

    NON_NEGATIVE_INTEGER: "nonNegativeInteger",
    POSITIVE_INTEGER: "positiveInteger",
    NEGATIVE_INTEGER: "negativeInteger",
    NON_POSITIVE_INTEGER: "nonPositiveInteger",
    UNSIGNED_INT: "unsignedInt",

    BYTE: "byte",
    SHORT: "short",
    UNSIGNED_BYTE: "unsignedByte",

    FLOAT: "float",
    TIME: "time",
    STRING: "string",
    DECIMAL: "decimal",

    INTEGER: "integer",
    INT: "integer",
    BOOLEAN: "boolean",
    BOOL: "boolean",
    DOUBLE: "double",
    LONG: "long",
    UNSIGNED_LONG: "unsignedLong",
    HEX_BINARY: "hexBinary",
    BASE64_BINARY: "base64Binary",

    URI: "anyURI"
});

#[test]
fn test_primitive() {
    let x = G_YEAR;
    println!("{}", x);
}

// impl<T> ToSchemaClass for PhantomData<T> {
//     fn to_class() -> &'static str {
//         UNIT
//     }
// }

macro_rules! to_schema_class {
    ($typ:ty: $cls:ident) => {

        impl ToSchemaClass for $typ {
            fn to_class() -> &'static str {
                $cls
            }
        }

        impl ToMaybeTDBSchema for $typ {
            // default
        }

        impl Primitive for $typ {}
    };

    // map single TDB class to multiple Rust types
    // like STRING => [usize, String, ...]
    ($cls:ident => [$($typ:ty),*]) => {
        $(
            to_schema_class!($typ: $cls);
        )*
    };

    ({$($typ:ty: $cls:ident),*}) => {
        $(
            to_schema_class!($typ: $cls);
        )*
    };
}

pub type posint = Refinement<isize, Positive>;

to_schema_class!({
    usize: UNSIGNED_INT,
    u32: UNSIGNED_INT,
    i32: INTEGER,
    i64: LONG,
    isize: INTEGER,
    String: STRING,
    f64: FLOAT,
    f32: FLOAT,
    bool: BOOLEAN,
    u8: UNSIGNED_BYTE,
    i8: BYTE,
    // Address: STRING,
    u64: UNSIGNED_LONG,
    u128: NON_NEGATIVE_INTEGER,
    posint: POSITIVE_INTEGER
    // Timestamp: POSITIVE_INTEGER
});

impl<T: ToSchemaClass> ToSchemaClass for Vec<T> {
    fn to_class() -> &'static str {
        T::to_class()
    }
}

impl<T: ToSchemaClass> ToSchemaClass for Box<T> {
    fn to_class() -> &'static str {
        T::to_class()
    }
}

// impl<T: ToSchemaClass> ToSchemaClass for PhantomData<T> {
//     fn to_class() -> &'static str {
//         T::to_class()
//     }
// }

//
// INSTANCE PROPERTIES
//

impl From<f64> for InstanceProperty {
    fn from(num: f64) -> Self {
        Self::Primitive(num.into())
    }
}

// todo: why cant this be done safely?
impl From<f64> for PrimitiveValue {
    fn from(num: f64) -> Self {
        Self::Number(serde_json::Number::from_f64(num).expect("parse f64 to serde_json Number"))
    }
}

impl From<posint> for PrimitiveValue {
    fn from(num: posint) -> Self {
        Self::Number(
            serde_json::Number::from_u128((*num) as u128)
                .expect("parse posint to serde_json Number"),
        )
    }
}

impl From<f32> for InstanceProperty {
    fn from(num: f32) -> Self {
        Self::Primitive(num.into())
    }
}

// todo: why cant this be done safely?
impl From<f32> for PrimitiveValue {
    fn from(num: f32) -> Self {
        Self::Number(
            serde_json::Number::from_f64(num as f64).expect("parse f64 to serde_json Number"),
        )
    }
}

impl From<u128> for PrimitiveValue {
    fn from(num: u128) -> Self {
        Self::Number(serde_json::Number::from_u128(num).expect("parse u128 to serde_json Number"))
    }
}

impl<Parent> ToInstanceProperty<Parent> for f64 {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl<Parent> ToInstanceProperty<Parent> for f32 {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

#[macro_export]
macro_rules! to_instance_prop {
    ($typ:ty) => {
        to_instance_prop!($typ |no_vec|);

        impl From<Vec<$typ>> for InstanceProperty {
            fn from(from: Vec<$typ>) -> Self {
                Self::Primitives(from.into_iter().map(Into::into).collect())
            }
        }
    };

    ($typ:ty |no_vec|) => {
        impl From<$typ> for InstanceProperty {
            fn from(s: $typ) -> Self {
                Self::Primitive(s.into())
            }
        }

        impl<Parent> ToInstanceProperty<Parent> for $typ {
            fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
                self.into()
            }
        }
    };
}

macro_rules! num_to_prop {
    ($typ:ident) => {
        impl From<$typ> for PrimitiveValue {
            fn from(num: $typ) -> Self {
                Self::Number(num.into())
            }
        }

        to_instance_prop!($typ);
    };
}

num_to_prop!(isize);
num_to_prop!(usize);
num_to_prop!(u32);
num_to_prop!(i32);
num_to_prop!(i64);
num_to_prop!(u64);
num_to_prop!(u16);
// num_to_prop!(u128);
num_to_prop!(i16);
num_to_prop!(u8);
num_to_prop!(i8);

impl From<&str> for PrimitiveValue {
    fn from(s: &str) -> Self {
        Self::String(s.into())
    }
}

impl From<bool> for PrimitiveValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<String> for PrimitiveValue {
    fn from(s: String) -> Self {
        Self::String(s.into())
    }
}

impl From<&String> for PrimitiveValue {
    fn from(s: &String) -> Self {
        Self::String(s.into())
    }
}

to_instance_prop!(&str);
to_instance_prop!(String);
to_instance_prop!(bool);
to_instance_prop!(&String);

impl<T: Into<InstanceProperty>> From<Option<T>> for InstanceProperty {
    fn from(s: Option<T>) -> Self {
        match s {
            None => Self::Primitive(PrimitiveValue::Null),
            Some(v) => v.into(),
        }
    }
}

// impl<T: Into<InstanceProperty>> ToInstanceProperty for Option<T> {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         self.into()
//     }
// }

impl<Parent, T: ToInstanceProperty<Self>> ToInstanceProperty<Parent> for Option<T> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        if self.is_none() {
            InstanceProperty::Primitive(PrimitiveValue::Null)
        } else {
            self.unwrap().to_property(field_name, parent)
        }
    }
}

impl<T: Into<Instance>> From<Vec<T>> for InstanceProperty {
    fn from(objs: Vec<T>) -> Self {
        Self::Relation(RelationValue::More(
            objs.into_iter().map(Into::into).collect(),
        ))
    }
}

// // todo: make generic over IntoIterator?
// impl<Parent, T: ToInstanceProperty<Self>> ToInstanceProperty<Parent> for Vec<T> {
//     default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         InstanceProperty::Any(
//             self.into_iter()
//                 .map(|t| ToInstanceProperty::to_property(t, field_name, parent))
//                 .collect(),
//         )
//     }
// }

// todo: make generic over IntoIterator?
impl<Parent, T: ToInstanceProperty<Self>> ToInstanceProperty<Parent> for BTreeSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Any(
            self.into_iter()
                .map(|t| ToInstanceProperty::to_property(t, field_name, parent))
                .collect(),
        )
    }
}


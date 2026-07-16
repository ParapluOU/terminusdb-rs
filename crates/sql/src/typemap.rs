//! Total mapping from TerminusDB datatypes (xsd / sys) to Arrow logical types.
//!
//! The mapping is **total**: every datatype the server can attach to a datatype
//! property either maps to a named Arrow [`DataType`] or is **explicitly rejected
//! with a reason** ([`RejectReason`]). There is deliberately no silent fallback to
//! `Utf8` — where we do use `Utf8`, the accompanying [`Semantic`] records *why*, so
//! a lexical string is never a silent guess.
//!
//! DataFusion (not us) does the actual type checking of literals against these
//! column types; and the actual value evaluation happens server-side in WOQL. The
//! Arrow types here therefore only need to be faithful enough for DataFusion's
//! *plan-time* coercion and our emitter's literal wrapping — see the notes on
//! `decimal`/`integer` below.

use datafusion_common::arrow::datatypes::{DataType, TimeUnit};

/// Why a column has the Arrow type it does — in particular, why `Utf8` was chosen.
/// This exists so a lexical `Utf8` is never a *silent* fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Semantic {
    /// The Arrow type is a faithful native representation of the datatype.
    Native,
    /// The value is kept as its lexical string form because no native Arrow type
    /// fits (e.g. encoded binary).
    LexicalString,
    /// An IRI / URI kept as a string (`xsd:anyURI`).
    Iri,
}

/// Reason a datatype is not representable as a SQL column in v1. A property with
/// such a type is omitted from the catalog *with this reason recorded*, so a query
/// referencing it produces "column exists but is unsupported (…)", never
/// "no such column".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// `sys:JSON` / `sys:JSONDocument` — structured JSON, out of SQL v1 scope.
    Json,
    /// `sys:Unit` — carries no representable value.
    Unit,
    /// A temporal/duration type with no faithful Arrow representation. Kept out
    /// of v1 rather than mapped to `Utf8`, because `Utf8` would make `ORDER BY` /
    /// range comparisons silently wrong. The `&str` names the concrete type.
    NoArrowType(&'static str),
    /// An unknown or unhandled type name — the catch-all that keeps the map total.
    Unknown(String),
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectReason::Json => write!(f, "sys:JSON is out of SQL v1 scope"),
            RejectReason::Unit => write!(f, "sys:Unit has no representable value"),
            RejectReason::NoArrowType(t) => {
                write!(f, "{t} has no faithful SQL type (out of v1 scope)")
            }
            RejectReason::Unknown(t) => write!(f, "unknown datatype `{t}`"),
        }
    }
}

/// Map a datatype class name (`xsd:string`, `sys:JSON`, …) to an Arrow type +
/// [`Semantic`], or reject it with a [`RejectReason`]. Total over all inputs.
pub fn datatype_to_arrow(class: &str) -> Result<(DataType, Semantic), RejectReason> {
    // sys:* datatypes (is_primitive only recognises xsd:*, so handle these first).
    match class {
        "sys:JSON" | "sys:JSONDocument" => return Err(RejectReason::Json),
        "sys:Unit" => return Err(RejectReason::Unit),
        _ => {}
    }

    let local = class.strip_prefix("xsd:").unwrap_or(class);
    use Semantic::*;
    let mapped: (DataType, Semantic) = match local {
        // --- strings ---
        "string" | "normalizedString" | "token" | "language" | "NMTOKEN" | "Name" | "NCName" => {
            (DataType::Utf8, Native)
        }
        "anyURI" => (DataType::Utf8, Iri),

        // --- boolean ---
        "boolean" => (DataType::Boolean, Native),

        // --- decimal: arbitrary precision folded to a fixed Decimal128 for
        //     plan-time coercion only; real arithmetic is server-side in WOQL. ---
        "decimal" => (DataType::Decimal128(38, 10), Native),

        // --- IEEE floats ---
        "double" => (DataType::Float64, Native),
        "float" => (DataType::Float32, Native),

        // --- bounded integers ---
        "byte" => (DataType::Int8, Native),
        "short" => (DataType::Int16, Native),
        "int" => (DataType::Int32, Native),
        "long" => (DataType::Int64, Native),
        "unsignedByte" => (DataType::UInt8, Native),
        "unsignedShort" => (DataType::UInt16, Native),
        "unsignedInt" => (DataType::UInt32, Native),
        "unsignedLong" => (DataType::UInt64, Native),

        // --- unbounded integers: folded to 64-bit for plan-time coercion only. ---
        "integer" => (DataType::Int64, Native),
        "nonNegativeInteger" | "positiveInteger" => (DataType::UInt64, Native),
        "negativeInteger" | "nonPositiveInteger" => (DataType::Int64, Native),

        // --- date / time. xsd:dateTime's timezone is OPTIONAL, so it maps to a
        //     tz-naive timestamp; xsd:dateTimeStamp REQUIRES a tz, so it is
        //     tz-aware. ---
        "date" => (DataType::Date32, Native),
        "time" => (DataType::Time64(TimeUnit::Microsecond), Native),
        "dateTime" => (DataType::Timestamp(TimeUnit::Microsecond, None), Native),
        "dateTimeStamp" => (
            DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
            Native,
        ),

        // --- encoded binary: kept as its encoded text form. ---
        "hexBinary" | "base64Binary" => (DataType::Utf8, LexicalString),

        // --- rejected: durations and partial/recurring dates have no faithful
        //     Arrow type; mapping them to Utf8 would make ORDER BY / range
        //     comparisons silently wrong, so they are omitted with a reason. ---
        "duration" => return Err(RejectReason::NoArrowType("xsd:duration")),
        "yearMonthDuration" => return Err(RejectReason::NoArrowType("xsd:yearMonthDuration")),
        "dayTimeDuration" => return Err(RejectReason::NoArrowType("xsd:dayTimeDuration")),
        "gYear" => return Err(RejectReason::NoArrowType("xsd:gYear")),
        "gYearMonth" => return Err(RejectReason::NoArrowType("xsd:gYearMonth")),
        "gYearRange" => return Err(RejectReason::NoArrowType("xsd:gYearRange")),
        "gMonth" => return Err(RejectReason::NoArrowType("xsd:gMonth")),
        "gDay" => return Err(RejectReason::NoArrowType("xsd:gDay")),
        "gMonthDay" => return Err(RejectReason::NoArrowType("xsd:gMonthDay")),

        // --- the totality guarantee: any unknown name is rejected, never Utf8. ---
        other => return Err(RejectReason::Unknown(format!("xsd:{other}"))),
    };
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema as prim;

    /// The map is total over every xsd const the schema crate defines: each is
    /// either a tagged Arrow type or an explicit (non-Unknown) rejection — never a
    /// panic and never an untagged silent fallback.
    #[test]
    fn typemap_is_total_over_known_consts() {
        let known = [
            prim::STRING,
            prim::BOOLEAN,
            prim::DECIMAL,
            prim::INTEGER,
            prim::DOUBLE,
            prim::FLOAT,
            prim::LONG,
            prim::BYTE,
            prim::SHORT,
            prim::UNSIGNED_BYTE,
            prim::UNSIGNED_INT,
            prim::UNSIGNED_LONG,
            prim::NON_NEGATIVE_INTEGER,
            prim::POSITIVE_INTEGER,
            prim::NEGATIVE_INTEGER,
            prim::NON_POSITIVE_INTEGER,
            prim::DATE,
            prim::DATETIME,
            prim::DATE_TIMESTAMP,
            prim::TIME,
            prim::HEX_BINARY,
            prim::BASE64_BINARY,
            prim::URI,
            prim::DURATION,
            prim::YEAR_MONTH_DURATION,
            prim::DAY_TIME_DURATION,
            prim::G_YEAR,
            prim::G_YEAR_MONTH,
            prim::G_YEAR_RANGE,
            prim::G_MONTH_DAY,
        ];
        for cls in known {
            match datatype_to_arrow(cls) {
                Ok(_) => {}
                Err(RejectReason::Unknown(u)) => panic!("known const `{cls}` fell through: {u}"),
                Err(_) => {}
            }
        }
    }

    #[test]
    fn sys_types_rejected_with_reason() {
        assert_eq!(datatype_to_arrow("sys:JSON"), Err(RejectReason::Json));
        assert_eq!(
            datatype_to_arrow("sys:JSONDocument"),
            Err(RejectReason::Json)
        );
        assert_eq!(datatype_to_arrow("sys:Unit"), Err(RejectReason::Unit));
    }

    #[test]
    fn representative_mappings() {
        assert_eq!(
            datatype_to_arrow("xsd:string"),
            Ok((DataType::Utf8, Semantic::Native))
        );
        assert_eq!(
            datatype_to_arrow("xsd:anyURI"),
            Ok((DataType::Utf8, Semantic::Iri))
        );
        assert_eq!(
            datatype_to_arrow("xsd:integer"),
            Ok((DataType::Int64, Semantic::Native))
        );
        assert_eq!(
            datatype_to_arrow("xsd:decimal"),
            Ok((DataType::Decimal128(38, 10), Semantic::Native))
        );
        assert!(matches!(
            datatype_to_arrow("xsd:duration"),
            Err(RejectReason::NoArrowType(_))
        ));
        assert!(matches!(
            datatype_to_arrow("xsd:bogusType"),
            Err(RejectReason::Unknown(_))
        ));
    }
}

//! `xsd:*` datatype **CURIE constants** (`STRING == "xsd:string"`, …).
//!
//! These name the XML Schema datatypes TerminusDB understands, in the compact
//! `xsd:` form used throughout schema documents and WOQL. They are the canonical
//! home for this vocabulary; `terminusdb-schema` re-exports them so
//! `terminusdb_schema::STRING` etc. keep working.
//!
//! For classifying an arbitrary datatype string into a coarse value category,
//! see [`crate::datatype`]; for prefix helpers see [`crate::prefix`].

macro_rules! xsd_const {
    ($(#[$m:meta])* $name:ident = $local:literal) => {
        $(#[$m])*
        pub const $name: &str = concat!("xsd:", $local);
    };
    ($( $(#[$m:meta])* $name:ident = $local:literal ),* $(,)?) => {
        $( xsd_const!($(#[$m])* $name = $local); )*
    };
}

xsd_const! {
    // Gregorian date fragments (the `G` is for "Gregorian").
    G_YEAR = "gYear",
    G_YEAR_MONTH = "gYearMonth",
    /// Not a standard XSD datatype, but a TerminusDB-recognised range kept for
    /// backward compatibility (referenced by the SQL type map).
    G_YEAR_RANGE = "gYearRange",
    G_MONTH_DAY = "gMonthDay",

    YEAR_MONTH_DURATION = "yearMonthDuration",
    DAY_TIME_DURATION = "dayTimeDuration",
    DURATION = "duration",

    DATE = "date",
    DATETIME = "dateTime",
    DATE_TIMESTAMP = "dateTimeStamp",

    NON_NEGATIVE_INTEGER = "nonNegativeInteger",
    POSITIVE_INTEGER = "positiveInteger",
    NEGATIVE_INTEGER = "negativeInteger",
    NON_POSITIVE_INTEGER = "nonPositiveInteger",
    UNSIGNED_INT = "unsignedInt",

    BYTE = "byte",
    SHORT = "short",
    UNSIGNED_BYTE = "unsignedByte",

    FLOAT = "float",
    TIME = "time",
    STRING = "string",
    DECIMAL = "decimal",

    INTEGER = "integer",
    /// Legacy alias: historically bound to `xsd:integer` (not `xsd:int`). Kept
    /// verbatim for backward compatibility — prefer [`INTEGER`].
    INT = "integer",
    BOOLEAN = "boolean",
    /// Alias of [`BOOLEAN`] (`xsd:boolean`), retained for existing consumers.
    BOOL = "boolean",
    DOUBLE = "double",
    LONG = "long",
    UNSIGNED_LONG = "unsignedLong",
    HEX_BINARY = "hexBinary",
    BASE64_BINARY = "base64Binary",

    URI = "anyURI",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_values() {
        assert_eq!(STRING, "xsd:string");
        assert_eq!(DATETIME, "xsd:dateTime");
        assert_eq!(UNSIGNED_INT, "xsd:unsignedInt");
        assert_eq!(URI, "xsd:anyURI");
        // Legacy aliases preserved verbatim.
        assert_eq!(INT, "xsd:integer");
        assert_eq!(BOOL, "xsd:boolean");
    }
}

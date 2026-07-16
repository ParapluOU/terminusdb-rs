//! Canonical XSD datatype classification.
//!
//! Before this crate, the SQL compiler (`runner.rs::typed_literal`) and the
//! SPARQL compiler (`lower.rs::lower_literal`) each carried their own copy of
//! the "strip the xsd prefix, then match the local name into a value category"
//! table — with subtly different groupings. This module owns the one canonical
//! table both consume, so a literal always lands in the same category no matter
//! which compiler decoded it.

use crate::prefix::{xsd_local_name, RDF_LANGSTRING_IRI, RDF_PREFIX};

/// A coarse value category for an XSD (or `rdf:langString`) datatype, oriented
/// at *value marshalling* rather than storage width. Consumers that need finer
/// detail (e.g. mapping `xsd:int` vs `xsd:long` to distinct Arrow widths) should
/// match on the local name directly; this category is the shared coarse split
/// that both the SQL binding decoder and the SPARQL literal lowerer agree on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XsdCategory {
    /// `string`, `normalizedString`, `token`, `language`, `anyURI`, `Name`, … —
    /// anything whose value is carried as a plain string. Also `rdf:langString`.
    Text,
    /// `boolean`.
    Boolean,
    /// Any signed/unsigned bounded or unbounded integer type.
    Integer,
    /// `decimal` (arbitrary precision).
    Decimal,
    /// `double` / `float` (IEEE binary).
    Double,
    /// `date`, `dateTime`, `dateTimeStamp`, `time` and other temporal types.
    Temporal,
    /// Binary-encoded data (`hexBinary`, `base64Binary`).
    Binary,
    /// A datatype outside the recognized set (custom/unknown). Callers typically
    /// keep the lexical value as-is and preserve the datatype IRI.
    Other,
}

/// Classify an XSD local name (e.g. `"integer"`, `"dateTime"`) into a coarse
/// value category. Unrecognized names map to [`XsdCategory::Other`].
pub fn classify_xsd_local(local: &str) -> XsdCategory {
    match local {
        "string" | "normalizedString" | "token" | "language" | "NMTOKEN" | "Name" | "NCName"
        | "anyURI" => XsdCategory::Text,
        "boolean" => XsdCategory::Boolean,
        "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
        | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" | "unsignedInt"
        | "unsignedLong" | "unsignedShort" | "unsignedByte" => XsdCategory::Integer,
        "decimal" => XsdCategory::Decimal,
        "double" | "float" => XsdCategory::Double,
        "date" | "dateTime" | "dateTimeStamp" | "time" | "gYear" | "gYearMonth" | "gMonth"
        | "gMonthDay" | "gDay" | "duration" | "yearMonthDuration" | "dayTimeDuration" => {
            XsdCategory::Temporal
        }
        "hexBinary" | "base64Binary" => XsdCategory::Binary,
        _ => XsdCategory::Other,
    }
}

/// Classify a datatype string given in *either* CURIE (`xsd:integer`) or full-IRI
/// (`http://www.w3.org/2001/XMLSchema#integer`) form. `rdf:langString` (either
/// form) classifies as [`XsdCategory::Text`]; everything non-XSD is
/// [`XsdCategory::Other`].
pub fn classify_datatype(datatype: &str) -> XsdCategory {
    if let Some(local) = xsd_local_name(datatype) {
        classify_xsd_local(local)
    } else if datatype == RDF_LANGSTRING_IRI || datatype == "rdf:langString" {
        XsdCategory::Text
    } else if datatype.starts_with(RDF_PREFIX) {
        XsdCategory::Other
    } else {
        XsdCategory::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curie_and_iri_agree() {
        assert_eq!(classify_datatype("xsd:integer"), XsdCategory::Integer);
        assert_eq!(
            classify_datatype("http://www.w3.org/2001/XMLSchema#integer"),
            XsdCategory::Integer
        );
        assert_eq!(classify_datatype("xsd:dateTime"), XsdCategory::Temporal);
        assert_eq!(classify_datatype("xsd:decimal"), XsdCategory::Decimal);
        assert_eq!(classify_datatype("xsd:double"), XsdCategory::Double);
        assert_eq!(classify_datatype("xsd:boolean"), XsdCategory::Boolean);
        assert_eq!(classify_datatype("xsd:anyURI"), XsdCategory::Text);
    }

    #[test]
    fn langstring_is_text_and_unknown_is_other() {
        assert_eq!(classify_datatype(RDF_LANGSTRING_IRI), XsdCategory::Text);
        assert_eq!(classify_datatype("rdf:langString"), XsdCategory::Text);
        assert_eq!(classify_datatype("http://example.com/my#Type"), XsdCategory::Other);
        assert_eq!(classify_datatype("xsd:hexBinary"), XsdCategory::Binary);
    }
}

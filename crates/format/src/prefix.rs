//! Namespace prefixes, full IRIs, and IRI ⇄ CURIE helpers.
//!
//! TerminusDB refers to datatypes and vocabulary in two interchangeable forms:
//! a compact CURIE (`xsd:integer`, `rdf:type`, `@schema:Person`) and a full IRI
//! (`http://www.w3.org/2001/XMLSchema#integer`). WOQL/schema documents use the
//! CURIE form; SPARQL/RDF terms arrive as full IRIs. These helpers convert
//! between the two and classify prefixes.

// ---- CURIE prefixes ----

/// `xsd:` — the XML Schema datatype prefix.
pub const XSD_PREFIX: &str = "xsd:";
/// `sys:` — TerminusDB's system vocabulary prefix (`sys:Unit`, `sys:JSON`, …).
pub const SYS_PREFIX: &str = "sys:";
/// `rdf:` — the RDF vocabulary prefix.
pub const RDF_PREFIX: &str = "rdf:";
/// `rdfs:` — the RDF Schema vocabulary prefix.
pub const RDFS_PREFIX: &str = "rdfs:";
/// `@schema:` — the compact prefix for a class in the current schema namespace.
pub const SCHEMA_PREFIX: &str = "@schema:";

// ---- Full IRI namespace bases ----

/// `http://www.w3.org/2001/XMLSchema#`
pub const XSD_IRI: &str = "http://www.w3.org/2001/XMLSchema#";
/// `http://www.w3.org/1999/02/22-rdf-syntax-ns#`
pub const RDF_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
/// `http://www.w3.org/2000/01/rdf-schema#`
pub const RDFS_IRI: &str = "http://www.w3.org/2000/01/rdf-schema#";
/// The default TerminusDB schema namespace (`http://terminusdb.com/schema#`).
pub const SCHEMA_IRI_DEFAULT: &str = "http://terminusdb.com/schema#";

// ---- Well-known terms ----

/// `rdf:type` (CURIE form).
pub const RDF_TYPE_CURIE: &str = "rdf:type";
/// `http://www.w3.org/1999/02/22-rdf-syntax-ns#type` (full IRI form).
pub const RDF_TYPE_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// `http://www.w3.org/1999/02/22-rdf-syntax-ns#langString`.
pub const RDF_LANGSTRING_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// Whether a class/datatype string is an XSD primitive (`xsd:…` CURIE form).
///
/// This matches `terminusdb_schema::is_primitive` exactly — the value-vs-link
/// decision in the schema layer keys on the `xsd:` prefix. It deliberately does
/// NOT accept the full XSD IRI (schema-layer classes are always CURIEs); use
/// [`xsd_local_name`] when the input may be either form.
pub fn is_primitive(class: &str) -> bool {
    class.starts_with(XSD_PREFIX)
}

/// Whether a class/datatype string is in the `sys:` vocabulary.
pub fn is_sys(class: &str) -> bool {
    class.starts_with(SYS_PREFIX)
}

/// Extract the XSD local name from either the CURIE (`xsd:integer`) or full-IRI
/// (`http://…/XMLSchema#integer`) form. Returns `None` if it is not an XSD type.
///
/// ```
/// use terminusdb_format::prefix::xsd_local_name;
/// assert_eq!(xsd_local_name("xsd:integer"), Some("integer"));
/// assert_eq!(xsd_local_name("http://www.w3.org/2001/XMLSchema#dateTime"), Some("dateTime"));
/// assert_eq!(xsd_local_name("sys:Unit"), None);
/// assert_eq!(xsd_local_name("Person"), None);
/// ```
pub fn xsd_local_name(ty: &str) -> Option<&str> {
    ty.strip_prefix(XSD_PREFIX)
        .or_else(|| ty.strip_prefix(XSD_IRI))
}

/// Qualify a bare class name into the `@schema:` namespace, leaving strings that
/// already carry a prefix (`xsd:…`, `@schema:…`, a full IRI) untouched.
///
/// ```
/// use terminusdb_format::prefix::schema_curie;
/// assert_eq!(schema_curie("Person"), "@schema:Person");
/// assert_eq!(schema_curie("xsd:string"), "xsd:string");
/// ```
pub fn schema_curie(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("{SCHEMA_PREFIX}{name}")
    }
}

/// Contract a full IRI into its CURIE form against the known namespaces.
///
/// Members of `schema_base` contract to `@schema:`; `rdf:`/`rdfs:`/`xsd:`
/// contract to their prefixes; anything else (e.g. instance IRIs like
/// `terminusdb:///data/…`) passes through unchanged.
///
/// ```
/// use terminusdb_format::prefix::{contract_iri, RDF_TYPE_IRI};
/// let base = "http://terminusdb.com/schema#";
/// assert_eq!(contract_iri("http://terminusdb.com/schema#Person", base), "@schema:Person");
/// assert_eq!(contract_iri(RDF_TYPE_IRI, base), "rdf:type");
/// assert_eq!(contract_iri("terminusdb:///data/Person/x", base), "terminusdb:///data/Person/x");
/// ```
pub fn contract_iri(iri: &str, schema_base: &str) -> String {
    if let Some(local) = iri.strip_prefix(schema_base) {
        return format!("{SCHEMA_PREFIX}{local}");
    }
    if let Some(local) = iri.strip_prefix(RDF_IRI) {
        return format!("{RDF_PREFIX}{local}");
    }
    if let Some(local) = iri.strip_prefix(RDFS_IRI) {
        return format!("{RDFS_PREFIX}{local}");
    }
    if let Some(local) = iri.strip_prefix(XSD_IRI) {
        return format!("{XSD_PREFIX}{local}");
    }
    iri.to_string()
}

/// Known XML namespace prefixes that must be preserved (not stripped) by
/// [`strip_schema_prefix`].
const XML_NS_PREFIXES: &[&str] = &[
    "xlink", "xml", "xmlns", "xsl", "xi", "mml", "oasis", "ali", "tbx", "mtl", "c",
];

/// Extract the local name from a TDB type or property string: strips namespace
/// URIs (`https://ns#Name` → `Name`) and TDB schema prefixes (`gd:Name` → `Name`),
/// while preserving known XML namespace prefixes (`xlink:href` stays as-is).
///
/// ```
/// use terminusdb_format::prefix::strip_schema_prefix;
/// assert_eq!(strip_schema_prefix("https://pubb.in/schemas/gdocs/1.0#DocumentType"), "DocumentType");
/// assert_eq!(strip_schema_prefix("gd:SectionType"), "SectionType");
/// assert_eq!(strip_schema_prefix("xlink:href"), "xlink:href");
/// assert_eq!(strip_schema_prefix("SecChild"), "SecChild");
/// ```
pub fn strip_schema_prefix(s: &str) -> &str {
    // Full URI: https://ns#Name → Name
    if let Some(pos) = s.rfind('#') {
        return &s[pos + 1..];
    }
    // Short prefix: gd:Name — but NOT xml namespace prefixes
    if let Some(pos) = s.find(':') {
        let prefix = &s[..pos];
        if XML_NS_PREFIXES.contains(&prefix) {
            return s;
        }
        // Keep URL schemes (contain /)
        if prefix.contains('/') {
            return s;
        }
        return &s[pos + 1..];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_and_sys() {
        assert!(is_primitive("xsd:string"));
        assert!(!is_primitive("http://www.w3.org/2001/XMLSchema#string"));
        assert!(!is_primitive("Person"));
        assert!(is_sys("sys:Unit"));
        assert!(!is_sys("xsd:string"));
    }

    #[test]
    fn contract_passthrough_for_instance_iri() {
        let base = SCHEMA_IRI_DEFAULT;
        assert_eq!(contract_iri("terminusdb:///data/Person/x", base), "terminusdb:///data/Person/x");
    }

    #[test]
    fn strip_keeps_jsonld_keywords() {
        assert_eq!(strip_schema_prefix("@type"), "@type");
    }
}

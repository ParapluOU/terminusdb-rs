//! `sys:*` system-vocabulary CURIE constants.
//!
//! TerminusDB's `sys:` namespace holds framework types that are not `xsd:`
//! datatypes but are also not user classes/links: the unit value and the opaque
//! JSON containers. `terminusdb-schema` re-exports these.

/// `sys:Unit` — the unit / empty value (codified as `[]` in JSON).
pub const UNIT: &str = "sys:Unit";
/// `sys:JSON` — an opaque embedded JSON value.
pub const JSON: &str = "sys:JSON";
/// `sys:JSONDocument` — an opaque embedded JSON document.
pub const JSON_DOCUMENT: &str = "sys:JSONDocument";

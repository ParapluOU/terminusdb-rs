//! # terminusdb-format
//!
//! Primitives for TerminusDB's JSON-LD-flavored wire format: the `@`-keywords,
//! the `xsd:`/`sys:`/`rdf:` vocabulary, IRI ⇄ CURIE conversion, literal-vs-node
//! value classification, and schema-document parsing.
//!
//! This crate exists to give the query compilers (SQL, SPARQL, …) and,
//! eventually, `terminusdb-schema` itself a single shared implementation of the
//! format concerns they were each hand-rolling. Its two design rules:
//!
//! 1. **Pure leaf.** It depends only on `serde`/`serde_json`/`thiserror`. It must
//!    never depend on `terminusdb-schema` (or anything that does) — that is what
//!    lets `terminusdb-schema` depend on *this* crate later without a cycle.
//! 2. **Format, not semantics.** It classifies and (de)serializes the wire shape.
//!    It does not model TerminusDB's runtime semantics (key strategies, the
//!    transaction capture protocol, subdocument ownership, …); those stay in the
//!    schema/client layers.
//!
//! TerminusDB's format is a *dialect* of JSON-LD, not W3C JSON-LD — see the
//! module docs for which keywords are standard vs TerminusDB-specific.

pub mod datatype;
pub mod error;
pub mod keyword;
pub mod prefix;
pub mod schema_doc;
pub mod sys;
pub mod value;
pub mod xsd;

pub use datatype::{classify_datatype, classify_xsd_local, XsdCategory};
pub use error::{FormatError, Result};
pub use schema_doc::{parse_schema, Family, RawClass, RawEnum, RawProperty, RawSchema};
pub use value::{classify_value, WireValue};

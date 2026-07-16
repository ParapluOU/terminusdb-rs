//! JSON-LD / TerminusDB `@`-keyword string constants.
//!
//! These are the reserved keys that appear in TerminusDB's JSON-LD-flavored
//! document and schema format. Centralizing them removes the hand-typed string
//! literals that were scattered across the SQL/SPARQL compilers (and the schema
//! crate) and documents the whole keyword surface in one place.
//!
//! Note: this is TerminusDB's *dialect* of JSON-LD, not W3C JSON-LD. Some keys
//! (`@id`, `@type`, `@value`, `@context`, `@base`, `@language`) are standard;
//! most (`@schema`, `@key`, `@subdocument`, `@class`, `@oneOf`, …) are
//! TerminusDB-specific.

/// `@id` — the identifier of a document/class.
pub const ID: &str = "@id";
/// `@type` — a class kind (`Class`/`Enum`/`TaggedUnion`), an instance class
/// name, a datatype (in a typed literal), or a type family. Same key, many roles.
pub const TYPE: &str = "@type";
/// `@value` — the payload of a typed literal `{"@type": …, "@value": …}`, or
/// (in an `Enum` schema) the array of member strings.
pub const VALUE: &str = "@value";
/// `@class` — the range class of a property / type-family wrapper.
pub const CLASS: &str = "@class";
/// `@context` — the namespace-prefix context object.
pub const CONTEXT: &str = "@context";
/// `@schema` — the schema namespace base (context key).
pub const SCHEMA: &str = "@schema";
/// `@base` — the instance-data namespace base (context key).
pub const BASE: &str = "@base";
/// `@key` — a class key strategy.
pub const KEY: &str = "@key";
/// `@fields` — key fields for Lexical/Hash key strategies.
pub const FIELDS: &str = "@fields";
/// `@inherits` — superclass or list of superclasses.
pub const INHERITS: &str = "@inherits";
/// `@abstract` — marks an abstract class (present with value `[]`).
pub const ABSTRACT: &str = "@abstract";
/// `@subdocument` — marks a subdocument class (present with value `[]`).
pub const SUBDOCUMENT: &str = "@subdocument";
/// `@unfoldable` — marks a class whose links unfold like subdocuments.
pub const UNFOLDABLE: &str = "@unfoldable";
/// `@oneOf` — disjoint (one-of) property groups on a class/tagged-union.
pub const ONE_OF: &str = "@oneOf";
/// `@documentation` — class/property/context documentation annotation.
pub const DOCUMENTATION: &str = "@documentation";
/// `@language` — language tag of a language-tagged string.
pub const LANGUAGE: &str = "@language";
/// `@dimensions` — dimensionality of an `Array` type family.
pub const DIMENSIONS: &str = "@dimensions";
/// `@cardinality` — exact cardinality of a `Set` type family.
pub const CARDINALITY: &str = "@cardinality";
/// `@min_cardinality` — minimum cardinality of a `Set` type family.
pub const MIN_CARDINALITY: &str = "@min_cardinality";
/// `@max_cardinality` — maximum cardinality of a `Set` type family.
pub const MAX_CARDINALITY: &str = "@max_cardinality";
/// `@ref` — a transaction-local capture reference `{"@ref": id}`.
pub const REF: &str = "@ref";
/// `@capture` — an insert-time id-capture declaration.
pub const CAPTURE: &str = "@capture";

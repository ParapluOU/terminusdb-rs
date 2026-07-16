//! The value-vs-link distinction for a property's range.
//!
//! This is the RDF/JSON-LD distinction the whole `terminusdb-format` extraction
//! is built around: a property is either a **datatype property** (its value is a
//! stored literal — an `xsd:*` datatype, or a `sys:*` framework value like
//! `sys:JSON`/`sys:Unit`) or an **object property / link** (its value is a
//! reference to another document — a graph edge).
//!
//! Historically this lived implicitly as `!class.starts_with("xsd:")` scattered
//! across the schema and the compilers; [`PropertyKind`] names it once.

use crate::prefix::{is_primitive, is_sys};

/// Whether a property's range is a stored value or a graph link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyKind {
    /// A datatype property: the value is a literal (`xsd:*`) or a `sys:*`
    /// framework value. Not a graph edge.
    Datatype,
    /// An object property: the value references another document (a graph edge).
    Link,
}

impl PropertyKind {
    /// Classify a property range (the `@class` string) as a datatype or a link.
    ///
    /// `xsd:*` and `sys:*` ranges are datatypes; everything else (a class id) is
    /// a link.
    ///
    /// ```
    /// use terminusdb_format::PropertyKind;
    /// assert_eq!(PropertyKind::of("xsd:string"), PropertyKind::Datatype);
    /// assert_eq!(PropertyKind::of("sys:JSON"), PropertyKind::Datatype);
    /// assert_eq!(PropertyKind::of("Person"), PropertyKind::Link);
    /// ```
    pub fn of(class: &str) -> Self {
        if is_primitive(class) || is_sys(class) {
            PropertyKind::Datatype
        } else {
            PropertyKind::Link
        }
    }

    /// True for a link (object property / graph edge).
    pub fn is_link(self) -> bool {
        matches!(self, PropertyKind::Link)
    }

    /// True for a datatype property (stored literal value).
    pub fn is_datatype(self) -> bool {
        matches!(self, PropertyKind::Datatype)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_datatype_vs_link() {
        assert!(PropertyKind::of("xsd:integer").is_datatype());
        assert!(PropertyKind::of("sys:Unit").is_datatype());
        assert!(PropertyKind::of("sys:JSON").is_datatype());
        assert!(PropertyKind::of("Person").is_link());
        assert!(PropertyKind::of("SomeClass").is_link());
    }
}

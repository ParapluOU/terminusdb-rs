//! Cache conditions: telling a caching proxy when a query's answer dies.
//!
//! # Why this exists
//!
//! A WOQL query can return data **without revealing which entities the data was
//! derived from**. Consider:
//!
//! ```text
//! select ?n where { alice friend ?f . ?f name ?n }
//! ```
//!
//! The answer is a list of names. Nothing in it identifies `?f`, so when one of
//! those friends is renamed the answer changes with no trace in the bytes that
//! were returned. The query *text* is no better: the subject is a variable, so
//! an analyzer reading the AST can only conclude "this might depend on
//! anything".
//!
//! A proxy therefore cannot work out when a WOQL result stops being true, and
//! must not cache one unless it is told. **The caller is the only party that
//! knows** — you wrote the query, so you know it is really "the names of
//! alice's friends" and dies when a `Person` changes.
//!
//! That is what a [`CacheCondition`] states. Pass them to
//! [`TerminusDBHttpClient::query_with_conditions`][crate::TerminusDBHttpClient::query_with_conditions]
//! and they travel as the `X-Reactor-Invalidate-On` header. With no conditions,
//! a proxy forwards the query uncached — which is the safe default, not a
//! failure.
//!
//! Document-API reads (`/api/document`) need none of this: their read set *is*
//! derivable from the request, so a proxy caches them on its own.
//!
//! ```
//! use terminusdb_client::CacheCondition;
//!
//! // "This answer dies when any Person changes."
//! let conditions = vec![CacheCondition::class("Person")];
//! assert_eq!(CacheCondition::header_value(&conditions).unwrap(), "class:Person");
//! ```
//!
//! # Being wrong is worse than saying nothing
//!
//! A condition is a promise. If you state `class:Person` for a query that also
//! reads `Company`, the proxy will keep serving a stale answer after a `Company`
//! changes — and nothing will look broken. State every class the query touches,
//! or state nothing and accept an uncached query.

use std::fmt;

/// A statement of when a query's cached answer stops being valid.
///
/// Rendered as text for the `X-Reactor-Invalidate-On` header. This type is a
/// plain renderer — it deliberately does not depend on the proxy crate — so the
/// encoding below is the contract between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheCondition {
    /// Any change to one document: `doc:Person/alice`.
    Doc(String),
    /// One property of one document: `docfield:Person/alice.name`.
    DocField(String, String),
    /// The outgoing edge set of one document's link property:
    /// `edge:Person/alice.friend`.
    Edge(String, String),
    /// A class gaining or losing an instance: `members:Person`.
    ///
    /// The right condition for a count or an id list, and notably *not* fired
    /// by an update to an existing instance.
    Members(String),
    /// Anything about any instance of a class: `class:Person`.
    ///
    /// The usual answer, and the safe one when unsure.
    Class(String),
    /// One property on any instance of a class: `classfield:Person.name`.
    ClassField(String, String),
    /// The set of a class's documents whose property equals a value:
    /// `index:Person.name="Ann"`.
    Index {
        /// The class.
        class: String,
        /// The property.
        prop: String,
        /// The value. Rendered as canonical JSON, so a string and a number are
        /// distinguishable — which matters, because the proxy compares it
        /// against the value in the document.
        value: serde_json::Value,
    },
    /// The set of documents pointing at a target:
    /// `backlink:Person/alice.friend<-Person`.
    Backlink {
        /// The document being pointed at.
        target: String,
        /// The link property on the pointing documents.
        prop: String,
        /// The class of the pointing documents.
        from_class: String,
    },
    /// Any change to any of these classes: `path:Person,Company`.
    ///
    /// For a traversal, where naming the hops is the honest description.
    Path {
        /// The starting document, if the traversal has a fixed origin.
        from: Option<String>,
        /// The classes the traversal can reach. Empty means "any", which is
        /// [`CacheCondition::Everything`] by another name.
        classes: Vec<String>,
    },
    /// A schema change: `schema`.
    Schema,
    /// Any commit at all: `everything`.
    ///
    /// Honest, and almost never worth caching against — the entry dies on the
    /// next write to the database. Prefer naming the classes you touch.
    Everything,
}

impl CacheCondition {
    /// Any change to one document.
    pub fn doc(id: impl Into<String>) -> CacheCondition {
        CacheCondition::Doc(id.into())
    }

    /// One property of one document.
    pub fn doc_field(id: impl Into<String>, prop: impl Into<String>) -> CacheCondition {
        CacheCondition::DocField(id.into(), prop.into())
    }

    /// The outgoing edge set of one document's link property.
    pub fn edge(id: impl Into<String>, prop: impl Into<String>) -> CacheCondition {
        CacheCondition::Edge(id.into(), prop.into())
    }

    /// A class gaining or losing an instance.
    pub fn members(class: impl Into<String>) -> CacheCondition {
        CacheCondition::Members(class.into())
    }

    /// Anything about any instance of a class.
    pub fn class(class: impl Into<String>) -> CacheCondition {
        CacheCondition::Class(class.into())
    }

    /// One property on any instance of a class.
    pub fn class_field(class: impl Into<String>, prop: impl Into<String>) -> CacheCondition {
        CacheCondition::ClassField(class.into(), prop.into())
    }

    /// The set of a class's documents whose property equals a value.
    pub fn index(
        class: impl Into<String>,
        prop: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> CacheCondition {
        CacheCondition::Index {
            class: class.into(),
            prop: prop.into(),
            value: value.into(),
        }
    }

    /// The set of documents pointing at a target.
    pub fn backlink(
        target: impl Into<String>,
        prop: impl Into<String>,
        from_class: impl Into<String>,
    ) -> CacheCondition {
        CacheCondition::Backlink {
            target: target.into(),
            prop: prop.into(),
            from_class: from_class.into(),
        }
    }

    /// Any change to any of these classes.
    pub fn path<I, S>(classes: I) -> CacheCondition
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        CacheCondition::Path {
            from: None,
            classes: classes.into_iter().map(Into::into).collect(),
        }
    }

    /// A traversal from a fixed origin.
    pub fn path_from<I, S>(from: impl Into<String>, classes: I) -> CacheCondition
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        CacheCondition::Path {
            from: Some(from.into()),
            classes: classes.into_iter().map(Into::into).collect(),
        }
    }

    /// A schema change.
    pub fn schema() -> CacheCondition {
        CacheCondition::Schema
    }

    /// Any commit at all.
    pub fn everything() -> CacheCondition {
        CacheCondition::Everything
    }

    /// Render a list as an `X-Reactor-Invalidate-On` value.
    ///
    /// `None` for an empty list, so a caller can pass "no conditions" and have
    /// no header sent at all — which is what makes the query uncached rather
    /// than cached against nothing.
    pub fn header_value(conditions: &[CacheCondition]) -> Option<String> {
        if conditions.is_empty() {
            return None;
        }
        Some(
            conditions
                .iter()
                .map(CacheCondition::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

impl fmt::Display for CacheCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheCondition::Doc(id) => write!(f, "doc:{id}"),
            CacheCondition::DocField(id, prop) => write!(f, "docfield:{id}.{prop}"),
            CacheCondition::Edge(id, prop) => write!(f, "edge:{id}.{prop}"),
            CacheCondition::Members(class) => write!(f, "members:{class}"),
            CacheCondition::Class(class) => write!(f, "class:{class}"),
            CacheCondition::ClassField(class, prop) => write!(f, "classfield:{class}.{prop}"),
            CacheCondition::Index { class, prop, value } => {
                // Canonical JSON: the proxy compares this against the value it
                // finds in the document, so `"30"` and `30` must not collapse.
                let rendered = serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned());
                write!(f, "index:{class}.{prop}={rendered}")
            }
            CacheCondition::Backlink {
                target,
                prop,
                from_class,
            } => write!(f, "backlink:{target}.{prop}<-{from_class}"),
            CacheCondition::Path { from, classes } => {
                let classes = if classes.is_empty() {
                    "*".to_owned()
                } else {
                    classes.join(",")
                };
                match from {
                    Some(from) => write!(f, "path:{from}>{classes}"),
                    None => write!(f, "path:{classes}"),
                }
            }
            CacheCondition::Schema => f.write_str("schema"),
            CacheCondition::Everything => f.write_str("everything"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_constructors_render_the_documented_encoding() {
        assert_eq!(
            CacheCondition::doc("Person/alice").to_string(),
            "doc:Person/alice"
        );
        assert_eq!(
            CacheCondition::doc_field("Person/alice", "name").to_string(),
            "docfield:Person/alice.name"
        );
        assert_eq!(
            CacheCondition::edge("Person/alice", "friend").to_string(),
            "edge:Person/alice.friend"
        );
        assert_eq!(
            CacheCondition::members("Person").to_string(),
            "members:Person"
        );
        assert_eq!(CacheCondition::class("Person").to_string(), "class:Person");
        assert_eq!(
            CacheCondition::class_field("Person", "name").to_string(),
            "classfield:Person.name"
        );
        assert_eq!(
            CacheCondition::backlink("Person/alice", "friend", "Person").to_string(),
            "backlink:Person/alice.friend<-Person"
        );
        assert_eq!(
            CacheCondition::path(["Person", "Company"]).to_string(),
            "path:Person,Company"
        );
        assert_eq!(
            CacheCondition::path_from("Person/alice", ["Person"]).to_string(),
            "path:Person/alice>Person"
        );
        assert_eq!(CacheCondition::schema().to_string(), "schema");
        assert_eq!(CacheCondition::everything().to_string(), "everything");
    }

    #[test]
    fn index_values_keep_their_json_type() {
        // A string and a number are different buckets in the store, so they
        // must stay different here.
        assert_eq!(
            CacheCondition::index("Person", "name", json!("Ann")).to_string(),
            "index:Person.name=\"Ann\""
        );
        assert_eq!(
            CacheCondition::index("Person", "age", json!(30)).to_string(),
            "index:Person.age=30"
        );
        assert_eq!(
            CacheCondition::index("Person", "active", json!(true)).to_string(),
            "index:Person.active=true"
        );
        assert_ne!(
            CacheCondition::index("Person", "age", json!(30)).to_string(),
            CacheCondition::index("Person", "age", json!("30")).to_string()
        );
    }

    #[test]
    fn a_path_with_no_classes_renders_the_barrier_spelling() {
        assert_eq!(
            CacheCondition::path(Vec::<String>::new()).to_string(),
            "path:*"
        );
    }

    #[test]
    fn an_empty_list_sends_no_header() {
        // This is what makes "no conditions" mean "do not cache", rather than
        // "cache against nothing".
        assert_eq!(CacheCondition::header_value(&[]), None);
    }

    #[test]
    fn a_list_renders_comma_separated() {
        let value = CacheCondition::header_value(&[
            CacheCondition::class("Person"),
            CacheCondition::members("Company"),
        ])
        .unwrap();
        assert_eq!(value, "class:Person,members:Company");
    }

    #[test]
    fn a_header_value_is_valid_as_a_header() {
        // No control characters or non-ASCII, whatever the inputs look like.
        let value = CacheCondition::header_value(&[
            CacheCondition::index("Person", "name", json!("Ann")),
            CacheCondition::path(["Person", "Company"]),
        ])
        .unwrap();
        assert!(http::HeaderValue::from_str(&value).is_ok(), "{value}");
    }
}

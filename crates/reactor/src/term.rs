//! Dependency terms: the invalidation algebra.
//!
//! A cached fragment records the *terms* it was derived from. A commit *fires* a
//! set of terms. A fragment is dirty when any term it holds **subsumes** a fired
//! term. Three things about that sentence are load-bearing:
//!
//! * **Subsumption, not equality.** A fragment that depends on `Doc(alice)` must
//!   be dirtied by a fired `DocField(alice, name)`, because "alice's name
//!   changed" implies "alice changed". The relation is directional and it is not
//!   symmetric: a fragment depending on `DocField(alice, name)` is *not* dirtied
//!   by a fired `DocField(alice, age)`. Getting a row of that table backwards
//!   either over-invalidates (a performance bug) or serves stale data forever (a
//!   correctness bug), which is why every row has its own test.
//! * **Terms are data.** They are serializable, hashable and orderable, so they
//!   can be stored with a fragment, logged, shown in `/_reactor/explain`, and
//!   used as keys in an inverted index.
//! * **Coarseness is always safe.** [`Term::Everything`] subsumes every fired
//!   term, so any analyzer that cannot prove a tight dependency can emit it and
//!   be sound. The variant order below is precise → coarse, and the derived
//!   `Ord` follows it, so sorting a dependency list puts the cheap, specific
//!   terms first.

use serde::{Deserialize, Serialize};

use crate::ids::{ClassName, DocId, PropName};
use crate::schema_index::SchemaIndex;

/// Normalized scalar for value-indexed terms: canonical JSON text, so `1`,
/// `1.0` and `"1"` stay distinguishable.
///
/// Canonical JSON rather than a parsed number because v12 returns numbers at
/// full rational precision and the workspace enables
/// `serde_json/arbitrary_precision`: going through `f64` to compare a filter
/// value against a document value would make `1e308` and `1e309` equal and would
/// make `10.0` and `10` collide. Object keys are sorted so that a term built
/// from a GraphQL filter matches a term built from a document body.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValueKey(String);

impl ValueKey {
    /// Normalize a JSON value into a value key.
    pub fn of(v: &serde_json::Value) -> Self {
        let mut out = String::new();
        write_canonical(v, 0, &mut out);
        ValueKey(out)
    }

    /// The canonical text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ValueKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Maximum nesting depth we canonicalize.
///
/// A `sys:JSON` property holds arbitrary client-supplied JSON and this walk is
/// recursive, so it needs a bound — a request must not be able to overflow the
/// stack. Values deeper than this collapse to a marker, which can only make two
/// distinct values compare *equal*, i.e. cause over-firing. Never the reverse.
const MAX_CANONICAL_DEPTH: usize = 64;

/// Write `v` as canonical JSON: object keys sorted, numbers kept as their
/// original lexical form, no insignificant whitespace.
fn write_canonical(v: &serde_json::Value, depth: usize, out: &mut String) {
    use serde_json::Value;
    if depth > MAX_CANONICAL_DEPTH {
        out.push_str("\"…\"");
        return;
    }
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        // `Number`'s Display is the preserved lexical form under
        // `arbitrary_precision`, which is precisely what we want.
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => match serde_json::to_string(s) {
            Ok(quoted) => out.push_str(&quoted),
            // Cannot happen for a String, but this is on the request path.
            Err(_) => {
                out.push('"');
                out.push_str(s);
                out.push('"');
            }
        },
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, depth + 1, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(&serde_json::Value::String(key.clone()), depth + 1, out);
                out.push(':');
                if let Some(value) = map.get(key) {
                    write_canonical(value, depth + 1, out);
                }
            }
            out.push('}');
        }
    }
}

/// A dependency: something a cached value was derived from.
///
/// Ordered precise → coarse. Design doc §5.2.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Term {
    /// One property of one document.
    DocField(DocId, PropName),
    /// One document, in any respect.
    Doc(DocId),
    /// The outgoing edge set of one document's link property. Distinct from
    /// `DocField` because a traversal depends on *where the edge points*, which
    /// is a different thing from the stored value of a datatype property.
    Edge(DocId, PropName),
    /// The set of `from_class` documents pointing at `target` via `prop`, i.e. a
    /// reverse edge. The only term that can express "who links to me", which is
    /// what makes a GraphQL backlink field cacheable.
    Backlink {
        /// The document being pointed at.
        target: DocId,
        /// The link property on the pointing documents.
        prop: PropName,
        /// The class of the pointing documents.
        from_class: ClassName,
    },
    /// The set of `class` documents whose `prop` equals `value`: a value index.
    Index {
        /// The indexed class.
        class: ClassName,
        /// The indexed property.
        prop: PropName,
        /// The indexed value, canonicalized.
        value: ValueKey,
    },
    /// `prop` on any instance of `class`.
    ClassField(ClassName, PropName),
    /// The *membership* of a class: an instance was created or deleted. Does not
    /// fire on an update, which is why a `_count` query is so much cheaper to
    /// keep fresh than a list of bodies.
    Members(ClassName),
    /// Anything about any instance of `class`.
    Class(ClassName),
    /// A traversal. Conservative by construction: any change to any class on the
    /// path dirties it, because proving which hop of a path actually moved would
    /// require evaluating the path.
    Path {
        /// The starting document, when one was bound.
        from: Option<DocId>,
        /// The classes the path can traverse. An **empty** list means "we could
        /// not derive any class", which makes the term a barrier equivalent to
        /// [`Term::Everything`] — never a term that matches nothing.
        classes: Vec<ClassName>,
    },
    /// The schema graph.
    Schema,
    /// Namespace-wide barrier: any commit fires it. Used for non-fast-forward
    /// movement, incomplete changesets, and analyzers that give up but still
    /// want an entry.
    Everything,
}

impl Term {
    /// The document this term is *about*, if any. For a `Backlink` that is the
    /// target, not the pointing document, because the target's identity is what
    /// the term is filed and matched under.
    pub fn doc_id(&self) -> Option<&DocId> {
        match self {
            Term::DocField(id, _) | Term::Doc(id) | Term::Edge(id, _) => Some(id),
            Term::Backlink { target, .. } => Some(target),
            Term::Path { from, .. } => from.as_ref(),
            _ => None,
        }
    }

    /// The class this term is *about*, if one is derivable.
    ///
    /// For document-scoped terms this is the class prefix of the id, which is
    /// why [`DocId::class`] has to be reliable. For a `Backlink` it is the
    /// pointing class.
    pub fn class(&self) -> Option<ClassName> {
        match self {
            Term::DocField(id, _) | Term::Doc(id) | Term::Edge(id, _) => id.class(),
            Term::Backlink { from_class, .. } => Some(from_class.clone()),
            Term::Index { class, .. }
            | Term::ClassField(class, _)
            | Term::Members(class)
            | Term::Class(class) => Some(class.clone()),
            Term::Path { .. } | Term::Schema | Term::Everything => None,
        }
    }

    /// Index buckets this term must be filed under.
    ///
    /// Two terms whose bucket sets are disjoint can never match, so firing is a
    /// hash lookup, not a scan. A `Path` term yields one `Class` bucket per
    /// listed class; everything else yields exactly one bucket.
    pub fn buckets(&self) -> Vec<TermBucket> {
        match self {
            Term::DocField(id, _) | Term::Doc(id) | Term::Edge(id, _) => {
                vec![TermBucket::Doc(id.clone())]
            }
            Term::Backlink { target, .. } => vec![TermBucket::Doc(target.clone())],
            Term::Index { class, .. }
            | Term::ClassField(class, _)
            | Term::Members(class)
            | Term::Class(class) => vec![TermBucket::Class(class.clone())],
            Term::Path { classes, .. } => {
                if classes.is_empty() {
                    // A path with no derivable class subsumes everything, so it
                    // has to be reachable from every fired term. Filing it
                    // anywhere narrower would make it match nothing, which is
                    // the one outcome that is actually unsafe.
                    vec![TermBucket::Everything]
                } else {
                    classes
                        .iter()
                        .map(|c| TermBucket::Class(c.clone()))
                        .collect()
                }
            }
            Term::Schema => vec![TermBucket::Schema],
            Term::Everything => vec![TermBucket::Everything],
        }
    }

    /// Buckets a *fired* term should be looked up under, i.e. the buckets of
    /// every dependency that could possibly subsume it: its own `Doc(id)` plus
    /// `Class(c)` for each ancestor of its class, plus `Everything`.
    ///
    /// Ancestors rather than descendants: a dependency is filed under *its own*
    /// class, and a coarse dependency on a superclass must be found by a change
    /// to a subclass instance. The two sides meet in the middle.
    ///
    /// Note that a fired [`Term::Everything`] cannot be expressed this way —
    /// every dependency subsumes it, and enumerating every bucket is not
    /// possible. [`crate::engine::TermIndex`] special-cases it by returning all
    /// entries.
    pub fn lookup_buckets(&self, schema: &SchemaIndex) -> Vec<TermBucket> {
        let mut out = Vec::with_capacity(4);
        if let Some(id) = self.doc_id() {
            out.push(TermBucket::Doc(id.clone()));
        }
        if let Some(class) = self.class() {
            for ancestor in schema.ancestors(&class) {
                out.push(TermBucket::Class(ancestor));
            }
        }
        if matches!(self, Term::Schema) {
            out.push(TermBucket::Schema);
        }
        out.push(TermBucket::Everything);
        out
    }

    /// True if `self` (a fragment's dependency) is dirtied by `fired`.
    ///
    /// Matching is *subsumption*, not equality: `self` must be implied by
    /// `fired`. The implementation is a direct transcription of the truth table
    /// in `SPEC.md`, plus the coarse-fire cases that the table's `Index` row
    /// establishes the principle for: a fired term that names a document or a
    /// property *without* naming the value is coarser than a value-indexed
    /// dependency, and so must dirty it.
    pub fn subsumes(&self, fired: &Term, schema: &SchemaIndex) -> bool {
        // Any commit fires through a barrier.
        if matches!(fired, Term::Everything) {
            return true;
        }
        match self {
            Term::Everything => true,

            // Only a schema change dirties a schema dependency. Note that a
            // response *shape* can depend on the schema (GraphQL field
            // resolution does); analyzers that build such a fragment must add
            // `Schema` to its dependencies explicitly.
            Term::Schema => matches!(fired, Term::Schema),

            Term::Doc(a) => match fired {
                Term::Doc(b) | Term::DocField(b, _) | Term::Edge(b, _) => a == b,
                _ => false,
            },

            Term::DocField(a, p) => match fired {
                // Same document, same property.
                Term::DocField(b, q) => a == b && p == q,
                // Whole-document fire: we do not know which property moved.
                Term::Doc(b) => a == b,
                _ => false,
            },

            Term::Edge(a, p) => match fired {
                Term::DocField(b, q) | Term::Edge(b, q) => a == b && p == q,
                Term::Doc(b) => a == b,
                _ => false,
            },

            Term::Class(c) => match fired {
                Term::Doc(id) | Term::DocField(id, _) | Term::Edge(id, _) => {
                    class_of_doc_is_under(id, c, schema)
                }
                Term::Members(d) | Term::Class(d) => schema.is_subclass_of(d, c),
                Term::Index { class, .. } | Term::ClassField(class, _) => {
                    schema.is_subclass_of(class, c)
                }
                // A fired Backlink is accompanied by a Doc/Edge fire on the
                // pointing document (see `fire`), so the class dependency is
                // already covered by those; matching here too would be
                // redundant over-invalidation.
                _ => false,
            },

            Term::Members(c) => match fired {
                // Only membership fires membership. An update to an existing
                // instance must NOT dirty a count or an id list.
                Term::Members(d) => schema.is_subclass_of(d, c),
                _ => false,
            },

            Term::ClassField(c, p) => match fired {
                Term::DocField(id, q) | Term::Edge(id, q) => {
                    p == q && class_of_doc_is_under(id, c, schema)
                }
                Term::ClassField(d, q) => p == q && schema.is_subclass_of(d, c),
                // Coarse whole-document fire: the property may have moved.
                Term::Doc(id) => class_of_doc_is_under(id, c, schema),
                _ => false,
            },

            Term::Index { class, prop, value } => match fired {
                Term::Index {
                    class: d,
                    prop: q,
                    value: w,
                } => prop == q && value == w && schema.is_subclass_of(d, class),
                // Everything below names the property or the document but not
                // the value, so we cannot tell whether this bucket changed.
                Term::DocField(id, q) | Term::Edge(id, q) => {
                    prop == q && class_of_doc_is_under(id, class, schema)
                }
                Term::ClassField(d, q) => prop == q && schema.is_subclass_of(d, class),
                Term::Doc(id) => class_of_doc_is_under(id, class, schema),
                // A create or delete changes which documents are in the bucket.
                Term::Members(d) => schema.is_subclass_of(d, class),
                _ => false,
            },

            Term::Backlink {
                target,
                prop,
                from_class,
            } => match fired {
                Term::Backlink {
                    target: t,
                    prop: q,
                    from_class: f,
                } => target == t && prop == q && schema.is_subclass_of(f, from_class),
                // Deliberately nothing else. A Backlink is the most expensive
                // term to keep precise and the whole point of paying for
                // before-state; widening it here would make every reverse-edge
                // fragment as coarse as `ClassField`. The soundness obligation
                // is discharged upstream instead: a ChangeSet that cannot see
                // the before-state is `complete: false` and fires `Everything`.
                _ => false,
            },

            Term::Path { classes, .. } => {
                if classes.is_empty() {
                    // No derivable class: behave as a barrier.
                    return true;
                }
                match fired.class() {
                    Some(fired_class) => classes
                        .iter()
                        .any(|listed| schema.is_subclass_of(&fired_class, listed)),
                    // `from` is deliberately ignored: proving a path does not
                    // reach a change would require evaluating the path.
                    None => false,
                }
            }
        }
    }
}

/// True when `id`'s class is `class` or a subclass of it. An id with no class
/// prefix matches nothing, which is correct: we cannot attribute it.
fn class_of_doc_is_under(id: &DocId, class: &ClassName, schema: &SchemaIndex) -> bool {
    match id.class() {
        Some(doc_class) => schema.is_subclass_of(&doc_class, class),
        None => false,
    }
}

/// The coarse partition terms are filed under in the inverted index.
///
/// Buckets exist purely to make firing a hash lookup instead of a scan over
/// every cached fragment. They are intentionally cheap and lossy: a bucket match
/// only says "these two terms *might* interact", and [`Term::subsumes`] decides.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TermBucket {
    /// Terms about one document.
    Doc(DocId),
    /// Terms about one class.
    Class(ClassName),
    /// Terms about the schema graph.
    Schema,
    /// Barrier terms, which every fire must consider.
    Everything,
}

/// A set of terms, deduplicated and in precise → coarse order.
///
/// `BTreeSet` rather than `HashSet` so that a fired set is deterministic: it is
/// logged, compared in tests, and iterated while matching, and a stable order
/// makes all three reproducible.
pub type TermSet = std::collections::BTreeSet<Term>;
// ------------------------------------------------------------ text encoding

/// The wire encoding of a [`Term`], for the `X-Reactor-Invalidate-On` header.
///
/// A client states a cache condition as text, so the encoding is part of the
/// public interface and has to be stable and exactly round-trippable:
///
/// ```text
/// doc:Person/alice
/// docfield:Person/alice.name
/// edge:Person/alice.friend
/// members:Person
/// class:Person
/// classfield:Person.name
/// index:Person.name="Ann"
/// backlink:Person/alice.friend<-Person
/// path:Person,Company
/// path:Person/alice>Person,Company
/// path:*
/// schema
/// everything
/// ```
///
/// Two decisions worth stating, because both are places where a convenient
/// choice would have been an unsound one:
///
/// * **`index:` values are canonical JSON, not bare scalars.** `index:Person.age=30`
///   is the number 30; `index:Person.name="Ann"` is the string `Ann`. A bare
///   `index:Person.name=Ann` is **rejected**. The tempting alternative — guess
///   the type, or treat anything unparseable as a string — silently produces a
///   [`ValueKey`] that does not equal the one `fire` derives from the document,
///   so the entry is never invalidated by the change it named. Stale forever, and
///   invisible. Rejecting is the only safe answer, and it matches
///   [`ValueKey::of`] exactly.
/// * **The last separator wins.** A document id may contain dots and slashes
///   (`Person/alice.v2`), so `docfield:`/`edge:`/`classfield:` split on the
///   *last* `.` and `backlink:` on the last `<-`. Property names containing a
///   dot are therefore not expressible; that is documented rather than guessed
///   at.
impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Term::DocField(id, prop) => write!(f, "docfield:{id}.{prop}"),
            Term::Doc(id) => write!(f, "doc:{id}"),
            Term::Edge(id, prop) => write!(f, "edge:{id}.{prop}"),
            Term::Backlink {
                target,
                prop,
                from_class,
            } => write!(f, "backlink:{target}.{prop}<-{from_class}"),
            Term::Index { class, prop, value } => {
                write!(f, "index:{class}.{prop}={}", value.as_str())
            }
            Term::ClassField(class, prop) => write!(f, "classfield:{class}.{prop}"),
            Term::Members(class) => write!(f, "members:{class}"),
            Term::Class(class) => write!(f, "class:{class}"),
            Term::Path { from, classes } => {
                // `*` for "no derivable classes", which is the barrier form.
                // It needs a spelling of its own so every `Term` round-trips;
                // an empty payload stays a parse error.
                let classes = if classes.is_empty() {
                    "*".to_owned()
                } else {
                    classes
                        .iter()
                        .map(ClassName::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                };
                match from {
                    Some(from) => write!(f, "path:{from}>{classes}"),
                    None => write!(f, "path:{classes}"),
                }
            }
            Term::Schema => f.write_str("schema"),
            Term::Everything => f.write_str("everything"),
        }
    }
}

impl Term {
    /// Parse the wire encoding. See [`Term`]'s `Display` for the grammar.
    ///
    /// `None` on anything unrecognized — never a guess. A caller that cannot
    /// parse a stated condition must refuse to cache, because a condition we
    /// misunderstood is worse than no condition at all.
    pub fn parse(text: &str) -> Option<Term> {
        let text = text.trim();
        match text {
            "schema" => return Some(Term::Schema),
            "everything" => return Some(Term::Everything),
            _ => {}
        }
        let (tag, rest) = text.split_once(':')?;
        let rest = rest.trim();
        if rest.is_empty() {
            return None;
        }
        // A comma separates *terms*, and no id, class or property name may
        // contain one. Rejecting it here is what stops a malformed list from
        // being absorbed into a name: `class:Person,typo` must fail, not become
        // a condition on a class called `Person,typo` that nothing ever fires.
        // `path:` is the one tag whose payload is itself comma-separated.
        if tag.trim() != "path" && rest.contains(',') {
            return None;
        }
        match tag.trim() {
            "doc" => Some(Term::Doc(DocId::parse(rest))),
            "members" => Some(Term::Members(ClassName::new(rest))),
            "class" => Some(Term::Class(ClassName::new(rest))),
            "docfield" => {
                let (id, prop) = split_last_dot(rest)?;
                Some(Term::DocField(DocId::parse(id), PropName::new(prop)))
            }
            "edge" => {
                let (id, prop) = split_last_dot(rest)?;
                Some(Term::Edge(DocId::parse(id), PropName::new(prop)))
            }
            "classfield" => {
                let (class, prop) = split_last_dot(rest)?;
                Some(Term::ClassField(ClassName::new(class), PropName::new(prop)))
            }
            "index" => {
                // Split on the FIRST `=`: a canonical JSON value may contain one.
                let (lhs, value) = rest.split_once('=')?;
                let (class, prop) = split_last_dot(lhs.trim())?;
                let parsed: serde_json::Value = serde_json::from_str(value.trim()).ok()?;
                Some(Term::Index {
                    class: ClassName::new(class),
                    prop: PropName::new(prop),
                    value: ValueKey::of(&parsed),
                })
            }
            "backlink" => {
                let idx = rest.rfind("<-")?;
                let (lhs, from_class) = (&rest[..idx], &rest[idx + 2..]);
                let (target, prop) = split_last_dot(lhs.trim())?;
                let from_class = from_class.trim();
                if from_class.is_empty() {
                    return None;
                }
                Some(Term::Backlink {
                    target: DocId::parse(target),
                    prop: PropName::new(prop),
                    from_class: ClassName::new(from_class),
                })
            }
            "path" => {
                let (from, classes) = match rest.split_once('>') {
                    Some((from, classes)) => {
                        let from = from.trim();
                        if from.is_empty() {
                            return None;
                        }
                        (Some(DocId::parse(from)), classes)
                    }
                    None => (None, rest),
                };
                let classes = classes.trim();
                if classes.is_empty() {
                    // An empty payload is a typo, not a barrier. The barrier is
                    // spelled `path:*` (or, more plainly, `everything`).
                    return None;
                }
                if classes == "*" {
                    return Some(Term::Path {
                        from,
                        classes: Vec::new(),
                    });
                }
                let classes: Vec<ClassName> = classes
                    .split(',')
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .map(ClassName::new)
                    .collect();
                if classes.is_empty() {
                    // The payload was only separators (`path:,,`): a typo.
                    return None;
                }
                Some(Term::Path { from, classes })
            }
            _ => None,
        }
    }

    /// Parse a comma-separated list of conditions.
    ///
    /// All-or-nothing: if any element fails, the whole list fails. A partially
    /// understood condition set is not a smaller condition set — it is a wrong
    /// one, and acting on it would under-invalidate.
    pub fn parse_list(text: &str) -> Option<Vec<Term>> {
        let mut out = Vec::new();
        for element in split_conditions(text) {
            out.push(Term::parse(element)?);
        }
        if out.is_empty() {
            return None;
        }
        out.sort();
        out.dedup();
        Some(out)
    }
}

impl std::str::FromStr for Term {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Term::parse(text).ok_or(())
    }
}

/// Split a condition list on commas that separate *terms*.
///
/// A `path:` term's own class list is comma-separated, so a naive split would
/// tear it apart. Splitting only on commas that are followed by a recognized
/// tag keeps both grammars in one header.
fn split_conditions(text: &str) -> Vec<&str> {
    const TAGS: &[&str] = &[
        "doc:",
        "docfield:",
        "edge:",
        "members:",
        "class:",
        "classfield:",
        "index:",
        "backlink:",
        "path:",
        "schema",
        "everything",
    ];
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte != b',' {
            continue;
        }
        let tail = text[i + 1..].trim_start();
        if TAGS.iter().any(|tag| tail.starts_with(tag)) {
            out.push(text[start..i].trim());
            start = i + 1;
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        out.push(last);
    }
    out.retain(|element| !element.is_empty());
    out
}

/// Split on the last `.`, rejecting an empty side.
fn split_last_dot(text: &str) -> Option<(&str, &str)> {
    let idx = text.rfind('.')?;
    let (left, right) = (&text[..idx], &text[idx + 1..]);
    if left.is_empty() || right.is_empty() {
        return None;
    }
    Some((left, right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use serde_json::json;

    fn vk(v: serde_json::Value) -> ValueKey {
        ValueKey::of(&v)
    }

    // ---------------------------------------------------------------- ValueKey

    #[test]
    fn value_key_distinguishes_number_shapes_and_strings() {
        assert_ne!(vk(json!(1)), vk(json!(1.0)));
        assert_ne!(vk(json!(1)), vk(json!("1")));
        assert_eq!(vk(json!(1)), vk(json!(1)));
        assert_eq!(vk(json!("x")).as_str(), "\"x\"");
    }

    #[test]
    fn value_key_preserves_high_precision_numbers() {
        // The reason this is canonical text and not an f64: these two must not
        // collide, and through `f64` they would.
        let a: serde_json::Value =
            serde_json::from_str("123456789012345678901234567890.0000000001").unwrap();
        let b: serde_json::Value =
            serde_json::from_str("123456789012345678901234567890.0000000002").unwrap();
        assert_ne!(vk(a), vk(b));
    }

    #[test]
    fn value_key_sorts_object_keys() {
        let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":2}"#).unwrap();
        let b: serde_json::Value = serde_json::from_str(r#"{"a":2,"b":1}"#).unwrap();
        assert_eq!(vk(a), vk(b));
        assert_eq!(
            vk(json!({"b":1,"a":[2,null,true]})).as_str(),
            "{\"a\":[2,null,true],\"b\":1}"
        );
    }

    // ----------------------------------------------------------- bucket layout

    #[test]
    fn buckets_are_one_per_term_except_path() {
        assert_eq!(
            Term::Doc(doc("Person/alice")).buckets(),
            vec![TermBucket::Doc(doc("Person/alice"))]
        );
        assert_eq!(
            Term::DocField(doc("Person/alice"), prop("name")).buckets(),
            vec![TermBucket::Doc(doc("Person/alice"))]
        );
        assert_eq!(
            Term::Backlink {
                target: doc("Company/acme"),
                prop: prop("employer"),
                from_class: cls("Person")
            }
            .buckets(),
            vec![TermBucket::Doc(doc("Company/acme"))],
            "a backlink is filed under its target, which is what it is matched on"
        );
        assert_eq!(
            Term::Index {
                class: cls("Person"),
                prop: prop("name"),
                value: vk(json!("alice"))
            }
            .buckets(),
            vec![TermBucket::Class(cls("Person"))]
        );
        assert_eq!(Term::Schema.buckets(), vec![TermBucket::Schema]);
        assert_eq!(Term::Everything.buckets(), vec![TermBucket::Everything]);
    }

    #[test]
    fn path_buckets_one_per_class_and_everything_when_empty() {
        let p = Term::Path {
            from: None,
            classes: vec![cls("Person"), cls("Company")],
        };
        assert_eq!(
            p.buckets(),
            vec![
                TermBucket::Class(cls("Person")),
                TermBucket::Class(cls("Company"))
            ]
        );
        let bare = Term::Path {
            from: Some(doc("Person/alice")),
            classes: vec![],
        };
        assert_eq!(bare.buckets(), vec![TermBucket::Everything]);
    }

    #[test]
    fn lookup_buckets_walk_up_the_hierarchy() {
        let schema = schema_fixture();
        let fired = Term::DocField(doc("Employee/e1"), prop("name"));
        let buckets = fired.lookup_buckets(&schema);
        assert!(buckets.contains(&TermBucket::Doc(doc("Employee/e1"))));
        assert!(buckets.contains(&TermBucket::Class(cls("Employee"))));
        assert!(buckets.contains(&TermBucket::Class(cls("Person"))));
        assert!(buckets.contains(&TermBucket::Class(cls("Agent"))));
        assert!(buckets.contains(&TermBucket::Everything));
        assert!(!buckets.contains(&TermBucket::Schema));
    }

    #[test]
    fn lookup_buckets_for_schema_and_everything() {
        let schema = schema_fixture();
        assert_eq!(
            Term::Schema.lookup_buckets(&schema),
            vec![TermBucket::Schema, TermBucket::Everything]
        );
        assert_eq!(
            Term::Everything.lookup_buckets(&schema),
            vec![TermBucket::Everything],
            "a fired Everything must be special-cased by the index"
        );
    }

    /// The property the bucket design rests on: if a dependency subsumes a fired
    /// term, their bucket sets must intersect, or the inverted index would never
    /// even consider them.
    #[test]
    fn bucket_pruning_never_hides_a_subsuming_pair() {
        let schema = schema_fixture();
        let population = term_population();
        for dep in &population {
            for fired in &population {
                if matches!(fired, Term::Everything) {
                    continue; // handled by the index, not by buckets
                }
                if dep.subsumes(fired, &schema) {
                    let dep_buckets = dep.buckets();
                    let fired_buckets = fired.lookup_buckets(&schema);
                    assert!(
                        dep_buckets.iter().any(|b| fired_buckets.contains(b)),
                        "bucket sets must intersect: dep={dep:?} fired={fired:?}\n  dep buckets {dep_buckets:?}\n  fired buckets {fired_buckets:?}"
                    );
                }
            }
        }
    }

    /// A broad mix of terms over the fixture schema, for the exhaustive
    /// cross-product properties above.
    fn term_population() -> Vec<Term> {
        let ids = [
            doc("Person/alice"),
            doc("Person/bob"),
            doc("Employee/e1"),
            doc("Company/acme"),
            doc("Department/d1"),
            doc("loose"),
        ];
        let props = [prop("name"), prop("employer"), prop("head")];
        let classes = [
            cls("Agent"),
            cls("Person"),
            cls("Employee"),
            cls("Company"),
            cls("Department"),
        ];
        let mut out = vec![Term::Schema, Term::Everything];
        for id in &ids {
            out.push(Term::Doc(id.clone()));
            for p in &props {
                out.push(Term::DocField(id.clone(), p.clone()));
                out.push(Term::Edge(id.clone(), p.clone()));
                for c in &classes {
                    out.push(Term::Backlink {
                        target: id.clone(),
                        prop: p.clone(),
                        from_class: c.clone(),
                    });
                }
            }
        }
        for c in &classes {
            out.push(Term::Members(c.clone()));
            out.push(Term::Class(c.clone()));
            for p in &props {
                out.push(Term::ClassField(c.clone(), p.clone()));
                out.push(Term::Index {
                    class: c.clone(),
                    prop: p.clone(),
                    value: vk(json!("v")),
                });
            }
        }
        out.push(Term::Path {
            from: None,
            classes: vec![cls("Person")],
        });
        out.push(Term::Path {
            from: Some(doc("Person/alice")),
            classes: vec![],
        });
        out
    }

    // ------------------------------------------------- the `subsumes` truth table
    //
    // One test per row of the table in SPEC.md. `a` is `Person/alice`, `b` is
    // `Person/bob`, `C` is `Person`, `D` is `Employee` (a descendant of Person),
    // and `Company` is the unrelated class.

    fn a() -> DocId {
        doc("Person/alice")
    }
    fn b() -> DocId {
        doc("Person/bob")
    }
    fn p() -> PropName {
        prop("name")
    }
    fn q() -> PropName {
        prop("employer")
    }

    #[test]
    fn row_doc_is_dirtied_by_doc_docfield_and_edge() {
        let s = schema_fixture();
        let dep = Term::Doc(a());
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(dep.subsumes(&Term::DocField(a(), p()), &s));
        assert!(dep.subsumes(&Term::Edge(a(), q()), &s));
    }

    #[test]
    fn row_doc_is_not_dirtied_by_another_doc() {
        let s = schema_fixture();
        assert!(!Term::Doc(a()).subsumes(&Term::Doc(b()), &s));
    }

    #[test]
    fn row_docfield_is_dirtied_by_same_field_and_by_whole_doc() {
        let s = schema_fixture();
        let dep = Term::DocField(a(), p());
        assert!(dep.subsumes(&Term::DocField(a(), p()), &s));
        assert!(
            dep.subsumes(&Term::Doc(a()), &s),
            "a coarse document fire does not say which field moved"
        );
    }

    #[test]
    fn row_docfield_is_not_dirtied_by_a_different_field() {
        let s = schema_fixture();
        assert!(!Term::DocField(a(), p()).subsumes(&Term::DocField(a(), q()), &s));
    }

    #[test]
    fn row_edge_is_dirtied_by_docfield_edge_and_doc() {
        let s = schema_fixture();
        let dep = Term::Edge(a(), q());
        assert!(dep.subsumes(&Term::DocField(a(), q()), &s));
        assert!(dep.subsumes(&Term::Edge(a(), q()), &s));
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(!dep.subsumes(&Term::Edge(a(), p()), &s));
        assert!(!dep.subsumes(&Term::Edge(b(), q()), &s));
    }

    #[test]
    fn row_class_is_dirtied_by_its_own_and_descendant_instances() {
        let s = schema_fixture();
        let dep = Term::Class(cls("Person"));
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(dep.subsumes(&Term::DocField(a(), p()), &s));
        assert!(dep.subsumes(&Term::Edge(a(), q()), &s));
        assert!(dep.subsumes(&Term::Members(cls("Person")), &s));
        assert!(dep.subsumes(&Term::ClassField(cls("Person"), p()), &s));
        assert!(dep.subsumes(
            &Term::Index {
                class: cls("Person"),
                prop: p(),
                value: vk(json!("x"))
            },
            &s
        ));

        // Descendant class: an Employee is a Person.
        assert!(dep.subsumes(&Term::Doc(doc("Employee/e1")), &s));
        assert!(dep.subsumes(&Term::DocField(doc("Employee/e1"), p()), &s));
        assert!(dep.subsumes(&Term::Members(cls("Employee")), &s));
        assert!(dep.subsumes(&Term::ClassField(cls("Employee"), p()), &s));
        assert!(dep.subsumes(
            &Term::Index {
                class: cls("Employee"),
                prop: p(),
                value: vk(json!("x"))
            },
            &s
        ));
    }

    #[test]
    fn row_class_is_not_dirtied_by_an_unrelated_class() {
        let s = schema_fixture();
        let dep = Term::Class(cls("Person"));
        assert!(!dep.subsumes(&Term::Doc(doc("Company/acme")), &s));
        assert!(!dep.subsumes(&Term::Members(cls("Company")), &s));
        assert!(!dep.subsumes(&Term::ClassField(cls("Company"), p()), &s));
        assert!(
            !dep.subsumes(&Term::Members(cls("Agent")), &s),
            "a superclass fire is not implied: fire() emits the subclass terms too"
        );
    }

    #[test]
    fn row_members_is_dirtied_by_members_of_itself_or_a_descendant() {
        let s = schema_fixture();
        let dep = Term::Members(cls("Person"));
        assert!(dep.subsumes(&Term::Members(cls("Person")), &s));
        assert!(dep.subsumes(&Term::Members(cls("Employee")), &s));
        assert!(!dep.subsumes(&Term::Members(cls("Company")), &s));
    }

    #[test]
    fn row_members_is_not_dirtied_by_a_field_change() {
        let s = schema_fixture();
        let dep = Term::Members(cls("Person"));
        assert!(
            !dep.subsumes(&Term::DocField(a(), p()), &s),
            "updating a document does not change class membership"
        );
        assert!(!dep.subsumes(&Term::Doc(a()), &s));
        assert!(!dep.subsumes(&Term::Class(cls("Person")), &s));
    }

    #[test]
    fn row_classfield_is_dirtied_by_matching_docfield_and_classfield() {
        let s = schema_fixture();
        let dep = Term::ClassField(cls("Person"), p());
        assert!(dep.subsumes(&Term::DocField(a(), p()), &s));
        assert!(dep.subsumes(&Term::ClassField(cls("Person"), p()), &s));
        assert!(dep.subsumes(&Term::ClassField(cls("Employee"), p()), &s));
        assert!(dep.subsumes(&Term::DocField(doc("Employee/e1"), p()), &s));
    }

    #[test]
    fn row_classfield_is_not_dirtied_by_a_different_property() {
        let s = schema_fixture();
        let dep = Term::ClassField(cls("Person"), p());
        assert!(!dep.subsumes(&Term::DocField(a(), q()), &s));
        assert!(!dep.subsumes(&Term::ClassField(cls("Person"), q()), &s));
        assert!(!dep.subsumes(&Term::DocField(doc("Company/acme"), p()), &s));
    }

    /// Not a row of the table, but implied by its `Index` row: a coarse fire
    /// that names the document but not the property must dirty a property
    /// dependency, or we would serve stale data whenever the resolver could only
    /// report "this document changed".
    #[test]
    fn classfield_is_dirtied_by_a_coarse_document_fire() {
        let s = schema_fixture();
        let dep = Term::ClassField(cls("Person"), p());
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(!dep.subsumes(&Term::Doc(doc("Company/acme")), &s));
    }

    #[test]
    fn row_index_is_dirtied_by_the_same_index_including_descendants() {
        let s = schema_fixture();
        let dep = Term::Index {
            class: cls("Person"),
            prop: p(),
            value: vk(json!("alice")),
        };
        assert!(dep.subsumes(
            &Term::Index {
                class: cls("Person"),
                prop: p(),
                value: vk(json!("alice"))
            },
            &s
        ));
        assert!(dep.subsumes(
            &Term::Index {
                class: cls("Employee"),
                prop: p(),
                value: vk(json!("alice"))
            },
            &s
        ));
    }

    #[test]
    fn row_index_is_not_dirtied_by_a_different_value() {
        let s = schema_fixture();
        let dep = Term::Index {
            class: cls("Person"),
            prop: p(),
            value: vk(json!("alice")),
        };
        assert!(!dep.subsumes(
            &Term::Index {
                class: cls("Person"),
                prop: p(),
                value: vk(json!("bob"))
            },
            &s
        ));
        assert!(!dep.subsumes(
            &Term::Index {
                class: cls("Person"),
                prop: q(),
                value: vk(json!("alice"))
            },
            &s
        ));
        assert!(!dep.subsumes(
            &Term::Index {
                class: cls("Company"),
                prop: p(),
                value: vk(json!("alice"))
            },
            &s
        ));
    }

    #[test]
    fn row_index_is_dirtied_by_a_coarse_doc_fire() {
        let s = schema_fixture();
        let dep = Term::Index {
            class: cls("Person"),
            prop: p(),
            value: vk(json!("alice")),
        };
        assert!(
            dep.subsumes(&Term::Doc(a()), &s),
            "value unknown, so we must assume the bucket moved"
        );
        assert!(dep.subsumes(&Term::DocField(a(), p()), &s));
        assert!(dep.subsumes(&Term::ClassField(cls("Person"), p()), &s));
        assert!(
            dep.subsumes(&Term::Members(cls("Person")), &s),
            "a create or delete changes which documents are in the bucket"
        );
        assert!(!dep.subsumes(&Term::Doc(doc("Company/acme")), &s));
    }

    #[test]
    fn row_backlink_is_dirtied_by_the_same_backlink_including_descendants() {
        let s = schema_fixture();
        let dep = Term::Backlink {
            target: doc("Company/acme"),
            prop: q(),
            from_class: cls("Person"),
        };
        assert!(dep.subsumes(
            &Term::Backlink {
                target: doc("Company/acme"),
                prop: q(),
                from_class: cls("Person")
            },
            &s
        ));
        assert!(
            dep.subsumes(
                &Term::Backlink {
                    target: doc("Company/acme"),
                    prop: q(),
                    from_class: cls("Employee")
                },
                &s
            ),
            "an Employee pointing at acme is a Person pointing at acme"
        );
    }

    #[test]
    fn row_backlink_is_not_dirtied_by_another_target() {
        let s = schema_fixture();
        let dep = Term::Backlink {
            target: doc("Company/acme"),
            prop: q(),
            from_class: cls("Person"),
        };
        assert!(!dep.subsumes(
            &Term::Backlink {
                target: doc("Company/other"),
                prop: q(),
                from_class: cls("Person")
            },
            &s
        ));
        assert!(!dep.subsumes(
            &Term::Backlink {
                target: doc("Company/acme"),
                prop: p(),
                from_class: cls("Person")
            },
            &s
        ));
        assert!(
            !dep.subsumes(
                &Term::Backlink {
                    target: doc("Company/acme"),
                    prop: q(),
                    from_class: cls("Agent")
                },
                &s
            ),
            "a superclass fire is not implied: fire() emits the ancestor terms itself"
        );
        assert!(
            !dep.subsumes(&Term::Doc(doc("Company/acme")), &s),
            "the target changing does not change who points at it"
        );
    }

    #[test]
    fn row_path_is_dirtied_by_anything_on_a_listed_class_or_below() {
        let s = schema_fixture();
        let dep = Term::Path {
            from: Some(a()),
            classes: vec![cls("Person"), cls("Department")],
        };
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(dep.subsumes(&Term::Doc(doc("Employee/e1")), &s));
        assert!(dep.subsumes(&Term::Members(cls("Department")), &s));
        assert!(dep.subsumes(
            &Term::Backlink {
                target: doc("Company/acme"),
                prop: q(),
                from_class: cls("Person")
            },
            &s
        ));
        assert!(!dep.subsumes(&Term::Doc(doc("Company/acme")), &s));
        assert!(!dep.subsumes(&Term::Schema, &s));
    }

    #[test]
    fn path_without_classes_is_a_barrier() {
        let s = schema_fixture();
        let dep = Term::Path {
            from: None,
            classes: vec![],
        };
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(dep.subsumes(&Term::Schema, &s));
        assert!(dep.subsumes(&Term::Members(cls("Company")), &s));
    }

    #[test]
    fn row_schema_is_dirtied_only_by_schema() {
        let s = schema_fixture();
        assert!(Term::Schema.subsumes(&Term::Schema, &s));
    }

    #[test]
    fn row_schema_is_not_dirtied_by_a_document() {
        let s = schema_fixture();
        assert!(!Term::Schema.subsumes(&Term::Doc(a()), &s));
        assert!(!Term::Schema.subsumes(&Term::Class(cls("Person")), &s));
    }

    #[test]
    fn row_everything_is_dirtied_by_anything_including_schema() {
        let s = schema_fixture();
        for fired in term_population() {
            assert!(
                Term::Everything.subsumes(&fired, &s),
                "Everything must be dirtied by {fired:?}"
            );
        }
    }

    #[test]
    fn row_anything_is_dirtied_by_everything() {
        let s = schema_fixture();
        for dep in term_population() {
            assert!(
                dep.subsumes(&Term::Everything, &s),
                "{dep:?} must be dirtied by a fired Everything"
            );
        }
    }

    // ------------------------------------------------------- degraded schema

    #[test]
    fn with_an_empty_schema_only_exact_class_matches_fire() {
        let s = SchemaIndex::default();
        let dep = Term::Class(cls("Person"));
        assert!(dep.subsumes(&Term::Doc(a()), &s));
        assert!(
            !dep.subsumes(&Term::Doc(doc("Employee/e1")), &s),
            "without a schema we cannot know Employee is a Person; analyzers must \
             therefore emit coarse terms when the index is empty"
        );
    }

    #[test]
    fn terms_sort_precise_before_coarse() {
        let mut terms = vec![
            Term::Everything,
            Term::Class(cls("Person")),
            Term::Doc(a()),
            Term::DocField(a(), p()),
            Term::Schema,
        ];
        terms.sort();
        assert_eq!(
            terms,
            vec![
                Term::DocField(a(), p()),
                Term::Doc(a()),
                Term::Class(cls("Person")),
                Term::Schema,
                Term::Everything,
            ]
        );
    }

    // ----------------------------------------------------- text encoding

    /// Every variant, through `Display` and back. This encoding is a public
    /// interface — clients write it into a header — so a silent change here
    /// would break callers and, worse, could change which term a stated
    /// condition means.
    #[test]
    fn every_term_round_trips_through_its_text_encoding() {
        for term in term_population() {
            let text = term.to_string();
            let parsed =
                Term::parse(&text).unwrap_or_else(|| panic!("{text} (from {term:?}) must parse"));
            assert_eq!(parsed, term, "round trip changed {text}");
        }
    }

    #[test]
    fn the_documented_spellings_parse_to_the_expected_terms() {
        let cases: Vec<(&str, Term)> = vec![
            ("doc:Person/alice", Term::Doc(doc("Person/alice"))),
            (
                "docfield:Person/alice.name",
                Term::DocField(doc("Person/alice"), prop("name")),
            ),
            (
                "edge:Person/alice.friend",
                Term::Edge(doc("Person/alice"), prop("friend")),
            ),
            ("members:Person", Term::Members(cls("Person"))),
            ("class:Person", Term::Class(cls("Person"))),
            (
                "classfield:Person.name",
                Term::ClassField(cls("Person"), prop("name")),
            ),
            (
                "index:Person.name=\"Ann\"",
                Term::Index {
                    class: cls("Person"),
                    prop: prop("name"),
                    value: vk(json!("Ann")),
                },
            ),
            (
                "backlink:Person/alice.friend<-Person",
                Term::Backlink {
                    target: doc("Person/alice"),
                    prop: prop("friend"),
                    from_class: cls("Person"),
                },
            ),
            (
                "path:Person,Company",
                Term::Path {
                    from: None,
                    classes: vec![cls("Person"), cls("Company")],
                },
            ),
            (
                "path:Person/alice>Person,Company",
                Term::Path {
                    from: Some(doc("Person/alice")),
                    classes: vec![cls("Person"), cls("Company")],
                },
            ),
            (
                "path:*",
                Term::Path {
                    from: None,
                    classes: vec![],
                },
            ),
            (
                "path:Person/alice>*",
                Term::Path {
                    from: Some(doc("Person/alice")),
                    classes: vec![],
                },
            ),
            ("schema", Term::Schema),
            ("everything", Term::Everything),
        ];
        for (text, expected) in cases {
            assert_eq!(Term::parse(text), Some(expected.clone()), "parsing {text}");
            assert_eq!(expected.to_string(), text, "rendering {expected:?}");
        }
    }

    #[test]
    fn ids_containing_dots_and_slashes_survive_the_encoding() {
        // The id is everything before the LAST dot, so a dotted id is fine.
        let term = Term::DocField(doc("Person/alice.v2/sub"), prop("name"));
        assert_eq!(term.to_string(), "docfield:Person/alice.v2/sub.name");
        assert_eq!(Term::parse(&term.to_string()), Some(term));

        let edge = Term::Edge(doc("A/b.c.d"), prop("p"));
        assert_eq!(Term::parse(&edge.to_string()), Some(edge));

        let backlink = Term::Backlink {
            target: doc("Person/a.b"),
            prop: prop("friend"),
            from_class: cls("Person"),
        };
        assert_eq!(Term::parse(&backlink.to_string()), Some(backlink));
    }

    #[test]
    fn an_index_value_containing_an_equals_sign_survives() {
        // Split on the FIRST `=`, so the value may contain more of them.
        let term = Term::Index {
            class: cls("Person"),
            prop: prop("motto"),
            value: vk(json!("a=b=c")),
        };
        assert_eq!(term.to_string(), "index:Person.motto=\"a=b=c\"");
        assert_eq!(Term::parse(&term.to_string()), Some(term));
    }

    #[test]
    fn index_values_are_canonical_json_so_types_cannot_be_confused() {
        // The number 30 and the string "30" are different buckets, and `fire`
        // distinguishes them. A bare scalar would make us guess which was
        // meant, and a wrong guess means the entry is never invalidated.
        let number = Term::parse("index:Person.age=30").expect("a JSON number");
        let string = Term::parse("index:Person.age=\"30\"").expect("a JSON string");
        assert_ne!(number, string);
        assert_eq!(
            number,
            Term::Index {
                class: cls("Person"),
                prop: prop("age"),
                value: vk(json!(30))
            }
        );

        // A bare, unquoted scalar is rejected rather than assumed to be a
        // string: see the encoding docs for why.
        assert_eq!(Term::parse("index:Person.name=Ann"), None);
    }

    #[test]
    fn an_index_value_matches_what_fire_would_derive() {
        // The whole point of the encoding: a client's stated condition has to
        // equal the term a commit fires, or it invalidates nothing.
        let stated = Term::parse("index:Person.name=\"Ann\"").unwrap();
        let fired = Term::Index {
            class: cls("Person"),
            prop: prop("name"),
            value: ValueKey::of(&json!("Ann")),
        };
        assert_eq!(stated, fired);
    }

    #[test]
    fn unparseable_conditions_are_rejected_not_guessed() {
        for text in [
            "",
            "   ",
            "nonsense",
            "doc:",
            "doc",
            "docfield:Person/alice",          // no property
            "docfield:.name",                 // no id
            "docfield:Person/alice.",         // empty property
            "classfield:Person",              // no property
            "index:Person.name",              // no value
            "index:Person=\"Ann\"",           // no property
            "index:Person.name=Ann",          // not canonical JSON
            "index:Person.name=",             // empty value
            "backlink:Person/alice.friend",   // no from-class
            "backlink:Person/alice<-Person",  // no property
            "backlink:Person/alice.friend<-", // empty from-class
            "path:",
            "path:Person/alice>", // from but no classes
            "path:>Person",       // classes but empty from
            "future_term:x",
            "SCHEMA", // case-sensitive on purpose
        ] {
            assert_eq!(Term::parse(text), None, "{text:?} must not parse");
        }
    }

    #[test]
    fn whitespace_around_a_condition_is_tolerated() {
        assert_eq!(
            Term::parse("  class:Person  "),
            Some(Term::Class(cls("Person")))
        );
        assert_eq!(Term::parse(" schema "), Some(Term::Schema));
    }

    #[test]
    fn a_condition_list_parses_and_is_deduplicated() {
        let parsed = Term::parse_list("class:Person, members:Company ,doc:Person/alice").unwrap();
        assert_eq!(
            parsed,
            vec![
                Term::Doc(doc("Person/alice")),
                Term::Members(cls("Company")),
                Term::Class(cls("Person")),
            ],
            "sorted precise-to-coarse and deduplicated"
        );
        assert_eq!(
            Term::parse_list("class:Person,class:Person").unwrap().len(),
            1
        );
    }

    #[test]
    fn a_path_terms_own_comma_list_is_not_split_into_separate_terms() {
        // `path:` is the one term whose payload contains commas.
        let parsed = Term::parse_list("path:Person,Company,class:Agent").unwrap();
        assert_eq!(
            parsed,
            vec![
                Term::Class(cls("Agent")),
                Term::Path {
                    from: None,
                    classes: vec![cls("Person"), cls("Company")]
                },
            ]
        );
    }

    #[test]
    fn a_name_may_not_absorb_a_following_term() {
        // Regression, and it was a soundness hole: because a comma only split
        // the list when followed by a recognized tag, `class:Person,typo`
        // parsed as a condition on a class literally called `Person,typo` —
        // which nothing ever fires, so the entry would have been cached and
        // then never invalidated. No name may contain a comma.
        assert_eq!(Term::parse("class:Person,typo"), None);
        assert_eq!(Term::parse("members:Person,typo"), None);
        assert_eq!(Term::parse("doc:Person/alice,typo"), None);
        assert_eq!(Term::parse("classfield:Person,x.name"), None);
        assert_eq!(Term::parse_list("class:Person,typo"), None);
    }

    #[test]
    fn a_list_with_one_bad_element_fails_entirely() {
        // All-or-nothing: a partly understood condition set is a wrong one, not
        // a smaller one.
        assert_eq!(Term::parse_list("class:Person,nonsense"), None);
        assert_eq!(Term::parse_list("nonsense,class:Person"), None);
        assert_eq!(Term::parse_list(""), None);
        assert_eq!(Term::parse_list(" , "), None);
    }

    #[test]
    fn from_str_agrees_with_parse() {
        use std::str::FromStr;
        assert_eq!(
            Term::from_str("class:Person").unwrap(),
            Term::Class(cls("Person"))
        );
        assert!(Term::from_str("nope").is_err());
    }

    /// The client's `CacheCondition` renders the same encoding this parses.
    ///
    /// `terminusdb-client` must not depend on this crate, so its renderer is a
    /// separate implementation of the same grammar — which means the two can
    /// drift. This test is the only thing that would notice, so it covers every
    /// constructor.
    #[test]
    fn client_cache_conditions_parse_as_the_terms_they_name() {
        use terminusdb_client::CacheCondition;
        let cases: Vec<(CacheCondition, Term)> = vec![
            (
                CacheCondition::doc("Person/alice"),
                Term::Doc(doc("Person/alice")),
            ),
            (
                CacheCondition::doc_field("Person/alice", "name"),
                Term::DocField(doc("Person/alice"), prop("name")),
            ),
            (
                CacheCondition::edge("Person/alice", "friend"),
                Term::Edge(doc("Person/alice"), prop("friend")),
            ),
            (
                CacheCondition::members("Person"),
                Term::Members(cls("Person")),
            ),
            (CacheCondition::class("Person"), Term::Class(cls("Person"))),
            (
                CacheCondition::class_field("Person", "name"),
                Term::ClassField(cls("Person"), prop("name")),
            ),
            (
                CacheCondition::index("Person", "name", json!("Ann")),
                Term::Index {
                    class: cls("Person"),
                    prop: prop("name"),
                    value: vk(json!("Ann")),
                },
            ),
            (
                CacheCondition::index("Person", "age", json!(30)),
                Term::Index {
                    class: cls("Person"),
                    prop: prop("age"),
                    value: vk(json!(30)),
                },
            ),
            (
                CacheCondition::backlink("Person/alice", "friend", "Person"),
                Term::Backlink {
                    target: doc("Person/alice"),
                    prop: prop("friend"),
                    from_class: cls("Person"),
                },
            ),
            (
                CacheCondition::path(["Person", "Company"]),
                Term::Path {
                    from: None,
                    classes: vec![cls("Person"), cls("Company")],
                },
            ),
            (
                CacheCondition::path_from("Person/alice", ["Person"]),
                Term::Path {
                    from: Some(doc("Person/alice")),
                    classes: vec![cls("Person")],
                },
            ),
            (
                CacheCondition::path(Vec::<String>::new()),
                Term::Path {
                    from: None,
                    classes: vec![],
                },
            ),
            (CacheCondition::schema(), Term::Schema),
            (CacheCondition::everything(), Term::Everything),
        ];
        for (condition, expected) in cases {
            let rendered = condition.to_string();
            assert_eq!(
                Term::parse(&rendered),
                Some(expected.clone()),
                "the client renders {condition:?} as {rendered:?}, which must parse as \
                 {expected:?}"
            );
            assert_eq!(
                expected.to_string(),
                rendered,
                "and both sides must render it identically"
            );
        }
    }

    #[test]
    fn a_client_condition_list_parses_as_a_whole() {
        use terminusdb_client::CacheCondition;
        let header = CacheCondition::header_value(&[
            CacheCondition::class("Person"),
            CacheCondition::members("Company"),
            CacheCondition::index("Person", "age", json!(30)),
        ])
        .expect("a non-empty list");
        let parsed = Term::parse_list(&header).expect("the client's header must parse");
        assert_eq!(parsed.len(), 3, "got {parsed:?} from {header}");
        assert!(parsed.contains(&Term::Class(cls("Person"))));
        assert!(parsed.contains(&Term::Members(cls("Company"))));
    }

    #[test]
    fn terms_round_trip_through_json() {
        let terms = term_population();
        let json = serde_json::to_string(&terms).expect("terms must serialize");
        let back: Vec<Term> = serde_json::from_str(&json).expect("terms must deserialize");
        assert_eq!(terms, back);
    }
}

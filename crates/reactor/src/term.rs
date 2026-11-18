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

    #[test]
    fn terms_round_trip_through_json() {
        let terms = term_population();
        let json = serde_json::to_string(&terms).expect("terms must serialize");
        let back: Vec<Term> = serde_json::from_str(&json).expect("terms must deserialize");
        assert_eq!(terms, back);
    }
}

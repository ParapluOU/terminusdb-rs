//! Commits in, terms out.
//!
//! This module is the bridge between "what TerminusDB did" and "what Reactor
//! must invalidate". It is deliberately pure: a [`ChangeSet`] is plain data, and
//! [`fire`] is a total function of it and the schema. That is what makes the
//! engine testable against an oracle — no clock, no I/O, no ordering.
//!
//! The expensive design decision recorded here is that we pay for **before**
//! state. `/api/changes` would tell us which documents changed for free, but
//! "which documents changed" cannot distinguish `Index{Person, city, "Berlin"}`
//! from `Index{Person, city, "Paris"}` when someone moves house, and it cannot
//! tell us that `Company/old` just *lost* a backlink. Both of those are
//! precisely the terms that make reverse edges and filtered lists cacheable, so
//! we pay for `/api/diff` to get the before side. See design doc §5.3.

use serde_json::Value;

use crate::ids::{ClassName, DocId, Namespace, PropName};
use crate::schema_index::{PropKind, SchemaIndex};
use crate::term::{Term, TermSet, ValueKey};
use terminusdb_client::CommitId;

/// Maximum JSON nesting we will walk when normalizing a property value.
///
/// A `sys:JSON` property can hold arbitrary client-supplied JSON, and these
/// walks are recursive, so there has to be a bound. Collapsing beyond the bound
/// loses precision (two very deep values may normalize alike) which can only
/// cause *over*-firing, never under-firing.
const MAX_VALUE_DEPTH: usize = 32;

/// One document's transition in one commit. Exactly one side may be `None`
/// (create / delete). Values are the document JSON as TerminusDB reports it.
#[derive(Clone, Debug)]
pub struct DocChange {
    /// The document's id, normalized to short form.
    pub id: DocId,
    /// The document's class. For a delete this is the class it had.
    pub class: ClassName,
    /// The document before the commit, or `None` for a create.
    pub before: Option<Value>,
    /// The document after the commit, or `None` for a delete.
    pub after: Option<Value>,
}

impl DocChange {
    /// A convenience constructor for an update.
    pub fn updated(id: DocId, class: ClassName, before: Value, after: Value) -> Self {
        Self {
            id,
            class,
            before: Some(before),
            after: Some(after),
        }
    }

    /// A convenience constructor for a create.
    pub fn created(id: DocId, class: ClassName, after: Value) -> Self {
        Self {
            id,
            class,
            before: None,
            after: Some(after),
        }
    }

    /// A convenience constructor for a delete.
    pub fn deleted(id: DocId, class: ClassName, before: Value) -> Self {
        Self {
            id,
            class,
            before: Some(before),
            after: None,
        }
    }

    /// True when the document did not exist before this commit.
    pub fn is_create(&self) -> bool {
        self.before.is_none() && self.after.is_some()
    }

    /// True when the document does not exist after this commit.
    pub fn is_delete(&self) -> bool {
        self.before.is_some() && self.after.is_none()
    }

    /// Properties whose value differs between before and after.
    ///
    /// `@`-keywords are ignored except that a changed `@type` counts — a
    /// retyped document is a different thing, and anything that depended on its
    /// old class must be invalidated. For a create or delete this is every
    /// property present on the side that exists.
    ///
    /// Note that value comparison is structural over the *lexical* JSON (under
    /// `arbitrary_precision`, `1` and `1.0` are different `Number`s), so a
    /// re-serialization that changes a number's spelling reads as a change.
    /// Over-firing, therefore acceptable.
    pub fn changed_props(&self) -> Vec<PropName> {
        let mut out: std::collections::BTreeSet<PropName> = Default::default();
        match (&self.before, &self.after) {
            (Some(before), Some(after)) => {
                for key in object_keys(before).chain(object_keys(after)) {
                    if key.starts_with('@') && key != "@type" {
                        continue;
                    }
                    if before.get(key) != after.get(key) {
                        out.insert(PropName::new(key));
                    }
                }
            }
            (None, Some(side)) | (Some(side), None) => {
                for key in object_keys(side) {
                    if key.starts_with('@') {
                        continue;
                    }
                    out.insert(PropName::new(key));
                }
            }
            (None, None) => {}
        }
        out.into_iter().collect()
    }

    /// before/after values of one property, each normalized to a list of
    /// scalars (so a Set/List property yields its elements).
    ///
    /// Normalization unwraps JSON-LD `{"@value": …}` literal boxes and flattens
    /// nested arrays, but deliberately keeps `{"@id": …}` objects intact: those
    /// are link targets, and [`fire`] needs to recognize them.
    pub fn prop_values(&self, prop: &PropName) -> (Vec<Value>, Vec<Value>) {
        (
            normalized_values(self.before.as_ref(), prop),
            normalized_values(self.after.as_ref(), prop),
        )
    }
}

fn object_keys(v: &Value) -> impl Iterator<Item = &str> {
    v.as_object()
        .into_iter()
        .flat_map(|map| map.keys().map(String::as_str))
}

fn normalized_values(side: Option<&Value>, prop: &PropName) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(value) = side.and_then(|doc| doc.get(prop.as_str())) {
        push_normalized(value, 0, &mut out);
    }
    out
}

fn push_normalized(v: &Value, depth: usize, out: &mut Vec<Value>) {
    if depth > MAX_VALUE_DEPTH {
        out.push(v.clone());
        return;
    }
    match v {
        // An absent value and an explicit null are the same thing to us: the
        // property holds nothing.
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                push_normalized(item, depth + 1, out);
            }
        }
        Value::Object(map) => match map.get("@value") {
            Some(inner) => push_normalized(inner, depth + 1, out),
            None => out.push(v.clone()),
        },
        other => out.push(other.clone()),
    }
}

/// Everything one commit did, as far as we could determine.
///
/// The `complete` flag is the whole honesty mechanism of the capture pipeline:
/// a resolver that is not sure says so, and [`fire`] turns that into a
/// namespace-wide barrier.
#[derive(Clone, Debug)]
pub struct ChangeSet {
    /// Which namespace this commit landed on.
    pub ns: Namespace,
    /// The commit id. Opaque — never compared for order.
    pub commit: CommitId,
    /// The commit's parent, when the log told us. Per-branch history is linear,
    /// so there is at most one.
    pub parent: Option<CommitId>,
    /// Per-document transitions.
    pub changes: Vec<DocChange>,
    /// True when the commit touched the schema graph. Cheaply detectable from a
    /// log entry's migration list, or from a diff that names schema documents.
    pub schema_changed: bool,
    /// False when we could not enumerate changes precisely: an unreadable diff,
    /// a response truncated by `count`, a commit too large for the configured
    /// diff threshold, or a non-fast-forward move. An incomplete ChangeSet MUST
    /// fire [`Term::Everything`].
    ///
    /// This also covers the subtler case of a changeset whose *before* side is
    /// missing: without it, a link change cannot fire the losing
    /// [`Term::Backlink`], and a value change cannot fire the vacated
    /// [`Term::Index`]. A resolver that can only see the after state must set
    /// this to `false` rather than emit a half-changeset.
    pub complete: bool,
}

impl ChangeSet {
    /// A complete, empty changeset: a commit that did nothing we track.
    pub fn empty(ns: Namespace, commit: CommitId, parent: Option<CommitId>) -> Self {
        Self {
            ns,
            commit,
            parent,
            changes: Vec::new(),
            schema_changed: false,
            complete: true,
        }
    }

    /// An explicitly incomplete changeset: we know a commit happened, but not
    /// what it did. Fires [`Term::Everything`].
    pub fn incomplete(ns: Namespace, commit: CommitId, parent: Option<CommitId>) -> Self {
        Self {
            complete: false,
            ..Self::empty(ns, commit, parent)
        }
    }
}

/// Derive the terms a commit fires. Design doc §5.3.
///
/// The before-state is what makes `Index`/`Backlink` firing exact; that is why
/// we pay for `/api/diff`.
///
/// Where the schema cannot tell us whether a property is a value or a link, this
/// fires **both** interpretations. Over-firing costs a recomputation;
/// under-firing serves wrong bytes until something else happens to invalidate
/// the entry, which may be never.
pub fn fire(cs: &ChangeSet, schema: &SchemaIndex) -> TermSet {
    let mut out = TermSet::new();

    // An incomplete changeset tells us nothing reliable, so it has to be a
    // barrier. Returning early matters: the partial information in `changes`
    // must not be mistaken for the whole truth.
    if !cs.complete {
        out.insert(Term::Everything);
        return out;
    }

    if cs.schema_changed {
        out.insert(Term::Schema);
    }

    for change in &cs.changes {
        let ancestors = schema.ancestors(&change.class);

        out.insert(Term::Doc(change.id.clone()));
        for class in &ancestors {
            out.insert(Term::Class(class.clone()));
        }

        // Membership only moves on create and delete. An update must not
        // invalidate a `_count` or an id list.
        if change.is_create() || change.is_delete() {
            for class in &ancestors {
                out.insert(Term::Members(class.clone()));
            }
        }

        for prop in change.changed_props() {
            // NON-LOCAL INVARIANT: this fires unconditionally, before the
            // Value/Link branch below, and `Term::subsumes` depends on it.
            //
            // There, a `DocField(id, p)` *dependency* is deliberately NOT
            // matched by a fired `Edge(id, p)` — the two terms mean different
            // things, and conflating them is what the Edge/DocField split
            // exists to avoid. That asymmetry is only sound because a link
            // property's change always fires `DocField` here as well as
            // `Edge`, so a fragment that read the property's stored value is
            // still invalidated. Move this inside the branch and traversal
            // caching silently goes stale.
            out.insert(Term::DocField(change.id.clone(), prop.clone()));

            // The classes that actually declare this property (inheritance
            // resolved). With no schema we fall back to the document's own
            // class, which is the only thing we know.
            let declaring: Vec<ClassName> = ancestors
                .iter()
                .filter(|class| schema.prop(class, &prop).is_some())
                .cloned()
                .collect();
            let declaring = if declaring.is_empty() {
                vec![change.class.clone()]
            } else {
                declaring
            };
            for class in &declaring {
                out.insert(Term::ClassField(class.clone(), prop.clone()));
            }

            let (before, after) = change.prop_values(&prop);
            match schema.prop(&change.class, &prop) {
                Some(PropKind::Value { .. }) => {
                    fire_value_index(&mut out, &declaring, &prop, &before, &after);
                }
                Some(PropKind::Link { .. }) => {
                    fire_link(&mut out, change, &ancestors, &prop, &before, &after);
                }
                None => {
                    // Unknown kind: fire both shapes. The value form covers a
                    // datatype property we failed to classify; the link form
                    // covers an edge we failed to classify.
                    fire_value_index(&mut out, &declaring, &prop, &before, &after);
                    fire_link(&mut out, change, &ancestors, &prop, &before, &after);
                }
            }
        }
    }

    out
}

/// Fire one `Index` term per value that entered or left the property, for every
/// class that declares it.
///
/// The *symmetric difference* is the point: if a set-valued property gains one
/// element, only that element's bucket changed, and a query filtering on a
/// different value of the same property should survive the commit.
fn fire_value_index(
    out: &mut TermSet,
    declaring: &[ClassName],
    prop: &PropName,
    before: &[Value],
    after: &[Value],
) {
    let before_keys: std::collections::BTreeSet<ValueKey> =
        before.iter().map(ValueKey::of).collect();
    let after_keys: std::collections::BTreeSet<ValueKey> = after.iter().map(ValueKey::of).collect();
    for value in before_keys.symmetric_difference(&after_keys) {
        for class in declaring {
            out.insert(Term::Index {
                class: class.clone(),
                prop: prop.clone(),
                value: value.clone(),
            });
        }
    }
}

/// Fire the outgoing `Edge` term plus a `Backlink` term for every target on
/// *either* side: old targets lose a backlink, new ones gain one, and a
/// fragment that listed either one's referrers is now wrong.
fn fire_link(
    out: &mut TermSet,
    change: &DocChange,
    ancestors: &[ClassName],
    prop: &PropName,
    before: &[Value],
    after: &[Value],
) {
    out.insert(Term::Edge(change.id.clone(), prop.clone()));
    let mut targets: Vec<DocId> = Vec::new();
    for value in before.iter().chain(after.iter()) {
        if let Some(target) = link_target(value) {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    for target in targets {
        for from_class in ancestors {
            out.insert(Term::Backlink {
                target: target.clone(),
                prop: prop.clone(),
                from_class: from_class.clone(),
            });
        }
    }
}

/// Interpret a normalized property value as a link target.
///
/// Accepts an unfolded document or reference object (`{"@id": …}`) and a bare id
/// string. A bare string only counts when it has a class prefix, which is the
/// one cheap signal that distinguishes `"Company/acme"` from `"acme"` — without
/// it we would treat every string field as a possible edge and fire a
/// `Backlink` for every word in the database.
fn link_target(value: &Value) -> Option<DocId> {
    match value {
        Value::String(s) => {
            let id = DocId::parse(s);
            id.class().is_some().then_some(id)
        }
        Value::Object(map) => {
            let id = map.get("@id")?.as_str()?;
            Some(DocId::parse(id))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use serde_json::json;

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    fn changeset(changes: Vec<DocChange>) -> ChangeSet {
        ChangeSet {
            ns: ns(),
            commit: CommitId::new("c1"),
            parent: Some(CommitId::new("c0")),
            changes,
            schema_changed: false,
            complete: true,
        }
    }

    fn vk(v: serde_json::Value) -> ValueKey {
        ValueKey::of(&v)
    }

    // ------------------------------------------------------------- DocChange

    #[test]
    fn create_and_delete_are_distinguishable() {
        let created = DocChange::created(doc("Person/alice"), cls("Person"), json!({"name": "A"}));
        assert!(created.is_create());
        assert!(!created.is_delete());

        let deleted = DocChange::deleted(doc("Person/alice"), cls("Person"), json!({"name": "A"}));
        assert!(deleted.is_delete());
        assert!(!deleted.is_create());

        let updated = DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "A"}),
            json!({"name": "B"}),
        );
        assert!(!updated.is_create() && !updated.is_delete());
    }

    #[test]
    fn changed_props_ignores_keywords_but_not_type() {
        let change = DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"@id": "Person/alice", "@type": "Person", "name": "A", "age": 1}),
            json!({"@id": "Person/alice", "@type": "Employee", "name": "B", "age": 1}),
        );
        assert_eq!(
            change.changed_props(),
            vec![prop("@type"), prop("name")],
            "@id is ignored, @type counts, unchanged props are excluded"
        );
    }

    #[test]
    fn changed_props_covers_added_and_removed_props() {
        let change = DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "A", "gone": 1}),
            json!({"name": "A", "fresh": 2}),
        );
        assert_eq!(change.changed_props(), vec![prop("fresh"), prop("gone")]);
    }

    #[test]
    fn changed_props_on_create_is_every_property() {
        let change = DocChange::created(
            doc("Person/alice"),
            cls("Person"),
            json!({"@id": "Person/alice", "@type": "Person", "name": "A", "age": 1}),
        );
        assert_eq!(change.changed_props(), vec![prop("age"), prop("name")]);
    }

    #[test]
    fn prop_values_flattens_and_unwraps() {
        let change = DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"nicknames": ["al", "ali"], "name": {"@value": "A", "@type": "xsd:string"}}),
            json!({"nicknames": "al", "name": null}),
        );
        let (before, after) = change.prop_values(&prop("nicknames"));
        assert_eq!(before, vec![json!("al"), json!("ali")]);
        assert_eq!(after, vec![json!("al")]);

        let (before, after) = change.prop_values(&prop("name"));
        assert_eq!(before, vec![json!("A")], "@value boxes are unwrapped");
        assert_eq!(after, Vec::<Value>::new(), "null is no value at all");

        let (before, after) = change.prop_values(&prop("absent"));
        assert!(before.is_empty() && after.is_empty());
    }

    #[test]
    fn prop_values_keeps_id_objects_intact() {
        let change = DocChange::created(
            doc("Person/alice"),
            cls("Person"),
            json!({"employer": {"@id": "Company/acme", "@type": "Company"}}),
        );
        let (_, after) = change.prop_values(&prop("employer"));
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["@id"], json!("Company/acme"));
    }

    // ------------------------------------------------------------------- fire

    #[test]
    fn incomplete_changeset_fires_everything_and_nothing_else() {
        let schema = schema_fixture();
        let mut cs = changeset(vec![DocChange::created(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "A"}),
        )]);
        cs.complete = false;
        cs.schema_changed = true;
        let fired = fire(&cs, &schema);
        assert_eq!(fired.len(), 1);
        assert!(fired.contains(&Term::Everything));
    }

    #[test]
    fn schema_change_fires_schema() {
        let schema = schema_fixture();
        let mut cs = changeset(vec![]);
        cs.schema_changed = true;
        let fired = fire(&cs, &schema);
        assert_eq!(fired, TermSet::from([Term::Schema]));
    }

    #[test]
    fn empty_complete_changeset_fires_nothing() {
        let schema = schema_fixture();
        assert!(fire(&changeset(vec![]), &schema).is_empty());
    }

    #[test]
    fn create_fires_doc_class_and_members_up_the_hierarchy() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::created(
            doc("Employee/e1"),
            cls("Employee"),
            json!({"@type": "Employee", "name": "A"}),
        )]);
        let fired = fire(&cs, &schema);

        assert!(fired.contains(&Term::Doc(doc("Employee/e1"))));
        for class in ["Employee", "Person", "Agent"] {
            assert!(
                fired.contains(&Term::Class(cls(class))),
                "missing Class({class})"
            );
            assert!(
                fired.contains(&Term::Members(cls(class))),
                "missing Members({class})"
            );
        }
        assert!(!fired.contains(&Term::Members(cls("Company"))));
    }

    #[test]
    fn update_does_not_fire_members() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "A"}),
            json!({"name": "B"}),
        )]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::Class(cls("Person"))));
        assert!(
            !fired.contains(&Term::Members(cls("Person"))),
            "membership is unchanged by an update, and counts depend on that"
        );
    }

    #[test]
    fn delete_fires_members_and_the_before_state_props() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::deleted(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "A", "employer": "Company/acme"}),
        )]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::Members(cls("Person"))));
        assert!(fired.contains(&Term::DocField(doc("Person/alice"), prop("name"))));
        assert!(fired.contains(&Term::Index {
            class: cls("Agent"),
            prop: prop("name"),
            value: vk(json!("A"))
        }));
        assert!(
            fired.contains(&Term::Backlink {
                target: doc("Company/acme"),
                prop: prop("employer"),
                from_class: cls("Person")
            }),
            "acme lost a referrer and any list of its referrers is now wrong"
        );
    }

    #[test]
    fn value_prop_change_fires_both_old_and_new_index_terms() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "Ann"}),
            json!({"name": "Anne"}),
        )]);
        let fired = fire(&cs, &schema);

        assert!(fired.contains(&Term::DocField(doc("Person/alice"), prop("name"))));
        // `name` is declared on Agent, so ClassField/Index fire for Agent too.
        assert!(fired.contains(&Term::ClassField(cls("Agent"), prop("name"))));
        assert!(fired.contains(&Term::ClassField(cls("Person"), prop("name"))));
        for value in [json!("Ann"), json!("Anne")] {
            assert!(
                fired.contains(&Term::Index {
                    class: cls("Person"),
                    prop: prop("name"),
                    value: vk(value.clone())
                }),
                "both the vacated and the occupied bucket must fire: {value}"
            );
        }
        assert!(
            !fired.contains(&Term::Edge(doc("Person/alice"), prop("name"))),
            "a value property is not an edge"
        );
    }

    #[test]
    fn value_prop_change_fires_only_the_classes_that_declare_it() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Employee/e1"),
            cls("Employee"),
            json!({"salary": 1}),
            json!({"salary": 2}),
        )]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::ClassField(cls("Employee"), prop("salary"))));
        assert!(
            !fired.contains(&Term::ClassField(cls("Person"), prop("salary"))),
            "Person does not declare salary"
        );
    }

    #[test]
    fn list_valued_prop_fires_only_the_symmetric_difference() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"nicknames": ["al", "ali", "stays"]}),
            json!({"nicknames": ["ali", "stays", "new"]}),
        )]);
        let fired = fire(&cs, &schema);
        for gone_or_new in ["al", "new"] {
            assert!(
                fired.contains(&Term::Index {
                    class: cls("Person"),
                    prop: prop("nicknames"),
                    value: vk(json!(gone_or_new))
                }),
                "{gone_or_new} entered or left the set"
            );
        }
        for unchanged in ["ali", "stays"] {
            assert!(
                !fired.contains(&Term::Index {
                    class: cls("Person"),
                    prop: prop("nicknames"),
                    value: vk(json!(unchanged))
                }),
                "{unchanged} is in both sides, so its bucket did not move"
            );
        }
    }

    #[test]
    fn link_prop_change_fires_edge_and_both_backlink_targets() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"employer": "Company/old"}),
            json!({"employer": {"@id": "Company/new"}}),
        )]);
        let fired = fire(&cs, &schema);

        assert!(fired.contains(&Term::Edge(doc("Person/alice"), prop("employer"))));
        assert!(fired.contains(&Term::DocField(doc("Person/alice"), prop("employer"))));
        for target in ["Company/old", "Company/new"] {
            assert!(
                fired.contains(&Term::Backlink {
                    target: doc(target),
                    prop: prop("employer"),
                    from_class: cls("Person")
                }),
                "{target} gained or lost a referrer"
            );
        }
        assert!(
            fired.contains(&Term::Backlink {
                target: doc("Company/new"),
                prop: prop("employer"),
                from_class: cls("Agent")
            }),
            "a backlink fires for every ancestor class of the pointing document, \
             so a dependency declared against a superclass still matches"
        );
        assert!(
            !fired.iter().any(|t| matches!(
                t,
                Term::Index {
                    prop,
                    ..
                } if prop.as_str() == "employer"
            )),
            "a known link property must not fire value-index terms"
        );
    }

    #[test]
    fn set_valued_link_fires_backlinks_for_both_sides() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Company/acme"),
            cls("Company"),
            json!({"departments": ["Department/a", "Department/b"]}),
            json!({"departments": ["Department/b", "Department/c"]}),
        )]);
        let fired = fire(&cs, &schema);
        for target in ["Department/a", "Department/b", "Department/c"] {
            assert!(
                fired.contains(&Term::Backlink {
                    target: doc(target),
                    prop: prop("departments"),
                    from_class: cls("Company")
                }),
                "every target on either side fires: {target}"
            );
        }
    }

    #[test]
    fn unknown_prop_kind_fires_both_interpretations() {
        let schema = schema_fixture();
        // `ghost`'s range is never declared, so its kind is unknown.
        let cs = changeset(vec![DocChange::updated(
            doc("Department/d1"),
            cls("Department"),
            json!({"ghost": "Person/alice"}),
            json!({"ghost": "scalar"}),
        )]);
        let fired = fire(&cs, &schema);

        assert!(fired.contains(&Term::Edge(doc("Department/d1"), prop("ghost"))));
        assert!(
            fired.contains(&Term::Backlink {
                target: doc("Person/alice"),
                prop: prop("ghost"),
                from_class: cls("Department")
            }),
            "the id-shaped value is treated as a possible edge"
        );
        assert!(
            fired.contains(&Term::Index {
                class: cls("Department"),
                prop: prop("ghost"),
                value: vk(json!("scalar"))
            }),
            "and the same change is also treated as a possible value"
        );
    }

    #[test]
    fn with_no_schema_everything_degrades_to_the_documents_own_class() {
        let schema = SchemaIndex::default();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"employer": "Company/old", "name": "A"}),
            json!({"employer": "Company/new", "name": "B"}),
        )]);
        let fired = fire(&cs, &schema);

        assert!(fired.contains(&Term::Class(cls("Person"))));
        assert!(fired.contains(&Term::ClassField(cls("Person"), prop("name"))));
        assert!(
            !fired.contains(&Term::ClassField(cls("Agent"), prop("name"))),
            "with no schema we do not know about Agent at all"
        );
        // Both interpretations, for both properties.
        assert!(fired.contains(&Term::Edge(doc("Person/alice"), prop("employer"))));
        assert!(fired.contains(&Term::Backlink {
            target: doc("Company/old"),
            prop: prop("employer"),
            from_class: cls("Person")
        }));
        assert!(fired.contains(&Term::Index {
            class: cls("Person"),
            prop: prop("name"),
            value: vk(json!("B"))
        }));
    }

    #[test]
    fn type_change_fires_the_type_field() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"@type": "Person", "name": "A"}),
            json!({"@type": "Employee", "name": "A"}),
        )]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::DocField(doc("Person/alice"), prop("@type"))));
    }

    #[test]
    fn subdocument_link_still_fires_an_edge() {
        let schema = schema_fixture();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"address": {"@id": "Person/alice/address/Address/1", "city": "Berlin"}}),
            json!({"address": {"@id": "Person/alice/address/Address/2", "city": "Paris"}}),
        )]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::Edge(doc("Person/alice"), prop("address"))));
        assert!(fired.contains(&Term::Backlink {
            target: doc("Person/alice/address/Address/2"),
            prop: prop("address"),
            from_class: cls("Person")
        }));
    }

    #[test]
    fn multiple_changes_accumulate() {
        let schema = schema_fixture();
        let cs = changeset(vec![
            DocChange::created(doc("Person/alice"), cls("Person"), json!({"name": "A"})),
            DocChange::deleted(doc("Company/acme"), cls("Company"), json!({"name": "Acme"})),
        ]);
        let fired = fire(&cs, &schema);
        assert!(fired.contains(&Term::Doc(doc("Person/alice"))));
        assert!(fired.contains(&Term::Doc(doc("Company/acme"))));
        assert!(fired.contains(&Term::Members(cls("Person"))));
        assert!(fired.contains(&Term::Members(cls("Company"))));
        assert!(fired.contains(&Term::Members(cls("Agent"))));
    }

    #[test]
    fn deeply_nested_values_do_not_recurse_without_bound() {
        let schema = schema_fixture();
        let mut nested = json!("leaf");
        for _ in 0..512 {
            nested = json!([nested]);
        }
        let cs = changeset(vec![DocChange::created(
            doc("Department/d1"),
            cls("Department"),
            json!({"ghost": nested}),
        )]);
        // The point is that this returns at all.
        assert!(!fire(&cs, &schema).is_empty());
    }
}

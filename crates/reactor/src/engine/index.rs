//! The inverted index: fired terms → the fragments they dirty.
//!
//! Without this, applying a commit would mean testing every fired term against
//! every cached fragment's dependency list — O(fragments) per commit, which is
//! the thing that killed the prototype. Instead every dependency is filed under
//! the coarse [`TermBucket`]s it could possibly interact with, and a fired term
//! is looked up under the buckets that could hold a dependency subsuming it.
//!
//! The index is *not* the arbiter of dirtiness. Bucket overlap only establishes
//! "these two terms might interact"; [`Term::subsumes`] decides. The index's
//! obligation is purely one-sided and absolute: **it must never prune away a
//! pair that does subsume**. `term.rs` asserts exactly that property over a
//! cross product of terms.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ids::Position;
use crate::schema_index::SchemaIndex;
use crate::term::{Term, TermBucket, TermSet};

use super::fragment::FragmentKey;

/// Identifies one registration in the index.
///
/// A dense integer rather than the key itself, because a key is cloned into
/// every bucket it occupies and keys are not small.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(u64);

/// One registered fragment version and the dependencies it was filled with.
#[derive(Clone, Debug)]
pub struct IndexEntry {
    /// This registration's id.
    pub id: EntryId,
    /// The fragment it belongs to.
    pub key: FragmentKey,
    /// The `from` position of the specific version registered, so the writer
    /// can close exactly that version and not a newer one.
    pub from: Position,
    /// The read set.
    pub deps: Vec<Term>,
}

/// `TermBucket` → registered entries.
///
/// Only the *current* (open) version of a key is registered: a version with a
/// known end position is already bounded and cannot be invalidated again.
#[derive(Debug, Default)]
pub struct TermIndex {
    entries: HashMap<EntryId, IndexEntry>,
    buckets: HashMap<TermBucket, BTreeSet<EntryId>>,
    by_key: HashMap<FragmentKey, EntryId>,
    next_id: u64,
}

impl TermIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a version's dependencies.
    ///
    /// Replaces any existing registration for the same key: a key has at most
    /// one invalidatable version, the current one.
    ///
    /// A dependency list that is empty registers nothing — which is correct for
    /// an immutable (commit-pinned) fragment: nothing can ever invalidate it.
    pub fn register(&mut self, key: FragmentKey, from: Position, deps: Vec<Term>) -> EntryId {
        self.unregister_key(&key);
        let id = EntryId(self.next_id);
        self.next_id += 1;
        for bucket in deps.iter().flat_map(Term::buckets) {
            self.buckets.entry(bucket).or_default().insert(id);
        }
        self.by_key.insert(key.clone(), id);
        self.entries.insert(
            id,
            IndexEntry {
                id,
                key,
                from,
                deps,
            },
        );
        id
    }

    /// Remove a registration.
    pub fn unregister(&mut self, id: EntryId) -> Option<IndexEntry> {
        let entry = self.entries.remove(&id)?;
        for bucket in entry.deps.iter().flat_map(Term::buckets) {
            if let Some(set) = self.buckets.get_mut(&bucket) {
                set.remove(&id);
                if set.is_empty() {
                    self.buckets.remove(&bucket);
                }
            }
        }
        if self.by_key.get(&entry.key) == Some(&id) {
            self.by_key.remove(&entry.key);
        }
        Some(entry)
    }

    /// Remove the registration for a key, if any.
    pub fn unregister_key(&mut self, key: &FragmentKey) -> Option<IndexEntry> {
        let id = *self.by_key.get(key)?;
        self.unregister(id)
    }

    /// Forget everything. Used when a history rewrite leaves us unable to say
    /// what is still valid.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.buckets.clear();
        self.by_key.clear();
    }

    /// Registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up one entry.
    pub fn entry(&self, id: EntryId) -> Option<&IndexEntry> {
        self.entries.get(&id)
    }

    /// The registration for a key, if it has a current version.
    pub fn entry_for_key(&self, key: &FragmentKey) -> Option<&IndexEntry> {
        self.entries.get(self.by_key.get(key)?)
    }

    /// Entries whose buckets overlap the fired set: the candidates that must be
    /// subsumption-tested.
    ///
    /// A fired [`Term::Everything`] short-circuits to *every* entry, because no
    /// finite bucket enumeration can express "all dependencies".
    pub fn candidates(&self, fired: &TermSet, schema: &SchemaIndex) -> Vec<EntryId> {
        if fired.contains(&Term::Everything) {
            let mut all: Vec<EntryId> = self.entries.keys().copied().collect();
            all.sort_unstable();
            return all;
        }
        let mut seen: HashSet<EntryId> = HashSet::new();
        let mut out = Vec::new();
        for term in fired {
            for bucket in term.lookup_buckets(schema) {
                if let Some(ids) = self.buckets.get(&bucket) {
                    for id in ids {
                        if seen.insert(*id) {
                            out.push(*id);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The entries actually dirtied by the fired set: candidates that pass the
    /// subsumption test.
    ///
    /// Returns `(id, key, from)` rather than borrowed entries so the caller can
    /// mutate the index while acting on the result.
    pub fn dirtied(
        &self,
        fired: &TermSet,
        schema: &SchemaIndex,
    ) -> Vec<(EntryId, FragmentKey, Position)> {
        self.candidates(fired, schema)
            .into_iter()
            .filter_map(|id| {
                let entry = self.entries.get(&id)?;
                let hit = entry
                    .deps
                    .iter()
                    .any(|dep| fired.iter().any(|f| dep.subsumes(f, schema)));
                hit.then(|| (id, entry.key.clone(), entry.from))
            })
            .collect()
    }

    /// Every registration, for `/_reactor/explain` and tests.
    pub fn iter(&self) -> impl Iterator<Item = &IndexEntry> {
        self.entries.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ContentHash, Epoch, Namespace, Position, Seq, ShapeId};
    use crate::testutil::{cls, doc, prop, schema_fixture};

    fn pos(seq: u64) -> Position {
        Position {
            epoch: Epoch(0),
            seq: Seq(seq),
        }
    }

    fn key(name: &str) -> FragmentKey {
        FragmentKey::request(
            Namespace::new("admin", "db", "main"),
            ContentHash::of(name.as_bytes()),
            ShapeId::of(b"shape"),
        )
    }

    fn fired(terms: impl IntoIterator<Item = Term>) -> TermSet {
        terms.into_iter().collect()
    }

    #[test]
    fn registers_and_finds_an_exact_match() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        let k = key("alice");
        index.register(k.clone(), pos(1), vec![Term::Doc(doc("Person/alice"))]);

        let hits = index.dirtied(&fired([Term::Doc(doc("Person/alice"))]), &schema);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, k);
        assert_eq!(hits[0].2, pos(1));
    }

    #[test]
    fn prunes_unrelated_documents_without_a_subsumption_test() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(key("alice"), pos(1), vec![Term::Doc(doc("Person/alice"))]);

        let f = fired([Term::Doc(doc("Person/bob"))]);
        assert!(
            index.candidates(&f, &schema).is_empty(),
            "a different document shares no bucket, so it is not even a candidate"
        );
        assert!(index.dirtied(&f, &schema).is_empty());
    }

    #[test]
    fn finds_a_coarse_class_dependency_from_a_subclass_change() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(key("all people"), pos(1), vec![Term::Class(cls("Person"))]);

        let hits = index.dirtied(
            &fired([Term::DocField(doc("Employee/e1"), prop("name"))]),
            &schema,
        );
        assert_eq!(hits.len(), 1, "an Employee change dirties a Person listing");
    }

    #[test]
    fn candidate_generation_can_overshoot_but_dirtied_does_not() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(
            key("alice name"),
            pos(1),
            vec![Term::DocField(doc("Person/alice"), prop("name"))],
        );

        // Same document, different property: same bucket, so it is a candidate,
        // but it must not be reported as dirty.
        let f = fired([Term::DocField(doc("Person/alice"), prop("employer"))]);
        assert_eq!(index.candidates(&f, &schema).len(), 1);
        assert!(index.dirtied(&f, &schema).is_empty());
    }

    #[test]
    fn a_fired_everything_dirties_every_entry() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(key("a"), pos(1), vec![Term::Doc(doc("Person/alice"))]);
        index.register(key("b"), pos(1), vec![Term::Schema]);
        index.register(key("c"), pos(1), vec![Term::Members(cls("Company"))]);

        let f = fired([Term::Everything]);
        assert_eq!(index.candidates(&f, &schema).len(), 3);
        assert_eq!(index.dirtied(&f, &schema).len(), 3);
    }

    #[test]
    fn an_empty_dependency_list_can_never_be_dirtied() {
        // This is the commit-pinned/immutable case.
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(key("pinned"), pos(1), vec![]);
        assert_eq!(index.len(), 1);
        for f in [
            fired([Term::Everything]),
            fired([Term::Schema]),
            fired([Term::Doc(doc("Person/alice"))]),
        ] {
            // `Everything` reaches it as a candidate but it holds no dependency
            // that could subsume anything.
            assert!(index.dirtied(&f, &schema).is_empty());
        }
    }

    #[test]
    fn re_registering_a_key_replaces_its_previous_entry() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        let k = key("same");
        index.register(k.clone(), pos(1), vec![Term::Doc(doc("Person/alice"))]);
        index.register(k.clone(), pos(2), vec![Term::Doc(doc("Person/bob"))]);

        assert_eq!(index.len(), 1);
        assert!(index
            .dirtied(&fired([Term::Doc(doc("Person/alice"))]), &schema)
            .is_empty());
        let hits = index.dirtied(&fired([Term::Doc(doc("Person/bob"))]), &schema);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].2,
            pos(2),
            "the newer version's position is reported"
        );
    }

    #[test]
    fn unregistering_removes_it_from_every_bucket() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        let k = key("multi");
        let id = index.register(
            k.clone(),
            pos(1),
            vec![
                Term::Doc(doc("Person/alice")),
                Term::Members(cls("Person")),
                Term::Schema,
            ],
        );
        assert!(index.unregister(id).is_some());
        assert!(index.is_empty());
        assert!(index.buckets.is_empty(), "buckets must not leak");
        assert!(index.entry_for_key(&k).is_none());
        for f in [
            fired([Term::Doc(doc("Person/alice"))]),
            fired([Term::Members(cls("Person"))]),
            fired([Term::Schema]),
            fired([Term::Everything]),
        ] {
            assert!(index.dirtied(&f, &schema).is_empty());
        }
        assert!(
            index.unregister(id).is_none(),
            "double unregister is a no-op"
        );
    }

    #[test]
    fn clear_forgets_everything() {
        let mut index = TermIndex::new();
        index.register(key("a"), pos(1), vec![Term::Doc(doc("Person/alice"))]);
        index.clear();
        assert!(index.is_empty());
        assert!(index.buckets.is_empty());
        assert!(index.by_key.is_empty());
    }

    #[test]
    fn one_commit_can_dirty_many_entries_once_each() {
        let schema = schema_fixture();
        let mut index = TermIndex::new();
        index.register(key("a"), pos(1), vec![Term::Doc(doc("Person/alice"))]);
        index.register(key("b"), pos(1), vec![Term::Class(cls("Person"))]);
        index.register(
            key("c"),
            pos(1),
            vec![
                // Two dependencies that both match: the entry must be reported
                // exactly once.
                Term::Doc(doc("Person/alice")),
                Term::Class(cls("Agent")),
            ],
        );
        index.register(key("d"), pos(1), vec![Term::Members(cls("Company"))]);

        let hits = index.dirtied(
            &fired([
                Term::Doc(doc("Person/alice")),
                Term::DocField(doc("Person/alice"), prop("name")),
                Term::Class(cls("Person")),
            ]),
            &schema,
        );
        let mut names: Vec<ContentHash> = hits
            .iter()
            .map(|(_, k, _)| match &k.root {
                super::super::fragment::RootKey::Request(h) => *h,
                _ => unreachable!(),
            })
            .collect();
        names.sort();
        let mut expected = vec![
            ContentHash::of(b"a"),
            ContentHash::of(b"b"),
            ContentHash::of(b"c"),
        ];
        expected.sort();
        assert_eq!(names, expected);
    }
}

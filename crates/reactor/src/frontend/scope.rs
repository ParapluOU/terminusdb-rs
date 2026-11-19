//! Recording scopes: how a derived value's dependencies get **observed**
//! instead of declared.
//!
//! An application that computes something from TerminusDB data has to tell
//! Reactor what the result depends on, or the result cannot be invalidated. The
//! obvious way is to ask it to declare — "this depends on `Person`" — which is
//! what the prototype this project replaces did, and a wrong declaration is the
//! one way to get stale data out of an otherwise sound cache.
//!
//! But the computation's reads go *through* Reactor, and Reactor already
//! computes a precise read set for every request it serves. So the dependency
//! set can be watched rather than asserted. A client labels its reads with a
//! scope id (`X-Reactor-Record: <id>`), Reactor unions the read sets of every
//! labelled request into that scope, and a later store names the scope instead
//! of listing terms.
//!
//! # Why accumulate here rather than in the client
//!
//! Because a client reads through `terminusdb-client` and the ORM, which return
//! deserialized values and not HTTP responses. Asking an application to thread
//! header-collection through every read is asking it to get it right on every
//! path, and the failure mode of missing one is a value with under-reported
//! dependencies — stale, silently, forever. The server sees every read whether
//! the client bothers to look at it or not.
//!
//! # What makes a scope refuse
//!
//! Three things, each of which would otherwise produce a confidently wrong
//! entry:
//!
//! * **An untracked read.** A bypass, a forwarded write, a response too large
//!   to cache: Reactor did not derive a read set, so a value computed from it
//!   has inputs nobody is watching. The scope is poisoned and stays poisoned.
//! * **A read at a different commit.** Every read in a scope must be pinned at
//!   the *same* commit, or the value is a composite of several snapshots — true
//!   at no single point in the branch's history, and therefore not something a
//!   validity interval can describe. An unpinned read is the same problem: it
//!   answers "as of whenever", which is not a commit we can name.
//! * **Nothing at all.** An empty scope means no read was recorded. That is
//!   either a computation that touched no data (which has to say so explicitly
//!   — see `nothing` in [`super::value`]) or, far more likely, a client whose
//!   reads did not carry the label.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::ids::Namespace;
use crate::term::Term;
use terminusdb_client::CommitId;

/// The request header that labels a read as belonging to a scope.
pub const SCOPE_HEADER: &str = "x-reactor-record";

/// How long an unfinished scope is kept.
///
/// A scope lives for one computation, so this is a leak bound and not a
/// feature: a client that crashes between its reads and its store leaves one
/// behind, and nothing else would ever clean it up.
const SCOPE_TTL: Duration = Duration::from_secs(300);

/// What one scope has observed.
#[derive(Clone, Debug)]
pub struct ScopeRecord {
    /// The namespace the reads were against. A value may only be stored in the
    /// namespace its inputs came from.
    pub ns: Option<Namespace>,
    /// The union of the read sets, ordered precise → coarse by `Term`'s `Ord`.
    pub terms: BTreeSet<Term>,
    /// The commits the recorded reads were pinned at. More than one means the
    /// computation saw more than one snapshot.
    pub commits: BTreeSet<String>,
    /// Set when a read could not be attributed: see the module docs.
    pub poisoned: Option<&'static str>,
    /// How many reads were recorded, for the diagnostic endpoint.
    pub reads: usize,
    created: Instant,
}

impl ScopeRecord {
    fn new() -> Self {
        Self {
            ns: None,
            terms: BTreeSet::new(),
            commits: BTreeSet::new(),
            poisoned: None,
            reads: 0,
            created: Instant::now(),
        }
    }

    /// The reason this scope cannot back a stored value, if there is one.
    ///
    /// Stated as a sentence because it is handed to an application developer in
    /// an HTTP response, and "409" on its own has never helped anybody.
    pub fn refusal(&self, at: &CommitId) -> Option<String> {
        if let Some(reason) = self.poisoned {
            return Some(format!(
                "a read in this computation was not tracked ({reason}), so a value derived from \
                 it has inputs nothing is watching"
            ));
        }
        if self.reads == 0 {
            return Some(
                "no read was recorded in this scope: either the computation read nothing — say \
                 so with `X-Reactor-Deps: nothing` — or its reads did not carry the \
                 X-Reactor-Record header"
                    .to_owned(),
            );
        }
        if !self.commits.contains(at.as_str()) || self.commits.len() > 1 {
            let seen: Vec<&str> = self.commits.iter().map(String::as_str).collect();
            return Some(format!(
                "the reads were pinned at {:?} but the value claims to be at {at}: a value \
                 computed across several snapshots is true at none of them",
                seen
            ));
        }
        None
    }
}

/// Every open scope on this node.
#[derive(Debug)]
pub struct Recorder {
    scopes: DashMap<String, ScopeRecord>,
    /// Bound on how many scopes are tracked at once. Over it, a new scope is
    /// *not* created, so the store that follows is refused — the fail-safe
    /// direction, since the alternative is unbounded memory.
    cap: usize,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl Recorder {
    /// A recorder bounded to `cap` concurrent scopes.
    pub fn new(cap: usize) -> Self {
        Self {
            scopes: DashMap::new(),
            cap,
        }
    }

    /// Note a tracked read.
    ///
    /// `pinned` is the commit the read was issued against, `None` for a read of
    /// the live branch — which poisons the scope, because "as of whenever" is
    /// not a snapshot.
    pub fn record(&self, id: &str, ns: &Namespace, pinned: Option<&CommitId>, terms: &[Term]) {
        self.sweep();
        let Some(mut record) = self.entry(id) else {
            return;
        };
        record.reads += 1;
        match &record.ns {
            None => record.ns = Some(ns.clone()),
            Some(seen) if seen != ns => {
                record.poisoned = Some("it read a different namespace");
            }
            Some(_) => {}
        }
        match pinned {
            Some(commit) => {
                record.commits.insert(commit.as_str().to_owned());
            }
            None => record.poisoned = Some("it was not pinned at a commit"),
        }
        for term in terms {
            if matches!(term, Term::Everything) {
                record.poisoned = Some("its dependencies could not be narrowed");
            }
            record.terms.insert(term.clone());
        }
    }

    /// Note a read that could not be attributed at all.
    ///
    /// Takes no namespace and no terms on purpose: there is nothing to record,
    /// only something to remember about every value this scope might produce.
    pub fn poison(&self, id: &str, reason: &'static str) {
        self.sweep();
        if let Some(mut record) = self.entry(id) {
            record.reads += 1;
            record.poisoned = Some(reason);
        }
    }

    /// Read a scope without consuming it.
    pub fn peek(&self, id: &str) -> Option<ScopeRecord> {
        self.scopes.get(id).map(|entry| entry.clone())
    }

    /// Take a scope, so one scope backs one stored value.
    ///
    /// Consuming rather than copying is what stops a client from recording a
    /// handful of reads and then storing a dozen values against them, each
    /// claiming dependencies it does not have.
    pub fn take(&self, id: &str) -> Option<ScopeRecord> {
        self.scopes.remove(id).map(|(_, record)| record)
    }

    /// Drop everything. Used by `/_reactor/invalidate`.
    pub fn clear(&self) {
        self.scopes.clear();
    }

    /// How many scopes are open.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Whether no scope is open.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    fn entry(&self, id: &str) -> Option<dashmap::mapref::one::RefMut<'_, String, ScopeRecord>> {
        if !self.scopes.contains_key(id) && self.scopes.len() >= self.cap {
            tracing::warn!(
                open = self.scopes.len(),
                "recording scope cap reached; this computation will not be cached"
            );
            return None;
        }
        Some(
            self.scopes
                .entry(id.to_owned())
                .or_insert_with(ScopeRecord::new),
        )
    }

    fn sweep(&self) {
        let now = Instant::now();
        self.scopes
            .retain(|_, record| now.duration_since(record.created) < SCOPE_TTL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ClassName, DocId};

    fn ns() -> Namespace {
        Namespace::new("admin", "people", "main")
    }

    fn commit(id: &str) -> CommitId {
        CommitId::new(id)
    }

    #[test]
    fn a_scope_unions_the_read_sets_it_is_told_about() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[
                Term::Doc(DocId::parse("Person/alice")),
                Term::Members(ClassName::new("Person")),
            ],
        );
        let record = recorder.peek("s1").expect("the scope exists");
        assert_eq!(record.reads, 2);
        assert_eq!(record.terms.len(), 2, "the duplicate is one term, not two");
        assert_eq!(record.refusal(&commit("c1")), None);
    }

    #[test]
    fn an_untracked_read_poisons_the_scope_for_good() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        recorder.poison("s1", "it bypassed the cache");
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Doc(DocId::parse("Person/alice"))],
        );
        let refusal = recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c1"))
            .expect("poisoned");
        assert!(refusal.contains("not tracked"), "{refusal}");
        assert!(refusal.contains("bypassed"), "{refusal}");
    }

    #[test]
    fn everything_is_not_a_dependency_set_worth_having() {
        let recorder = Recorder::default();
        recorder.record("s1", &ns(), Some(&commit("c1")), &[Term::Everything]);
        assert!(recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c1"))
            .is_some());
    }

    #[test]
    fn an_unpinned_read_is_not_a_snapshot() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            None,
            &[Term::Members(ClassName::new("Person"))],
        );
        let refusal = recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c1"))
            .expect("unpinned");
        assert!(refusal.contains("not pinned"), "{refusal}");
    }

    #[test]
    fn two_snapshots_are_one_too_many() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c2")),
            &[Term::Members(ClassName::new("Company"))],
        );
        let refusal = recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c1"))
            .expect("two commits");
        assert!(refusal.contains("several snapshots"), "{refusal}");
    }

    #[test]
    fn a_value_may_not_claim_a_commit_its_reads_did_not_use() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        assert!(recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c9"))
            .is_some());
    }

    #[test]
    fn reading_a_second_namespace_poisons_it() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        recorder.record(
            "s1",
            &Namespace::new("admin", "other", "main"),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        let refusal = recorder
            .peek("s1")
            .unwrap()
            .refusal(&commit("c1"))
            .expect("two namespaces");
        assert!(refusal.contains("different namespace"), "{refusal}");
    }

    #[test]
    fn an_empty_scope_refuses_rather_than_claims_independence() {
        let recorder = Recorder::default();
        recorder.record("s1", &ns(), Some(&commit("c1")), &[]);
        // A read with no terms at all: it happened, so the scope is not empty,
        // but it contributes nothing.
        assert_eq!(recorder.peek("s1").unwrap().refusal(&commit("c1")), None);

        let empty = Recorder::default();
        empty.poison("s2", "nothing was recorded");
        assert!(empty.peek("s2").unwrap().refusal(&commit("c1")).is_some());
        assert!(
            empty.peek("s3").is_none(),
            "an unknown scope is not a scope"
        );
    }

    #[test]
    fn a_scope_backs_exactly_one_value() {
        let recorder = Recorder::default();
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        assert!(recorder.take("s1").is_some());
        assert!(
            recorder.take("s1").is_none(),
            "a taken scope cannot back a second value"
        );
    }

    #[test]
    fn the_cap_refuses_new_scopes_rather_than_growing() {
        let recorder = Recorder::new(1);
        recorder.record(
            "s1",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        recorder.record(
            "s2",
            &ns(),
            Some(&commit("c1")),
            &[Term::Members(ClassName::new("Person"))],
        );
        assert_eq!(recorder.len(), 1);
        assert!(
            recorder.peek("s2").is_none(),
            "over the cap, a scope is not created — so the store is refused"
        );
    }
}

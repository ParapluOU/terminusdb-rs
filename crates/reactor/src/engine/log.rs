//! The fired log: a bounded ring of what each recent commit invalidated.
//!
//! This exists for exactly one reason, and it is the subtlest correctness
//! requirement in the engine. A fill is issued *pinned at a position* `P` and
//! completes some time later, by which point commits may have landed. The fill's
//! bytes are not wrong — they are genuinely the value at `P` — but they are not
//! necessarily the value *now*. So before storing a completed fill as current,
//! the writer asks this log: did anything between `P` and now invalidate these
//! dependencies? If yes, the version is stored with a bounded validity interval
//! and is *not* registered as current.
//!
//! Without the log that race is unsolvable and the usual answer is to throw
//! races away and hope they are rare. With it, the answer is exact.
//!
//! The ring is bounded, so the log eventually forgets. Forgetting is safe only
//! if we *know* we have forgotten, hence [`FiredLog::oldest_retained`]: a fill
//! pinned below that position cannot be validated and must be discarded.

use std::collections::VecDeque;

use crate::ids::Position;
use crate::schema_index::SchemaIndex;
use crate::term::{Term, TermSet};
use terminusdb_client::CommitId;

/// One applied commit and what it fired.
#[derive(Clone, Debug)]
pub struct FiredEntry {
    /// The Reactor-local position assigned to the commit.
    pub pos: Position,
    /// The commit id, for explain output. Never compared for order.
    pub commit: CommitId,
    /// The terms it fired.
    pub fired: TermSet,
}

/// A bounded, position-ordered history of fired term sets.
#[derive(Debug)]
pub struct FiredLog {
    entries: VecDeque<FiredEntry>,
    capacity: usize,
    /// Lower bound on what we still know about, used when the ring is empty.
    floor: Position,
}

impl FiredLog {
    /// A log retaining at most `capacity` commits.
    ///
    /// Capacity is a direct trade of memory against how long a slow fill may
    /// take before it has to be thrown away. It should comfortably exceed
    /// `commit rate × worst-case fill latency`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
            floor: Position::ZERO,
        }
    }

    /// Record a commit's fired terms, evicting the oldest if full.
    pub fn push(&mut self, pos: Position, commit: CommitId, fired: TermSet) {
        self.entries.push_back(FiredEntry { pos, commit, fired });
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
            // Everything below the new front is now unknown.
            if let Some(front) = self.entries.front() {
                self.floor = front.pos;
            }
        }
    }

    /// The oldest position we still have complete fire information for.
    ///
    /// A fill pinned below this cannot be validated — we no longer know
    /// everything that happened after it — so it must be discarded rather than
    /// trusted.
    ///
    /// This is a *floor*, not the oldest entry's position: an empty or sparse
    /// log is not the same thing as a forgetful one. A fresh log has lost
    /// nothing, so its floor is [`Position::ZERO`] and every pin is
    /// validatable; a log that was cleared knows everything from the clear
    /// position onwards, even after only one commit has landed since. Only
    /// eviction actually raises it.
    pub fn oldest_retained(&self) -> Position {
        self.floor
    }

    /// Entries strictly after `pos`, oldest first.
    pub fn after(&self, pos: Position) -> impl Iterator<Item = &FiredEntry> {
        self.entries.iter().filter(move |entry| entry.pos > pos)
    }

    /// The earliest position after `pin` at which something fired a term that
    /// `deps` depends on — the `K` of the insert protocol.
    ///
    /// `None` means nothing invalidated these dependencies, so a fill pinned at
    /// `pin` is still current.
    ///
    /// Matching here is [`Term::subsumes_pessimistically`], not
    /// [`Term::subsumes`]. The difference is confined to
    /// [`Term::Predicate`] and it is load-bearing: a predicate fire is only
    /// emitted for predicates the writer had *registered* when the commit was
    /// applied, and this fill's predicate was by definition not registered
    /// while the fill was in flight. So for the replay — and only for the
    /// replay — a predicate dependency is treated as dirtied by the coarse
    /// footprint of a change on its class. The cost is an occasional needless
    /// refill; the alternative is a fragment that is stale forever because the
    /// commit that invalidated it could not name it yet.
    pub fn first_invalidating_after(
        &self,
        pin: Position,
        deps: &[Term],
        schema: &SchemaIndex,
    ) -> Option<Position> {
        if deps.is_empty() {
            // Nothing to invalidate: an immutable fragment.
            return None;
        }
        self.after(pin)
            .find(|entry| {
                entry.fired.iter().any(|f| {
                    deps.iter()
                        .any(|dep| dep.subsumes_pessimistically(f, schema))
                })
            })
            .map(|entry| entry.pos)
    }

    /// Forget everything, declaring `floor` the new lower bound of knowledge.
    ///
    /// Called when a history rewrite makes the old log meaningless. Setting the
    /// floor matters: it is what makes in-flight fills pinned in the abandoned
    /// history get rejected rather than silently accepted.
    pub fn clear(&mut self, floor: Position) {
        self.entries.clear();
        self.floor = floor;
    }

    /// Number of retained commits.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no commits are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The retained entries, newest last. For `/_reactor/explain`.
    pub fn iter(&self) -> impl Iterator<Item = &FiredEntry> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{Epoch, Seq};
    use crate::testutil::{cls, doc, prop, schema_fixture};

    fn pos(seq: u64) -> Position {
        Position {
            epoch: Epoch(0),
            seq: Seq(seq),
        }
    }

    fn set(terms: impl IntoIterator<Item = Term>) -> TermSet {
        terms.into_iter().collect()
    }

    #[test]
    fn a_fresh_log_knows_everything_from_zero() {
        let log = FiredLog::with_capacity(4);
        assert_eq!(log.oldest_retained(), Position::ZERO);
        assert!(log.is_empty());
    }

    #[test]
    fn finds_the_first_invalidating_commit_after_the_pin() {
        let schema = schema_fixture();
        let mut log = FiredLog::with_capacity(16);
        log.push(
            pos(1),
            CommitId::new("c1"),
            set([Term::Doc(doc("Person/bob"))]),
        );
        log.push(
            pos(2),
            CommitId::new("c2"),
            set([Term::Doc(doc("Person/alice"))]),
        );
        log.push(
            pos(3),
            CommitId::new("c3"),
            set([Term::Doc(doc("Person/alice"))]),
        );

        let deps = vec![Term::Doc(doc("Person/alice"))];
        assert_eq!(
            log.first_invalidating_after(pos(0), &deps, &schema),
            Some(pos(2)),
            "the earliest matching commit after the pin"
        );
        assert_eq!(
            log.first_invalidating_after(pos(2), &deps, &schema),
            Some(pos(3)),
            "strictly after the pin: the commit at the pin is already reflected"
        );
        assert_eq!(log.first_invalidating_after(pos(3), &deps, &schema), None);
    }

    #[test]
    fn unrelated_commits_do_not_invalidate() {
        let schema = schema_fixture();
        let mut log = FiredLog::with_capacity(16);
        log.push(
            pos(1),
            CommitId::new("c1"),
            set([
                Term::Doc(doc("Company/acme")),
                Term::Members(cls("Company")),
            ]),
        );
        let deps = vec![Term::DocField(doc("Person/alice"), prop("name"))];
        assert_eq!(log.first_invalidating_after(pos(0), &deps, &schema), None);
    }

    #[test]
    fn subsumption_not_equality_decides() {
        let schema = schema_fixture();
        let mut log = FiredLog::with_capacity(16);
        log.push(
            pos(1),
            CommitId::new("c1"),
            set([Term::DocField(doc("Employee/e1"), prop("name"))]),
        );
        // A coarse class dependency is invalidated by a precise subclass fire.
        let deps = vec![Term::Class(cls("Person"))];
        assert_eq!(
            log.first_invalidating_after(pos(0), &deps, &schema),
            Some(pos(1))
        );
    }

    #[test]
    fn an_empty_dependency_list_is_never_invalidated() {
        let schema = schema_fixture();
        let mut log = FiredLog::with_capacity(16);
        log.push(pos(1), CommitId::new("c1"), set([Term::Everything]));
        assert_eq!(
            log.first_invalidating_after(pos(0), &[], &schema),
            None,
            "a commit-pinned fragment depends on nothing and survives anything"
        );
    }

    #[test]
    fn the_ring_evicts_and_raises_the_floor() {
        let schema = schema_fixture();
        let mut log = FiredLog::with_capacity(2);
        log.push(
            pos(1),
            CommitId::new("c1"),
            set([Term::Doc(doc("Person/alice"))]),
        );
        log.push(
            pos(2),
            CommitId::new("c2"),
            set([Term::Doc(doc("Person/bob"))]),
        );
        assert_eq!(
            log.oldest_retained(),
            Position::ZERO,
            "nothing has been evicted yet, so nothing is unknown"
        );

        log.push(
            pos(3),
            CommitId::new("c3"),
            set([Term::Doc(doc("Person/carol"))]),
        );
        assert_eq!(log.len(), 2);
        assert_eq!(
            log.oldest_retained(),
            pos(2),
            "we no longer know what commit 1 did"
        );
        // And the forgotten commit's fire is simply not found any more, which is
        // why the writer must reject fills pinned below `oldest_retained`.
        let deps = vec![Term::Doc(doc("Person/alice"))];
        assert_eq!(log.first_invalidating_after(pos(0), &deps, &schema), None);
    }

    #[test]
    fn a_cleared_log_knows_everything_since_the_clear() {
        // The regression this guards: reporting the oldest *entry* instead of
        // the floor made a pin at the clear position unvalidatable as soon as
        // one commit landed after it, so every fill issued immediately after a
        // purge or rebase was thrown away.
        let mut log = FiredLog::with_capacity(8);
        let cleared_at = Position {
            epoch: Epoch(2),
            seq: Seq(0),
        };
        log.clear(cleared_at);
        assert_eq!(log.oldest_retained(), cleared_at);

        log.push(
            Position {
                epoch: Epoch(2),
                seq: Seq(1),
            },
            CommitId::new("after"),
            set([Term::Schema]),
        );
        assert_eq!(
            log.oldest_retained(),
            cleared_at,
            "a fill pinned at the clear position is still validatable"
        );
    }

    #[test]
    fn clearing_sets_a_floor_so_stale_pins_are_rejected() {
        let mut log = FiredLog::with_capacity(4);
        log.push(pos(1), CommitId::new("c1"), set([Term::Everything]));
        let new_epoch = Position {
            epoch: Epoch(1),
            seq: Seq(0),
        };
        log.clear(new_epoch);
        assert!(log.is_empty());
        assert_eq!(log.oldest_retained(), new_epoch);
    }

    #[test]
    fn after_is_strict_and_ordered() {
        let mut log = FiredLog::with_capacity(8);
        for i in 1..=4 {
            log.push(pos(i), CommitId::new(format!("c{i}")), set([Term::Schema]));
        }
        let seen: Vec<Position> = log.after(pos(2)).map(|e| e.pos).collect();
        assert_eq!(seen, vec![pos(3), pos(4)]);
        assert_eq!(log.iter().count(), 4);
    }
}

//! Fragment keys, versions and slots: what a cache entry *is*.

use std::time::Instant;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::ids::{ContentHash, DerivedKey, DocId, Namespace, Position, ShapeId};
use crate::term::Term;
use terminusdb_client::CommitId;

/// What a cached entry is keyed by.
///
/// The key deliberately contains no commit and no position: a key names a
/// *question*, and [`FragmentVersion`] names the answers it has had over time.
/// That is what lets one key accumulate a validity history and lets backdating
/// recognize an unchanged answer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FragmentKey {
    /// The namespace the question is asked of.
    pub ns: Namespace,
    /// What the entry is about.
    pub root: RootKey,
    /// Which projection of it.
    pub shape: ShapeId,
}

impl FragmentKey {
    /// A whole-response entry.
    pub fn request(ns: Namespace, hash: ContentHash, shape: ShapeId) -> Self {
        Self {
            ns,
            root: RootKey::Request(hash),
            shape,
        }
    }

    /// A single document's projection.
    pub fn doc(ns: Namespace, id: DocId, shape: ShapeId) -> Self {
        Self {
            ns,
            root: RootKey::Doc(id),
            shape,
        }
    }

    /// An application-computed value.
    ///
    /// The shape is a constant, because the [`DerivedKey`] already identifies
    /// the whole question — there is no projection of a derived value to factor
    /// out, the way a document has many projections. The sentinel starts with a
    /// NUL so it cannot collide with the hash of any request shape text.
    pub fn derived(ns: Namespace, key: DerivedKey) -> Self {
        Self {
            ns,
            root: RootKey::Derived(key),
            shape: ShapeId::of(b"\x00reactor/derived"),
        }
    }
}

/// What a fragment is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RootKey {
    /// A single document's projection. The decomposable case: many responses
    /// can reference the same document fragment, so one changed document
    /// invalidates one leaf rather than every response that mentioned it.
    Doc(DocId),
    /// A whole-response entry: hash of (method, path, canonical query/body).
    Request(ContentHash),
    /// An application-computed value: the result of a transformation Reactor
    /// did not run.
    ///
    /// Everything else about it is ordinary — it has versions, a validity
    /// interval, dependency terms and a content hash, and it goes through the
    /// same writer and the same insert protocol — because the engine never
    /// cared where bytes came from, only what they depend on and which position
    /// they were computed at.
    Derived(DerivedKey),
}

/// Where a body lives.
///
/// Bodies are immutable and content-addressed, so a version can reference one
/// without owning it. Small bodies are inlined because a hash lookup plus a
/// clone costs more than the bytes themselves; large ones live in the store so
/// that eviction has something to evict.
#[derive(Clone, Debug)]
pub enum BodyRef {
    /// Held directly by the version.
    Inline(Bytes),
    /// Held by the [`crate::store::FragmentStore`] under this hash.
    Cas(ContentHash),
}

impl BodyRef {
    /// The hash under which this body is stored, if it is in the store.
    pub fn cas_hash(&self) -> Option<ContentHash> {
        match self {
            BodyRef::Cas(hash) => Some(*hash),
            BodyRef::Inline(_) => None,
        }
    }
}

/// Response metadata we must replay verbatim alongside the body.
///
/// Only the fields that are part of the *answer*. Hop-by-hop and
/// connection-specific headers are regenerated per response, and anything that
/// varies per request (`Date`, `Age`) must not be frozen into a cache entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseMeta {
    /// The upstream status code.
    pub status: u16,
    /// The upstream `Content-Type`, which for TerminusDB distinguishes a JSON
    /// document from an NDJSON stream and therefore changes how a response is
    /// composed.
    pub content_type: Option<String>,
    /// The upstream `TerminusDB-Data-Version`, if it sent one. Echoed back so a
    /// client's monotonic-read bookkeeping keeps working through the cache.
    pub data_version: Option<String>,
}

impl ResponseMeta {
    /// A plain `200 application/json` response.
    pub fn json() -> Self {
        Self {
            status: 200,
            content_type: Some("application/json".to_owned()),
            data_version: None,
        }
    }
}

/// One version of a fragment, valid for positions in `[from, to)`.
#[derive(Clone, Debug)]
pub struct FragmentVersion {
    /// First position at which this answer is known to be correct: the position
    /// the fill was pinned at.
    pub from: Position,
    /// First position at which it is known to be wrong. `None` = still current.
    pub to: Option<Position>,
    /// The commit this was filled at. Opaque; recorded so an entry can be
    /// explained, and so a pinned re-read can reproduce it exactly.
    pub commit: CommitId,
    /// Hash of the body. Also the `ETag`.
    pub hash: ContentHash,
    /// Where the bytes live.
    pub body: BodyRef,
    /// Metadata to replay with the bytes.
    pub meta: ResponseMeta,
    /// The read set: every term this answer depends on.
    pub deps: Vec<Term>,
    /// Child fragments this one is composed from, if it was decomposed.
    pub children: Vec<FragmentKey>,
    /// Backdating: the position at which this content last actually changed.
    /// Used for ETags and to stop propagation. `<= from`.
    ///
    /// This is the russian-doll trick: a recomputation that produces identical
    /// bytes inherits the old change position, so a parent composed from this
    /// fragment does not have to be rebuilt and a client holding the matching
    /// `ETag` still gets a 304.
    pub changed_at: Position,
    /// When this version was filled, for `Age` and TTL.
    ///
    /// Not in the original spec's struct; added because `Age` is part of the
    /// client-facing header protocol and a TTL policy needs a wall-clock-free
    /// monotonic reference. Kept out of [`FragmentVersion::covers`] so that
    /// validity remains a pure function of positions and stays testable.
    pub filled_at: Instant,
}

impl FragmentVersion {
    /// True when this version is valid at `at`.
    ///
    /// Half-open on purpose: `to` is the position of the commit that invalidated
    /// the version, and at that position the new value is already the truth.
    pub fn covers(&self, at: Position) -> bool {
        self.from <= at && self.to.map_or(true, |to| at < to)
    }

    /// True when no commit has invalidated this version yet.
    pub fn is_open(&self) -> bool {
        self.to.is_none()
    }

    /// Time since the fill, for the `Age` header.
    pub fn age(&self) -> std::time::Duration {
        self.filled_at.elapsed()
    }
}

/// Newest-first version list for one key.
///
/// Keep it short: GC versions no live reader needs (below the minimum latched
/// position). Several versions coexist because readers latch a position at
/// request start and must keep seeing a consistent answer even as commits land
/// underneath them.
#[derive(Debug, Default)]
pub struct FragmentSlot {
    /// Versions, ordered by `from` descending.
    pub versions: Vec<FragmentVersion>,
    /// Logical timestamp of the last lookup, from the namespace's own counter.
    ///
    /// An `AtomicU64` because a *reader* writes it: a lookup holds a shared
    /// reference to the slot (a sharded read guard) and must not have to take
    /// the write side to record that it used the entry. A logical clock rather
    /// than `Instant` because eviction only ever needs relative recency, and
    /// because the comparison must be total.
    last_used: std::sync::atomic::AtomicU64,
}

impl Clone for FragmentSlot {
    fn clone(&self) -> Self {
        Self {
            versions: self.versions.clone(),
            last_used: std::sync::atomic::AtomicU64::new(self.last_used()),
        }
    }
}

impl FragmentSlot {
    /// Record that this slot was just used, at logical time `now`.
    ///
    /// Monotonic by construction (the caller's clock only moves forward), so a
    /// racing older touch cannot make a hot key look cold.
    pub fn touch(&self, now: u64) {
        self.last_used
            .store(now, std::sync::atomic::Ordering::Relaxed);
    }

    /// The logical time of the last use, for least-recently-used eviction.
    pub fn last_used(&self) -> u64 {
        self.last_used.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Insert a version, keeping the list ordered by `from` descending.
    ///
    /// Sorted insertion rather than push-to-front because fills can land out of
    /// order: two fills of the same key pinned at different commits race, and
    /// the one pinned earlier may complete later.
    pub fn insert(&mut self, version: FragmentVersion) {
        let at = self
            .versions
            .iter()
            .position(|existing| existing.from <= version.from)
            .unwrap_or(self.versions.len());
        self.versions.insert(at, version);
    }

    /// The newest version valid at `at`.
    pub fn covering(&self, at: Position) -> Option<&FragmentVersion> {
        self.versions.iter().find(|v| v.covers(at))
    }

    /// The newest version that nothing has invalidated.
    pub fn current(&self) -> Option<&FragmentVersion> {
        self.versions.iter().find(|v| v.is_open())
    }

    /// Close the version that starts at `from`, marking it invalid from `at`.
    ///
    /// Returns true if a version was closed. Idempotent: closing an already
    /// closed version leaves the earlier (and therefore more conservative)
    /// end position in place.
    pub fn close(&mut self, from: Position, at: Position) -> bool {
        for version in self.versions.iter_mut() {
            if version.from == from {
                match version.to {
                    Some(existing) if existing <= at => {}
                    _ => version.to = Some(at),
                }
                return true;
            }
        }
        false
    }

    /// Drop versions that no reader can still be latched below, and cap the
    /// history length.
    ///
    /// `horizon` should be the lowest position any live reader may still hold.
    /// A closed version whose `to` is at or below it can never be returned
    /// again. The `max_versions` cap is a backstop for the case where we have no
    /// reader information: holding unbounded history for a hot key would be a
    /// slow memory leak.
    pub fn gc(&mut self, horizon: Position, max_versions: usize) {
        self.versions
            .retain(|v| v.to.map_or(true, |to| to > horizon));
        if self.versions.len() > max_versions {
            // Keep the newest; the dropped ones are the oldest closed versions.
            let keep_open: Vec<FragmentVersion> = self
                .versions
                .drain(..)
                .enumerate()
                .filter(|(i, v)| *i < max_versions || v.is_open())
                .map(|(_, v)| v)
                .collect();
            self.versions = keep_open;
        }
    }

    /// True when this slot holds nothing.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{Epoch, Seq};
    use crate::testutil::doc;

    fn pos(epoch: u64, seq: u64) -> Position {
        Position {
            epoch: Epoch(epoch),
            seq: Seq(seq),
        }
    }

    fn version(from: Position, to: Option<Position>, body: &str) -> FragmentVersion {
        let bytes = Bytes::from(body.to_owned());
        FragmentVersion {
            from,
            to,
            commit: CommitId::new(body),
            hash: ContentHash::of(&bytes),
            body: BodyRef::Inline(bytes),
            meta: ResponseMeta::json(),
            deps: vec![Term::Doc(doc("Person/alice"))],
            children: vec![],
            changed_at: from,
            filled_at: Instant::now(),
        }
    }

    #[test]
    fn covers_is_half_open() {
        let v = version(pos(0, 2), Some(pos(0, 5)), "x");
        assert!(!v.covers(pos(0, 1)), "before the fill");
        assert!(v.covers(pos(0, 2)), "at the fill position");
        assert!(v.covers(pos(0, 4)));
        assert!(
            !v.covers(pos(0, 5)),
            "at the invalidating commit the new value is already the truth"
        );
        assert!(!v.covers(pos(0, 9)));
    }

    #[test]
    fn open_versions_cover_everything_from_their_start() {
        let v = version(pos(0, 2), None, "x");
        assert!(v.is_open());
        assert!(v.covers(pos(0, 2)));
        assert!(v.covers(pos(0, 9999)));
        assert!(
            v.covers(pos(1, 0)),
            "an open version survives an epoch bump it was not invalidated by"
        );
        assert!(!v.covers(pos(0, 1)));
    }

    #[test]
    fn slot_keeps_versions_newest_first_even_when_fills_land_out_of_order() {
        let mut slot = FragmentSlot::default();
        slot.insert(version(pos(0, 5), None, "late pin"));
        slot.insert(version(
            pos(0, 2),
            Some(pos(0, 5)),
            "early pin, landed later",
        ));
        let froms: Vec<Position> = slot.versions.iter().map(|v| v.from).collect();
        assert_eq!(froms, vec![pos(0, 5), pos(0, 2)]);
    }

    #[test]
    fn slot_lookup_picks_the_newest_covering_version() {
        let mut slot = FragmentSlot::default();
        slot.insert(version(pos(0, 2), Some(pos(0, 5)), "old"));
        slot.insert(version(pos(0, 5), None, "new"));

        assert_eq!(slot.covering(pos(0, 3)).map(|v| v.from), Some(pos(0, 2)));
        assert_eq!(slot.covering(pos(0, 7)).map(|v| v.from), Some(pos(0, 5)));
        assert!(slot.covering(pos(0, 1)).is_none());
        assert_eq!(slot.current().map(|v| v.from), Some(pos(0, 5)));
    }

    #[test]
    fn closing_a_version_is_idempotent_and_keeps_the_earliest_end() {
        let mut slot = FragmentSlot::default();
        slot.insert(version(pos(0, 2), None, "v"));
        assert!(slot.close(pos(0, 2), pos(0, 4)));
        assert_eq!(slot.versions[0].to, Some(pos(0, 4)));
        assert!(slot.close(pos(0, 2), pos(0, 9)));
        assert_eq!(
            slot.versions[0].to,
            Some(pos(0, 4)),
            "a later close must not extend validity"
        );
        assert!(!slot.close(pos(0, 3), pos(0, 9)), "no such version");
    }

    #[test]
    fn gc_drops_versions_below_the_horizon_but_never_the_open_one() {
        let mut slot = FragmentSlot::default();
        slot.insert(version(pos(0, 1), Some(pos(0, 2)), "a"));
        slot.insert(version(pos(0, 2), Some(pos(0, 3)), "b"));
        slot.insert(version(pos(0, 3), None, "c"));

        slot.gc(pos(0, 2), 10);
        let froms: Vec<Position> = slot.versions.iter().map(|v| v.from).collect();
        assert_eq!(
            froms,
            vec![pos(0, 3), pos(0, 2)],
            "the version closed at or below the horizon is unreachable"
        );

        slot.gc(pos(0, 99), 10);
        assert_eq!(slot.versions.len(), 1);
        assert!(slot.current().is_some(), "the open version always survives");
    }

    #[test]
    fn gc_caps_history_length() {
        let mut slot = FragmentSlot::default();
        for i in 1..10 {
            slot.insert(version(pos(0, i), Some(pos(0, i + 1)), "v"));
        }
        slot.insert(version(pos(0, 20), None, "open"));
        slot.gc(pos(0, 0), 3);
        assert_eq!(slot.versions.len(), 3);
        assert_eq!(slot.versions[0].from, pos(0, 20));
    }

    #[test]
    fn body_ref_reports_its_cas_hash() {
        let bytes = Bytes::from_static(b"body");
        let hash = ContentHash::of(&bytes);
        assert_eq!(BodyRef::Cas(hash).cas_hash(), Some(hash));
        assert_eq!(BodyRef::Inline(bytes).cas_hash(), None);
    }
}

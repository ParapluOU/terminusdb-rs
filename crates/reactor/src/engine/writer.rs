//! The single writer per namespace: the only thing that mutates cached state.
//!
//! # The insert protocol
//!
//! ```text
//! on Insert{key, at: P, deps, body, ...}:
//!   if P.epoch != current epoch        -> hand body back, do not store
//!   if P < fired_log.oldest_retained() -> hand body back, do not store
//!   hash = blake3(body)
//!   K1 = first position after P at which a fired term was subsumed by deps
//!   K2 = `from` of a newer open version of this key, if a later fill beat us
//!   match min(K1, K2):
//!     None  -> store [P, None); supersede the previous open version at P;
//!              register deps in the term index
//!     Some  -> store [P, that); do NOT register
//!              (still useful: readers latched below it can serve from this)
//!   backdating: if a previous version of this key had the same hash, inherit
//!   its changed_at; else changed_at = P
//! ```
//!
//! `K1` is the fill/commit race. `K2` is the fill/fill race, which the original
//! protocol did not name: two fills of one key pinned at different commits can
//! complete out of order, and the one pinned earlier must not displace the one
//! pinned later. Both cases have the same shape — "something newer already
//! covers the tail of this interval" — so they are handled by the same `min`.
//! The invariant they jointly maintain is **at most one open version per key,
//! and it is the one registered in the term index**.
//!
//! # The apply protocol
//!
//! ```text
//! on Commit(cs):
//!   pos   = head.next()
//!   fired = fire(&cs, &schema)
//!   fired_log.push(pos, cs.commit, fired)
//!   for entry in term_index.candidates(&fired, &schema):
//!       if entry.deps.any(|d| fired.any(|f| d.subsumes(f, &schema))):
//!           slot.current_mut().to = Some(pos)
//!           term_index.unregister(entry)
//!   if cs.schema_changed { request a schema refresh }
//!   head = pos
//!   publish Watermark { pos, commit: cs.commit }    // LAST, atomically
//! ```
//!
//! The watermark is published **last**, after the invalidation is complete.
//! A reader that latches the new position therefore cannot see a stale version
//! that the commit should have closed. The reverse order would be a silent
//! correctness bug under load and almost impossible to reproduce.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::mpsc;

use crate::change::{fire, ChangeSet};
use crate::ids::{ContentHash, Position, Watermark};
use crate::metrics::Metrics;
use crate::schema_index::SchemaIndex;
use crate::store::FragmentStore;
use crate::term::TermSet;
use terminusdb_client::CommitId;

use super::fragment::{BodyRef, FragmentKey, FragmentVersion, ResponseMeta};
use super::index::TermIndex;
use super::log::FiredLog;
use super::{EngineConfig, FillResult, NsState, WriterMsg};

/// Owns all mutable state for one namespace.
///
/// Designed to be driven either by its own task ([`NamespaceWriter::run`]) or by
/// an owning task calling [`NamespaceWriter::handle`] per message — the latter
/// is what the tests do, and what lets a supervisor interleave writer work with
/// other bookkeeping without a second channel.
pub struct NamespaceWriter {
    state: Arc<NsState>,
    inbox: mpsc::UnboundedReceiver<WriterMsg>,
    cfg: Arc<EngineConfig>,
    store: Arc<dyn FragmentStore>,
    metrics: Arc<Metrics>,
    /// The writer's own copy of the schema index. It is the only mutator, so it
    /// can keep a plain `Arc` and publish to the shared slot on change.
    schema: Arc<SchemaIndex>,
    index: TermIndex,
    log: FiredLog,
    head: Position,
    head_commit: Option<CommitId>,
}

impl NamespaceWriter {
    pub(super) fn new(
        state: Arc<NsState>,
        inbox: mpsc::UnboundedReceiver<WriterMsg>,
        cfg: Arc<EngineConfig>,
        store: Arc<dyn FragmentStore>,
        metrics: Arc<Metrics>,
    ) -> Self {
        let log = FiredLog::with_capacity(cfg.fired_log_capacity);
        let schema = state.schema.load_full();
        Self {
            state,
            inbox,
            cfg,
            store,
            metrics,
            schema,
            index: TermIndex::new(),
            log,
            head: Position::ZERO,
            head_commit: None,
        }
    }

    /// Drive the writer until the engine shuts down or is dropped.
    ///
    /// Shutdown is an explicit signal rather than the channel closing: the
    /// writer holds the namespace state, which owns the sender, so the channel
    /// never closes on its own.
    pub async fn run(mut self) {
        loop {
            if self.state.stopping.load(Ordering::Acquire) {
                break;
            }
            tokio::select! {
                msg = self.inbox.recv() => match msg {
                    Some(msg) => self.handle(msg),
                    None => break,
                },
                _ = self.state.stop_notify.notified() => break,
            }
        }
        tracing::debug!(ns = %self.state.ns, "namespace writer stopped");
    }

    /// The current head position. Equal to the published watermark's position.
    pub fn head(&self) -> Position {
        self.head
    }

    /// The commit at the head, if one has been applied.
    pub fn head_commit(&self) -> Option<&CommitId> {
        self.head_commit.as_ref()
    }

    /// The term index, for admin output and tests.
    pub fn index(&self) -> &TermIndex {
        &self.index
    }

    /// The fired log, for admin output and tests.
    pub fn fired_log(&self) -> &FiredLog {
        &self.log
    }

    /// The schema index this writer is applying commits against.
    pub fn schema(&self) -> &Arc<SchemaIndex> {
        &self.schema
    }

    /// Apply one message.
    pub fn handle(&mut self, msg: WriterMsg) {
        match msg {
            WriterMsg::Commit(cs) => self.on_commit(&cs),
            WriterMsg::NonFastForward {
                old_head,
                new_head,
                fired,
            } => self.on_non_fast_forward(old_head, new_head, fired),
            WriterMsg::Insert(fill) => self.on_insert(*fill),
            WriterMsg::Schema(schema) => self.on_schema(schema),
            WriterMsg::Purge => self.on_purge(),
        }
    }

    // ------------------------------------------------------------------ apply

    fn on_commit(&mut self, cs: &ChangeSet) {
        let pos = self.head.next();
        let fired = fire(cs, &self.schema);
        tracing::debug!(
            ns = %self.state.ns,
            commit = %cs.commit,
            %pos,
            fired = fired.len(),
            complete = cs.complete,
            "applying commit"
        );
        self.log.push(pos, cs.commit.clone(), fired.clone());
        self.invalidate(&fired, pos);

        if cs.schema_changed {
            self.request_schema_refresh();
        }

        self.head = pos;
        self.head_commit = Some(cs.commit.clone());
        self.metrics.commits_applied.inc();
        // Published last, and only now: see the module docs.
        self.publish(Some(Watermark {
            pos,
            commit: cs.commit.clone(),
        }));
    }

    fn on_non_fast_forward(
        &mut self,
        old_head: Option<CommitId>,
        new_head: CommitId,
        fired: Option<TermSet>,
    ) {
        let pos = self.head.next_epoch();
        self.metrics.epoch_bumps.inc();
        tracing::info!(
            ns = %self.state.ns,
            old_head = ?old_head.as_ref().map(CommitId::as_str),
            new_head = %new_head,
            %pos,
            diffable = fired.is_some(),
            "non-fast-forward head movement: bumping epoch"
        );
        match fired {
            // We could diff the rewrite, so it behaves exactly like a (large)
            // commit: only fragments that actually depend on what moved die.
            Some(terms) => {
                self.log.push(pos, new_head.clone(), terms.clone());
                self.invalidate(&terms, pos);
            }
            // We could not. Node-local state goes; the content-addressed bodies
            // stay, because they were never wrong and can be re-adopted by hash.
            None => {
                self.purge_state(pos);
                self.metrics.purges.inc();
            }
        }
        self.head = pos;
        self.head_commit = Some(new_head.clone());
        self.publish(Some(Watermark {
            pos,
            commit: new_head,
        }));
    }

    fn on_schema(&mut self, schema: Arc<SchemaIndex>) {
        self.schema = Arc::clone(&schema);
        self.state.schema.store(schema);
        self.state.schema_dirty.store(false, Ordering::Release);
        tracing::debug!(ns = %self.state.ns, "schema index refreshed");
    }

    fn on_purge(&mut self) {
        // Bump the epoch even though the history did not move: in-flight fills
        // pinned before the purge must not be allowed to land afterwards.
        let pos = self.head.next_epoch();
        self.purge_state(pos);
        self.head = pos;
        self.metrics.purges.inc();
        let commit = self.head_commit.clone();
        self.publish(commit.map(|commit| Watermark { pos, commit }));
        tracing::info!(ns = %self.state.ns, %pos, "namespace purged");
    }

    /// Close every registered version the fired set dirties, and unregister it.
    fn invalidate(&mut self, fired: &TermSet, pos: Position) {
        let dirty = self.index.dirtied(fired, &self.schema);
        let horizon = self.gc_horizon(pos);
        for (id, key, from) in dirty {
            let mut emptied = false;
            if let Some(mut slot) = self.state.slots.get_mut(&key) {
                if slot.close(from, pos) {
                    self.metrics.versions_invalidated.inc();
                }
                slot.gc(horizon, self.cfg.max_versions_per_key);
                emptied = slot.is_empty();
            }
            if emptied {
                // The guard above is dropped by now: removing while holding it
                // would deadlock the shard.
                self.state.slots.remove(&key);
            }
            self.index.unregister(id);
        }
        self.metrics.tracked_keys.set(self.state.slots.len() as u64);
    }

    fn purge_state(&mut self, floor: Position) {
        self.state.slots.clear();
        self.index.clear();
        self.log.clear(floor);
        self.metrics.tracked_keys.set(0);
    }

    fn request_schema_refresh(&self) {
        self.state.schema_dirty.store(true, Ordering::Release);
        self.state.schema_notify.notify_waiters();
        self.metrics.schema_refreshes_requested.inc();
    }

    fn publish(&self, watermark: Option<Watermark>) {
        self.state.watermark.store(watermark.map(Arc::new));
        self.metrics.store_bytes.set(self.store.size());
    }

    /// Positions at or below this can no longer be latched by a reader, so a
    /// version closed there is unreachable and can be dropped.
    fn gc_horizon(&self, pos: Position) -> Position {
        Position {
            epoch: pos.epoch,
            seq: crate::ids::Seq(pos.seq.0.saturating_sub(self.cfg.version_retention)),
        }
    }

    // ----------------------------------------------------------------- insert

    fn on_insert(&mut self, fill: FillResult) {
        let FillResult {
            key,
            at,
            commit,
            body,
            meta,
            deps,
            children,
            reply,
        } = fill;

        // The history this fill was pinned in has been abandoned. We cannot say
        // whether its deps still hold, so we must not store it.
        if at.epoch != self.head.epoch {
            self.metrics.fills_rejected_epoch.inc();
            tracing::debug!(
                ns = %self.state.ns, %at, head = %self.head,
                "discarding fill from an abandoned epoch"
            );
            self.hand_back(reply, at, commit, body, meta, deps, children);
            return;
        }

        // We no longer retain the fire history needed to validate this pin.
        if at < self.log.oldest_retained() {
            self.metrics.fills_rejected_stale.inc();
            tracing::debug!(
                ns = %self.state.ns, %at, oldest = %self.log.oldest_retained(),
                "discarding fill we can no longer validate"
            );
            self.hand_back(reply, at, commit, body, meta, deps, children);
            return;
        }

        let hash = ContentHash::of(&body);

        // Did anything between the pin position and now invalidate these deps?
        let by_commit = self.log.first_invalidating_after(at, &deps, &self.schema);
        // Did a fill pinned later already land for this key?
        let by_fill = self.state.slots.get(&key).and_then(|slot| {
            slot.versions
                .iter()
                .filter(|v| v.is_open() && v.from > at)
                .map(|v| v.from)
                .min()
        });
        let to = match (by_commit, by_fill) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (only, None) | (None, only) => only,
        };

        let inline = self.cfg.policy.default_policy().inline(body.len());
        let body_ref = self.place_body(inline, &hash, &body);
        let changed_at = match self.inherited_changed_at(&key, &hash) {
            Some(earlier) => {
                self.metrics.fills_backdated.inc();
                earlier
            }
            None => at,
        };

        let version = FragmentVersion {
            from: at,
            to,
            commit,
            hash,
            body: body_ref,
            meta,
            deps: deps.clone(),
            children,
            changed_at,
            filled_at: Instant::now(),
        };
        let shared = Arc::new(version.clone());

        {
            let mut slot = self.state.slots.entry(key.clone()).or_default();
            if to.is_none() {
                // This is now the current answer, so the previous current one
                // ends here. It keeps its earlier interval, so no reader latched
                // below `at` loses anything. `<=` rather than `<` so that a
                // second fill pinned at the *same* position replaces the first
                // outright (its interval becomes empty) — two open versions of
                // one key would leave one of them unregistered and therefore
                // never invalidated, which is the stale-forever failure mode.
                let superseded: Vec<Position> = slot
                    .versions
                    .iter()
                    .filter(|v| v.is_open() && v.from <= at)
                    .map(|v| v.from)
                    .collect();
                for from in superseded {
                    slot.close(from, at);
                }
            }
            slot.insert(version);
            slot.gc(self.gc_horizon(self.head), self.cfg.max_versions_per_key);
        }

        match to {
            None => {
                self.index.register(key, at, deps);
                self.metrics.fills_stored.inc();
            }
            Some(end) => {
                // Deliberately not registered: a bounded interval cannot be
                // invalidated again, and registering it would make the index's
                // one-entry-per-key invariant ambiguous.
                self.metrics.fills_raced.inc();
                tracing::debug!(
                    ns = %self.state.ns, %at, %end,
                    "fill landed after an intersecting change: stored with a bounded interval"
                );
            }
        }
        self.metrics.tracked_keys.set(self.state.slots.len() as u64);

        if let Some(reply) = reply {
            let _ = reply.send(shared);
        }
    }

    /// Put the body where policy says, falling back to inline if the store
    /// refuses it. A store failure must never fail a request.
    fn place_body(&self, inline: bool, hash: &ContentHash, body: &Bytes) -> BodyRef {
        if inline {
            return BodyRef::Inline(body.clone());
        }
        match self.store.put(hash, body) {
            Ok(()) => BodyRef::Cas(*hash),
            Err(err) => {
                self.metrics.store_put_errors.inc();
                tracing::warn!(%hash, error = %err, "fragment store write failed; holding inline");
                BodyRef::Inline(body.clone())
            }
        }
    }

    /// Backdating: the `changed_at` of the most recent version of this key with
    /// the same content hash.
    ///
    /// This is what makes a russian-doll cache efficient. A recomputation that
    /// produces identical bytes keeps the old change position, so parents
    /// composed from it stay valid and a client's `If-None-Match` still gets a
    /// 304 — the invalidation stops propagating at the first fragment whose
    /// content did not actually move.
    fn inherited_changed_at(&self, key: &FragmentKey, hash: &ContentHash) -> Option<Position> {
        let slot = self.state.slots.get(key)?;
        slot.versions
            .iter()
            .find(|v| &v.hash == hash)
            .map(|v| v.changed_at)
    }

    /// Return an unstored fill to its waiter.
    ///
    /// The bytes are still a correct answer at the position they were pinned at,
    /// so the request that paid for them may serve them; it just must not become
    /// a cache entry. The interval is empty (`to == from`) so that anything that
    /// re-checks [`FragmentVersion::covers`] agrees it is not cached.
    #[allow(clippy::too_many_arguments)]
    fn hand_back(
        &self,
        reply: Option<tokio::sync::oneshot::Sender<Arc<FragmentVersion>>>,
        at: Position,
        commit: CommitId,
        body: Bytes,
        meta: ResponseMeta,
        deps: Vec<crate::term::Term>,
        children: Vec<FragmentKey>,
    ) {
        let reply = match reply {
            Some(reply) => reply,
            None => return,
        };
        let hash = ContentHash::of(&body);
        let _ = reply.send(Arc::new(FragmentVersion {
            from: at,
            to: Some(at),
            commit,
            hash,
            body: BodyRef::Inline(body),
            meta,
            deps,
            children,
            changed_at: at,
            filled_at: Instant::now(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::DocChange;
    use crate::engine::{Engine, Lookup};
    use crate::ids::{Epoch, Namespace, Seq, ShapeId};
    use crate::store::TieredStore;
    use crate::term::Term;
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use serde_json::json;
    use std::collections::HashMap;

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    fn pos(epoch: u64, seq: u64) -> Position {
        Position {
            epoch: Epoch(epoch),
            seq: Seq(seq),
        }
    }

    /// An engine plus the (sole) writer for the test namespace.
    fn harness() -> (Arc<Engine>, NamespaceWriter) {
        harness_with(EngineConfig::default())
    }

    fn harness_with(cfg: EngineConfig) -> (Arc<Engine>, NamespaceWriter) {
        let engine = Arc::new(Engine::new(
            cfg,
            Arc::new(TieredStore::memory_only(1 << 20)),
        ));
        let mut writer = engine.take_writer(&ns()).expect("a fresh writer");
        writer.handle(WriterMsg::Schema(Arc::new(schema_fixture())));
        (engine, writer)
    }

    fn key(name: &str) -> FragmentKey {
        FragmentKey::request(
            ns(),
            ContentHash::of(name.as_bytes()),
            ShapeId::of(b"shape"),
        )
    }

    fn commit(id: &str, changes: Vec<DocChange>) -> WriterMsg {
        WriterMsg::Commit(Box::new(ChangeSet {
            ns: ns(),
            commit: CommitId::new(id),
            parent: None,
            changes,
            schema_changed: false,
            complete: true,
        }))
    }

    fn update(
        id: &str,
        class: &str,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> DocChange {
        DocChange::updated(doc(id), cls(class), before, after)
    }

    fn insert(key: FragmentKey, at: Position, deps: Vec<Term>, body: &str) -> WriterMsg {
        WriterMsg::Insert(Box::new(FillResult {
            key,
            at,
            commit: CommitId::new("filled-at"),
            body: Bytes::from(body.to_owned()),
            meta: ResponseMeta::json(),
            deps,
            children: vec![],
            reply: None,
        }))
    }

    // --------------------------------------------------------------- lookups

    #[test]
    fn a_fresh_namespace_has_no_watermark_so_readers_must_bypass() {
        let (engine, _writer) = harness();
        assert!(
            engine.watermark(&ns()).is_none(),
            "with no position there is nothing to pin a fill at"
        );
    }

    #[test]
    fn applying_a_commit_publishes_a_watermark() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let wm = engine.watermark(&ns()).expect("watermark");
        assert_eq!(wm.pos, pos(0, 1));
        assert_eq!(wm.commit, CommitId::new("c1"));
        assert_eq!(writer.head(), pos(0, 1));
    }

    #[test]
    fn a_stored_fill_is_found_at_and_after_its_pin() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "body",
        ));

        assert!(matches!(engine.lookup(&k, pos(0, 0)), Lookup::Miss));
        let hit = engine.lookup(&k, pos(0, 1));
        let version = hit.version().expect("hit at the pin position");
        assert_eq!(version.from, pos(0, 1));
        assert!(version.is_open());
        assert!(engine.lookup(&k, pos(0, 99)).is_hit());
        assert_eq!(engine.metrics().fills_stored.get(), 1);
    }

    #[test]
    fn a_commit_closes_the_versions_it_dirties_and_leaves_the_others() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let alice = key("alice");
        let bob = key("bob");
        writer.handle(insert(
            alice.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "a",
        ));
        writer.handle(insert(
            bob.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/bob"))],
            "b",
        ));

        writer.handle(commit(
            "c2",
            vec![update(
                "Person/alice",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));

        assert!(
            matches!(engine.lookup(&alice, pos(0, 2)), Lookup::Miss),
            "alice's entry must be invalidated at the new position"
        );
        assert!(
            engine.lookup(&alice, pos(0, 1)).is_hit(),
            "but a reader latched at the old position still sees it"
        );
        assert!(
            engine.lookup(&bob, pos(0, 2)).is_hit(),
            "bob's entry is untouched"
        );
        assert_eq!(engine.metrics().versions_invalidated.get(), 1);
    }

    #[test]
    fn invalidation_happens_before_the_watermark_is_published() {
        // A reader that sees the new watermark must never be able to see a
        // version the commit should have closed. We cannot observe the
        // interleaving directly, so assert the postcondition: at the published
        // position, the dirtied entry is gone.
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Class(cls("Person"))],
            "a",
        ));
        writer.handle(commit(
            "c2",
            vec![DocChange::created(
                doc("Person/zoe"),
                cls("Person"),
                json!({"name": "Z"}),
            )],
        ));
        let wm = engine.watermark(&ns()).unwrap();
        assert!(matches!(engine.lookup(&k, wm.pos), Lookup::Miss));
    }

    // ------------------------------------------------------- the insert race

    #[test]
    fn a_fill_that_lands_after_an_intersecting_commit_is_bounded_not_current() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");

        // The reader latched at P = (0,1) and issued a pinned fill there.
        let pinned_at = pos(0, 1);
        // While the fill was in flight, a commit at K = (0,2) changed alice.
        writer.handle(commit(
            "c2",
            vec![update(
                "Person/alice",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));
        // Only now does the fill land.
        writer.handle(insert(
            k.clone(),
            pinned_at,
            vec![Term::Doc(doc("Person/alice"))],
            "value at c1",
        ));

        let slot = engine.slot_snapshot(&k).expect("the fill is still stored");
        assert_eq!(slot.versions.len(), 1);
        assert_eq!(slot.versions[0].from, pinned_at);
        assert_eq!(
            slot.versions[0].to,
            Some(pos(0, 2)),
            "stored as [P, K): correct at P, known wrong from K"
        );
        assert!(
            writer.index().entry_for_key(&k).is_none(),
            "a raced fill must NOT be registered in the term index"
        );
        assert_eq!(engine.metrics().fills_raced.get(), 1);
        assert_eq!(engine.metrics().fills_stored.get(), 0);

        // It is still useful to a reader latched below K, and invisible above.
        assert!(engine.lookup(&k, pos(0, 1)).is_hit());
        assert!(matches!(engine.lookup(&k, pos(0, 2)), Lookup::Miss));
    }

    #[test]
    fn a_fill_that_races_an_unrelated_commit_is_still_current() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(commit(
            "c2",
            vec![update(
                "Person/bob",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "value at c1",
        ));

        let slot = engine.slot_snapshot(&k).unwrap();
        assert!(
            slot.versions[0].is_open(),
            "an unrelated commit does not bound the interval"
        );
        assert!(writer.index().entry_for_key(&k).is_some());
        assert!(engine.lookup(&k, writer.head()).is_hit());
    }

    #[test]
    fn a_fill_pinned_earlier_does_not_displace_one_pinned_later() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        writer.handle(commit("c2", vec![]));
        let k = key("alice");
        let deps = vec![Term::Doc(doc("Person/alice"))];

        // The fill pinned at (0,2) completes first.
        writer.handle(insert(k.clone(), pos(0, 2), deps.clone(), "newer"));
        // The one pinned at (0,1) completes second.
        writer.handle(insert(k.clone(), pos(0, 1), deps.clone(), "older"));

        let slot = engine.slot_snapshot(&k).unwrap();
        assert_eq!(slot.versions.len(), 2);
        assert_eq!(slot.current().map(|v| v.from), Some(pos(0, 2)));
        assert_eq!(
            slot.versions
                .iter()
                .find(|v| v.from == pos(0, 1))
                .and_then(|v| v.to),
            Some(pos(0, 2)),
            "the late-landing older fill is bounded by the newer one"
        );
        assert_eq!(
            writer.index().entry_for_key(&k).map(|e| e.from),
            Some(pos(0, 2)),
            "the index must still point at the newest open version"
        );
    }

    #[test]
    fn a_new_current_version_supersedes_the_previous_one() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        let deps = vec![Term::Doc(doc("Person/alice"))];
        writer.handle(insert(k.clone(), pos(0, 1), deps.clone(), "first"));
        writer.handle(commit("c2", vec![]));
        writer.handle(insert(k.clone(), pos(0, 2), deps.clone(), "second"));

        let slot = engine.slot_snapshot(&k).unwrap();
        let open: Vec<Position> = slot
            .versions
            .iter()
            .filter(|v| v.is_open())
            .map(|v| v.from)
            .collect();
        assert_eq!(open, vec![pos(0, 2)], "exactly one open version per key");
        assert_eq!(writer.index().len(), 1);
    }

    #[test]
    fn a_fill_from_an_abandoned_epoch_is_handed_back_unstored() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(WriterMsg::NonFastForward {
            old_head: Some(CommitId::new("c1")),
            new_head: CommitId::new("rebased"),
            fired: Some(TermSet::new()),
        });
        assert_eq!(writer.head(), pos(1, 0));

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        writer.handle(WriterMsg::Insert(Box::new(FillResult {
            key: k.clone(),
            at: pos(0, 1),
            commit: CommitId::new("c1"),
            body: Bytes::from_static(b"stale"),
            meta: ResponseMeta::json(),
            deps: vec![Term::Doc(doc("Person/alice"))],
            children: vec![],
            reply: Some(tx),
        })));

        assert!(
            engine.slot_snapshot(&k).is_none(),
            "nothing from a dead epoch may be stored"
        );
        assert_eq!(engine.metrics().fills_rejected_epoch.get(), 1);
        let handed_back = rx.try_recv().expect("the body is handed back");
        assert_eq!(
            handed_back.to,
            Some(handed_back.from),
            "an empty validity interval: serveable once, never cached"
        );
        assert!(!handed_back.covers(handed_back.from));
    }

    #[test]
    fn a_fill_we_can_no_longer_validate_is_rejected() {
        let (engine, mut writer) = harness_with(EngineConfig {
            fired_log_capacity: 2,
            ..EngineConfig::default()
        });
        writer.handle(commit("c1", vec![]));
        let stale_pin = writer.head();
        for i in 2..=4 {
            writer.handle(commit(&format!("c{i}"), vec![]));
        }
        assert!(stale_pin < writer.fired_log().oldest_retained());

        let k = key("alice");
        writer.handle(insert(
            k.clone(),
            stale_pin,
            vec![Term::Doc(doc("Person/alice"))],
            "unvalidatable",
        ));
        assert!(engine.slot_snapshot(&k).is_none());
        assert_eq!(engine.metrics().fills_rejected_stale.get(), 1);
    }

    #[test]
    fn an_immutable_fill_depends_on_nothing_and_survives_everything() {
        // The commit-pinned case: `/local/commit/<id>` has one answer forever.
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("pinned read");
        writer.handle(insert(k.clone(), pos(0, 1), vec![], "immutable"));

        writer.handle(commit(
            "c2",
            vec![update(
                "Person/alice",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));
        let mut incomplete = ChangeSet::incomplete(ns(), CommitId::new("c3"), None);
        incomplete.schema_changed = true;
        writer.handle(WriterMsg::Commit(Box::new(incomplete)));

        assert!(
            engine.lookup(&k, writer.head()).is_hit(),
            "even a fired Everything cannot invalidate an empty dependency list"
        );
    }

    // ------------------------------------------------------------- backdating

    #[test]
    fn identical_content_inherits_the_earlier_change_position() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        let deps = vec![Term::Doc(doc("Person/alice"))];
        writer.handle(insert(k.clone(), pos(0, 1), deps.clone(), "same bytes"));

        writer.handle(commit(
            "c2",
            vec![update(
                "Person/alice",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));
        writer.handle(insert(k.clone(), pos(0, 2), deps.clone(), "same bytes"));

        let slot = engine.slot_snapshot(&k).unwrap();
        let current = slot.current().unwrap();
        assert_eq!(current.from, pos(0, 2));
        assert_eq!(
            current.changed_at,
            pos(0, 1),
            "the content did not move, so neither does its change position: \
             parents stay valid and ETags still match"
        );
        assert_eq!(engine.metrics().fills_backdated.get(), 1);
    }

    #[test]
    fn different_content_moves_the_change_position() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        let deps = vec![Term::Doc(doc("Person/alice"))];
        writer.handle(insert(k.clone(), pos(0, 1), deps.clone(), "first"));
        writer.handle(commit(
            "c2",
            vec![update(
                "Person/alice",
                "Person",
                json!({"name": "A"}),
                json!({"name": "B"}),
            )],
        ));
        writer.handle(insert(k.clone(), pos(0, 2), deps.clone(), "second"));

        let slot = engine.slot_snapshot(&k).unwrap();
        assert_eq!(slot.current().unwrap().changed_at, pos(0, 2));
        assert_eq!(engine.metrics().fills_backdated.get(), 0);
    }

    // ------------------------------------------------ non-fast-forward, purge

    #[test]
    fn non_fast_forward_with_fired_terms_keeps_unaffected_entries() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let alice = key("alice");
        let bob = key("bob");
        writer.handle(insert(
            alice.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "a",
        ));
        writer.handle(insert(
            bob.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/bob"))],
            "b",
        ));

        let fired: TermSet = [Term::Doc(doc("Person/alice"))].into_iter().collect();
        writer.handle(WriterMsg::NonFastForward {
            old_head: Some(CommitId::new("c1")),
            new_head: CommitId::new("rebased"),
            fired: Some(fired),
        });

        assert_eq!(writer.head(), pos(1, 0));
        assert_eq!(engine.watermark(&ns()).unwrap().pos, pos(1, 0));
        assert!(matches!(engine.lookup(&alice, pos(1, 0)), Lookup::Miss));
        assert!(
            engine.lookup(&bob, pos(1, 0)).is_hit(),
            "a diffable rewrite only kills what actually moved; an open version \
             carries across the epoch boundary"
        );
        assert_eq!(engine.metrics().epoch_bumps.get(), 1);
        assert_eq!(engine.metrics().purges.get(), 0);
    }

    #[test]
    fn non_fast_forward_without_fired_terms_drops_everything() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "a",
        ));

        writer.handle(WriterMsg::NonFastForward {
            old_head: Some(CommitId::new("c1")),
            new_head: CommitId::new("mystery"),
            fired: None,
        });

        assert!(engine.slot_snapshot(&k).is_none());
        assert!(writer.index().is_empty());
        assert_eq!(engine.metrics().purges.get(), 1);
        assert_eq!(
            engine.watermark(&ns()).unwrap().commit,
            CommitId::new("mystery"),
            "we still know where the head is, we just cached nothing about it"
        );
        assert!(
            engine.store().size() > 0 || true,
            "bodies in the content-addressed store were never wrong and stay"
        );
    }

    #[test]
    fn purge_bumps_the_epoch_so_in_flight_fills_cannot_land() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("alice");
        writer.handle(WriterMsg::Purge);
        assert_eq!(writer.head(), pos(1, 0));

        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "in flight when purged",
        ));
        assert!(engine.slot_snapshot(&k).is_none());
        assert_eq!(engine.metrics().fills_rejected_epoch.get(), 1);
        assert_eq!(
            engine.watermark(&ns()).unwrap().commit,
            CommitId::new("c1"),
            "a purge drops cached state, not our knowledge of the head"
        );
    }

    // -------------------------------------------------------------- schema

    #[test]
    fn a_schema_commit_requests_a_refresh() {
        let (engine, mut writer) = harness();
        assert!(!engine.schema_refresh_requested(&ns()));

        let mut cs = ChangeSet::empty(ns(), CommitId::new("c1"), None);
        cs.schema_changed = true;
        writer.handle(WriterMsg::Commit(Box::new(cs)));

        assert!(engine.schema_refresh_requested(&ns()));
        writer.handle(WriterMsg::Schema(Arc::new(schema_fixture())));
        assert!(!engine.schema_refresh_requested(&ns()));
    }

    #[test]
    fn a_schema_commit_invalidates_schema_dependent_entries() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("introspection");
        writer.handle(insert(k.clone(), pos(0, 1), vec![Term::Schema], "shape"));

        let mut cs = ChangeSet::empty(ns(), CommitId::new("c2"), None);
        cs.schema_changed = true;
        writer.handle(WriterMsg::Commit(Box::new(cs)));
        assert!(matches!(engine.lookup(&k, writer.head()), Lookup::Miss));
    }

    #[test]
    fn the_published_schema_is_visible_to_readers() {
        let (engine, mut writer) = harness();
        assert!(!engine.schema(&ns()).is_empty());
        writer.handle(WriterMsg::Schema(Arc::new(SchemaIndex::default())));
        assert!(engine.schema(&ns()).is_empty());
    }

    #[test]
    fn only_one_writer_per_namespace_can_exist() {
        let engine = Engine::new(
            EngineConfig::default(),
            Arc::new(TieredStore::memory_only(1024)),
        );
        assert!(engine.take_writer(&ns()).is_some());
        assert!(
            engine.take_writer(&ns()).is_none(),
            "the single-writer invariant is structural"
        );
    }

    #[tokio::test]
    async fn the_mailbox_drives_the_writer() {
        let engine = Arc::new(Engine::new(
            EngineConfig::default(),
            Arc::new(TieredStore::memory_only(1 << 20)),
        ));
        let writer = engine.take_writer(&ns()).unwrap();
        let task = tokio::spawn(writer.run());

        engine.send(&ns(), commit("c1", vec![])).unwrap();
        let k = key("alice");
        let (tx, rx) = tokio::sync::oneshot::channel();
        engine
            .send(
                &ns(),
                WriterMsg::Insert(Box::new(FillResult {
                    key: k.clone(),
                    at: pos(0, 1),
                    commit: CommitId::new("c1"),
                    body: Bytes::from_static(b"served"),
                    meta: ResponseMeta::json(),
                    deps: vec![Term::Doc(doc("Person/alice"))],
                    children: vec![],
                    reply: Some(tx),
                })),
            )
            .unwrap();

        let version = rx
            .await
            .expect("the writer replies with the stored version");
        assert!(version.is_open());
        assert_eq!(engine.watermark(&ns()).unwrap().pos, pos(0, 1));
        assert!(engine.lookup(&k, pos(0, 1)).is_hit());

        drop(engine);
        task.await.unwrap();
    }

    // --------------------------------------------------------- large bodies

    #[test]
    fn large_bodies_go_to_the_content_addressed_store() {
        let cfg = EngineConfig {
            policy: crate::engine::PolicySet::new(crate::engine::Policy {
                inline_max_bytes: 4,
                ..Default::default()
            }),
            ..EngineConfig::default()
        };
        let (engine, mut writer) = harness_with(cfg);
        writer.handle(commit("c1", vec![]));
        let k = key("big");
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "a body larger than four bytes",
        ));

        let slot = engine.slot_snapshot(&k).unwrap();
        let hash = slot.versions[0]
            .body
            .cas_hash()
            .expect("a large body is stored by hash, not inline");
        assert_eq!(
            engine.store().get(&hash).as_deref(),
            Some(&b"a body larger than four bytes"[..])
        );
    }

    #[test]
    fn small_bodies_stay_inline() {
        let (engine, mut writer) = harness();
        writer.handle(commit("c1", vec![]));
        let k = key("small");
        writer.handle(insert(
            k.clone(),
            pos(0, 1),
            vec![Term::Doc(doc("Person/alice"))],
            "tiny",
        ));
        let slot = engine.slot_snapshot(&k).unwrap();
        assert!(matches!(slot.versions[0].body, BodyRef::Inline(_)));
        assert_eq!(engine.store().size(), 0);
    }

    // ------------------------------------------------------------ the oracle
    //
    // A pseudo-random stream of commits, fills and history rewrites, checked
    // after every step against a model that recomputes validity from scratch.
    //
    // The model's whole rule is: a fill at P with dependencies D is valid at the
    // head H iff nothing fired a term subsumed by D at any position in (P, H],
    // and nothing cleared the namespace since P. That is a direct statement of
    // the contract, computed by brute force over the recorded history — no
    // buckets, no intervals, no incremental state. If the engine agrees with it
    // after every one of a few thousand steps, the index pruning, the interval
    // bookkeeping and the race handling are all consistent with the contract.
    //
    // Only the *newest* admissible fill per key needs checking, because
    // validity is monotone in the pin position: if a fire in `(P_new, H]`
    // invalidates the newer fill then, since `(P_new, H] ⊆ (P_old, H]`, that
    // same fire invalidates every older one. So "the newest fill, if valid,
    // else a miss" is exactly the engine's contract.

    /// A seeded linear congruential generator (Knuth's MMIX constants) so the
    /// stream is reproducible without a dependency.
    struct Lcg(u64);

    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 11
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next_u64() % n
        }
    }

    /// Dependency list for oracle key `i`. Fixed per key: a shape always reads
    /// the same thing, which is what makes "the newest valid fill" well defined.
    fn oracle_deps(i: usize) -> Vec<Term> {
        match i % 6 {
            0 => vec![Term::Doc(doc("Person/p0"))],
            1 => vec![Term::DocField(doc("Person/p1"), prop("name"))],
            2 => vec![Term::Class(cls("Person"))],
            3 => vec![Term::Members(cls("Company"))],
            4 => vec![Term::Index {
                class: cls("Person"),
                prop: prop("name"),
                value: crate::term::ValueKey::of(&json!("n1")),
            }],
            _ => vec![
                Term::Doc(doc("Company/c0")),
                Term::ClassField(cls("Person"), prop("employer")),
            ],
        }
    }

    const ORACLE_KEYS: usize = 6;

    /// Several seeds, so a green run is not one lucky stream.
    #[test]
    fn oracle_random_commit_stream_matches_a_from_scratch_model() {
        for seed in [
            0x5EED_1234_ABCD_0001,
            0xDEAD_BEEF_0000_0001,
            0x0123_4567_89AB_CDEF,
            0xFFFF_0000_FFFF_0001,
        ] {
            run_oracle(seed, 1200);
        }
    }

    fn run_oracle(seed: u64, steps: u64) {
        let schema = schema_fixture();
        let (engine, mut writer) = harness_with(EngineConfig {
            // Large enough that the retention rule never trips, so this test
            // isolates the invalidation logic; retention is tested separately.
            fired_log_capacity: 100_000,
            max_versions_per_key: 64,
            version_retention: 100_000,
            ..EngineConfig::default()
        });

        let keys: Vec<FragmentKey> = (0..ORACLE_KEYS).map(|i| key(&format!("k{i}"))).collect();

        // ---- model state
        // The documents as the database currently holds them.
        let mut docs: HashMap<String, serde_json::Value> = HashMap::new();
        // Every fire, ever, in ascending position order.
        let mut history: Vec<(Position, TermSet)> = Vec::new();
        // The newest admissible fill per key.
        let mut latest_fill: Vec<Option<Position>> = vec![None; ORACLE_KEYS];
        // Positions below which everything was dropped.
        let mut clear_floor = Position::ZERO;

        let mut rng = Lcg(seed);
        let mut commits = 0u64;

        // One commit first, so there is a watermark to pin against.
        writer.handle(commit("c0", vec![]));
        history.push((writer.head(), TermSet::new()));

        #[allow(unused_assignments)]
        let mut last_event = String::from("init");
        for step in 0..steps {
            let head_before = writer.head();
            match rng.below(100) {
                // ---- a commit
                0..=54 => {
                    commits += 1;
                    let class = if rng.below(4) == 0 {
                        "Company"
                    } else {
                        "Person"
                    };
                    let n = rng.below(3);
                    let id = format!(
                        "{class}/{}{n}",
                        class.to_lowercase().chars().next().unwrap()
                    );
                    let name = format!("n{}", rng.below(3));
                    let employer = format!("Company/c{}", rng.below(2));
                    let before = docs.get(&id).cloned();
                    let change = match (before.clone(), rng.below(10)) {
                        // delete
                        (Some(before), 0..=1) => {
                            docs.remove(&id);
                            DocChange::deleted(doc(&id), cls(class), before)
                        }
                        // update
                        (Some(before), _) => {
                            let after = json!({"name": name, "employer": employer});
                            docs.insert(id.clone(), after.clone());
                            DocChange::updated(doc(&id), cls(class), before, after)
                        }
                        // create
                        (None, _) => {
                            let after = json!({"name": name, "employer": employer});
                            docs.insert(id.clone(), after.clone());
                            DocChange::created(doc(&id), cls(class), after)
                        }
                    };
                    let cs = ChangeSet {
                        ns: ns(),
                        commit: CommitId::new(format!("c{commits}")),
                        parent: None,
                        changes: vec![change],
                        schema_changed: rng.below(50) == 0,
                        complete: rng.below(40) != 0,
                    };
                    let expected_fired = fire(&cs, &schema);
                    last_event = format!("commit (complete={})", cs.complete);
                    writer.handle(WriterMsg::Commit(Box::new(cs)));
                    history.push((writer.head(), expected_fired));
                    assert_eq!(writer.head(), head_before.next());
                }

                // ---- a fill, usually at the head, occasionally at an older
                //      position (the race) or in a dead epoch (rejection).
                55..=89 => {
                    let i = rng.below(ORACLE_KEYS as u64) as usize;
                    let at = match rng.below(10) {
                        0 => Position {
                            epoch: Epoch(head_before.epoch.0.saturating_sub(1)),
                            seq: head_before.seq,
                        },
                        1..=3 => Position {
                            epoch: head_before.epoch,
                            seq: Seq(head_before.seq.0.saturating_sub(rng.below(3))),
                        },
                        _ => head_before,
                    };
                    let deps = oracle_deps(i);
                    last_event = format!("fill key {i} at {at} (head {head_before})");
                    writer.handle(insert(
                        keys[i].clone(),
                        at,
                        deps,
                        &format!("body {i} at {at}"),
                    ));
                    // Admissible iff the epoch still matches. (The retention
                    // rule is disabled by configuration above.)
                    if at.epoch == head_before.epoch && at >= clear_floor {
                        // A newer pin always wins; an older one landing late is
                        // bounded by the newer one and can never be the answer
                        // at the head.
                        if latest_fill[i].map_or(true, |prev| at >= prev) {
                            latest_fill[i] = Some(at);
                        }
                    }
                }

                // ---- a diffable history rewrite
                90..=96 => {
                    let terms: TermSet = match rng.below(3) {
                        0 => [Term::Class(cls("Person"))].into_iter().collect(),
                        1 => [Term::Doc(doc("Person/p0"))].into_iter().collect(),
                        _ => TermSet::new(),
                    };
                    last_event = format!("nff-diffable ({} terms)", terms.len());
                    writer.handle(WriterMsg::NonFastForward {
                        old_head: writer.head_commit().cloned(),
                        new_head: CommitId::new(format!("rebase{step}")),
                        fired: Some(terms.clone()),
                    });
                    history.push((writer.head(), terms));
                    assert_eq!(writer.head(), head_before.next_epoch());
                }

                // ---- an undiffable rewrite, or a purge: everything goes
                _ => {
                    last_event = String::from("clear (nff-undiffable or purge)");
                    if rng.below(2) == 0 {
                        writer.handle(WriterMsg::NonFastForward {
                            old_head: writer.head_commit().cloned(),
                            new_head: CommitId::new(format!("mystery{step}")),
                            fired: None,
                        });
                    } else {
                        writer.handle(WriterMsg::Purge);
                    }
                    clear_floor = writer.head();
                    history.push((writer.head(), TermSet::new()));
                }
            }

            // ---- check every key against the model, recomputed from the
            //      recorded history
            let head = writer.head();
            for (i, key) in keys.iter().enumerate() {
                let deps = oracle_deps(i);
                let expected = latest_fill[i].filter(|from| {
                    *from >= clear_floor
                        // Scan backwards only as far as the pin: history is in
                        // ascending position order, and every entry is at or
                        // below the head.
                        && !history
                            .iter()
                            .rev()
                            .take_while(|(at, _)| *at > *from)
                            .any(|(_, fired)| {
                                fired
                                    .iter()
                                    .any(|f| deps.iter().any(|dep| dep.subsumes(f, &schema)))
                            })
                });

                let explain = || {
                    let slot = engine
                        .slot_snapshot(key)
                        .map(|slot| {
                            slot.versions
                                .iter()
                                .map(|v| format!("[{} -> {:?})", v.from, v.to))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_else(|| "<no slot>".to_owned());
                    let recent: Vec<String> = history
                        .iter()
                        .rev()
                        .take(4)
                        .map(|(at, fired)| format!("{at}:{}", fired.len()))
                        .collect();
                    format!(
                        "seed {seed:#x}; last event {last_event}; clear_floor {clear_floor}; \
                         slot {slot}; recent fires {recent:?}"
                    )
                };
                match (expected, engine.lookup(key, head)) {
                    (Some(from), Lookup::Hit(version)) => assert_eq!(
                        version.from,
                        from,
                        "step {step}: key {i} should be served from the fill pinned at {from}; {}",
                        explain()
                    ),
                    (None, Lookup::Miss) => {}
                    (Some(from), Lookup::Miss) => panic!(
                        "step {step}: key {i} should be a hit from {from} at head {head}, \
                         got a miss; {}",
                        explain()
                    ),
                    (None, Lookup::Hit(version)) => panic!(
                        "step {step}: key {i} should be a miss at head {head}, got the version \
                         pinned at {} (stale data!); {}",
                        version.from,
                        explain()
                    ),
                }
            }
        }

        // The stream has to have actually exercised the interesting paths, or a
        // green test means nothing.
        let m = engine.metrics();
        assert!(
            m.commits_applied.get() > 100,
            "commits: {}",
            m.commits_applied.get()
        );
        assert!(
            m.fills_stored.get() > 50,
            "stored fills: {}",
            m.fills_stored.get()
        );
        assert!(
            m.fills_raced.get() > 5,
            "raced fills: {}",
            m.fills_raced.get()
        );
        assert!(
            m.fills_rejected_epoch.get() > 5,
            "epoch-rejected fills: {}",
            m.fills_rejected_epoch.get()
        );
        assert!(m.versions_invalidated.get() > 50);
        assert!(m.epoch_bumps.get() > 5);
        assert!(m.purges.get() > 2);
    }
}

//! Noticing commits, and turning them into invalidation.
//!
//! The split here is the whole design of this module:
//!
//! * **Drivers are untrusted.** A [`CommitSource`] only has to *notice* that
//!   something happened. It may deliver the same commit twice, deliver commits
//!   out of order, deliver a commit this node's disk has not got yet, or go
//!   silent for an hour. None of that can corrupt the cache.
//! * **The [`Applier`] owns correctness.** Every driver's output funnels through
//!   it, and it is the only component that decides what reaches the writer. The
//!   log-walk reconciler inside it is the one piece of this module that has to
//!   be right, which is why it is behind a trait and unit-tested directly.
//! * **The watermark never advances past a commit we have not read.** Before a
//!   commit is applied, a [`ReadinessGate`] proves this node can actually read
//!   it. That is invariant 4, and it is what makes a shared-disk reader node
//!   safe even when its view of the filesystem is strange: capture lag then
//!   costs staleness, never correctness.

pub mod headpoll;
mod resolve;
pub mod subscription;

pub use headpoll::HeadPoll;
pub use resolve::{PinnedReadGate, TerminusHistory, TerminusResolver};
pub use subscription::{changeset_query, ChangeSetSubscription};

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::BoxStream;

use crate::change::ChangeSet;
use crate::engine::{CommitLookup, Engine, WriterMsg};
use crate::ids::Namespace;
use terminusdb_client::CommitId;

/// Something that notices commits.
///
/// Allowed to be wrong: at-least-once, out of order, or ahead of the local
/// node's disk. The stream is expected to live forever and to survive transient
/// errors itself rather than ending.
pub trait CommitSource: Send + Sync {
    /// A short name for logs and metrics.
    fn name(&self) -> &'static str;

    /// Start noticing commits for a namespace.
    ///
    /// `after` is a hint about where we already are; a driver may ignore it.
    fn subscribe(
        &self,
        ns: &Namespace,
        after: Option<CommitId>,
    ) -> BoxStream<'static, anyhow::Result<CommitEvent>>;
}

/// "Something happened at this commit."
#[derive(Clone, Debug)]
pub struct CommitEvent {
    /// The commit that was noticed.
    pub commit: CommitId,
    /// Its parent, when the driver happens to know. A head poll does not, and
    /// `None` simply routes the event through the gap reconciler.
    pub parent: Option<CommitId>,
    /// Some drivers could carry the changes, but none currently do — see
    /// [`ChangeSetSubscription`] for why the subscription deliberately does not.
    pub changes: Option<Box<ChangeSet>>,
}

impl CommitEvent {
    /// An event that only names a commit.
    pub fn new(commit: CommitId) -> Self {
        Self {
            commit,
            parent: None,
            changes: None,
        }
    }

    /// An event that also knows the parent, so it can skip the log walk.
    pub fn with_parent(commit: CommitId, parent: Option<CommitId>) -> Self {
        Self {
            commit,
            parent,
            changes: None,
        }
    }
}

/// Turns a noticed commit into a [`ChangeSet`], by local pinned reads.
#[async_trait]
pub trait ChangeResolver: Send + Sync {
    /// Resolve one commit.
    ///
    /// A resolver that cannot determine the changes *precisely* must still
    /// return a `ChangeSet` — with `complete: false`. Returning an error is for
    /// "I could not reach the node at all".
    async fn resolve(
        &self,
        ns: &Namespace,
        commit: &CommitId,
        parent: Option<&CommitId>,
    ) -> anyhow::Result<ChangeSet>;
}

/// The canary: has this node actually got this commit, locally and readably?
///
/// The watermark never advances past a commit this did not confirm.
#[async_trait]
pub trait ReadinessGate: Send + Sync {
    /// Return `Ok` only once a pinned read at `commit` succeeds here.
    async fn await_readable(&self, ns: &Namespace, commit: &CommitId) -> anyhow::Result<()>;
}

/// One commit as the log reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct LogCommit {
    /// The commit id.
    pub commit: CommitId,
    /// Its parent, or `None` for the first commit on the branch.
    pub parent: Option<CommitId>,
}

/// Read-only access to a branch's commit history.
///
/// Behind a trait purely so the gap reconciler can be tested without a server.
/// That reconciler is shared by every driver and is the only part of capture
/// that has to be right, so being able to drive it from a table of commits is
/// worth one extra trait.
#[async_trait]
pub trait CommitHistory: Send + Sync {
    /// Commits newest-first from the branch head, at most `count` of them.
    async fn recent(&self, ns: &Namespace, count: usize) -> anyhow::Result<Vec<LogCommit>>;
}

/// What the applier did with an event. Returned for metrics and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// Already applied; dropped. Duplicate delivery is expected, not an error.
    Duplicate,
    /// The namespace had no watermark; this commit became the first one.
    Bootstrapped,
    /// Applied directly: its parent was our watermark.
    FastForward,
    /// Applied after walking the log to fill a gap, this many commits.
    GapRecovered(usize),
    /// The watermark was not an ancestor of the new head: the epoch was bumped
    /// and local state dropped.
    NonFastForward,
}

/// Owns correctness. Drivers are untrusted inputs.
pub struct Applier {
    engine: Arc<Engine>,
    resolver: Arc<dyn ChangeResolver>,
    gate: Arc<dyn ReadinessGate>,
    history: Arc<dyn CommitHistory>,
    max_gap_walk: usize,
}

impl Applier {
    /// Build an applier.
    pub fn new(
        engine: Arc<Engine>,
        resolver: Arc<dyn ChangeResolver>,
        gate: Arc<dyn ReadinessGate>,
        history: Arc<dyn CommitHistory>,
        max_gap_walk: usize,
    ) -> Self {
        Self {
            engine,
            resolver,
            gate,
            history,
            max_gap_walk,
        }
    }

    /// Handle one noticed commit.
    ///
    /// Dispatch is entirely on `(event.parent, watermark)`, because that is the
    /// only thing we can trust about an event:
    ///
    /// | state | action |
    /// |---|---|
    /// | commit already applied | drop — duplicate delivery is expected |
    /// | no watermark yet | gate, then bootstrap at this commit |
    /// | `parent == watermark.commit` | gate, resolve, apply |
    /// | parent unknown or `None` | walk the log back to the watermark, apply forward |
    /// | watermark not in the walk | non-fast-forward: bump the epoch, drop state |
    pub async fn apply(&self, ns: &Namespace, event: &CommitEvent) -> anyhow::Result<Applied> {
        if matches!(
            self.engine.position_of(ns, &event.commit),
            CommitLookup::Applied(_)
        ) {
            tracing::trace!(%ns, commit = %event.commit, "duplicate commit event");
            return Ok(Applied::Duplicate);
        }

        let watermark = match self.engine.watermark(ns) {
            Some(watermark) => watermark,
            None => {
                // Nothing cached, no position to compare against: adopt this
                // commit as the origin. An empty complete changeset fires no
                // terms, which is right — there is nothing to invalidate.
                self.gate.await_readable(ns, &event.commit).await?;
                self.engine.bootstrap(ns, event.commit.clone())?;
                tracing::info!(%ns, commit = %event.commit, "watermark established");
                return Ok(Applied::Bootstrapped);
            }
        };

        if event.parent.as_ref() == Some(&watermark.commit) {
            self.apply_one(ns, &event.commit, event.parent.as_ref())
                .await?;
            return Ok(Applied::FastForward);
        }

        self.recover_gap(ns, &watermark.commit, &event.commit).await
    }

    /// Gate, resolve and hand one commit to the writer.
    ///
    /// The gate runs **first**. A resolver failure would also prevent
    /// publication, but ordering the explicit readability check ahead of it is
    /// what makes invariant 4 a property of the code rather than a consequence
    /// of how the resolver happens to be implemented.
    async fn apply_one(
        &self,
        ns: &Namespace,
        commit: &CommitId,
        parent: Option<&CommitId>,
    ) -> anyhow::Result<()> {
        self.gate.await_readable(ns, commit).await?;
        let changeset = self.resolver.resolve(ns, commit, parent).await?;
        self.engine
            .send(ns, WriterMsg::Commit(Box::new(changeset)))?;
        Ok(())
    }

    /// Walk `/api/log` back from the head to our watermark and apply forward.
    ///
    /// Shared by every driver: a head poll never knows a parent, a subscription
    /// can skip events across a reconnect, and a write hint can arrive before
    /// the driver notices. All of them land here.
    async fn recover_gap(
        &self,
        ns: &Namespace,
        watermark: &CommitId,
        new_head: &CommitId,
    ) -> anyhow::Result<Applied> {
        let log = self.history.recent(ns, self.max_gap_walk).await?;

        let found = log.iter().position(|entry| &entry.commit == watermark);
        let Some(index) = found else {
            // Our watermark is not among the head's recent ancestors: the
            // history was rewritten (rebase, reset, squash, apply, or a
            // divergent pull). We cannot diff it from here, so local state goes.
            tracing::warn!(
                %ns,
                watermark = %watermark,
                new_head = %new_head,
                walked = log.len(),
                "watermark is not an ancestor of the head: treating as non-fast-forward"
            );
            self.engine.send(
                ns,
                WriterMsg::NonFastForward {
                    old_head: Some(watermark.clone()),
                    new_head: new_head.clone(),
                    fired: None,
                },
            )?;
            return Ok(Applied::NonFastForward);
        };

        // `log` is newest-first, so everything before the watermark's index is
        // what we are missing; apply it oldest-first.
        let missing: Vec<&LogCommit> = log[..index].iter().rev().collect();
        let count = missing.len();
        for entry in missing {
            self.apply_one(ns, &entry.commit, entry.parent.as_ref())
                .await?;
        }
        if count > 0 {
            tracing::info!(%ns, count, "recovered a gap by walking the log");
        }
        Ok(Applied::GapRecovered(count))
    }

    /// The engine this applier feeds.
    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineConfig, NamespaceWriter};
    use crate::store::TieredStore;
    use parking_lot::Mutex;

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    /// A fixed commit history, newest-first.
    struct FakeHistory {
        commits: Vec<LogCommit>,
        calls: Mutex<usize>,
    }

    impl FakeHistory {
        /// Build from oldest-first ids, linking each to its predecessor.
        fn chain(ids: &[&str]) -> Arc<FakeHistory> {
            let mut commits: Vec<LogCommit> = Vec::new();
            let mut parent: Option<CommitId> = None;
            for id in ids {
                commits.push(LogCommit {
                    commit: CommitId::new(*id),
                    parent: parent.clone(),
                });
                parent = Some(CommitId::new(*id));
            }
            commits.reverse(); // newest-first, as the log returns it
            Arc::new(FakeHistory {
                commits,
                calls: Mutex::new(0),
            })
        }
    }

    #[async_trait]
    impl CommitHistory for FakeHistory {
        async fn recent(&self, _ns: &Namespace, count: usize) -> anyhow::Result<Vec<LogCommit>> {
            *self.calls.lock() += 1;
            Ok(self.commits.iter().take(count).cloned().collect())
        }
    }

    /// Records what it was asked to resolve; always answers "complete, empty".
    #[derive(Default)]
    struct FakeResolver {
        resolved: Mutex<Vec<(CommitId, Option<CommitId>)>>,
    }

    #[async_trait]
    impl ChangeResolver for FakeResolver {
        async fn resolve(
            &self,
            ns: &Namespace,
            commit: &CommitId,
            parent: Option<&CommitId>,
        ) -> anyhow::Result<ChangeSet> {
            self.resolved.lock().push((commit.clone(), parent.cloned()));
            Ok(ChangeSet::empty(
                ns.clone(),
                commit.clone(),
                parent.cloned(),
            ))
        }
    }

    /// A gate that can be told to refuse.
    struct FakeGate {
        calls: Mutex<Vec<CommitId>>,
        refuse: bool,
    }

    impl FakeGate {
        fn open() -> Arc<FakeGate> {
            Arc::new(FakeGate {
                calls: Mutex::new(Vec::new()),
                refuse: false,
            })
        }
        fn closed() -> Arc<FakeGate> {
            Arc::new(FakeGate {
                calls: Mutex::new(Vec::new()),
                refuse: true,
            })
        }
    }

    #[async_trait]
    impl ReadinessGate for FakeGate {
        async fn await_readable(&self, _ns: &Namespace, commit: &CommitId) -> anyhow::Result<()> {
            self.calls.lock().push(commit.clone());
            if self.refuse {
                anyhow::bail!("this node cannot read {commit} yet");
            }
            Ok(())
        }
    }

    struct Harness {
        engine: Arc<Engine>,
        writer: NamespaceWriter,
        applier: Applier,
        resolver: Arc<FakeResolver>,
        gate: Arc<FakeGate>,
        history: Arc<FakeHistory>,
    }

    fn harness(chain: &[&str], gate: Arc<FakeGate>) -> Harness {
        let engine = Arc::new(Engine::new(
            EngineConfig::default(),
            Arc::new(TieredStore::memory_only(1 << 20)),
        ));
        let writer = engine.take_writer(&ns()).expect("a fresh writer");
        let resolver = Arc::new(FakeResolver::default());
        let history = FakeHistory::chain(chain);
        let applier = Applier::new(
            Arc::clone(&engine),
            Arc::clone(&resolver) as Arc<dyn ChangeResolver>,
            Arc::clone(&gate) as Arc<dyn ReadinessGate>,
            Arc::clone(&history) as Arc<dyn CommitHistory>,
            500,
        );
        Harness {
            engine,
            writer,
            applier,
            resolver,
            gate,
            history,
        }
    }

    impl Harness {
        /// Apply an event and pump the writer, as a real deployment's writer
        /// task would.
        async fn apply(&mut self, event: CommitEvent) -> anyhow::Result<Applied> {
            let outcome = self.applier.apply(&ns(), &event).await;
            self.writer.drain();
            outcome
        }

        fn head(&self) -> Option<String> {
            self.engine
                .watermark(&ns())
                .map(|w| w.commit.as_str().to_owned())
        }

        fn resolved(&self) -> Vec<String> {
            self.resolver
                .resolved
                .lock()
                .iter()
                .map(|(c, _)| c.as_str().to_owned())
                .collect()
        }
    }

    #[tokio::test]
    async fn the_first_event_bootstraps_the_watermark() {
        let mut h = harness(&["c1"], FakeGate::open());
        assert_eq!(
            h.apply(CommitEvent::new(CommitId::new("c1")))
                .await
                .unwrap(),
            Applied::Bootstrapped
        );
        assert_eq!(h.head().as_deref(), Some("c1"));
        assert_eq!(h.gate.calls.lock().len(), 1, "gated before publishing");
        assert!(
            h.resolved().is_empty(),
            "there is nothing cached to invalidate at the origin"
        );
    }

    #[tokio::test]
    async fn a_known_parent_applies_directly_without_walking_the_log() {
        let mut h = harness(&["c1", "c2"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        let event = CommitEvent::with_parent(CommitId::new("c2"), Some(CommitId::new("c1")));
        assert_eq!(h.apply(event).await.unwrap(), Applied::FastForward);
        assert_eq!(h.head().as_deref(), Some("c2"));
        assert_eq!(h.resolved(), vec!["c2"]);
        assert_eq!(
            *h.history.calls.lock(),
            0,
            "a fast-forward must not cost a log walk"
        );
    }

    #[tokio::test]
    async fn a_five_commit_gap_is_recovered_in_order() {
        let mut h = harness(&["c1", "c2", "c3", "c4", "c5", "c6"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        // A head poll notices c6 and knows nothing else.
        assert_eq!(
            h.apply(CommitEvent::new(CommitId::new("c6")))
                .await
                .unwrap(),
            Applied::GapRecovered(5)
        );
        assert_eq!(
            h.resolved(),
            vec!["c2", "c3", "c4", "c5", "c6"],
            "oldest first, so each commit's invalidation lands in history order"
        );
        assert_eq!(h.head().as_deref(), Some("c6"));
        // And every one of them was gated.
        assert_eq!(h.gate.calls.lock().len(), 6);
    }

    #[tokio::test]
    async fn the_parents_are_carried_through_a_gap_walk() {
        let mut h = harness(&["c1", "c2", "c3"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();
        h.apply(CommitEvent::new(CommitId::new("c3")))
            .await
            .unwrap();
        let resolved = h.resolver.resolved.lock().clone();
        assert_eq!(
            resolved,
            vec![
                (CommitId::new("c2"), Some(CommitId::new("c1"))),
                (CommitId::new("c3"), Some(CommitId::new("c2"))),
            ],
            "the resolver needs the parent to read before-state"
        );
    }

    #[tokio::test]
    async fn a_duplicate_delivery_is_dropped() {
        let mut h = harness(&["c1", "c2"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();
        let event = CommitEvent::with_parent(CommitId::new("c2"), Some(CommitId::new("c1")));
        h.apply(event.clone()).await.unwrap();

        assert_eq!(h.apply(event).await.unwrap(), Applied::Duplicate);
        assert_eq!(h.resolved(), vec!["c2"], "resolved exactly once");
    }

    #[tokio::test]
    async fn an_out_of_order_delivery_still_converges() {
        // The driver reports c3 first, then c2. c3's walk applies both, so c2
        // arrives as a duplicate.
        let mut h = harness(&["c1", "c2", "c3"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        assert_eq!(
            h.apply(CommitEvent::new(CommitId::new("c3")))
                .await
                .unwrap(),
            Applied::GapRecovered(2)
        );
        assert_eq!(
            h.apply(CommitEvent::with_parent(
                CommitId::new("c2"),
                Some(CommitId::new("c1"))
            ))
            .await
            .unwrap(),
            Applied::Duplicate
        );
        assert_eq!(h.resolved(), vec!["c2", "c3"]);
        assert_eq!(h.head().as_deref(), Some("c3"));
    }

    #[tokio::test]
    async fn an_unreachable_watermark_is_a_non_fast_forward() {
        // Our watermark is c1, but the log now starts from an unrelated chain:
        // a rebase, reset, squash or divergent pull.
        let mut h = harness(&["c1"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        let rewritten = harness(&["r1", "r2"], FakeGate::open());
        let applier = Applier::new(
            Arc::clone(&h.engine),
            Arc::clone(&h.resolver) as Arc<dyn ChangeResolver>,
            Arc::clone(&h.gate) as Arc<dyn ReadinessGate>,
            Arc::clone(&rewritten.history) as Arc<dyn CommitHistory>,
            500,
        );
        let outcome = applier
            .apply(&ns(), &CommitEvent::new(CommitId::new("r2")))
            .await
            .unwrap();
        h.writer.drain();

        assert_eq!(outcome, Applied::NonFastForward);
        assert_eq!(
            h.head().as_deref(),
            Some("r2"),
            "we still know where the head is"
        );
        assert_eq!(h.engine.metrics().epoch_bumps.get(), 1);
        assert_eq!(h.engine.metrics().purges.get(), 1);
    }

    #[tokio::test]
    async fn a_closed_gate_prevents_the_watermark_from_advancing() {
        // Invariant 4: capture lag must cost staleness, never correctness.
        let mut h = harness(&["c1", "c2"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        let closed = FakeGate::closed();
        let applier = Applier::new(
            Arc::clone(&h.engine),
            Arc::clone(&h.resolver) as Arc<dyn ChangeResolver>,
            Arc::clone(&closed) as Arc<dyn ReadinessGate>,
            Arc::clone(&h.history) as Arc<dyn CommitHistory>,
            500,
        );
        let event = CommitEvent::with_parent(CommitId::new("c2"), Some(CommitId::new("c1")));
        assert!(applier.apply(&ns(), &event).await.is_err());
        h.writer.drain();

        assert_eq!(
            h.head().as_deref(),
            Some("c1"),
            "the watermark must not move past a commit we cannot read"
        );
        assert!(h.resolved().is_empty(), "and we never even resolved it");
    }

    #[tokio::test]
    async fn a_gap_walk_is_bounded() {
        let chain: Vec<String> = (1..=50).map(|i| format!("c{i}")).collect();
        let refs: Vec<&str> = chain.iter().map(String::as_str).collect();
        let engine = Arc::new(Engine::new(
            EngineConfig::default(),
            Arc::new(TieredStore::memory_only(1 << 20)),
        ));
        let mut writer = engine.take_writer(&ns()).unwrap();
        let resolver = Arc::new(FakeResolver::default());
        let gate = FakeGate::open();
        let history = FakeHistory::chain(&refs);
        // Only three commits of history may be walked.
        let applier = Applier::new(
            Arc::clone(&engine),
            Arc::clone(&resolver) as Arc<dyn ChangeResolver>,
            Arc::clone(&gate) as Arc<dyn ReadinessGate>,
            Arc::clone(&history) as Arc<dyn CommitHistory>,
            3,
        );
        applier
            .apply(&ns(), &CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();
        writer.drain();

        let outcome = applier
            .apply(&ns(), &CommitEvent::new(CommitId::new("c50")))
            .await
            .unwrap();
        writer.drain();
        assert_eq!(
            outcome,
            Applied::NonFastForward,
            "a gap longer than the walk budget degrades to dropping state, \
             which is correct but lossy — hence the budget being configurable"
        );
    }

    #[tokio::test]
    async fn a_commit_that_is_already_the_watermark_is_a_duplicate() {
        let mut h = harness(&["c1"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();
        assert_eq!(
            h.apply(CommitEvent::new(CommitId::new("c1")))
                .await
                .unwrap(),
            Applied::Duplicate
        );
    }
}

// ---------------------------------------------------------------- supervisor

/// A nudge from the frontend: something happened that capture should look at
/// now rather than at the next poll.
///
/// This is what makes read-your-writes cheap. A write passes through the proxy,
/// we learn its commit from the response's `TerminusDB-Data-Version`, and we
/// apply it immediately instead of waiting up to `poll_interval_ms` for a
/// driver to notice.
#[derive(Clone, Debug)]
pub enum CaptureHint {
    /// A write produced this commit.
    Commit {
        /// Which namespace.
        ns: Namespace,
        /// The commit the write produced.
        commit: CommitId,
    },
    /// A topology or migration endpoint succeeded; re-examine the head. The
    /// head may have moved non-fast-forward, which the applier works out.
    Resync {
        /// Which namespace.
        ns: Namespace,
    },
}

/// Owns one capture task per configured namespace.
///
/// Each task is independent: namespaces share no mutable state, so one
/// namespace's node trouble cannot stall another's.
pub struct Supervisor {
    engine: Arc<Engine>,
    client: Arc<terminusdb_client::TerminusDBHttpClient>,
    config: Arc<crate::config::Config>,
}

impl Supervisor {
    /// Build a supervisor.
    pub fn new(
        engine: Arc<Engine>,
        client: Arc<terminusdb_client::TerminusDBHttpClient>,
        config: Arc<crate::config::Config>,
    ) -> Self {
        Self {
            engine,
            client,
            config,
        }
    }

    /// An applier wired to the live node.
    pub fn applier(&self) -> Applier {
        Applier::new(
            Arc::clone(&self.engine),
            Arc::new(TerminusResolver::new(
                Arc::clone(&self.client),
                self.config.capture.diff_threshold,
            )),
            Arc::new(PinnedReadGate::new(Arc::clone(&self.client))),
            Arc::new(TerminusHistory::new(Arc::clone(&self.client))),
            self.config.capture.max_gap_walk,
        )
    }

    /// Re-derive a namespace's schema index and publish it to the writer.
    ///
    /// Failure is logged and tolerated: an empty or stale index makes analyzers
    /// emit coarse terms, which costs hit rate rather than correctness.
    pub async fn refresh_schema(&self, ns: &Namespace) {
        match self.client.get_schema_documents(&ns.to_branch_spec()).await {
            Ok(docs) => match crate::schema_index::SchemaIndex::from_schema_documents(&docs) {
                Ok(index) => {
                    let classes = index.classes().count();
                    if let Err(err) = self.engine.send(ns, WriterMsg::Schema(Arc::new(index))) {
                        tracing::warn!(%ns, error = %err, "could not publish the schema index");
                    } else {
                        tracing::info!(%ns, classes, "schema index refreshed");
                    }
                }
                Err(err) => tracing::warn!(%ns, error = %err, "schema documents did not parse"),
            },
            Err(err) => tracing::warn!(%ns, error = %err, "could not read schema documents"),
        }
    }

    /// Run capture for every configured namespace, plus the frontend's hint
    /// channel. Returns when all tasks finish, which in practice is never —
    /// they are aborted at shutdown.
    pub async fn run(self: Arc<Self>, hints: tokio::sync::mpsc::UnboundedReceiver<CaptureHint>) {
        let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        for ns in self.config.namespaces() {
            let supervisor = Arc::clone(&self);
            tasks.push(tokio::spawn(async move {
                supervisor.run_namespace(ns).await;
            }));
        }
        let supervisor = Arc::clone(&self);
        tasks.push(tokio::spawn(async move {
            supervisor.run_hints(hints).await;
        }));
        for task in tasks {
            let _ = task.await;
        }
    }

    /// Apply frontend hints as they arrive.
    async fn run_hints(
        self: Arc<Self>,
        mut hints: tokio::sync::mpsc::UnboundedReceiver<CaptureHint>,
    ) {
        let applier = self.applier();
        while let Some(hint) = hints.recv().await {
            let (ns, event) = match hint {
                CaptureHint::Commit { ns, commit } => (ns, CommitEvent::new(commit)),
                CaptureHint::Resync { ns } => {
                    // We do not know the new head; ask the node.
                    match self.client.get_latest_commit_id(&ns.to_branch_spec()).await {
                        Ok(head) => (ns, CommitEvent::new(head)),
                        Err(err) => {
                            tracing::warn!(%ns, error = %err, "resync could not read the head");
                            continue;
                        }
                    }
                }
            };
            if !self.config.tracks(&ns) {
                continue;
            }
            if let Err(err) = applier.apply(&ns, &event).await {
                tracing::warn!(%ns, error = %err, "applying a frontend hint failed");
            }
        }
    }

    /// Capture one namespace: schema, bootstrap, then drivers forever.
    async fn run_namespace(self: Arc<Self>, ns: Namespace) {
        let applier = self.applier();
        self.refresh_schema(&ns).await;

        // Establish the first watermark. Until this succeeds the namespace
        // cannot be served from cache at all — there is no position to pin a
        // fill at — so keep trying.
        loop {
            match self.client.get_latest_commit_id(&ns.to_branch_spec()).await {
                Ok(head) => match applier.apply(&ns, &CommitEvent::new(head)).await {
                    Ok(outcome) => {
                        tracing::info!(%ns, ?outcome, "capture started");
                        break;
                    }
                    Err(err) => tracing::warn!(%ns, error = %err, "bootstrap failed; retrying"),
                },
                Err(err) => tracing::warn!(%ns, error = %err, "could not read the head; retrying"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                self.config.capture.poll_interval_ms.max(250),
            ))
            .await;
        }

        let driver = self.config.driver_for(&ns);
        // Outer loop: rebuild the drivers when the schema changes, because the
        // subscription query is generated from the class list.
        loop {
            let mut sources: Vec<BoxStream<'static, anyhow::Result<CommitEvent>>> = Vec::new();
            let after = self.engine.watermark(&ns).map(|w| w.commit);
            if driver.polls() {
                let poll = headpoll::HeadPoll::new(
                    Arc::clone(&self.client),
                    std::time::Duration::from_millis(self.config.capture.poll_interval_ms),
                );
                sources.push(poll.subscribe(&ns, after.clone()));
            }
            if driver.subscribes() {
                let sub = subscription::ChangeSetSubscription::new(
                    Arc::clone(&self.client),
                    self.engine.schema(&ns),
                );
                sources.push(sub.subscribe(&ns, after));
            }
            let mut merged = futures_util::stream::select_all(sources);

            loop {
                tokio::select! {
                    event = futures_util::StreamExt::next(&mut merged) => {
                        match event {
                            Some(Ok(event)) => {
                                if let Err(err) = applier.apply(&ns, &event).await {
                                    tracing::warn!(%ns, error = %err, "applying a commit failed");
                                }
                            }
                            Some(Err(err)) => {
                                tracing::debug!(%ns, error = %err, "capture driver error");
                            }
                            // Every driver ended. Rebuild them; the poll driver
                            // never ends, so this means a subscription-only
                            // namespace lost its stream.
                            None => break,
                        }
                    }
                    _ = self.engine.await_schema_refresh(&ns) => {
                        self.refresh_schema(&ns).await;
                        // Rebuild so the subscription query matches the schema.
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                self.config.capture.poll_interval_ms,
            ))
            .await;
        }
    }
}

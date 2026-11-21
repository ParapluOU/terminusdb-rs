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

/// Normalize a commit *document id* to the bare hash a resource path needs.
///
/// TerminusDB reports commits in two shapes: `LogEntry::identifier` and a
/// subscription's `_CommitMetadata._id` are bare hashes or prefixed document ids
/// (`ValidCommit/<hash>`) depending on the source, while `LogEntry::parent` is
/// always the prefixed form. A resource path wants the bare hash: pinning at
/// `…/local/commit/ValidCommit/<hash>` is rejected with `api:InvalidResourcePath`,
/// which previously made every before-state read fail, which made every
/// changeset `complete: false`, which made every commit fire `Everything` — a
/// correct cache with no precision at all, and silent about it.
///
/// Every driver must funnel commit ids through here so the two cannot drift.
pub(crate) fn commit_id_from_any(id: &str) -> Option<CommitId> {
    let hash = id.rsplit('/').next().unwrap_or(id).trim();
    if hash.is_empty() {
        return None;
    }
    Some(CommitId::new(hash))
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
    /// A pinned resource path takes the bare hash. `LogEntry::parent` does not
    /// give you one, and using it verbatim silently costs the cache all of its
    /// precision — see `commit_id_from_any`.
    #[test]
    fn a_prefixed_commit_document_id_normalizes_to_the_bare_hash() {
        let bare = "vjfq3rblf2kyo0pbs60ljjvyyo1uerq";
        for spelling in [
            "vjfq3rblf2kyo0pbs60ljjvyyo1uerq",
            "ValidCommit/vjfq3rblf2kyo0pbs60ljjvyyo1uerq",
            "terminusdb:///data/ValidCommit/vjfq3rblf2kyo0pbs60ljjvyyo1uerq",
        ] {
            assert_eq!(
                commit_id_from_any(spelling).map(|c| c.as_str().to_string()),
                Some(bare.to_string()),
                "{spelling} should normalize to the bare hash"
            );
        }
        assert_eq!(commit_id_from_any(""), None);
        assert_eq!(commit_id_from_any("ValidCommit/"), None);
    }

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

    #[test]
    fn a_subscription_limit_refusal_is_recognized() {
        // The fork answers 429 with this body; the driver surfaces it as an
        // error string, so the classification is on the message.
        for text in [
            "429 Too Many Requests {\"code\":\"TOO_MANY_REQUESTS\",\"message\":\"Subscription limit exceeded\"}",
            "subscription limit exceeded",
            "TOO_MANY_REQUESTS",
            // Verified against the embedded server: the client renders the
            // refusal as "GraphQL subscription refused (429 Too Many
            // Requests): ...", so every real refusal carries one of the
            // phrases above. A bare "429" is deliberately NOT recognized —
            // see `is_subscription_limit`.
            "GraphQL subscription refused (429 Too Many Requests): {\"errors\":[]}",
        ] {
            assert!(
                is_subscription_limit(&anyhow::anyhow!("{text}")),
                "{text:?} must be recognized as a subscription refusal"
            );
        }
    }

    #[test]
    fn an_ordinary_error_is_not_mistaken_for_a_subscription_limit() {
        // Mistaking a transient failure for a cap would permanently demote a
        // namespace to polling for no reason.
        for text in [
            "connection reset by peer",
            "error sending request",
            "500 Internal Server Error",
            "unknown database",
        ] {
            assert!(
                !is_subscription_limit(&anyhow::anyhow!("{text}")),
                "{text:?} must NOT be treated as a subscription refusal"
            );
        }
    }

    #[test]
    fn a_subscription_limit_is_recognized_through_an_error_chain() {
        // The client wraps errors, so the marker is usually not the outermost
        // message.
        let inner = anyhow::anyhow!("TOO_MANY_REQUESTS: Subscription limit exceeded");
        let wrapped = inner.context("opening the _ChangeSet subscription");
        assert!(is_subscription_limit(&wrapped));
    }

    /// Demotion is sticky, so misreading an unrelated error as a cap silently
    /// gives up low-latency capture for the whole process.
    #[test]
    fn an_unrelated_error_that_merely_contains_429_is_not_a_limit() {
        for text in [
            "error sending request for url (http://localhost:42900/api/graphql/admin/db)",
            "connection refused talking to 127.0.0.1:4296",
            "commit 429abc3f is not on this branch",
        ] {
            assert!(
                !is_subscription_limit(&anyhow::anyhow!("{text}")),
                "{text} must not be read as a subscription-limit refusal"
            );
        }
    }

    #[test]
    fn the_class_fingerprint_ignores_changes_the_query_does_not_depend_on() {
        use crate::schema_index::SchemaIndex;
        use terminusdb_format::schema_doc::{RawClass, RawProperty, RawSchema};

        let class = |id: &str, prop: &str| RawClass {
            id: id.to_owned(),
            is_abstract: false,
            is_subdocument: false,
            inherits: Vec::new(),
            properties: vec![RawProperty {
                name: prop.to_owned(),
                class: "xsd:string".to_owned(),
                family: None,
                force_nullable: false,
            }],
        };
        let schema_of = |classes: Vec<RawClass>| {
            SchemaIndex::from_raw(RawSchema {
                classes,
                enums: Vec::new(),
            })
        };

        let before = schema_of(vec![class("Person", "name")]);
        // A renamed property does not change the generated subscription query,
        // so it must not cost us a resubscribe.
        let renamed = schema_of(vec![class("Person", "fullName")]);
        assert_eq!(
            class_fingerprint(&before),
            class_fingerprint(&renamed),
            "only the class list is what the query is generated from"
        );

        // A new class does.
        let added = schema_of(vec![class("Person", "name"), class("Company", "name")]);
        assert_ne!(class_fingerprint(&before), class_fingerprint(&added));

        // And the order the server listed them in must not matter.
        let reordered = schema_of(vec![class("Company", "name"), class("Person", "name")]);
        assert_eq!(class_fingerprint(&added), class_fingerprint(&reordered));
    }

    /// A source that refuses with a 429 the way the server does.
    struct RefusingSource;

    impl CommitSource for RefusingSource {
        fn name(&self) -> &'static str {
            "refusing"
        }

        fn subscribe(
            &self,
            _ns: &Namespace,
            _after: Option<CommitId>,
        ) -> BoxStream<'static, anyhow::Result<CommitEvent>> {
            Box::pin(futures_util::stream::once(async {
                Err(anyhow::anyhow!(
                    "429 Too Many Requests {{\"code\":\"TOO_MANY_REQUESTS\",\"message\":\"Subscription limit exceeded\"}}"
                ))
            }))
        }
    }

    #[tokio::test]
    async fn a_refused_subscription_is_a_demotion_and_polling_still_applies_commits() {
        // The product bug this guards: a Reactor that keeps reopening a refused
        // subscription burns slots and, with `driver = "both"`, loses low-latency
        // capture silently. The refusal must be recognized once, counted, and
        // then left alone while another source carries the namespace.
        use futures_util::StreamExt;
        let mut h = harness(&["c1", "c2"], FakeGate::open());
        h.apply(CommitEvent::new(CommitId::new("c1")))
            .await
            .unwrap();

        // Drain the refusing source exactly as the supervisor's loop does.
        let mut stream = RefusingSource.subscribe(&ns(), None);
        let first = stream.next().await.expect("one event");
        let err = first.expect_err("a refusal");
        assert!(
            is_subscription_limit(&err),
            "the supervisor must classify this as a demotion: {err:#}"
        );
        assert!(
            stream.next().await.is_none(),
            "and the stream ends, so the loop does not spin on it"
        );

        // Meanwhile a poll-shaped event still drives capture.
        let outcome = h
            .apply(CommitEvent::with_parent(
                CommitId::new("c2"),
                Some(CommitId::new("c1")),
            ))
            .await
            .unwrap();
        assert_eq!(outcome, Applied::FastForward);
        assert_eq!(h.head().as_deref(), Some("c2"));
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
    /// Start capturing a namespace that was registered at run time.
    ///
    /// The registry decides *whether* a namespace is tracked; this is what
    /// makes tracking mean something — without a capture task there is no
    /// watermark, and without a watermark nothing in that namespace is served
    /// from cache at all.
    Track {
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
    /// Which namespaces have a capture task, so registration can be idempotent.
    running: dashmap::DashSet<Namespace>,
    /// Shared with the front end: which namespaces are tracked, and how long
    /// since each was read. The second half is what lets an idle namespace be
    /// polled rarely.
    registry: crate::registry::SharedRegistry,
}

impl Supervisor {
    /// Build a supervisor.
    pub fn new(
        engine: Arc<Engine>,
        client: Arc<terminusdb_client::TerminusDBHttpClient>,
        config: Arc<crate::config::Config>,
        registry: crate::registry::SharedRegistry,
    ) -> Self {
        Self {
            engine,
            client,
            config,
            running: dashmap::DashSet::new(),
            registry,
        }
    }

    /// The head-poll interval for a namespace, by how recently it was read.
    ///
    /// The whole point of the idle interval: with a database per tenant, most
    /// are idle most of the time, and polling every head every 150ms to notice
    /// commits nobody is waiting for is the dominant cost of that layout.
    fn poll_interval(&self, ns: &Namespace) -> std::time::Duration {
        let idle_after = std::time::Duration::from_millis(self.config.capture.idle_after_ms);
        let idle = self
            .registry
            .idle_for(ns)
            .map(|idle| idle >= idle_after)
            .unwrap_or(false);
        std::time::Duration::from_millis(if idle {
            self.config.capture.idle_poll_interval_ms
        } else {
            self.config.capture.poll_interval_ms
        })
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
            tasks.push(self.start(ns));
        }
        let supervisor = Arc::clone(&self);
        tasks.push(tokio::spawn(async move {
            supervisor.run_hints(hints).await;
        }));
        for task in tasks {
            let _ = task.await;
        }
    }

    /// Spawn capture for one namespace, once.
    ///
    /// Idempotent because registration is: a client that registers a namespace
    /// on every request must not accumulate capture tasks, and a second task
    /// for one namespace would poll the same head twice and fight over the same
    /// writer mailbox.
    pub fn start(self: &Arc<Self>, ns: Namespace) -> tokio::task::JoinHandle<()> {
        if !self.running.insert(ns.clone()) {
            tracing::debug!(%ns, "capture is already running for this namespace");
            return tokio::spawn(async {});
        }
        // A namespace registered at run time has no writer yet: the writers for
        // configured namespaces are spawned at startup. `take_writer` yields a
        // writer exactly once per namespace, so this cannot produce a second
        // one — the single-writer invariant stays structural rather than
        // becoming a thing to remember here.
        if let Some(writer) = self.engine.take_writer(&ns) {
            tracing::debug!(%ns, "starting a writer for a namespace registered at run time");
            tokio::spawn(writer.run());
        }
        let supervisor = Arc::clone(self);
        tokio::spawn(async move {
            supervisor.run_namespace(ns).await;
        })
    }

    /// Apply frontend hints as they arrive.
    async fn run_hints(
        self: Arc<Self>,
        mut hints: tokio::sync::mpsc::UnboundedReceiver<CaptureHint>,
    ) {
        let applier = self.applier();
        while let Some(hint) = hints.recv().await {
            let (ns, event) = match hint {
                CaptureHint::Track { ns } => {
                    // Not an event: a namespace to start watching. The task
                    // establishes the watermark itself, which is the thing that
                    // makes the namespace cacheable.
                    tracing::info!(%ns, "tracking a namespace registered at run time");
                    self.start(ns);
                    continue;
                }
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
            if !self.registry.tracks(&ns) {
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

        // One subscription per namespace, held for as long as it stays valid.
        //
        // Three things used to churn it, and churn is expensive: the server
        // caps SSE subscriptions **per user** (20 by default, from
        // `TERMINUSDB_SSE_MAX_SUBSCRIPTIONS_PER_USER`), and a Reactor tracking
        // several namespaces and reopening streams will exhaust that cap and be
        // refused — losing low-latency capture for the whole process. So:
        //
        // * resubscribe only when the thing the query is generated from (the
        //   class list) actually changed, not on every schema-dirty signal;
        // * drop the old stream before opening a new one, so we never hold two
        //   slots for one namespace;
        // * back off between attempts instead of reopening every poll interval.
        let mut classes = class_fingerprint(&self.engine.schema(&ns));
        let mut demoted = false;
        let mut backoff =
            std::time::Duration::from_millis(self.config.capture.poll_interval_ms.max(100));

        loop {
            let want_subscription = driver.subscribes() && !demoted;
            let mut sources: Vec<BoxStream<'static, anyhow::Result<CommitEvent>>> = Vec::new();
            let after = self.engine.watermark(&ns).map(|w| w.commit);
            if driver.polls() {
                // The interval is asked for per tick rather than fixed here, so
                // a namespace that nobody has read lately backs off without the
                // stream being rebuilt. See `Supervisor::poll_interval`.
                let supervisor = Arc::clone(&self);
                let polled = ns.clone();
                let poll = headpoll::HeadPoll::with_interval(
                    Arc::clone(&self.client),
                    Arc::new(move || supervisor.poll_interval(&polled)),
                );
                sources.push(poll.subscribe(&ns, after.clone()));
            }
            if want_subscription {
                let sub = subscription::ChangeSetSubscription::new(
                    Arc::clone(&self.client),
                    self.engine.schema(&ns),
                );
                sources.push(sub.subscribe(&ns, after));
                self.engine.metrics().subscription_resubscribes.inc();
            }

            if sources.is_empty() {
                // Subscription-only and demoted: there is nothing left to drive
                // capture. Say so loudly and keep the task alive, because the
                // namespace is now only as fresh as the next operator action.
                tracing::error!(
                    %ns,
                    "capture has no usable driver: the subscription was refused and polling \
                     is disabled for this namespace. Set driver = \"both\" or \"poll\"."
                );
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }

            let mut merged = futures_util::stream::select_all(sources);
            let mut rebuild_subscription = false;

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
                                if is_subscription_limit(&err) {
                                    // A demotion, not a failure: the server has
                                    // no slot for us. Polling still carries the
                                    // namespace, so correctness is unaffected —
                                    // but latency is, and with `driver = both`
                                    // that would otherwise be invisible.
                                    tracing::warn!(
                                        %ns,
                                        error = %err,
                                        "the server refused our _ChangeSet subscription \
                                         (subscription limit). Giving it up and relying on \
                                         head polling; raise \
                                         TERMINUSDB_SSE_MAX_SUBSCRIPTIONS_PER_USER if this \
                                         recurs."
                                    );
                                    self.engine.metrics().subscription_demotions.inc();
                                    demoted = true;
                                    break;
                                }
                                tracing::debug!(%ns, error = %err, "capture driver error");
                            }
                            // Every driver ended. The poll driver never does,
                            // so this means a subscription-only namespace lost
                            // its stream.
                            None => break,
                        }
                    }
                    _ = self.engine.await_schema_refresh(&ns) => {
                        self.refresh_schema(&ns).await;
                        // Wait for the writer to acknowledge the new index,
                        // otherwise the flag stays set and this arm spins.
                        self.await_schema_settled(&ns).await;
                        let next = class_fingerprint(&self.engine.schema(&ns));
                        if next != classes {
                            classes = next;
                            rebuild_subscription = true;
                            break;
                        }
                        // The class list is unchanged, so the generated query
                        // would be identical: keep the stream we have.
                        tracing::debug!(
                            %ns,
                            "schema refreshed but the class list is unchanged; keeping the \
                             existing subscription"
                        );
                    }
                }
            }

            // Release the slot BEFORE any sleep or reopen, so a namespace never
            // holds two subscriptions at once.
            drop(merged);

            if rebuild_subscription {
                // The query really is stale; reopen promptly.
                continue;
            }
            if want_subscription {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.config.capture.poll_interval_ms,
                ))
                .await;
            }
        }
    }

    /// Wait briefly for the writer to apply a schema refresh.
    ///
    /// `await_schema_refresh` resolves immediately while the dirty flag is set,
    /// and only the writer clears it, so without this the schema arm of the
    /// select spins until the writer happens to drain.
    async fn await_schema_settled(&self, ns: &Namespace) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if !self.engine.schema_refresh_requested(ns) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

/// A fingerprint of everything the `_ChangeSet` query is generated from.
///
/// The query lists one block per concrete class, so the class list is exactly
/// what makes a subscription stale — and comparing it is what lets us ignore a
/// schema change that does not affect it (a renamed property, a documentation
/// edit) instead of reopening the stream for nothing.
fn class_fingerprint(schema: &crate::schema_index::SchemaIndex) -> String {
    let mut classes: Vec<&str> = schema
        .classes()
        .filter(|class| !schema.is_enum(class) && !schema.is_abstract(class))
        .map(|class| class.as_str())
        .collect();
    classes.sort_unstable();
    classes.join(",")
}

/// Is this the server refusing us a subscription slot?
///
/// Matched on the message because the client surfaces the body as an error
/// string. The fork answers `429` with
/// `{"code":"TOO_MANY_REQUESTS","message":"Subscription limit exceeded"}`.
fn is_subscription_limit(err: &anyhow::Error) -> bool {
    let text = format!("{err:#}").to_ascii_lowercase();
    // Deliberately NOT a bare "429" substring test. Demotion is sticky — the
    // namespace stops subscribing for the rest of the process — so a false
    // positive silently costs low-latency capture. "429" occurs in plenty of
    // unrelated text, a random server port (`localhost:42900`) or a commit id
    // among it, and a mere connection refusal would then look like a cap.
    // These three phrases all come from the server's own refusal, which sends
    // `TOO_MANY_REQUESTS` as the code and "Subscription limit exceeded" as the
    // message, and which the client renders as "(429 Too Many Requests)".
    text.contains("too_many_requests")
        || text.contains("too many requests")
        || text.contains("subscription limit")
}

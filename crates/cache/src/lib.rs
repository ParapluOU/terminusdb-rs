//! Application-level derived values in Reactor's dependency graph.
//!
//! Reactor caches what it can prove it may cache: a document read, a GraphQL
//! selection, a WOQL answer against stated conditions. What it cannot cache is
//! the thing an application spends most of its time on — the value it *computes*
//! from those reads. A rendered fragment, a projection, a roll-up, anything with
//! a `map` in it: Reactor never saw the function, so it cannot know what the
//! result depends on.
//!
//! This crate closes that gap, and the trick is that it does not ask you to
//! declare the dependencies. Your reads already go through Reactor, and Reactor
//! already computes a precise read set for every one of them. So the dependency
//! set of a computation can be **observed**:
//!
//! ```no_run
//! # use terminusdb_cache::{derivation, Cache, Scope};
//! # use terminusdb_client::BranchSpec;
//! # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
//! # pub struct Card { title: String }
//! # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
//! # pub struct Overview { cards: Vec<Card> }
//! derivation! {
//!     /// The quarter's overview page.
//!     pub struct OverviewOf { pub quarter: u32, pub limit: usize }
//!     -> Overview
//!     |this, scope| {
//!         // An ordinary read, through an ordinary client — pinned at the
//!         // scope's commit and recorded, because the scope's client does both
//!         // without being asked. Use the ORM here if that is how you read.
//!         let total = scope
//!             .client()
//!             .document_count(scope.spec(), Default::default())
//!             .await?;
//!
//!         // Your transformation. Reactor cannot see inside this and does not
//!         // need to.
//!         Ok(Overview {
//!             cards: (0..total.min(this.limit as u64))
//!                 .map(|n| Card { title: format!("#{n} of Q{}", this.quarter) })
//!                 .collect(),
//!         })
//!     }
//! }
//!
//! # async fn example() -> anyhow::Result<()> {
//! let cache = Cache::new("http://localhost:6364", "admin", "root", "admin")?;
//! let spec = BranchSpec::with_branch("mydb", "main");
//!
//! // Computed once. Served from the cache until something it read changes.
//! let page = cache.get(&spec, OverviewOf { quarter: 4, limit: 20 }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # How a value becomes cacheable
//!
//! 1. [`Cache::get`] asks Reactor for the branch head and **latches that
//!    commit**.
//! 2. It looks the value up at that commit. A hit is the end of the story and
//!    your closure is never called.
//! 3. On a miss it opens a [`Scope`]: a client whose every request is pinned at
//!    the latched commit and labelled with the scope's id. Reactor unions the
//!    read set of every labelled request into the scope.
//! 4. Your closure runs and returns a value.
//! 5. The value is stored against the scope. Reactor takes the union as the
//!    dependency set, places the entry at the latched commit, and from then on
//!    the value is an ordinary cache entry — invalidated by the same `fire()`
//!    that invalidates a document read.
//!
//! Step 5 goes through the same insert protocol as any other fill, which is the
//! part worth knowing: if a commit landed while your closure was running, the
//! value is not rejected and not stale. It gets a bounded validity interval and
//! comes back marked [`Outcome::Stored`] with `raced: true`.
//!
//! # What will refuse to cache, and why that is the feature
//!
//! A computation is only memoizable if its inputs are known and come from one
//! snapshot. Reactor refuses the store — loudly, with a sentence — when:
//!
//! * a read **bypassed** the cache (an unknown route, a WOQL query with no
//!   stated condition, a body too large), so nothing is watching that input;
//! * a read was **not pinned** at the scope's commit, so the result is a
//!   mixture of two points in the branch's history and true at neither;
//! * a read's dependencies could only be described as `everything`;
//! * **no read was recorded at all** — which almost always means the reads did
//!   not go through [`Scope::client`].
//!
//! In each case [`Cache::get`] still returns your value: the computation
//! succeeded, only the caching failed. It is reported as
//! [`Outcome::NotStored`] with the reason, logged at `warn`, and visible per
//! scope at `GET /_reactor/scope/{id}`. **Watch for it in development** — a
//! derivation that silently never caches looks exactly like one that does.
//!
//! # The hazard this cannot see
//!
//! If your closure reads something the scope does not go through — another
//! database, the clock, an environment variable, an HTTP service — the value is
//! memoized against inputs that can change without any commit. Nothing will
//! tell you. SkipLabs' Skip enforces mapper purity in its compiler; the
//! prototype this design comes from had the same hole, and so does this. Keep
//! the closure a pure function of what it reads through the scope.
//!
//! # Nesting is how you get granularity
//!
//! A change to one input invalidates the whole derived value, because Reactor
//! cannot re-run part of a function it cannot see. The way to get Skip-like
//! per-key incrementality is to *structure* the computation: one derivation per
//! input, and an outer derivation over those.
//!
//! ```ignore
//! let cards = scope.derive_all(ids.into_iter().map(|id| CardFor { id })).await?;
//! ```
//!
//! Each `CardFor` is its own cache entry with its own dependencies, so one
//! changed document re-maps one card. The outer value still rebuilds — it
//! inherits the inner values' dependency terms, which is what keeps it from
//! outliving them — but the rebuild reads the other cards from cache instead of
//! recomputing them. And a card is shared: the overview and the detail page that
//! both need it compute it once between them.
//!
//! # A rule worth remembering
//!
//! **Push filters into the query; keep transformations in code.** A
//! `.filter(|p| p.active)` in Rust is invisible, so the value depends on
//! everything the read returned. The same predicate in the GraphQL read gets
//! Reactor's predicate index, and a write that does not move a document into or
//! out of the set fires nothing at all.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod http;
pub mod key;
mod macros;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde::Serialize;

use terminusdb_client::{BranchSpec, CacheCondition, CommitId, TerminusDBHttpClient};

pub use key::{params_hash, EpochPolicy};

/// A computation whose result can be cached.
///
/// Written by hand for anything with generics or an unusual key; otherwise use
/// the [`derivation!`] macro, which writes this impl and derives the key parts
/// that can be derived.
#[async_trait]
pub trait Derive: Serialize + Send + Sync {
    /// What the computation produces.
    ///
    /// Cached as its JSON serialization. Byte-faithfulness — the rule that
    /// Reactor never re-serializes a TerminusDB body — does not apply here: a
    /// derived value *is* whatever you serialized, so the round trip is its
    /// definition rather than a corruption of it.
    type Output: Serialize + DeserializeOwned + Send;

    /// Stable name of the computation, and the first third of its key.
    ///
    /// Renaming it orphans every entry (they expire on their own); *reusing* one
    /// for a different computation makes two values share a slot, which nothing
    /// can detect for you.
    const NAME: &'static str;

    /// A hash of the computation's own source text, or `0` when unknown.
    ///
    /// Set by [`derivation!`] from `stringify!` of the body. Only consulted
    /// under [`EpochPolicy::CodeHash`].
    const CODE: u64 = 0;

    /// The computation.
    async fn compute(&self, scope: &Scope) -> anyhow::Result<Self::Output>;

    /// Where the dependency set comes from. Recorded, unless you say otherwise.
    fn dependencies(&self) -> Dependencies {
        Dependencies::Recorded
    }

    /// Replace the whole key.
    ///
    /// For a human-readable key, or a deliberately coarser one (several
    /// parameter sets sharing an entry). You own uniqueness if you override
    /// this — including the epoch, which is no longer added for you.
    fn key_override(&self) -> Option<String> {
        None
    }
}

/// Where a derived value's dependency set comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Dependencies {
    /// Observed: Reactor reports the read set of every read in the scope, and
    /// the union is the value's dependency set. The default, and the only
    /// option that cannot be wrong about what the computation read.
    Recorded,
    /// Declared, for a computation whose reads Reactor cannot attribute — a
    /// WOQL query being the usual case, since WOQL can return data without
    /// revealing which entities it came from.
    ///
    /// You are promising that these conditions cover every input. Naming too
    /// few is the one way to get stale data out of this crate.
    Declared(Vec<CacheCondition>),
    /// Independent of the database: a pure function of its parameters.
    ///
    /// Never invalidated, which is right only if the computation truly reads
    /// nothing. Stated out loud because "no dependencies" and "we failed to
    /// find the dependencies" must not look the same.
    Independent,
}

/// What happened to a derived value.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Served from the cache. The closure was not called.
    Hit,
    /// Computed and accepted into the dependency graph.
    Stored {
        /// A commit landed while the computation was running, so the entry was
        /// stored with a bounded validity interval and the next read will
        /// recompute. The value returned is still correct — it is the answer at
        /// the commit it was computed at.
        raced: bool,
    },
    /// Computed, and *not* cached. The string is Reactor's reason, in a
    /// sentence. See the crate docs: this is usually a read that did not go
    /// through [`Scope::client`].
    NotStored(String),
}

impl Outcome {
    /// Whether the closure had to run.
    pub fn computed(&self) -> bool {
        !matches!(self, Outcome::Hit)
    }

    /// Whether the value is now in the cache.
    pub fn cached(&self) -> bool {
        matches!(self, Outcome::Hit | Outcome::Stored { .. })
    }
}

/// A handle on Reactor's derived-value store.
///
/// Cheap to clone; one per process is plenty.
#[derive(Clone)]
pub struct Cache {
    inner: Arc<Inner>,
}

struct Inner {
    reactor: url::Url,
    user: String,
    pass: String,
    org: String,
    epoch: EpochPolicy,
    timeout: Option<Duration>,
    http: reqwest::Client,
    /// One computation per key per process. Two tasks that miss the same key
    /// should not both run the closure; across processes they still will, which
    /// costs duplicated work and never a wrong answer.
    locks: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

impl Cache {
    /// Point a cache at a Reactor.
    ///
    /// The URL is Reactor's own listen address — the one your clients already
    /// talk to — not TerminusDB's.
    pub fn new(
        reactor: &str,
        user: impl Into<String>,
        pass: impl Into<String>,
        org: impl Into<String>,
    ) -> anyhow::Result<Cache> {
        Ok(Cache {
            inner: Arc::new(Inner {
                reactor: url::Url::parse(reactor)?,
                user: user.into(),
                pass: pass.into(),
                org: org.into(),
                epoch: EpochPolicy::default(),
                timeout: None,
                http: reqwest::Client::new(),
                locks: DashMap::new(),
            }),
        })
    }

    /// Choose how the epoch part of a key is derived. See [`EpochPolicy`].
    pub fn with_epoch(mut self, epoch: EpochPolicy) -> Cache {
        Arc::get_mut(&mut self.inner)
            .expect("with_epoch is called before the cache is shared")
            .epoch = epoch;
        self
    }

    /// Request timeout for the clients handed to derivations.
    pub fn with_timeout(mut self, timeout: Duration) -> Cache {
        Arc::get_mut(&mut self.inner)
            .expect("with_timeout is called before the cache is shared")
            .timeout = Some(timeout);
        self
    }

    /// The key a derivation will be stored under.
    ///
    /// Exposed because a key is the thing you need in your hand when you are
    /// looking at `/_reactor/` output and wondering which entry is yours.
    pub fn key_of<D: Derive>(&self, derivation: &D) -> anyhow::Result<String> {
        if let Some(key) = derivation.key_override() {
            return Ok(key);
        }
        let epoch = match &self.inner.epoch {
            EpochPolicy::Deploy(epoch) => epoch.clone(),
            EpochPolicy::CodeHash => format!("c{:016x}", D::CODE),
        };
        Ok(key::compose(
            D::NAME,
            &key::params_hash(derivation)?,
            &epoch,
        ))
    }

    /// Get the value, computing it if the cache does not have it.
    pub async fn get<D: Derive>(
        &self,
        spec: &BranchSpec,
        derivation: D,
    ) -> anyhow::Result<D::Output> {
        self.get_reported(spec, derivation).await.map(|(v, _)| v)
    }

    /// As [`Cache::get`], and say what happened.
    ///
    /// Worth using in tests and in a metric: `Outcome::NotStored` is the one
    /// outcome that is invisible from the value alone, and a derivation that
    /// never caches behaves exactly like one that does except for the bill.
    pub async fn get_reported<D: Derive>(
        &self,
        spec: &BranchSpec,
        derivation: D,
    ) -> anyhow::Result<(D::Output, Outcome)> {
        self.get_reported_after(spec, derivation, None).await
    }

    /// As [`Cache::get`], but refuse to answer from a snapshot older than
    /// `after`.
    ///
    /// Read-your-writes for a derived value. A computation pins itself at one
    /// commit before its first read, so a fence on the *reads* comes too late:
    /// by then the stale snapshot has already been chosen. This demands the
    /// freshness at the latch instead, and Reactor waits (up to
    /// `[consistency].min_version_wait_ms`) for its watermark to reach the
    /// commit — erroring rather than quietly handing back an older pin.
    ///
    /// The commit to pass is the one a write returned, which
    /// `insert_instance_with_commit_id` already gives you. It composes across
    /// databases without any cross-database machinery: a page assembled from a
    /// room and a shared chunk store fences each read at the commit it wrote
    /// there, because each demand is about one history.
    pub async fn get_after<D: Derive>(
        &self,
        spec: &BranchSpec,
        derivation: D,
        after: &CommitId,
    ) -> anyhow::Result<D::Output> {
        self.get_reported_after(spec, derivation, Some(after))
            .await
            .map(|(v, _)| v)
    }

    /// [`Cache::get_after`], reporting what happened. `None` is [`Cache::get`].
    pub async fn get_reported_after<D: Derive>(
        &self,
        spec: &BranchSpec,
        derivation: D,
        after: Option<&CommitId>,
    ) -> anyhow::Result<(D::Output, Outcome)> {
        let ns = self.namespace(spec);
        let key = self.key_of(&derivation)?;
        let at = self.inner.head(&ns, after).await?;
        self.resolve(ns, key, at, derivation, None).await
    }

    /// Namespace coordinates for a spec, filling in this cache's org and the
    /// default branch.
    fn namespace(&self, spec: &BranchSpec) -> http::Ns {
        http::Ns {
            org: self.inner.org.clone(),
            db: spec.db.clone(),
            branch: spec.branch.clone().unwrap_or_else(|| "main".to_owned()),
        }
    }

    /// The shared get-or-compute path, used by both [`Cache::get`] and
    /// [`Scope::derive`]; `into` is the outer scope of a nested derivation.
    async fn resolve<D: Derive>(
        &self,
        ns: http::Ns,
        key: String,
        at: CommitId,
        derivation: D,
        into: Option<&str>,
    ) -> anyhow::Result<(D::Output, Outcome)> {
        if let Some(bytes) = self.inner.fetch(&ns, &key, &at, into).await? {
            if let Ok(value) = serde_json::from_slice::<D::Output>(&bytes) {
                return Ok((value, Outcome::Hit));
            }
            // Stored bytes that no longer deserialize: the shape of `Output`
            // changed without the epoch changing. Recompute rather than fail,
            // and say so, because the fix is an epoch bump.
            tracing::warn!(
                key = %key,
                "a cached derived value did not deserialize; recomputing. Its shape changed \
                 without the key's epoch changing — see EpochPolicy."
            );
        }

        // One computation per key per process.
        let lock = self
            .inner
            .locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone();
        let _guard = lock.lock().await;

        // Whoever held the lock may have filled it.
        if let Some(bytes) = self.inner.fetch(&ns, &key, &at, into).await? {
            if let Ok(value) = serde_json::from_slice::<D::Output>(&bytes) {
                return Ok((value, Outcome::Hit));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let scope = Scope {
            client: self.inner.client_for_scope(&id, self.inner.timeout).await?,
            id,
            commit: at.clone(),
            spec: BranchSpec {
                db: ns.db.clone(),
                branch: Some(ns.branch.clone()),
                ref_commit: Some(at.clone()),
            },
            cache: self.clone(),
            ns: ns.clone(),
            foreign: std::sync::Mutex::new(BTreeMap::new()),
        };
        let value = derivation.compute(&scope).await?;
        let body = serde_json::to_vec(&value)?;

        let outcome = self
            .inner
            .store(
                &ns,
                &key,
                &at,
                body.into(),
                match derivation.dependencies() {
                    Dependencies::Recorded => http::DepSource::Scope(&scope.id),
                    Dependencies::Declared(conditions) => {
                        http::DepSource::Declared(CacheCondition::header_value(&conditions))
                    }
                    Dependencies::Independent => http::DepSource::Independent,
                },
                into,
                &scope.foreign_pins(),
            )
            .await?;
        if let Outcome::NotStored(why) = &outcome {
            tracing::warn!(key = %key, reason = %why, "a derived value was not cached");
        }
        Ok((value, outcome))
    }
}

/// One computation's view of the database: a latched commit, a client that reads
/// at it, and the recording that makes the result cacheable.
///
/// Handed to [`Derive::compute`] rather than opened by it, because the pin and
/// the label have to be in place before the first read. Reaching for an ambient
/// client inside a closure is the mistake this shape is meant to make visible.
pub struct Scope {
    id: String,
    commit: CommitId,
    spec: BranchSpec,
    client: TerminusDBHttpClient,
    cache: Cache,
    ns: http::Ns,
    /// Other databases this computation has read, each latched at its own
    /// commit the first time it was asked for.
    ///
    /// A `Mutex` because `compute` takes `&Scope`: the pins have to outlive the
    /// call that creates them so the store can report them, and a computation
    /// that reads the same shared database twice must get the same snapshot
    /// both times.
    foreign: std::sync::Mutex<BTreeMap<http::Ns, ForeignScope>>,
}

/// Another database, as one computation sees it.
#[derive(Clone)]
pub struct ForeignScope {
    commit: CommitId,
    spec: BranchSpec,
    client: TerminusDBHttpClient,
}

impl ForeignScope {
    /// The client to read that database through. Pinned and recorded, exactly
    /// like [`Scope::client`].
    pub fn client(&self) -> &TerminusDBHttpClient {
        &self.client
    }

    /// The spec to read with: that branch, pinned at [`ForeignScope::commit`].
    pub fn spec(&self) -> &BranchSpec {
        &self.spec
    }

    /// The commit this computation sees that database at.
    pub fn commit(&self) -> &CommitId {
        &self.commit
    }
}

impl Scope {
    /// The client to read through. Every request it makes is pinned at
    /// [`Scope::commit`] and recorded against this scope.
    pub fn client(&self) -> &TerminusDBHttpClient {
        &self.client
    }

    /// The spec to read with: this branch, pinned at [`Scope::commit`].
    pub fn spec(&self) -> &BranchSpec {
        &self.spec
    }

    /// The commit every read in this scope sees.
    pub fn commit(&self) -> &CommitId {
        &self.commit
    }

    /// This scope's id, as `GET /_reactor/scope/{id}` wants it.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Read **another database** inside this computation.
    ///
    /// The composition case: a page assembled from this room plus a shared
    /// chunk store. Each database is latched at its own commit — there is no
    /// single snapshot across databases that commit independently, so the value
    /// is not "true at commit C" but "composed of exactly these revisions", and
    /// that is what Reactor records.
    ///
    /// Invalidation reaches across: a commit to the other database that touches
    /// something this computation read **evicts** the stored value, and the next
    /// reader recomputes. The reads are recorded, so the dependency set is
    /// observed here too — you are not declaring anything.
    ///
    /// Two things to know. The value is stored in *this* scope's database, so
    /// that is where it is looked up and who may read it. And the pin is taken
    /// once per database per computation: ask twice and you get the same
    /// snapshot, which is what makes the result coherent.
    pub async fn foreign(&self, db: &str) -> anyhow::Result<ForeignScope> {
        self.foreign_in(db, None).await
    }

    /// [`Scope::foreign`] on a named branch rather than `main`.
    pub async fn foreign_branch(&self, db: &str, branch: &str) -> anyhow::Result<ForeignScope> {
        self.foreign_in(db, Some(branch)).await
    }

    async fn foreign_in(&self, db: &str, branch: Option<&str>) -> anyhow::Result<ForeignScope> {
        let ns = http::Ns {
            org: self.ns.org.clone(),
            db: db.to_owned(),
            branch: branch.unwrap_or("main").to_owned(),
        };
        if ns == self.ns {
            anyhow::bail!(
                "{db} is this scope's own database: read it with Scope::client, which is \
                 already pinned at {}",
                self.commit
            );
        }
        if let Some(found) = self
            .foreign
            .lock()
            .expect("a poisoned scope lock means a panic inside a computation")
            .get(&ns)
        {
            return Ok(found.clone());
        }
        let commit = self.cache.inner.head(&ns, None).await?;
        let entry = ForeignScope {
            spec: BranchSpec {
                db: ns.db.clone(),
                branch: Some(ns.branch.clone()),
                ref_commit: Some(commit.clone()),
            },
            // The same recording client: Reactor files each read under the
            // namespace it was addressed to, so one label covers both.
            client: self.client.clone(),
            commit,
        };
        let mut pinned = self
            .foreign
            .lock()
            .expect("a poisoned scope lock means a panic inside a computation");
        // Another task may have latched it first; keep that one, so one
        // computation never sees two snapshots of one database.
        Ok(pinned.entry(ns).or_insert(entry).clone())
    }

    /// The commit each other database was latched at, for the store.
    fn foreign_pins(&self) -> BTreeMap<http::Ns, CommitId> {
        self.foreign
            .lock()
            .expect("a poisoned scope lock means a panic inside a computation")
            .iter()
            .map(|(ns, entry)| (ns.clone(), entry.commit.clone()))
            .collect()
    }

    /// A nested derivation, at this scope's commit.
    ///
    /// The inner value is its own cache entry with its own dependencies, so a
    /// change to one of its inputs re-computes it alone. Its dependency terms
    /// are inherited by the outer value, which is what stops the outer one from
    /// outliving them.
    pub async fn derive<D: Derive>(&self, derivation: D) -> anyhow::Result<D::Output> {
        let key = self.cache.key_of(&derivation)?;
        self.cache
            .resolve(
                self.ns.clone(),
                key,
                self.commit.clone(),
                derivation,
                Some(&self.id),
            )
            .await
            .map(|(value, _)| value)
    }

    /// [`Scope::derive`] over an iterator, in order.
    ///
    /// Sequential on purpose: these are localhost round trips against one
    /// Reactor, and the useful parallelism (one upstream request for many
    /// documents) is already inside it. Spawn your own tasks if a derivation is
    /// CPU-bound.
    pub async fn derive_all<D, I>(&self, derivations: I) -> anyhow::Result<Vec<D::Output>>
    where
        D: Derive,
        I: IntoIterator<Item = D>,
    {
        let mut out = Vec::new();
        for derivation in derivations {
            out.push(self.derive(derivation).await?);
        }
        Ok(out)
    }
}

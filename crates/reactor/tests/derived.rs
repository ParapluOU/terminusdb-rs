#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Application-computed values in the dependency graph, end to end.
//!
//! These are the tests for the half of the design Reactor cannot derive for
//! itself: a value produced by a transformation it never saw. The claim under
//! test is not "we can store bytes" — it is that a value computed by ordinary
//! application code, reading through an ordinary client, acquires a *correct*
//! dependency set without the application declaring one, and is then invalidated
//! by the same commits that invalidate everything else.
//!
//! Every test runs against a live TerminusDB 12 node, because the dependency
//! set is derived from what the node actually returns.

mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use common::models::{person, Person};
use common::Reactor;
use serde::{Deserialize, Serialize};
use terminusdb_cache::{derivation, Cache, Dependencies, Derive, Outcome, Scope};
use terminusdb_client::{DocumentInsertArgs, GetOpts};

/// How many times a closure ran, per database, so a "hit" can be the absence of
/// work rather than a header we chose to believe.
///
/// Per database because these tests run in parallel in one process: a single
/// global counter would make one test's computations look like another's, and
/// the assertion would pass or fail on scheduling.
static RUNS: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);

fn note_run(db: &str) {
    let mut guard = RUNS.lock().expect("the counter is never poisoned");
    *guard
        .get_or_insert_with(HashMap::new)
        .entry(db.to_owned())
        .or_insert(0) += 1;
}

fn runs(db: &str) -> usize {
    RUNS.lock()
        .expect("the counter is never poisoned")
        .as_ref()
        .and_then(|counts| counts.get(db).copied())
        .unwrap_or(0)
}

/// A derived value: one document, transformed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shout {
    /// The name, as the transformation produced it.
    pub name: String,
}

derivation! {
    /// Read one person and shout their name. The transformation is trivial on
    /// purpose: what is under test is the dependency set, not the arithmetic.
    pub struct ShoutOf { pub id: String }
    -> Shout
    |this, scope| {
        note_run(&scope.spec().db);
        let mut de = terminusdb_client::DefaultTDBDeserializer;
        let person: Person = scope
            .client()
            .get_instance(&this.id, scope.spec(), &mut de)
            .await?;
        Ok(Shout { name: person.name.to_uppercase() })
    }
}

derivation! {
    /// A derivation that reads *nothing* through its scope, which is the
    /// mistake the recording design has to catch.
    pub struct Untracked;
    -> Shout
    |_this, _scope| {
        Ok(Shout { name: "from thin air".to_owned() })
    }
}

/// A value declared independent of the database, by hand: the macro covers the
/// common case and this is what the escape hatch looks like.
#[derive(Debug, Clone, Serialize)]
pub struct Constant;

#[async_trait::async_trait]
impl Derive for Constant {
    type Output = Shout;
    const NAME: &'static str = "Constant";

    async fn compute(&self, scope: &Scope) -> Result<Shout> {
        note_run(&scope.spec().db);
        Ok(Shout {
            name: "CONSTANT".to_owned(),
        })
    }

    fn dependencies(&self) -> Dependencies {
        Dependencies::Independent
    }
}

/// An outer derivation over per-document inner ones: the shape that buys
/// per-key granularity from a cache that cannot see inside a function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roster {
    /// One shout per member, in the order asked for.
    pub names: Vec<String>,
}

static OUTER_RUNS: AtomicUsize = AtomicUsize::new(0);

derivation! {
    pub struct RosterOf { pub ids: Vec<String> }
    -> Roster
    |this, scope| {
        OUTER_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _ = scope.spec();
        // Collected rather than passed as a lazy iterator: a closure borrowing
        // `this` inside a boxed trait future is not general enough for the
        // lifetime the trait demands, and the Vec costs nothing here.
        let members: Vec<ShoutOf> = this
            .ids
            .iter()
            .map(|id| ShoutOf { id: id.clone() })
            .collect();
        let shouts = scope.derive_all(members).await?;
        Ok(Roster {
            names: shouts.into_iter().map(|shout| shout.name).collect(),
        })
    }
}

fn cache(reactor: &Reactor) -> Result<Cache> {
    Cache::new(reactor.base.as_str(), "admin", "root", "admin")
}

/// The whole point, in one test: computed once, served thereafter.
#[tokio::test]
async fn a_derived_value_is_computed_once_and_then_served() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_once", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;
            let before = runs(&spec.db);

            let (first, outcome) = cache
                .get_reported(
                    &spec,
                    ShoutOf {
                        id: "Person/alice".to_owned(),
                    },
                )
                .await?;
            assert_eq!(first.name, "ANN");
            assert!(
                matches!(outcome, Outcome::Stored { raced: false }),
                "the first call computes and stores: {outcome:?}"
            );
            assert_eq!(runs(&spec.db), before + 1);

            let (second, outcome) = cache
                .get_reported(
                    &spec,
                    ShoutOf {
                        id: "Person/alice".to_owned(),
                    },
                )
                .await?;
            assert_eq!(second, first);
            assert_eq!(outcome, Outcome::Hit, "the second call must not compute");
            assert_eq!(
                runs(&spec.db),
                before + 1,
                "a hit is the absence of work, not a header"
            );
            Ok(())
        })
        .await
}

/// Read-your-writes for a derived value: `get_after` refuses to answer from a
/// snapshot that predates a commit the caller has already seen.
///
/// Fencing the *reads* would be too late — a computation pins itself at one
/// commit before its first read, so a stale pin is chosen before any read is
/// issued. The demand therefore belongs at the latch, and because it is a
/// demand about one history it composes across databases without any
/// cross-database machinery.
#[tokio::test]
async fn get_after_waits_for_a_commit_the_caller_has_seen() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_after", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;
            let what = || ShoutOf {
                id: "Person/alice".to_owned(),
            };

            let first = cache.get(&spec, what()).await?;
            assert_eq!(first.name, "ANN");

            // Write straight at the node, so Reactor has not necessarily
            // noticed the commit yet, and ask for a value no older than it.
            let (_, commit) = direct
                .insert_instance_with_commit_id(&person("alice", "Zoe", 31)?, args)
                .await?;
            let after = cache.get_after(&spec, what(), &commit).await?;
            assert_eq!(
                after.name, "ZOE",
                "a value demanded at or after {commit} must not be computed from an older \
                 snapshot"
            );
            Ok(())
        })
        .await
}

/// Invalidation, without anything having been declared: the dependency set came
/// from watching the read.
#[tokio::test]
async fn a_write_to_an_input_recomputes_it_and_a_write_elsewhere_does_not() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_invalidate", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bo", 41)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;
            let derivation = || ShoutOf {
                id: "Person/alice".to_owned(),
            };

            let (first, _) = cache.get_reported(&spec, derivation()).await?;
            assert_eq!(first.name, "ANN");

            // A write to somebody else. The value depends on Person/alice, so
            // it must survive — this is the half that makes the cache useful
            // rather than merely correct.
            let (_, commit) = direct
                .insert_instance_with_commit_id(&person("bob", "Bobby", 41)?, args.clone())
                .await?;
            assert!(reactor.await_commit(&commit).await, "capture kept up");
            let runs_before = runs(&spec.db);
            let (unchanged, outcome) = cache.get_reported(&spec, derivation()).await?;
            assert_eq!(unchanged, first);
            assert_eq!(
                outcome,
                Outcome::Hit,
                "a write to Person/bob is not a write to Person/alice"
            );
            assert_eq!(runs(&spec.db), runs_before);

            // A write to the document it read.
            let (_, commit) = direct
                .insert_instance_with_commit_id(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor.await_commit(&commit).await, "capture kept up");
            let (after, outcome) = cache.get_reported(&spec, derivation()).await?;
            assert_eq!(
                after.name, "ANNE",
                "the transformation re-ran over new data"
            );
            assert!(outcome.computed(), "it had to recompute: {outcome:?}");
            Ok(())
        })
        .await
}

/// The failure this design exists to make loud.
#[tokio::test]
async fn a_computation_that_reads_outside_its_scope_refuses_to_cache() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_untracked", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;

            let (value, outcome) = cache.get_reported(&spec, Untracked).await?;
            assert_eq!(
                value.name, "from thin air",
                "the caller still gets its value: the computation succeeded"
            );
            let Outcome::NotStored(why) = &outcome else {
                panic!("a value with no recorded reads must not be cached: {outcome:?}");
            };
            assert!(
                why.contains("no read was recorded"),
                "the reason has to point at the cause: {why}"
            );

            // And it stays uncached, rather than being half-stored.
            let (_, again) = cache.get_reported(&spec, Untracked).await?;
            assert!(matches!(again, Outcome::NotStored(_)), "{again:?}");
            Ok(())
        })
        .await
}

/// Independence, said out loud.
#[tokio::test]
async fn a_value_declared_independent_survives_every_commit() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_independent", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;

            let (_, outcome) = cache.get_reported(&spec, Constant).await?;
            assert!(outcome.cached(), "{outcome:?}");

            let (_, commit) = direct
                .insert_instance_with_commit_id(&person("alice", "Anne", 31)?, args)
                .await?;
            assert!(reactor.await_commit(&commit).await, "capture kept up");

            let runs_before = runs(&spec.db);
            let (_, outcome) = cache.get_reported(&spec, Constant).await?;
            assert_eq!(
                outcome,
                Outcome::Hit,
                "a value that reads nothing cannot be invalidated by a write"
            );
            assert_eq!(runs(&spec.db), runs_before);
            Ok(())
        })
        .await
}

/// Nesting: the inner values are entries of their own, so a change to one input
/// re-maps one of them.
#[tokio::test]
async fn a_nested_derivation_recomputes_only_the_member_that_changed() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_nested", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for (id, name) in [("alice", "Ann"), ("bob", "Bo"), ("carol", "Cass")] {
                direct
                    .insert_instance(&person(id, name, 30)?, args.clone())
                    .await?;
            }

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;
            let roster = || RosterOf {
                ids: vec![
                    "Person/alice".to_owned(),
                    "Person/bob".to_owned(),
                    "Person/carol".to_owned(),
                ],
            };

            let (first, outcome) = cache.get_reported(&spec, roster()).await?;
            assert_eq!(first.names, vec!["ANN", "BO", "CASS"]);
            assert!(outcome.cached(), "{outcome:?}");

            // Each member is its own entry, so asking for one directly is a hit
            // the roster warmed.
            let (member, outcome) = cache
                .get_reported(
                    &spec,
                    ShoutOf {
                        id: "Person/bob".to_owned(),
                    },
                )
                .await?;
            assert_eq!(member.name, "BO");
            assert_eq!(
                outcome,
                Outcome::Hit,
                "an inner value computed for the roster is shared with its own reader"
            );

            // Change one member. The roster inherits its members' dependencies,
            // so it rebuilds — but only the changed member is recomputed.
            let (_, commit) = direct
                .insert_instance_with_commit_id(&person("bob", "Bobby", 41)?, args)
                .await?;
            assert!(reactor.await_commit(&commit).await, "capture kept up");

            let inner_before = runs(&spec.db);
            let outer_before = OUTER_RUNS.load(Ordering::SeqCst);
            let (after, outcome) = cache.get_reported(&spec, roster()).await?;
            assert_eq!(after.names, vec!["ANN", "BOBBY", "CASS"]);
            assert!(outcome.computed(), "the roster had to rebuild: {outcome:?}");
            assert_eq!(
                OUTER_RUNS.load(Ordering::SeqCst),
                outer_before + 1,
                "the outer transformation ran once"
            );
            assert_eq!(
                runs(&spec.db),
                inner_before + 1,
                "and exactly one member was re-mapped: the other two were hits"
            );
            Ok(())
        })
        .await
}

/// Two parameter sets are two questions.
#[tokio::test]
async fn parameters_are_part_of_the_key() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_keys", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bo", 41)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = cache(&reactor)?;

            let alice = cache.key_of(&ShoutOf {
                id: "Person/alice".to_owned(),
            })?;
            let bob = cache.key_of(&ShoutOf {
                id: "Person/bob".to_owned(),
            })?;
            assert_ne!(alice, bob, "different arguments, different entries");
            assert!(alice.starts_with("ShoutOf/"), "{alice}");

            let (first, _) = cache
                .get_reported(
                    &spec,
                    ShoutOf {
                        id: "Person/alice".to_owned(),
                    },
                )
                .await?;
            let (second, outcome) = cache
                .get_reported(
                    &spec,
                    ShoutOf {
                        id: "Person/bob".to_owned(),
                    },
                )
                .await?;
            assert_eq!(first.name, "ANN");
            assert_eq!(second.name, "BO");
            assert!(
                outcome.computed(),
                "the second parameter set is not a hit on the first: {outcome:?}"
            );
            Ok(())
        })
        .await
}

/// Concurrent callers of a missing key share one computation.
#[tokio::test]
async fn concurrent_callers_compute_once() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_singleflight", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let cache = Arc::new(cache(&reactor)?);
            let before = runs(&spec.db);

            let mut tasks = Vec::new();
            for _ in 0..4 {
                let cache = Arc::clone(&cache);
                let spec = spec.clone();
                tasks.push(tokio::spawn(async move {
                    cache
                        .get(
                            &spec,
                            ShoutOf {
                                id: "Person/alice".to_owned(),
                            },
                        )
                        .await
                }));
            }
            for task in tasks {
                assert_eq!(task.await??.name, "ANN");
            }
            assert_eq!(
                runs(&spec.db),
                before + 1,
                "four callers, one computation: the rest waited and took the answer"
            );
            Ok(())
        })
        .await
}

// --------------------------------------------------------------- composition

/// A value composed from this database **and** a shared one.
///
/// The Pubbin shape: a room's publication plus chunks reused across every room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// From this scope's own database.
    pub local: String,
    /// From the shared database.
    pub shared: String,
}

/// Parameterised by the shared database's name, since a test's databases are
/// named at run time.
#[derive(Debug, Clone, Serialize)]
pub struct PageOf {
    /// The document to read here.
    pub id: String,
    /// The database holding the shared chunk.
    pub shared_db: String,
    /// The chunk to read there.
    pub chunk: String,
}

#[async_trait::async_trait]
impl Derive for PageOf {
    type Output = Page;
    const NAME: &'static str = "PageOf";

    async fn compute(&self, scope: &Scope) -> Result<Page> {
        note_run(&scope.spec().db);
        let mut de = terminusdb_client::DefaultTDBDeserializer;
        let local: Person = scope
            .client()
            .get_instance(&self.id, scope.spec(), &mut de)
            .await?;
        // The other database, latched at its own commit.
        let shared = scope.foreign(&self.shared_db).await?;
        let chunk: Person = shared
            .client()
            .get_instance(&self.chunk, shared.spec(), &mut de)
            .await?;
        Ok(Page {
            local: local.name,
            shared: chunk.name,
        })
    }
}

/// Create a second database with one chunk document in it.
///
/// Before Reactor starts, so it can be a configured namespace: a namespace
/// registered at run time is a different feature with its own tests.
async fn shared_database(
    direct: &terminusdb_client::TerminusDBHttpClient,
    name: &str,
) -> Result<terminusdb_client::BranchSpec> {
    direct.ensure_database(name).await?;
    let spec = terminusdb_client::BranchSpec::with_branch(name, "main");
    let args = DocumentInsertArgs::from(spec.clone());
    direct.insert_schemas::<(Person,)>(args.clone()).await?;
    direct
        .insert_instance(&person("intro", "Intro", 1)?, args)
        .await?;
    Ok(spec)
}

/// A value composed from two databases is cached, and a commit to **either** of
/// them recomputes it.
///
/// The cross-database case, end to end. A dependency term carries no namespace
/// — it is filed in one namespace's index, owned by that namespace's single
/// writer — so the foreign half is a registration with the *other* writer, and
/// what it does when it fires is **evict**: there is no position in this
/// namespace at which a commit over there happened, so there is no honest
/// interval to close, and a missing entry is safe at every position.
#[tokio::test]
async fn a_value_composed_from_two_databases_dies_with_either_of_them() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_cross", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;

            let shared_name = format!("{}_shared", spec.db);
            let shared = shared_database(&direct, &shared_name).await?;
            let reactor =
                Reactor::start_with(up, &spec.db, common::with_database(&shared_name)).await?;
            let shared_ns = terminusdb_reactor::ids::Namespace::new("admin", &shared_name, "main");
            assert!(
                common::eventually(std::time::Duration::from_secs(20), || reactor
                    .engine
                    .watermark(&shared_ns)
                    .is_some())
                .await,
                "the shared namespace must acquire a watermark"
            );
            let cache = cache(&reactor)?;
            let what = || PageOf {
                id: "Person/alice".to_owned(),
                shared_db: shared.db.clone(),
                chunk: "Person/intro".to_owned(),
            };

            let before = runs(&spec.db);
            let (page, outcome) = cache.get_reported(&spec, what()).await?;
            assert_eq!(
                page,
                Page {
                    local: "Ann".to_owned(),
                    shared: "Intro".to_owned()
                }
            );
            assert!(
                matches!(outcome, Outcome::Stored { raced: false }),
                "a composition over two databases must be storable: {outcome:?}"
            );
            assert_eq!(runs(&spec.db), before + 1);

            let (again, outcome) = cache.get_reported(&spec, what()).await?;
            assert_eq!(again, page);
            assert_eq!(outcome, Outcome::Hit, "and then served");
            assert_eq!(runs(&spec.db), before + 1);

            // A commit to the SHARED database must reach across and kill it.
            direct
                .insert_instance(
                    &person("intro", "Introduction", 1)?,
                    DocumentInsertArgs::from(shared.clone()),
                )
                .await?;

            // Capture has to notice the shared commit, the shared writer has to
            // fire it into its foreign index, and the eviction has to reach this
            // namespace's writer — so poll rather than assume a single round
            // trip is enough.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let mut fresh = None;
            while std::time::Instant::now() < deadline {
                let (page, _) = cache.get_reported(&spec, what()).await?;
                if page.shared == "Introduction" {
                    fresh = Some(page);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            let fresh = fresh
                .expect("a commit to the shared database must invalidate a value composed from it");
            assert_eq!(
                fresh,
                Page {
                    local: "Ann".to_owned(),
                    shared: "Introduction".to_owned()
                }
            );
            assert!(
                runs(&spec.db) > before + 1,
                "and it must have been recomputed, not served"
            );

            // And it is cached again afterwards, so the eviction is not a
            // permanent loss of the entry.
            let settled = runs(&spec.db);
            let (_, outcome) = cache.get_reported(&spec, what()).await?;
            assert_eq!(outcome, Outcome::Hit, "the recomputed value must stick");
            assert_eq!(runs(&spec.db), settled);

            // A commit to the LOCAL database still works too: the local half of
            // the dependency set is an ordinary registration in this
            // namespace's own index.
            direct
                .insert_instance(
                    &person("alice", "Annabel", 31)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let mut local_fresh = false;
            while std::time::Instant::now() < deadline {
                let (page, _) = cache.get_reported(&spec, what()).await?;
                if page.local == "Annabel" {
                    local_fresh = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            assert!(
                local_fresh,
                "a commit to the home database must invalidate it as it always did"
            );

            direct.delete_database(&shared.db).await?;
            Ok(())
        })
        .await
}

/// The cross-namespace half of the fill/commit race: a commit in a
/// contributing database that lands between a computation's reads and its store
/// must not leave a stale value behind.
///
/// Driven at the HTTP level because the race is a window a test cannot reliably
/// hit by running a real computation — and the mechanism is exactly "the
/// revisions this value claims to be composed from", which a store can state
/// directly. The foreign writer answers from its own fired log, so a pin that
/// is already out of date is refused rather than registered.
#[tokio::test]
async fn a_commit_in_a_contributing_database_loses_the_race() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_derived_race", |direct, spec| async move {
            direct
                .insert_instance(
                    &person("alice", "Ann", 30)?,
                    DocumentInsertArgs::from(spec.clone()),
                )
                .await?;
            let shared_name = format!("{}_shared", spec.db);
            let shared = shared_database(&direct, &shared_name).await?;
            let reactor =
                Reactor::start_with(up, &spec.db, common::with_database(&shared_name)).await?;
            let shared_ns = terminusdb_reactor::ids::Namespace::new("admin", &shared_name, "main");
            assert!(
                common::eventually(std::time::Duration::from_secs(20), || reactor
                    .engine
                    .watermark(&shared_ns)
                    .is_some())
                .await
            );

            // The revision our imaginary computation read the chunk at.
            let read_at = reactor
                .engine
                .watermark(&shared_ns)
                .expect("a watermark")
                .commit;

            // …and then the chunk changes, before the value is stored.
            direct
                .insert_instance(
                    &person("intro", "Introduction", 1)?,
                    DocumentInsertArgs::from(shared.clone()),
                )
                .await?;
            assert!(
                common::eventually(std::time::Duration::from_secs(20), || reactor
                    .engine
                    .watermark(&shared_ns)
                    .is_some_and(|w| w.commit != read_at))
                .await,
                "the shared commit must be applied before the store is attempted"
            );

            let at = reactor.head().expect("a watermark");
            let stored = reactor
                .post_with(
                    &format!("/_reactor/value/admin/{}/main?key=composed", spec.db),
                    serde_json::json!({"page": "stale"}),
                    &[
                        ("x-reactor-at", at.as_str()),
                        (
                            "x-reactor-deps",
                            &format!("doc:Person/alice, {}#doc:Person/intro", shared_ns.path()),
                        ),
                        (
                            "x-reactor-foreign-at",
                            &format!("{}={}", shared_ns.path(), read_at),
                        ),
                    ],
                )
                .await?;
            assert_eq!(stored.status, 200, "body: {}", stored.text());
            let report = stored.json()?;
            assert_eq!(
                report["raced"],
                serde_json::json!(true),
                "the chunk moved under it: {}",
                stored.text()
            );
            assert_eq!(
                report["current"],
                serde_json::json!(false),
                "and it must not be the current version: {}",
                stored.text()
            );

            // And it is not served to the next reader.
            let read = reactor
                .get(&format!(
                    "/_reactor/value/admin/{}/main?key=composed",
                    spec.db
                ))
                .await?;
            assert_eq!(
                read.status,
                404,
                "a value that lost the race must not be served: {}",
                read.text()
            );

            direct.delete_database(&shared.db).await?;
            Ok(())
        })
        .await
}

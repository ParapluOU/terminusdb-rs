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
        let document = scope
            .client()
            .get_document(&this.id, scope.spec(), GetOpts::default())
            .await?;
        let name = document["name"].as_str().unwrap_or_default().to_uppercase();
        Ok(Shout { name })
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

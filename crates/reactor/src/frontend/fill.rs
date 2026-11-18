//! Single-flight fills.
//!
//! When a commit invalidates a hot fragment, every in-flight request for it
//! misses at once. Without coalescing, the cache turns a load spike into a
//! *larger* load spike on the node it exists to protect — the thundering herd
//! that this module prevents.
//!
//! Three properties, each of which has bitten somebody before:
//!
//! * **Keyed by `(key, position)`, not by key.** Two readers latched at
//!   different positions are asking different questions and must not share an
//!   answer.
//! * **Detached execution.** The fill runs on its own task, so a client that
//!   disconnects mid-fill cannot cancel the work other waiters are blocked on.
//!   A cancelled future in the leader's request task would otherwise take the
//!   followers' answer with it.
//! * **Errors are never memoized.** The flight entry is removed *before* the
//!   result is published, so a waiter that wakes on an error and retries starts
//!   a fresh attempt instead of joining a dead flight. Invariant 7.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use bytes::Bytes;
use http::HeaderMap;
use parking_lot::Mutex;
use tokio::sync::{watch, Semaphore};

use crate::engine::{FragmentKey, FragmentVersion};
use crate::ids::Position;

/// What a completed fill produced.
#[derive(Clone, Debug)]
pub struct FillOutcome {
    /// Upstream status.
    pub status: u16,
    /// Upstream headers, already stripped of hop-by-hop entries.
    pub headers: HeaderMap,
    /// The body, verbatim.
    pub body: Bytes,
    /// The stored version, when the response was cacheable. `None` means the
    /// bytes are a correct answer that we chose not to (or could not) cache: an
    /// unpinnable response, an oversized body, a non-2xx status.
    pub version: Option<Arc<FragmentVersion>>,
}

/// A fill result shared between the leader and its followers.
///
/// The error is a `String` because it must be cheap to clone for every waiter,
/// and because by the time it reaches one the only useful thing left is the
/// message.
pub type SharedOutcome = Arc<Result<FillOutcome, String>>;

/// One flight's identity: a fragment as of a position.
type FlightKey = (FragmentKey, Position);

/// In-flight fills, shared with the detached tasks that run them.
type FlightMap = Mutex<HashMap<FlightKey, watch::Receiver<Option<SharedOutcome>>>>;

/// Whether this caller leads a flight or follows one.
enum Role {
    Follower(watch::Receiver<Option<SharedOutcome>>),
    Leader(
        watch::Sender<Option<SharedOutcome>>,
        watch::Receiver<Option<SharedOutcome>>,
    ),
}

/// Coordinates concurrent fills.
pub struct FillCoordinator {
    inflight: Arc<FlightMap>,
    /// Bounds concurrent upstream fills across the whole process, so a commit
    /// that dirties thousands of fragments cannot stampede the node.
    permits: Arc<Semaphore>,
}

impl FillCoordinator {
    /// Build a coordinator allowing `max_inflight` concurrent upstream fills.
    pub fn new(max_inflight: usize) -> Self {
        Self {
            inflight: Arc::new(Mutex::new(HashMap::new())),
            permits: Arc::new(Semaphore::new(max_inflight.max(1))),
        }
    }

    /// Number of distinct fills currently in flight.
    pub fn inflight(&self) -> usize {
        self.inflight.lock().len()
    }

    /// Fill `(key, at)` exactly once, however many callers ask at once.
    ///
    /// `make` is called only by the leader, and its future is moved onto a
    /// detached task — hence `'static + Send`.
    pub async fn fill<F, Fut>(&self, key: FragmentKey, at: Position, make: F) -> SharedOutcome
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<FillOutcome>> + Send + 'static,
    {
        let flight: FlightKey = (key, at);

        // Deciding leader-or-follower happens under one lock, so two callers
        // cannot both conclude they are the leader.
        let role = {
            let mut inflight = self.inflight.lock();
            match inflight.get(&flight) {
                Some(existing) => Role::Follower(existing.clone()),
                None => {
                    let (tx, rx) = watch::channel(None);
                    inflight.insert(flight.clone(), rx.clone());
                    Role::Leader(tx, rx)
                }
            }
        };

        match role {
            Role::Follower(receiver) => wait_for(receiver).await,
            Role::Leader(tx, receiver) => {
                self.spawn_leader(flight, tx, make());
                wait_for(receiver).await
            }
        }
    }

    /// Run the fill on its own task.
    fn spawn_leader<Fut>(
        &self,
        flight: FlightKey,
        tx: watch::Sender<Option<SharedOutcome>>,
        future: Fut,
    ) where
        Fut: Future<Output = anyhow::Result<FillOutcome>> + Send + 'static,
    {
        let permits = Arc::clone(&self.permits);
        let inflight = Arc::clone(&self.inflight);
        tokio::spawn(async move {
            // A fill that cannot get a permit waits rather than stampeding.
            let _permit = permits.acquire().await;
            let outcome: SharedOutcome = Arc::new(future.await.map_err(|err| format!("{err:#}")));
            // Removed before publishing: see the module docs.
            inflight.lock().remove(&flight);
            let _ = tx.send(Some(outcome));
        });
    }
}

/// Wait for a flight's result.
async fn wait_for(mut receiver: watch::Receiver<Option<SharedOutcome>>) -> SharedOutcome {
    loop {
        if let Some(outcome) = receiver.borrow_and_update().clone() {
            return outcome;
        }
        if receiver.changed().await.is_err() {
            // The leader's task vanished without publishing — a panic, or
            // runtime shutdown. Report it rather than hanging forever.
            return Arc::new(Err("fill task ended without a result".to_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ContentHash, Epoch, Namespace, Seq, ShapeId};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn key(name: &str) -> FragmentKey {
        FragmentKey::request(
            Namespace::new("admin", "mydb", "main"),
            ContentHash::of(name.as_bytes()),
            ShapeId::of(b"shape"),
        )
    }

    fn pos(seq: u64) -> Position {
        Position {
            epoch: Epoch(0),
            seq: Seq(seq),
        }
    }

    fn outcome(body: &'static str) -> FillOutcome {
        FillOutcome {
            status: 200,
            headers: HeaderMap::new(),
            body: Bytes::from_static(body.as_bytes()),
            version: None,
        }
    }

    #[tokio::test]
    async fn a_single_fill_returns_its_outcome() {
        let coordinator = FillCoordinator::new(4);
        let result = coordinator
            .fill(key("a"), pos(1), || async { Ok(outcome("body")) })
            .await;
        let outcome = result.as_ref().as_ref().expect("a successful fill");
        assert_eq!(outcome.body, Bytes::from_static(b"body"));
        assert_eq!(coordinator.inflight(), 0, "the flight is cleaned up");
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_upstream_request() {
        let coordinator = Arc::new(FillCoordinator::new(4));
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let coordinator = Arc::clone(&coordinator);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                coordinator
                    .fill(key("hot"), pos(1), move || {
                        let calls = Arc::clone(&calls);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            // Long enough that the others pile up behind it.
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            Ok(outcome("shared"))
                        }
                    })
                    .await
            }));
        }
        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(
                result.as_ref().as_ref().unwrap().body,
                Bytes::from_static(b"shared")
            );
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "sixteen waiters, one upstream request"
        );
    }

    #[tokio::test]
    async fn different_positions_are_different_flights() {
        // Two readers latched at different positions are asking different
        // questions; sharing an answer would serve one of them the wrong
        // revision.
        let coordinator = Arc::new(FillCoordinator::new(4));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for seq in [1u64, 2] {
            let coordinator = Arc::clone(&coordinator);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                coordinator
                    .fill(key("same"), pos(seq), move || {
                        let calls = Arc::clone(&calls);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            Ok(outcome("x"))
                        }
                    })
                    .await
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn different_keys_are_different_flights() {
        let coordinator = Arc::new(FillCoordinator::new(4));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for name in ["a", "b"] {
            let coordinator = Arc::clone(&coordinator);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                coordinator
                    .fill(key(name), pos(1), move || {
                        let calls = Arc::clone(&calls);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            Ok(outcome("x"))
                        }
                    })
                    .await
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_error_reaches_every_waiter_and_is_not_memoized() {
        let coordinator = Arc::new(FillCoordinator::new(4));
        let calls = Arc::new(AtomicUsize::new(0));

        // First round: the fill fails. Every waiter sees the error.
        let mut handles = Vec::new();
        for _ in 0..4 {
            let coordinator = Arc::clone(&coordinator);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                coordinator
                    .fill(key("flaky"), pos(1), move || {
                        let calls = Arc::clone(&calls);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            anyhow::bail!("upstream exploded")
                        }
                    })
                    .await
            }));
        }
        for handle in handles {
            let result = handle.await.unwrap();
            let err = result.as_ref().as_ref().err().expect("an error");
            assert!(err.contains("upstream exploded"), "got {err}");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(coordinator.inflight(), 0);

        // Second round: a retry must actually retry.
        let result = coordinator
            .fill(key("flaky"), pos(1), || async { Ok(outcome("recovered")) })
            .await;
        assert_eq!(
            result.as_ref().as_ref().unwrap().body,
            Bytes::from_static(b"recovered"),
            "a failed fill must never become a cached failure"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the failing closure was not rerun"
        );
    }

    #[tokio::test]
    async fn a_disconnected_leader_does_not_cancel_the_fill() {
        // The leader's request task is aborted mid-fill. A follower must still
        // get its answer, because the work runs detached.
        let coordinator = Arc::new(FillCoordinator::new(4));
        let completed = Arc::new(AtomicUsize::new(0));

        let leader = {
            let coordinator = Arc::clone(&coordinator);
            let completed = Arc::clone(&completed);
            tokio::spawn(async move {
                coordinator
                    .fill(key("slow"), pos(1), move || {
                        let completed = Arc::clone(&completed);
                        async move {
                            tokio::time::sleep(Duration::from_millis(80)).await;
                            completed.fetch_add(1, Ordering::SeqCst);
                            Ok(outcome("done"))
                        }
                    })
                    .await;
            })
        };

        // Let the leader register the flight, then join as a follower.
        tokio::time::sleep(Duration::from_millis(20)).await;
        let follower = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .fill(key("slow"), pos(1), || async { Ok(outcome("unused")) })
                    .await
            })
        };

        // The client that started it goes away.
        tokio::time::sleep(Duration::from_millis(10)).await;
        leader.abort();

        let result = follower.await.unwrap();
        assert_eq!(
            result.as_ref().as_ref().unwrap().body,
            Bytes::from_static(b"done"),
            "the detached fill survived its originator"
        );
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_permit_count_bounds_concurrency() {
        let coordinator = Arc::new(FillCoordinator::new(2));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..8 {
            let coordinator = Arc::clone(&coordinator);
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                let name = format!("k{i}");
                coordinator
                    .fill(key(&name), pos(1), move || {
                        let concurrent = Arc::clone(&concurrent);
                        let peak = Arc::clone(&peak);
                        async move {
                            let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            concurrent.fetch_sub(1, Ordering::SeqCst);
                            Ok(outcome("x"))
                        }
                    })
                    .await
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        assert!(
            peak.load(Ordering::SeqCst) <= 2,
            "peak concurrency was {}",
            peak.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn a_late_follower_after_completion_simply_refills() {
        let coordinator = FillCoordinator::new(4);
        coordinator
            .fill(key("a"), pos(1), || async { Ok(outcome("first")) })
            .await;
        let second = coordinator
            .fill(key("a"), pos(1), || async { Ok(outcome("second")) })
            .await;
        assert_eq!(
            second.as_ref().as_ref().unwrap().body,
            Bytes::from_static(b"second"),
            "the coordinator coalesces, it does not cache"
        );
    }
}

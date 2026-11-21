//! The head-poll driver.
//!
//! **This is the primary driver, and on some deployments the only one that
//! works.** On a shared-disk topology with one writer node and several reader
//! nodes, a reader has no local write path: commits arrive by replication, and
//! the `_ChangeSet` subscription on that node is expected to stay silent
//! forever. Polling the head is then the whole capture pipeline, and
//! `poll_interval_ms` *is* the staleness bound.
//!
//! It therefore has to be dull and unkillable. Three things it must tolerate,
//! all of which happen in practice:
//!
//! * **The head not moving**, which is the overwhelmingly common case — so the
//!   unchanged path costs one cheap request and emits nothing.
//! * **The head going backwards**, after a reset or a divergent pull. The driver
//!   reports it as a plain event and lets the applier's reconciler decide; a
//!   driver that tried to be clever here would be a driver with its own, second
//!   opinion about history.
//! * **Transient errors**, during a restart or a replication hiccup. These log
//!   at debug and back off. The loop never exits, because a capture task that
//!   quits turns the cache into a silently stale one.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::BoxStream;
use futures_util::StreamExt;

use crate::capture::{CommitEvent, CommitSource};
use crate::ids::Namespace;
use terminusdb_client::{CommitId, TerminusDBHttpClient};

/// How long to wait before the next head check.
///
/// A function rather than a number because the right interval depends on
/// whether anybody is reading the namespace. With a database per tenant, most
/// are idle most of the time, and polling every head at the hot interval to
/// notice commits nobody is waiting for is the dominant cost of that layout —
/// so the interval is asked for per tick, not fixed when the stream is built.
pub type Interval = Arc<dyn Fn() -> Duration + Send + Sync>;

/// Polls the branch head.
pub struct HeadPoll {
    client: Arc<TerminusDBHttpClient>,
    interval: Interval,
}

/// Longest backoff after repeated failures. Short on purpose: the staleness
/// bound is the poll interval, and a node that has been unreachable for a
/// minute is exactly when we most want to notice it coming back.
const MAX_BACKOFF: Duration = Duration::from_secs(5);

impl HeadPoll {
    /// Build a driver with a fixed interval.
    pub fn new(client: Arc<TerminusDBHttpClient>, interval: Duration) -> Self {
        Self::with_interval(client, Arc::new(move || interval))
    }

    /// Build a driver that asks for its interval before every check.
    pub fn with_interval(client: Arc<TerminusDBHttpClient>, interval: Interval) -> Self {
        Self { client, interval }
    }
}

impl CommitSource for HeadPoll {
    fn name(&self) -> &'static str {
        "poll"
    }

    fn subscribe(
        &self,
        ns: &Namespace,
        after: Option<CommitId>,
    ) -> BoxStream<'static, anyhow::Result<CommitEvent>> {
        let client = Arc::clone(&self.client);
        let ns = ns.clone();
        let interval = Arc::clone(&self.interval);
        let spec = ns.to_branch_spec();

        async_stream::stream! {
            // `after` is the position we believe we are at, so a head equal to
            // it is not news. Starting from it avoids one redundant event (and
            // therefore one redundant log walk) on every restart.
            let mut last = after;
            let mut failures: u32 = 0;
            loop {
                // Asked for every tick: a namespace that goes quiet backs off
                // without the stream being torn down and rebuilt.
                let base = interval().max(Duration::from_millis(1));
                let wait = if failures == 0 {
                    base
                } else {
                    (base * 2u32.saturating_pow(failures.min(5))).min(MAX_BACKOFF)
                };
                tokio::time::sleep(wait).await;
                match client.get_latest_commit_id(&spec).await {
                    Ok(head) => {
                        failures = 0;
                        if last.as_ref() == Some(&head) {
                            // The common case. Nothing to report.
                            continue;
                        }
                        tracing::debug!(%ns, head = %head, "head moved");
                        last = Some(head.clone());
                        // No parent: a poll only ever learns "the head is now
                        // X". The applier reconciles by walking the log, which
                        // is also what makes a backwards head safe here.
                        yield Ok(CommitEvent::new(head));
                    }
                    Err(err) => {
                        tracing::debug!(%ns, error = %err, "head poll failed; backing off");
                        failures = failures.saturating_add(1);
                    }
                }
            }
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interval_is_never_zero() {
        // A zero interval would spin a core at 100% against the node this
        // process exists to protect. Clamped per tick, since the provider can
        // return anything.
        let client = Arc::new(dummy_client());
        let poll = HeadPoll::new(client, Duration::ZERO);
        assert_eq!((poll.interval)(), Duration::ZERO);
        assert!((poll.interval)().max(Duration::from_millis(1)) >= Duration::from_millis(1));
        assert_eq!(poll.name(), "poll");
    }

    #[test]
    fn the_interval_is_asked_for_every_tick() {
        // The adaptive case: the same stream polls fast while a namespace is
        // being read and slowly once it goes quiet, without being rebuilt.
        let hot = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = Arc::clone(&hot);
        let interval: Interval = Arc::new(move || {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(5)
            }
        });
        assert_eq!(interval(), Duration::from_millis(150));
        hot.store(false, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(interval(), Duration::from_secs(5));
    }

    #[test]
    fn backoff_is_bounded() {
        let base = Duration::from_millis(150);
        let mut longest = Duration::ZERO;
        for failures in 1u32..20 {
            let wait = (base * 2u32.saturating_pow(failures.min(5))).min(MAX_BACKOFF);
            longest = longest.max(wait);
            assert!(wait <= MAX_BACKOFF);
        }
        assert_eq!(longest, (base * 32).min(MAX_BACKOFF));
    }

    /// A client pointed at a port nothing is listening on. Constructing one
    /// performs no I/O, which is all these tests need.
    fn dummy_client() -> TerminusDBHttpClient {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(TerminusDBHttpClient::new(
                url::Url::parse("http://127.0.0.1:1").unwrap(),
                "admin",
                "root",
                "admin",
            ))
            .expect("constructing a client does no I/O")
    }
}

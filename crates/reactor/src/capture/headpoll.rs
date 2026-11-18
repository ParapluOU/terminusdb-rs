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

/// Polls the branch head.
pub struct HeadPoll {
    client: Arc<TerminusDBHttpClient>,
    interval: Duration,
}

/// Longest backoff after repeated failures. Short on purpose: the staleness
/// bound is the poll interval, and a node that has been unreachable for a
/// minute is exactly when we most want to notice it coming back.
const MAX_BACKOFF: Duration = Duration::from_secs(5);

impl HeadPoll {
    /// Build a driver.
    pub fn new(client: Arc<TerminusDBHttpClient>, interval: Duration) -> Self {
        Self {
            client,
            interval: interval.max(Duration::from_millis(1)),
        }
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
        let interval = self.interval;
        let spec = ns.to_branch_spec();

        async_stream::stream! {
            // `after` is the position we believe we are at, so a head equal to
            // it is not news. Starting from it avoids one redundant event (and
            // therefore one redundant log walk) on every restart.
            let mut last = after;
            let mut backoff = interval;
            loop {
                tokio::time::sleep(backoff).await;
                match client.get_latest_commit_id(&spec).await {
                    Ok(head) => {
                        backoff = interval;
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
                        backoff = (backoff * 2).min(MAX_BACKOFF);
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
        // process exists to protect.
        let client = Arc::new(dummy_client());
        let poll = HeadPoll::new(client, Duration::ZERO);
        assert!(poll.interval >= Duration::from_millis(1));
        assert_eq!(poll.name(), "poll");
    }

    #[test]
    fn backoff_is_bounded() {
        let mut backoff = Duration::from_millis(150);
        for _ in 0..20 {
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
        assert_eq!(backoff, MAX_BACKOFF);
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

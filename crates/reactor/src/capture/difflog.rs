//! The diff-log driver: one endpoint for both halves of capture.
//!
//! The head-poll driver learns exactly one thing per tick — "the head is now
//! X" — so every event costs the applier a log walk to work out what happened
//! between X and where it was, and `/api/log` is `O(depth)`: 467 ms at depth
//! 261, growing. This driver reads the per-commit feed the server now serves at
//! `/api/graphql/_difflog/<org>/<db>` instead, and that changes the shape of
//! capture rather than just its speed:
//!
//! * **Nothing happened** costs one request that opens no layer and touches no
//!   triple. That is the case a poll is in almost always, and it is what makes
//!   a short interval affordable across a database per tenant.
//! * **Something happened** yields every intervening commit, in order, each
//!   with its parent — so there is no gap to reconcile. The head poll cannot
//!   supply a parent (it never sees one), which is precisely why it hands the
//!   applier a walk.
//! * **History was rewritten** is reported rather than inferred: the feed
//!   compares the commit we claim to be at against its own log and answers
//!   `rewoundTo`, so a reset arrives as a known discontinuity instead of a head
//!   that mysteriously went backwards.
//!
//! What this driver deliberately does NOT do is turn the feed's triples into
//! documents. A [`ChangeSet`](crate::change::ChangeSet) needs whole before and
//! after documents, and assembling those from triples means reimplementing
//! TerminusDB's document elaboration — subdocuments, lists, enums, typed
//! literals — inside Reactor, where a subtle disagreement with the server is
//! not a bug in a cache but wrong data served from it. The resolver keeps doing
//! pinned reads against the node that already knows how. The feed's triples are
//! still useful there, as a precise and free list of which subjects changed,
//! and that is the next step rather than this one.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::BoxStream;
use futures_util::StreamExt;
use serde::Deserialize;

use crate::capture::{CommitEvent, CommitHistory, CommitSource, Interval, LogCommit};
use crate::ids::Namespace;
use terminusdb_client::CommitId;

/// Longest backoff after repeated failures, matching the head poll's: the
/// staleness bound is the poll interval, and a node that has been unreachable
/// for a minute is exactly when noticing its return matters most.
const MAX_BACKOFF: Duration = Duration::from_secs(5);

/// How many records to take per request. The feed bounds its own work by this,
/// so it is a latency bound rather than a correctness one: a backlog is drained
/// over several polls instead of one enormous response.
const PAGE: usize = 256;

/// Reads a branch's diff log.
pub struct DiffLog {
    http: reqwest::Client,
    upstream: url::Url,
    user: String,
    pass: String,
    interval: Interval,
}

impl DiffLog {
    pub fn new(
        http: reqwest::Client,
        upstream: url::Url,
        user: impl Into<String>,
        pass: impl Into<String>,
        interval: Interval,
    ) -> Self {
        Self {
            http,
            upstream,
            user: user.into(),
            pass: pass.into(),
            interval,
        }
    }

    /// `/api/graphql/_difflog/<org>/<db>`.
    ///
    /// The graph selector leads rather than trails here, unlike `_commits`;
    /// that is forced by the server's router and documented there.
    fn endpoint(&self, ns: &Namespace) -> Result<url::Url, url::ParseError> {
        self.upstream.join(&format!(
            "/api/graphql/_difflog/{}/{}",
            ns.org, ns.db
        ))
    }

    /// Ask for one page. `after` is the commit we believe sits at `from - 1`,
    /// which is what lets the feed tell us if that is no longer true.
    async fn page(
        &self,
        ns: &Namespace,
        from: usize,
        after: Option<&CommitId>,
    ) -> anyhow::Result<Page> {
        let query = r#"query Feed($from: Int!, $limit: Int!, $after: String, $branch: String!) {
            feed(from: $from, limit: $limit, after: $after, branch: $branch) {
                epoch head nextOffset rewoundTo
                records { offset commit }
            }
        }"#;
        let body = serde_json::json!({
            "query": query,
            "variables": {
                "from": from as i64,
                "limit": PAGE as i64,
                "after": after.map(|commit| commit.to_string()),
                "branch": ns.branch,
            }
        });
        let response = self
            .http
            .post(self.endpoint(ns)?)
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("diff log answered {status}: {text}");
        }
        let envelope: Envelope = serde_json::from_str(&text)
            .map_err(|error| anyhow::anyhow!("diff log sent unreadable JSON: {error}: {text}"))?;
        if let Some(errors) = envelope.errors.filter(|errors| !errors.is_empty()) {
            let joined: Vec<&str> = errors.iter().map(|error| error.message.as_str()).collect();
            anyhow::bail!("diff log refused the query: {}", joined.join("; "));
        }
        envelope
            .data
            .map(|data| data.feed)
            .ok_or_else(|| anyhow::anyhow!("diff log returned neither data nor errors"))
    }
}

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct Data {
    feed: Page,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// Identifier of the chain's root commit: the identity of this history.
    pub epoch: Option<String>,
    /// Number of records in the log, which is where the next commit lands.
    pub head: i64,
    pub next_offset: i64,
    /// Set when `after` did not match, meaning history was rewritten.
    pub rewound_to: Option<i64>,
    pub records: Vec<Record>,
}

#[derive(Deserialize, Debug)]
pub struct Record {
    pub offset: i64,
    pub commit: String,
}

/// Whether the upstream carries an extension we want.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The server says it has it. Use it.
    Present,
    /// The server says it does not. Permanent for this server, so fall back to
    /// a weaker driver rather than back off forever.
    Absent,
    /// We could not ask: unreachable, unauthorized, a payload we did not
    /// understand. The extension may well be there, so retrying is right —
    /// demoting on this would be giving up on evidence we do not have.
    Unknown,
}

impl DiffLog {
    /// Ask `/api/info` whether this server carries the diff log.
    ///
    /// Asked of the server rather than inferred from a failed request, and
    /// asked once per server rather than once per namespace. `/api/info`
    /// reports a `distribution` and a list of `extensions`, and the capability
    /// check is against the latter: a distribution name says who built the
    /// server, not what it can do, so an older `paraplu` build answers
    /// `paraplu` while lacking this surface. Branching on the name would mean
    /// keeping a distribution-to-version-to-feature table that goes stale in
    /// silence.
    ///
    /// Three outcomes rather than two, because `Absent` and `Unknown` want
    /// opposite responses: a server that does not have the feed should fall
    /// back to polling now and permanently, while a server we cannot reach
    /// should be retried. Conflating them either downgrades a capable server
    /// after one blip, or leaves capture backing off forever against a node
    /// that will never answer — and that second one is a silently stale cache.
    pub async fn availability(&self) -> Availability {
        let endpoint = match self.upstream.join("/api/info") {
            Ok(endpoint) => endpoint,
            Err(_) => return Availability::Unknown,
        };
        let response = self
            .http
            .get(endpoint)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await;
        let Ok(response) = response else {
            return Availability::Unknown;
        };
        if !response.status().is_success() {
            return Availability::Unknown;
        }
        let Ok(text) = response.text().await else {
            return Availability::Unknown;
        };
        let Ok(body) = serde_json::from_str::<serde_json::Value>(&text) else {
            return Availability::Unknown;
        };
        // An upstream old enough to predate extension reporting says nothing
        // about them, which is not the same as reporting none: it cannot be
        // asked, so it is Unknown and the driver falls back through its own
        // failures instead of being told to.
        let Some(extensions) = body
            .get("api:info")
            .and_then(|info| info.get("extensions"))
            .and_then(|extensions| extensions.as_array())
        else {
            return Availability::Unknown;
        };
        if extensions
            .iter()
            .any(|extension| extension.as_str() == Some(EXTENSION))
        {
            Availability::Present
        } else {
            Availability::Absent
        }
    }
}

/// The name this driver's surface reports itself under in `/api/info`.
const EXTENSION: &str = "difflog";

impl CommitSource for DiffLog {
    fn name(&self) -> &'static str {
        "difflog"
    }

    fn subscribe(
        &self,
        ns: &Namespace,
        after: Option<CommitId>,
    ) -> BoxStream<'static, anyhow::Result<CommitEvent>> {
        let feed = DiffLog {
            http: self.http.clone(),
            upstream: self.upstream.clone(),
            user: self.user.clone(),
            pass: self.pass.clone(),
            interval: Arc::clone(&self.interval),
        };
        let ns = ns.clone();

        async_stream::stream! {
            // Where we are in the log, and the commit that proves it. Both
            // start unknown: on the first poll we ask from 0 and let the feed
            // tell us how long the log is, rather than assuming the history is
            // short enough to replay.
            let mut offset: Option<usize> = None;
            let mut last: Option<CommitId> = after;
            let mut epoch: Option<String> = None;
            let mut failures: u32 = 0;

            loop {
                let base = (feed.interval)().max(Duration::from_millis(1));
                let wait = if failures == 0 {
                    base
                } else {
                    (base * 2u32.saturating_pow(failures.min(5))).min(MAX_BACKOFF)
                };
                tokio::time::sleep(wait).await;

                // Until we know our offset, ask for the tail only: the point of
                // this driver is to notice what happens from now on, and
                // replaying a long history at startup would be a stampede, not
                // a catch-up. The applier's reconciler owns backfill.
                let (from, send_after) = match offset {
                    Some(offset) => (offset, last.as_ref()),
                    None => (usize::MAX, None),
                };
                let page = if from == usize::MAX {
                    // `from` beyond the head returns an empty page whose `head`
                    // tells us where to start.
                    feed.page(&ns, i64::MAX as usize, None).await
                } else {
                    feed.page(&ns, from, send_after).await
                };

                match page {
                    Ok(page) => {
                        failures = 0;

                        // A different root commit means a different history
                        // under the same name — a database recreated, not a
                        // branch extended. Treat it as a discontinuity.
                        let rewritten = match (&epoch, &page.epoch) {
                            (Some(known), Some(current)) if known != current => true,
                            _ => false,
                        };
                        if page.epoch.is_some() {
                            epoch = page.epoch.clone();
                        }

                        if offset.is_none() {
                            // First successful poll: start at the head, so the
                            // next one reports only new commits.
                            offset = Some(page.head.max(0) as usize);
                            continue;
                        }

                        if rewritten || page.rewound_to.is_some() {
                            let to = page.rewound_to.unwrap_or(0).max(0) as usize;
                            tracing::debug!(
                                %ns, rewound_to = to,
                                "diff log reports a rewritten history"
                            );
                            // Resume where the feed says the logs still agree,
                            // and let the applier reconcile: it is the only
                            // part of capture allowed an opinion about history.
                            offset = Some(to);
                            last = None;
                            continue;
                        }

                        for record in &page.records {
                            let commit = CommitId::from(record.commit.clone());
                            // Every commit arrives with the one before it, so
                            // the applier is told the parent instead of having
                            // to walk the log for it.
                            let event = CommitEvent::with_parent(commit.clone(), last.clone());
                            last = Some(commit);
                            yield Ok(event);
                        }
                        offset = Some(page.next_offset.max(0) as usize);
                    }
                    Err(error) => {
                        tracing::debug!(%ns, error = %error, "diff log poll failed; backing off");
                        failures = failures.saturating_add(1);
                    }
                }
            }
        }
        .boxed()
    }
}

#[async_trait::async_trait]
impl CommitHistory for DiffLog {
    /// Commits newest-first from the head.
    ///
    /// The feed is oldest-first and offset-addressed, so this reads the last
    /// `count` records and reverses them — which is `O(count)` rather than
    /// `/api/log`'s `O(depth)`, and is the reason the reconciler can afford to
    /// be asked.
    async fn recent(&self, ns: &Namespace, count: usize) -> anyhow::Result<Vec<LogCommit>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let head = self.page(ns, i64::MAX as usize, None).await?.head.max(0) as usize;
        let from = head.saturating_sub(count);
        let page = self.page(ns, from, None).await?;
        let mut commits: Vec<LogCommit> = Vec::with_capacity(page.records.len());
        // The parent of the record at offset n is the record at n-1; the oldest
        // record in the page only has one if the page did not start the log.
        let mut previous: Option<CommitId> = None;
        for record in &page.records {
            let commit = CommitId::from(record.commit.clone());
            commits.push(LogCommit {
                commit: commit.clone(),
                parent: previous.take(),
            });
            previous = Some(commit);
        }
        if from > 0 {
            // We did not see offset `from - 1`, so the oldest entry's parent is
            // unknown rather than absent. Saying `None` would claim it is the
            // first commit on the branch.
            if let Some(oldest) = commits.first_mut() {
                oldest.parent = None;
            }
        }
        commits.reverse();
        Ok(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(interval: Duration) -> DiffLog {
        DiffLog::new(
            reqwest::Client::new(),
            url::Url::parse("http://127.0.0.1:1").unwrap(),
            "admin",
            "root",
            Arc::new(move || interval),
        )
    }

    fn ns() -> Namespace {
        Namespace::new("admin", "people", "main")
    }

    #[test]
    fn the_endpoint_puts_the_selector_before_the_database() {
        // Forced by the server's router: a parameter route cannot sit beside
        // the subscription plugin's catch-all under /api/graphql.
        assert_eq!(
            "http://127.0.0.1:1/api/graphql/_difflog/admin/people",
            feed(Duration::from_millis(1)).endpoint(&ns()).unwrap().as_str()
        );
    }

    #[test]
    fn the_name_distinguishes_it_in_logs_and_metrics() {
        assert_eq!("difflog", feed(Duration::from_millis(1)).name());
    }

    #[test]
    fn a_page_parses_the_fields_the_driver_depends_on() {
        let page: Page = serde_json::from_value(serde_json::json!({
            "epoch": "c0",
            "head": 5,
            "nextOffset": 3,
            "rewoundTo": null,
            "records": [{"offset": 1, "commit": "c1"}, {"offset": 2, "commit": "c2"}]
        }))
        .unwrap();
        assert_eq!(5, page.head);
        assert_eq!(3, page.next_offset);
        assert_eq!(None, page.rewound_to);
        assert_eq!(2, page.records.len());
        assert_eq!("c1", page.records[0].commit);
    }

    #[test]
    fn a_rewind_is_carried_through_rather_than_defaulted_away() {
        let page: Page = serde_json::from_value(serde_json::json!({
            "epoch": "c0", "head": 9, "nextOffset": 2, "rewoundTo": 2, "records": []
        }))
        .unwrap();
        assert_eq!(Some(2), page.rewound_to);
        assert!(page.records.is_empty());
    }

    #[tokio::test]
    async fn an_unreachable_node_is_unknown_rather_than_absent() {
        // The distinction is load-bearing: Absent falls back to polling for
        // good, Unknown keeps retrying. Treating a down node as Absent would
        // permanently downgrade a capable server after one blip.
        let feed = feed(Duration::from_millis(1));
        assert_eq!(Availability::Unknown, feed.availability().await);
    }

    #[test]
    fn capability_comes_from_the_extension_list_not_the_distribution_name() {
        // An older paraplu build answers `paraplu` and does NOT carry the
        // feed, so the name cannot be the check.
        let reads = |body: serde_json::Value| -> Option<bool> {
            let extensions = body
                .get("api:info")?
                .get("extensions")?
                .as_array()?
                .iter()
                .any(|extension| extension.as_str() == Some(EXTENSION));
            Some(extensions)
        };
        assert_eq!(
            Some(true),
            reads(serde_json::json!({"api:info": {
                "distribution": "paraplu",
                "extensions": ["difflog", "commit_hooks"]
            }}))
        );
        assert_eq!(
            Some(false),
            reads(serde_json::json!({"api:info": {
                "distribution": "paraplu",
                "extensions": ["commit_hooks"]
            }}))
        );
        // An upstream that reports no extensions key at all cannot be asked,
        // which is Unknown rather than Absent — tested here as the absence of
        // an answer.
        assert_eq!(
            None,
            reads(serde_json::json!({"api:info": {"distribution": "terminusdb"}}))
        );
    }

    #[test]
    fn graphql_errors_are_not_mistaken_for_an_empty_log() {
        // The feed answers 200 with an `errors` array for a forbidden database.
        // Reading that as "no commits" would make an unreadable namespace look
        // like a quiet one, and the cache would never notice it was blind.
        let envelope: Envelope = serde_json::from_value(serde_json::json!({
            "data": null,
            "errors": [{"message": "bob may not read admin/people"}]
        }))
        .unwrap();
        assert!(envelope.data.is_none());
        assert_eq!(1, envelope.errors.unwrap().len());
    }
}

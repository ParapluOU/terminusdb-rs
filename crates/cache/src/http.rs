//! The three calls this crate makes against Reactor.
//!
//! Deliberately small and deliberately not typed against the reactor crate: an
//! application caches through the HTTP surface, exactly as a non-Rust one would,
//! and dependency terms travel as opaque text. If this file needed Reactor's
//! types, every application would be pinned to Reactor's version.

use bytes::Bytes;
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

use terminusdb_client::{CommitId, TerminusDBHttpClient};

use crate::{Inner, Outcome};

/// The commit a value was computed at.
const AT_HEADER: &str = "x-reactor-at";
/// The scope a read belongs to, or that a store draws its dependencies from.
const RECORD_HEADER: &str = "x-reactor-record";
/// A declared dependency set, for the cases a scope cannot cover.
const DEPS_HEADER: &str = "x-reactor-deps";
/// Union this value's dependencies into a second scope as well.
const INTO_HEADER: &str = "x-reactor-into";

/// The commit each *other* database's reads were pinned at.
const FOREIGN_AT_HEADER: &str = "x-reactor-foreign-at";

/// Namespace coordinates.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Ns {
    pub org: String,
    pub db: String,
    pub branch: String,
}

impl Ns {
    /// `org/db/local/branch/branch`, as `/_reactor/head` reports it.
    pub(crate) fn path(&self) -> String {
        format!("{}/{}/local/branch/{}", self.org, self.db, self.branch)
    }
}

/// Where a store's dependency set comes from.
pub(crate) enum DepSource<'a> {
    /// A recorded scope: Reactor has the terms already.
    Scope(&'a str),
    /// A declared list, rendered for the header. `None` renders as no
    /// dependencies, which Reactor refuses — the point being that declaring an
    /// empty set by accident is not the same as declaring independence.
    Declared(Option<String>),
    /// Explicitly independent of the database.
    Independent,
}

#[derive(Debug, Deserialize)]
struct HeadReport {
    namespace: String,
    commit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StoreReport {
    raced: bool,
}

impl Inner {
    fn value_url(&self, ns: &Ns, key: &str) -> anyhow::Result<url::Url> {
        Ok(self.reactor.join(&format!(
            "/_reactor/value/{}/{}/{}?key={}",
            urlencoding::encode(&ns.org),
            urlencoding::encode(&ns.db),
            urlencoding::encode(&ns.branch),
            urlencoding::encode(key),
        ))?)
    }

    /// The commit this node's cache is currently answering at.
    ///
    /// Latched once per computation: everything the computation reads is pinned
    /// at it, and the stored value is placed at it. A node that is not tracking
    /// the namespace reports no commit, and there is nothing to cache against.
    /// `after` demands that the latched commit not be older than one the caller
    /// has already seen — read-your-writes across a composition. Reactor waits
    /// for its watermark to reach it and answers `412` if it cannot, which is
    /// surfaced here as an error rather than a quietly stale pin.
    pub(crate) async fn head(&self, ns: &Ns, after: Option<&CommitId>) -> anyhow::Result<CommitId> {
        let wanted = ns.path();
        let mut url = self.reactor.join("/_reactor/head")?;
        url.query_pairs_mut().append_pair("namespace", &wanted);
        if let Some(after) = after {
            url.query_pairs_mut()
                .append_pair("min_version", after.as_str());
        }
        let response = self
            .http
            .get(url)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            anyhow::bail!("Reactor would not latch a commit for {wanted} ({status}): {detail}");
        }
        let reports: Vec<HeadReport> = response.json().await?;
        reports
            .into_iter()
            .find(|report| report.namespace == wanted)
            .and_then(|report| report.commit)
            .map(CommitId::new)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Reactor has no watermark for {wanted} yet, so nothing derived from it can \
                     be cached. The first read after a namespace is registered is always a \
                     miss: there is no position to pin a fill at until the watermark lands."
                )
            })
    }

    /// Look a value up, as of `at`.
    pub(crate) async fn fetch(
        &self,
        ns: &Ns,
        key: &str,
        at: &CommitId,
        record_into: Option<&str>,
    ) -> anyhow::Result<Option<Bytes>> {
        let mut request = self
            .http
            .get(self.value_url(ns, key)?)
            .basic_auth(&self.user, Some(&self.pass))
            .header(AT_HEADER, at.as_str());
        // A nested read: a hit here is an input of the outer computation, so its
        // dependencies belong to the outer scope too.
        if let Some(scope) = record_into {
            request = request.header(RECORD_HEADER, scope);
        }
        let response = request.send().await?;
        match response.status() {
            StatusCode::OK => Ok(Some(response.bytes().await?)),
            StatusCode::NOT_FOUND => Ok(None),
            status => anyhow::bail!(
                "Reactor refused to read {key}: {status} {}",
                response.text().await.unwrap_or_default().trim()
            ),
        }
    }

    /// Hand a value over.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn store(
        &self,
        ns: &Ns,
        key: &str,
        at: &CommitId,
        body: Bytes,
        deps: DepSource<'_>,
        into: Option<&str>,
        foreign: &BTreeMap<Ns, CommitId>,
    ) -> anyhow::Result<Outcome> {
        let mut request = self
            .http
            .post(self.value_url(ns, key)?)
            .basic_auth(&self.user, Some(&self.pass))
            .header(AT_HEADER, at.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        match deps {
            DepSource::Scope(id) => request = request.header(RECORD_HEADER, id),
            DepSource::Declared(Some(terms)) => request = request.header(DEPS_HEADER, terms),
            // No conditions at all: send nothing, and let Reactor's refusal
            // explain itself rather than inventing a local error message that
            // would have to be kept in step with it.
            DepSource::Declared(None) => {}
            DepSource::Independent => request = request.header(DEPS_HEADER, "nothing"),
        }
        if let Some(outer) = into {
            request = request.header(INTO_HEADER, outer);
        }
        if !foreign.is_empty() {
            // Which revision of each *other* database this value was composed
            // from. Reactor checks each against that namespace's own fired log
            // before it will let the value stand.
            let pins = foreign
                .iter()
                .map(|(ns, at)| format!("{}={}", ns.path(), at.as_str()))
                .collect::<Vec<_>>()
                .join(",");
            request = request.header(FOREIGN_AT_HEADER, pins);
        }

        let response = request.body(body).send().await?;
        if response.status() == StatusCode::OK {
            let report: StoreReport = response.json().await?;
            return Ok(Outcome::Stored {
                raced: report.raced,
            });
        }
        // A refusal is not an error: the computation succeeded and the caller
        // gets its value. Only the caching failed, and the body says why.
        let status = response.status();
        let why = response.text().await.unwrap_or_default();
        Ok(Outcome::NotStored(format!("{status}: {}", why.trim())))
    }

    /// A client whose every request is labelled for a scope.
    ///
    /// The label is a *default header* rather than something added per call,
    /// because a label added per call is a label forgotten on one path — and the
    /// failure mode of forgetting is a value whose dependencies are
    /// under-reported, which is stale data rather than an error.
    ///
    /// The pin is not here: it travels in the [`crate::Scope`]'s `BranchSpec`,
    /// because that is how `terminusdb-client` addresses a commit.
    pub(crate) async fn client_for_scope(
        &self,
        scope: &str,
        timeout: Option<Duration>,
    ) -> anyhow::Result<TerminusDBHttpClient> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static(RECORD_HEADER),
            reqwest::header::HeaderValue::from_str(scope)?,
        );
        TerminusDBHttpClient::new_with_headers(
            self.reactor.clone(),
            &self.user,
            &self.pass,
            &self.org,
            timeout,
            headers,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_namespace_path_is_the_one_head_reports() {
        let ns = Ns {
            org: "admin".to_owned(),
            db: "people".to_owned(),
            branch: "main".to_owned(),
        };
        assert_eq!(ns.path(), "admin/people/local/branch/main");
    }

    #[test]
    fn a_key_is_url_encoded_into_the_query() {
        // Keys contain slashes by construction (`name/params/epoch`), and may
        // contain anything at all if overridden, so they cannot go in the path.
        let encoded = urlencoding::encode("overview/ab12/dev").to_string();
        assert_eq!(encoded, "overview%2Fab12%2Fdev");
    }
}

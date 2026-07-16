//! The apply endpoint (`POST /api/apply/{resource}`): take the difference
//! between two commits and apply it to a branch — cherry-pick a change or
//! squash-merge between branches. See `docs/terminusdb/document-insertion.md`.

use anyhow::Context;
use serde::Serialize;
use serde_json::Value;

use crate::spec::BranchSpec;
use crate::TerminusDBHttpClient;

/// Options for [`TerminusDBHttpClient::apply_commit_diff`].
#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    /// Require the applied result to match the final state of `after_commit`
    /// exactly (default false).
    pub match_final_state: bool,
    /// On conflict, which fields to keep from the target branch, e.g.
    /// `{"@id": true}`.
    pub keep: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct CommitInfo<'a> {
    author: &'a str,
    message: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct ApplyRequest<'a> {
    before_commit: &'a str,
    after_commit: &'a str,
    commit_info: CommitInfo<'a>,
    match_final_state: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep: Option<&'a Value>,
}

impl TerminusDBHttpClient {
    /// Apply the difference between `before_commit` and `after_commit` onto the
    /// branch in `spec` as a new commit. `before`/`after` may be branch names or
    /// commit identifiers. Returns the server response (or an error describing
    /// an unresolvable conflict).
    pub async fn apply_commit_diff(
        &self,
        spec: &BranchSpec,
        before_commit: &str,
        after_commit: &str,
        author: &str,
        message: &str,
        options: ApplyOptions,
    ) -> anyhow::Result<Value> {
        let uri = self
            .build_url()
            .endpoint("apply")
            .database_with_branch(spec)
            .build();

        let body = ApplyRequest {
            before_commit,
            after_commit,
            commit_info: CommitInfo { author, message },
            match_final_state: options.match_final_state,
            keep: options.keep.as_ref(),
        };

        let _permit = self.acquire_write_permit().await;
        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .send()
            .await
            .context("failed to send apply request")?;

        self.parse_response(res).await
    }
}

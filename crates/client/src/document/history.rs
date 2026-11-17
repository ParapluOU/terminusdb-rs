//! Document history types and operations

use crate::CommitId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Parameters for querying document history
#[derive(Debug, Clone, Serialize, Default)]
pub struct DocumentHistoryParams {
    /// Starting index for pagination
    pub start: Option<u32>,
    /// Number of commits to return
    pub count: Option<u32>,
    /// if this is set to true, the result will be keyed with
    /// commits organized under a 'updated' property
    pub updated: Option<bool>,
    /// if this is set to true, the result will be keyed with
    /// commits organized under a 'created' property
    pub created: Option<bool>,
    /// TerminusDB 12.1 `diff`: include the diff between each commit's before
    /// and after state in the entry's `diff` field.
    pub diff: Option<bool>,
    /// TerminusDB 12.1 `complete`: walk the complete commit chain rather than
    /// stopping at the first commit that did not touch the document.
    pub complete: Option<bool>,
    /// TerminusDB 12.1 `fast`: skip the per-commit enrichment. This is the
    /// enterprise-accelerated path; on the community server it simply returns
    /// unenriched entries.
    pub fast: Option<bool>,
    /// TerminusDB 12.1 `before`: only commits before this ISO-8601 timestamp.
    pub before: Option<String>,
    /// TerminusDB 12.1 `after`: only commits after this ISO-8601 timestamp.
    pub after: Option<String>,
    /// TerminusDB 12.1 `graph_type`: `instance` (default) or `schema`.
    pub graph_type: Option<crate::document::DocumentType>,
}

impl DocumentHistoryParams {
    /// Create new history parameters with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the starting index
    pub fn with_start(mut self, start: u32) -> Self {
        self.start = Some(start);
        self
    }

    /// Set the number of commits to return
    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// Set whether to include last updated time
    pub fn with_updated(mut self, updated: bool) -> Self {
        self.updated = Some(updated);
        self
    }

    /// Set whether to include creation date
    pub fn with_created(mut self, created: bool) -> Self {
        self.created = Some(created);
        self
    }

    /// Include the per-commit diff in each entry (TerminusDB 12.1).
    pub fn with_diff(mut self, diff: bool) -> Self {
        self.diff = Some(diff);
        self
    }

    /// Walk the complete commit chain (TerminusDB 12.1).
    pub fn with_complete(mut self, complete: bool) -> Self {
        self.complete = Some(complete);
        self
    }

    /// Skip per-commit enrichment (TerminusDB 12.1).
    pub fn with_fast(mut self, fast: bool) -> Self {
        self.fast = Some(fast);
        self
    }

    /// Only commits before this ISO-8601 timestamp (TerminusDB 12.1).
    pub fn with_before(mut self, before: impl Into<String>) -> Self {
        self.before = Some(before.into());
        self
    }

    /// Only commits after this ISO-8601 timestamp (TerminusDB 12.1).
    pub fn with_after(mut self, after: impl Into<String>) -> Self {
        self.after = Some(after.into());
        self
    }

    /// Read the history of the instance or the schema graph (TerminusDB 12.1).
    pub fn with_graph_type(mut self, graph_type: crate::document::DocumentType) -> Self {
        self.graph_type = Some(graph_type);
        self
    }
}

/// A single commit entry in document history
#[derive(Debug, Clone, Deserialize)]
pub struct CommitHistoryEntry {
    /// The user who made the commit
    pub author: String,
    /// The commit identifier
    #[serde(deserialize_with = "deserialize_commit_id")]
    pub identifier: CommitId,
    /// The commit message
    pub message: String,
    /// When the commit was made (Unix timestamp as float)
    pub timestamp: f64,
    /// The diff between the document's state before and after this commit.
    /// Only present when the request set `diff=true` (TerminusDB 12.1).
    #[serde(default)]
    pub diff: Option<serde_json::Value>,
}

fn deserialize_commit_id<'de, D>(deserializer: D) -> Result<CommitId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(CommitId::from(s))
}

impl CommitHistoryEntry {
    /// Convert the timestamp float into a chrono DateTime<Utc>
    ///
    /// # Returns
    /// The parsed DateTime or an error if the timestamp is invalid
    ///
    /// # Example
    /// ```rust
    /// let entry = CommitHistoryEntry {
    ///     author: "user".to_string(),
    ///     identifier: "abc123".to_string(),
    ///     message: "Initial commit".to_string(),
    ///     timestamp: 1751808918.7295272,
    /// };
    ///
    /// let datetime = entry.timestamp_datetime()?;
    /// println!("Commit was made at: {}", datetime);
    /// ```
    pub fn timestamp_datetime(&self) -> anyhow::Result<DateTime<Utc>> {
        DateTime::from_timestamp(
            self.timestamp as i64,
            (self.timestamp.fract() * 1_000_000_000.0) as u32,
        )
        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp: {}", self.timestamp))
    }
}

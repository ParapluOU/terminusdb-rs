//! Document history types and operations

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
}

/// A single commit entry in document history
#[derive(Debug, Clone, Deserialize)]
pub struct CommitHistoryEntry {
    /// The user who made the commit
    pub author: String,
    /// The commit identifier
    pub identifier: String,
    /// The commit message
    pub message: String,
    /// When the commit was made (Unix timestamp as float)
    pub timestamp: f64,
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
            ((self.timestamp.fract() * 1_000_000_000.0) as u32),
        )
        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp: {}", self.timestamp))
    }
}

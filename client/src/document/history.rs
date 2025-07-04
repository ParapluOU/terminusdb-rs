//! Document history types and operations

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
    /// When the commit was made
    pub timestamp: String,
}
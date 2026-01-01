//! SSE Changeset Event Types
//!
//! This module provides types for consuming TerminusDB's SSE changeset stream,
//! which reports real-time changes to the database.

use serde::{Deserialize, Serialize};

/// SSE event data from TerminusDB changeset plugin
#[derive(Debug, Clone, Deserialize)]
pub struct ChangesetEvent {
    /// Resource path, e.g., "admin/dev/local/branch/main"
    pub resource: String,
    /// Branch name, e.g., "main"
    pub branch: String,
    /// Commit information
    pub commit: ChangesetCommitInfo,
    /// Metadata about changes
    pub metadata: MetadataInfo,
    /// List of document changes
    pub changes: Vec<DocumentChange>,
}

/// Information about the commit that triggered the event
#[derive(Debug, Clone, Deserialize)]
pub struct ChangesetCommitInfo {
    /// Commit ID
    pub id: String,
    /// Author ID, e.g., "User/system"
    pub author: String,
    /// Commit message
    pub message: String,
    /// Unix timestamp
    pub timestamp: f64,
}

/// Metadata about the number of changes
#[derive(Debug, Clone, Deserialize)]
pub struct MetadataInfo {
    /// Number of triple insertions
    pub inserts_count: u64,
    /// Number of triple deletions
    pub deletes_count: u64,
    /// Number of documents added
    pub documents_added: u64,
    /// Number of documents deleted
    pub documents_deleted: u64,
    /// Number of documents updated
    pub documents_updated: u64,
}

/// Document change information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChange {
    /// Document ID (e.g., "TypeName/id")
    pub id: String,
    /// Action type: "added", "deleted", or "updated"
    pub action: String,
}

impl DocumentChange {
    /// Check if this is an "added" action
    pub fn is_added(&self) -> bool {
        self.action == "added"
    }

    /// Check if this is a "deleted" action
    pub fn is_deleted(&self) -> bool {
        self.action == "deleted"
    }

    /// Check if this is an "updated" action
    pub fn is_updated(&self) -> bool {
        self.action == "updated"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_change_actions() {
        let added = DocumentChange {
            id: "Test/1".to_string(),
            action: "added".to_string(),
        };
        assert!(added.is_added());
        assert!(!added.is_deleted());
        assert!(!added.is_updated());

        let deleted = DocumentChange {
            id: "Test/2".to_string(),
            action: "deleted".to_string(),
        };
        assert!(!deleted.is_added());
        assert!(deleted.is_deleted());
        assert!(!deleted.is_updated());

        let updated = DocumentChange {
            id: "Test/3".to_string(),
            action: "updated".to_string(),
        };
        assert!(!updated.is_added());
        assert!(!updated.is_deleted());
        assert!(updated.is_updated());
    }
}

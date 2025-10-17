//! In-memory operation log for debugging recent client operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use terminusdb_woql2::prelude::Query;

/// Type of operation performed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Query,
    #[serde(rename = "graphql")]
    GraphQL,
    Insert,
    Update,
    Delete,
    CreateDatabase,
    DeleteDatabase,
    InsertSchema,
    UpdateSchema,
    GetDocument,
    GetInstance,
    Squash,
    Other(String),
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Query => write!(f, "query"),
            OperationType::GraphQL => write!(f, "graphql"),
            OperationType::Insert => write!(f, "insert"),
            OperationType::Update => write!(f, "update"),
            OperationType::Delete => write!(f, "delete"),
            OperationType::CreateDatabase => write!(f, "create_database"),
            OperationType::DeleteDatabase => write!(f, "delete_database"),
            OperationType::InsertSchema => write!(f, "insert_schema"),
            OperationType::UpdateSchema => write!(f, "update_schema"),
            OperationType::GetDocument => write!(f, "get_document"),
            OperationType::GetInstance => write!(f, "get_instance"),
            OperationType::Squash => write!(f, "squash"),
            OperationType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// A single operation entry in the log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    /// When the operation was executed
    pub timestamp: DateTime<Utc>,
    /// Type of operation
    pub operation_type: OperationType,
    /// The endpoint or method called
    pub endpoint: String,
    /// Database name (if applicable)
    pub database: Option<String>,
    /// Branch name (if applicable)
    pub branch: Option<String>,
    /// Whether the operation succeeded
    pub success: bool,
    /// Number of results (for queries)
    pub result_count: Option<usize>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if operation failed
    pub error: Option<String>,
    /// Additional context (e.g., query type)
    pub context: Option<String>,
    /// The WOQL query (for query operations)
    pub query: Option<Query>,
}

impl OperationEntry {
    /// Create a new operation entry
    pub fn new(operation_type: OperationType, endpoint: String) -> Self {
        Self {
            timestamp: Utc::now(),
            operation_type,
            endpoint,
            database: None,
            branch: None,
            success: false,
            result_count: None,
            duration_ms: 0,
            error: None,
            context: None,
            query: None,
        }
    }

    /// Set the database and branch context
    pub fn with_context(mut self, database: Option<String>, branch: Option<String>) -> Self {
        self.database = database;
        self.branch = branch;
        self
    }

    /// Mark the operation as successful
    pub fn success(mut self, result_count: Option<usize>, duration_ms: u64) -> Self {
        self.success = true;
        self.result_count = result_count;
        self.duration_ms = duration_ms;
        self
    }

    /// Mark the operation as failed
    pub fn failure(mut self, error: String, duration_ms: u64) -> Self {
        self.success = false;
        self.error = Some(error);
        self.duration_ms = duration_ms;
        self
    }

    /// Add additional context
    pub fn with_additional_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    /// Add a WOQL query
    pub fn with_query(mut self, query: Query) -> Self {
        self.query = Some(query);
        self
    }
}

/// Thread-safe ring buffer for storing recent operations
#[derive(Clone)]
pub struct OperationLog {
    entries: Arc<RwLock<VecDeque<OperationEntry>>>,
    max_size: usize,
}

impl OperationLog {
    /// Create a new operation log with the specified maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// Add an operation to the log
    pub fn push(&self, entry: OperationEntry) {
        if let Ok(mut entries) = self.entries.write() {
            // Remove oldest entry if at capacity
            if entries.len() >= self.max_size {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Get all entries in the log (newest last)
    pub fn get_all(&self) -> Vec<OperationEntry> {
        if let Ok(entries) = self.entries.read() {
            entries.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get the most recent N entries
    pub fn get_recent(&self, n: usize) -> Vec<OperationEntry> {
        if let Ok(entries) = self.entries.read() {
            entries
                .iter()
                .rev()
                .take(n)
                .rev()
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Get the current number of entries
    pub fn len(&self) -> usize {
        if let Ok(entries) = self.entries.read() {
            entries.len()
        } else {
            0
        }
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Update the maximum size of the log
    pub fn set_max_size(&mut self, new_size: usize) {
        self.max_size = new_size;
        if let Ok(mut entries) = self.entries.write() {
            // Trim entries if new size is smaller
            while entries.len() > new_size {
                entries.pop_front();
            }
        }
    }

    /// Get the last query operation
    pub fn get_last_query(&self) -> Option<Query> {
        if let Ok(entries) = self.entries.read() {
            entries.iter()
                .rev()
                .find(|entry| matches!(entry.operation_type, OperationType::Query))
                .and_then(|entry| entry.query.clone())
        } else {
            None
        }
    }
}

impl Default for OperationLog {
    fn default() -> Self {
        Self::new(50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_log_ring_buffer() {
        let mut log = OperationLog::new(3);

        // Add 4 entries to a log with max size 3
        for i in 0..4 {
            let entry = OperationEntry::new(
                OperationType::Query,
                format!("query_{}", i),
            );
            log.push(entry);
        }

        // Should only have 3 entries
        assert_eq!(log.len(), 3);

        // First entry should be query_1 (query_0 was removed)
        let entries = log.get_all();
        assert_eq!(entries[0].endpoint, "query_1");
        assert_eq!(entries[2].endpoint, "query_3");
    }

    #[test]
    fn test_operation_entry_builder() {
        let entry = OperationEntry::new(OperationType::Insert, "/api/db/test/document".to_string())
            .with_context(Some("test_db".to_string()), Some("main".to_string()))
            .success(Some(5), 123)
            .with_additional_context("bulk insert".to_string());

        assert_eq!(entry.operation_type, OperationType::Insert);
        assert_eq!(entry.database, Some("test_db".to_string()));
        assert_eq!(entry.branch, Some("main".to_string()));
        assert!(entry.success);
        assert_eq!(entry.result_count, Some(5));
        assert_eq!(entry.duration_ms, 123);
        assert_eq!(entry.context, Some("bulk insert".to_string()));
    }
}
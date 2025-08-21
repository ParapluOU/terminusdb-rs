//! Debugging and logging functionality for the TerminusDB client
//!
//! This module provides two main debugging features:
//! - An in-memory operation log (ring buffer) for recent operations
//! - A persistent query log file for audit trails

pub mod operation_log;
pub mod query_log;

pub use operation_log::{OperationEntry, OperationLog, OperationType};
pub use query_log::{QueryLogger, QueryLogEntry};

use std::path::Path;

/// Configuration for debugging features
#[derive(Debug, Clone)]
pub struct DebugConfig {
    /// Maximum number of operations to keep in memory
    pub operation_log_size: usize,
    /// Path to the query log file (None to disable)
    pub query_log_path: Option<String>,
    /// Whether to enable debug logging
    pub enabled: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            operation_log_size: 50,
            query_log_path: None,
            enabled: true,
        }
    }
}

impl DebugConfig {
    /// Create a new debug configuration with query logging enabled
    pub fn with_query_log<P: AsRef<Path>>(path: P) -> Self {
        Self {
            query_log_path: Some(path.as_ref().to_string_lossy().into_owned()),
            ..Default::default()
        }
    }
}
//! File-based query logging for persistent audit trails

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Filter options for operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFilter {
    /// Include all operations
    All,
    /// Include only query operations
    QueriesOnly,
    /// Include only non-query operations
    OperationsOnly,
}

/// Entry format for the query log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryLogEntry {
    /// ISO 8601 timestamp
    pub timestamp: DateTime<Utc>,
    /// Type of operation
    pub operation_type: String,
    /// Database name
    pub database: Option<String>,
    /// Branch name
    pub branch: Option<String>,
    /// Endpoint or method
    pub endpoint: String,
    /// Query details (WOQL as JSON or REST endpoint info)
    pub details: serde_json::Value,
    /// Whether the operation succeeded
    pub success: bool,
    /// Number of results (no actual data)
    pub result_count: Option<usize>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
}

impl QueryLogEntry {
    /// Create a new query log entry
    pub fn new(operation_type: String, endpoint: String, details: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            operation_type,
            database: None,
            branch: None,
            endpoint,
            details,
            success: false,
            result_count: None,
            duration_ms: 0,
            error: None,
        }
    }

    /// Convert to a JSON string for logging
    pub fn to_log_line(&self) -> String {
        match serde_json::to_string(self) {
            Ok(json) => json + "\n",
            Err(e) => {
                error!("Failed to serialize query log entry: {}", e);
                format!("{{\"error\": \"serialization failed: {}\"}}\n", e)
            }
        }
    }
}

/// Async file-based query logger
#[derive(Clone)]
pub struct QueryLogger {
    file_path: PathBuf,
    file_handle: Arc<Mutex<Option<File>>>,
    enabled: Arc<Mutex<bool>>,
}

impl QueryLogger {
    /// Create a new query logger
    pub async fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file_path = path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await?;

        Ok(Self {
            file_path,
            file_handle: Arc::new(Mutex::new(Some(file))),
            enabled: Arc::new(Mutex::new(true)),
        })
    }

    /// Write a log entry to the file
    pub async fn log(&self, entry: QueryLogEntry) {
        let enabled = *self.enabled.lock().await;
        if !enabled {
            return;
        }

        let log_line = entry.to_log_line();

        let mut file_guard = self.file_handle.lock().await;
        if let Some(file) = file_guard.as_mut() {
            if let Err(e) = file.write_all(log_line.as_bytes()).await {
                error!("Failed to write to query log: {}", e);
            }
            // Flush to ensure data is written
            if let Err(e) = file.flush().await {
                warn!("Failed to flush query log: {}", e);
            }
        }
    }

    /// Enable or disable logging
    pub async fn set_enabled(&self, enabled: bool) {
        *self.enabled.lock().await = enabled;
        debug!(
            "Query logging {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Check if logging is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.lock().await
    }

    /// Rotate the log file (close current, rename with timestamp, open new)
    pub async fn rotate(&self) -> anyhow::Result<()> {
        let mut file_guard = self.file_handle.lock().await;

        // Close current file
        file_guard.take();

        // Rename existing file with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let rotated_path = self.file_path.with_file_name(format!(
            "{}.{}",
            self.file_path.file_stem().unwrap().to_string_lossy(),
            timestamp
        ));

        if self.file_path.exists() {
            tokio::fs::rename(&self.file_path, rotated_path).await?;
        }

        // Open new file
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        *file_guard = Some(new_file);

        Ok(())
    }

    /// Get the current log file path
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    /// Retrieve slow entries from the log file
    ///
    /// # Arguments
    ///
    /// * `threshold` - Duration threshold for slow operations (default: 1 second)
    /// * `filter` - Filter by operation type (default: All)
    /// * `limit` - Maximum number of entries to return (default: unlimited)
    ///
    /// # Returns
    ///
    /// A vector of log entries sorted by duration (slowest first)
    pub async fn get_slow_entries(
        &self,
        threshold: Option<Duration>,
        filter: Option<OperationFilter>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<QueryLogEntry>> {
        let threshold_ms = threshold.unwrap_or(Duration::from_secs(1)).as_millis() as u64;

        let filter = filter.unwrap_or(OperationFilter::All);

        // Open the log file for reading
        let file = tokio::fs::File::open(&self.file_path).await?;
        let mut reader = BufReader::new(file);

        let mut entries = Vec::new();
        let mut line = String::new();

        // Read entries line by line
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse the JSON entry
            match serde_json::from_str::<QueryLogEntry>(trimmed) {
                Ok(entry) => {
                    // Check duration threshold
                    if entry.duration_ms >= threshold_ms {
                        // Apply operation filter
                        let include = match filter {
                            OperationFilter::All => true,
                            OperationFilter::QueriesOnly => entry.operation_type == "query",
                            OperationFilter::OperationsOnly => entry.operation_type != "query",
                        };

                        if include {
                            entries.push(entry);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse log entry: {}", e);
                    continue;
                }
            }
        }

        // Sort by duration (slowest first)
        entries.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));

        // Apply limit if specified
        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_query_logger() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let log_path = temp_dir.path().join("test_query.log");

        let logger = QueryLogger::new(&log_path).await?;

        // Create and log an entry
        let entry = QueryLogEntry {
            timestamp: Utc::now(),
            operation_type: "query".to_string(),
            database: Some("test_db".to_string()),
            branch: Some("main".to_string()),
            endpoint: "/api/db/test_db/query".to_string(),
            details: serde_json::json!({
                "query_type": "select",
                "variables": ["Subject", "Predicate", "Object"]
            }),
            success: true,
            result_count: Some(42),
            duration_ms: 123,
            error: None,
        };

        logger.log(entry).await;

        // Ensure file is written by dropping the logger
        drop(logger);

        // Read the log file
        let content = tokio::fs::read_to_string(&log_path).await?;
        assert!(!content.is_empty());

        // Parse the logged JSON
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let parsed: QueryLogEntry = serde_json::from_str(lines[0])?;
        assert_eq!(parsed.operation_type, "query");
        assert_eq!(parsed.result_count, Some(42));

        Ok(())
    }

    #[tokio::test]
    async fn test_query_logger_rotation() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let log_path = temp_dir.path().join("rotate_test.log");

        let logger = QueryLogger::new(&log_path).await?;

        // Write an entry
        let entry = QueryLogEntry::new(
            "test".to_string(),
            "/test".to_string(),
            serde_json::json!({"test": true}),
        );
        logger.log(entry).await;

        // Rotate the log
        logger.rotate().await?;

        // Original file should be renamed and new file created
        assert!(log_path.exists());

        // Check that we have a rotated file
        let dir_entries: Vec<_> = std::fs::read_dir(temp_dir.path())?
            .filter_map(Result::ok)
            .collect();

        assert!(dir_entries.len() >= 2); // Original + rotated

        Ok(())
    }

    #[tokio::test]
    async fn test_get_slow_entries() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let log_path = temp_dir.path().join("slow_test.log");

        let logger = QueryLogger::new(&log_path).await?;

        // Create entries with varying durations
        let entries = vec![
            QueryLogEntry {
                timestamp: Utc::now(),
                operation_type: "query".to_string(),
                database: Some("test_db".to_string()),
                branch: Some("main".to_string()),
                endpoint: "/api/db/test_db/query".to_string(),
                details: serde_json::json!({"query": "fast"}),
                success: true,
                result_count: Some(10),
                duration_ms: 50, // Fast query
                error: None,
            },
            QueryLogEntry {
                timestamp: Utc::now(),
                operation_type: "query".to_string(),
                database: Some("test_db".to_string()),
                branch: Some("main".to_string()),
                endpoint: "/api/db/test_db/query".to_string(),
                details: serde_json::json!({"query": "slow"}),
                success: true,
                result_count: Some(1000),
                duration_ms: 2500, // Slow query
                error: None,
            },
            QueryLogEntry {
                timestamp: Utc::now(),
                operation_type: "insert".to_string(),
                database: Some("test_db".to_string()),
                branch: Some("main".to_string()),
                endpoint: "/api/db/test_db/instance".to_string(),
                details: serde_json::json!({"entity_type": "Person"}),
                success: true,
                result_count: Some(1),
                duration_ms: 1500, // Slow insert
                error: None,
            },
            QueryLogEntry {
                timestamp: Utc::now(),
                operation_type: "query".to_string(),
                database: Some("test_db".to_string()),
                branch: Some("main".to_string()),
                endpoint: "/api/db/test_db/query".to_string(),
                details: serde_json::json!({"query": "medium"}),
                success: true,
                result_count: Some(100),
                duration_ms: 800, // Just under 1 second
                error: None,
            },
        ];

        // Log all entries
        for entry in &entries {
            logger.log(entry.clone()).await;
        }

        // Test 1: Get all slow entries with default threshold (1 second)
        let slow_entries = logger.get_slow_entries(None, None, None).await?;
        assert_eq!(slow_entries.len(), 2); // Should get the 2500ms and 1500ms entries
        assert_eq!(slow_entries[0].duration_ms, 2500); // Slowest first
        assert_eq!(slow_entries[1].duration_ms, 1500);

        // Test 2: Get slow entries with custom threshold (500ms)
        let slow_entries = logger
            .get_slow_entries(Some(Duration::from_millis(500)), None, None)
            .await?;
        assert_eq!(slow_entries.len(), 3); // Should get 2500ms, 1500ms, and 800ms

        // Test 3: Filter for queries only
        let slow_queries = logger
            .get_slow_entries(
                Some(Duration::from_millis(500)),
                Some(OperationFilter::QueriesOnly),
                None,
            )
            .await?;
        assert_eq!(slow_queries.len(), 2); // Should get only the slow queries
        assert!(slow_queries.iter().all(|e| e.operation_type == "query"));

        // Test 4: Filter for operations only (non-queries)
        let slow_ops = logger
            .get_slow_entries(
                Some(Duration::from_millis(500)),
                Some(OperationFilter::OperationsOnly),
                None,
            )
            .await?;
        assert_eq!(slow_ops.len(), 1); // Should get only the insert
        assert_eq!(slow_ops[0].operation_type, "insert");

        // Test 5: Apply limit
        let limited = logger
            .get_slow_entries(Some(Duration::from_millis(1)), None, Some(1))
            .await?;
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].duration_ms, 2500); // Should get the slowest one

        Ok(())
    }
}

//! File-based query logging for persistent audit trails

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::fs::{OpenOptions, File};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, warn};

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
    pub fn new(
        operation_type: String,
        endpoint: String,
        details: serde_json::Value,
    ) -> Self {
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
        debug!("Query logging {}", if enabled { "enabled" } else { "disabled" });
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
        let rotated_path = self.file_path.with_file_name(
            format!("{}.{}", 
                self.file_path.file_stem().unwrap().to_string_lossy(),
                timestamp
            )
        );
        
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
}
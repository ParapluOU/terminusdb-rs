//! SSE Changeset Event Types and Streaming
//!
//! This module provides types and functionality for consuming TerminusDB's
//! SSE changeset stream, which reports real-time changes to the database.

use anyhow::{anyhow, Context};
use futures_util::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

/// SSE event data from TerminusDB changeset plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// SSE stream connection to TerminusDB changeset endpoint
pub struct SseConnection {
    endpoint: String,
    user: String,
    pass: String,
    client: Client,
}

impl SseConnection {
    /// Create a new SSE connection
    pub fn new(endpoint: String, user: String, pass: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout for long-lived connections
            .build()
            .expect("Failed to create HTTP client");

        Self {
            endpoint,
            user,
            pass,
            client,
        }
    }

    /// Connect to the SSE stream and return an event iterator
    pub async fn connect(
        &self,
    ) -> anyhow::Result<impl futures_util::Stream<Item = Result<ChangesetEvent, anyhow::Error>>> {
        let url = format!("{}changesets/stream", self.endpoint);

        debug!("Connecting to TerminusDB SSE endpoint: {}", url);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Accept", "text/event-stream")
            .send()
            .await
            .context("Failed to connect to SSE endpoint")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "SSE connection failed with status: {}",
                response.status()
            ));
        }

        debug!("Successfully connected to TerminusDB SSE stream");

        Ok(Self::parse_stream(response))
    }

    /// Parse SSE byte stream into ChangesetEvent stream
    fn parse_stream(
        response: reqwest::Response,
    ) -> impl futures_util::Stream<Item = Result<ChangesetEvent, anyhow::Error>> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        async_stream::stream! {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes_chunk) => {
                        let text = String::from_utf8_lossy(&bytes_chunk);
                        buffer.push_str(&text);

                        // Process complete SSE events from buffer
                        while let Some((event, remaining)) = Self::extract_next_event(&buffer) {
                            buffer = remaining;

                            if let Some(data) = Self::parse_sse_event(&event) {
                                match serde_json::from_str::<ChangesetEvent>(&data) {
                                    Ok(changeset_event) => {
                                        yield Ok(changeset_event);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse changeset event: {}", e);
                                        debug!("Raw event data: {}", data);
                                        yield Err(anyhow!("Failed to parse changeset event: {}", e));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading SSE chunk: {}", e);
                        yield Err(anyhow!("Error reading SSE chunk: {}", e));
                    }
                }
            }
        }
    }

    /// Extract the next complete SSE event from the buffer
    fn extract_next_event(buffer: &str) -> Option<(String, String)> {
        // SSE events are separated by double newlines
        if let Some(end) = buffer.find("\n\n") {
            let event = buffer[..end].to_string();
            let remaining = buffer[end + 2..].to_string();
            Some((event, remaining))
        } else {
            None
        }
    }

    /// Parse SSE event and extract data field
    fn parse_sse_event(event: &str) -> Option<String> {
        let mut event_type = None;
        let mut data = None;

        for line in event.lines() {
            if line.starts_with("event: ") {
                event_type = Some(line[7..].trim());
            } else if line.starts_with("data: ") {
                data = Some(line[6..].trim());
            } else if line.starts_with(": ") {
                // This is a comment line (heartbeat), ignore
                continue;
            }
        }

        // We're only interested in changeset events
        if event_type == Some("changeset") {
            data.map(|d| d.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_event() {
        let event = "event: changeset\ndata: {\"resource\":\"admin/test\"}";
        let data = SseConnection::parse_sse_event(event);
        assert_eq!(data, Some("{\"resource\":\"admin/test\"}".to_string()));
    }

    #[test]
    fn test_extract_next_event() {
        let buffer = "event: changeset\ndata: {}\n\nevent: other";
        let (event, remaining) = SseConnection::extract_next_event(buffer).unwrap();
        assert_eq!(event, "event: changeset\ndata: {}");
        assert_eq!(remaining, "event: other");
    }

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

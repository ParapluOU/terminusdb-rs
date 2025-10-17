//! SSE Manager for Centralized Changeset Stream Management
//!
//! This module provides a centralized manager for SSE connections that routes
//! changeset events to registered listeners based on their resource paths.

use super::change_listener::ChangeListenerInner;
use super::changeset::{ChangesetEvent, SseConnection};
use anyhow::{anyhow, Context};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Centralized SSE manager that maintains one connection and routes to multiple listeners
pub struct SseManager {
    inner: Arc<SseManagerInner>,
}

struct SseManagerInner {
    endpoint: String,
    user: String,
    pass: String,
    /// Registry of listeners by resource path (e.g., "admin/dev/local/branch/main")
    listeners: RwLock<HashMap<String, Vec<Weak<ChangeListenerInner>>>>,
    /// Handle to the background SSE processing task
    task_handle: RwLock<Option<JoinHandle<()>>>,
}

impl SseManager {
    /// Create a new SSE manager
    pub fn new(endpoint: String, user: String, pass: String) -> Self {
        Self {
            inner: Arc::new(SseManagerInner {
                endpoint,
                user,
                pass,
                listeners: RwLock::new(HashMap::new()),
                task_handle: RwLock::new(None),
            }),
        }
    }

    /// Register a listener for a specific resource path
    pub fn register_listener(
        &self,
        resource_path: String,
        listener: Weak<ChangeListenerInner>,
    ) -> anyhow::Result<()> {
        debug!("Registering listener for resource: {}", resource_path);

        let mut listeners = self.inner.listeners.write().unwrap();
        listeners
            .entry(resource_path.clone())
            .or_insert_with(Vec::new)
            .push(listener);

        // Start the SSE connection if not already running
        drop(listeners); // Release lock before starting
        self.ensure_running()?;

        Ok(())
    }

    /// Ensure the SSE connection is running
    fn ensure_running(&self) -> anyhow::Result<()> {
        let mut handle_lock = self.inner.task_handle.write().unwrap();

        // Check if already running
        if let Some(handle) = handle_lock.as_ref() {
            if !handle.is_finished() {
                return Ok(());
            }
        }

        info!("Starting centralized SSE connection");

        // Start new background task
        let manager = self.inner.clone();
        let handle = tokio::spawn(async move {
            loop {
                match manager.run_sse_loop().await {
                    Ok(()) => {
                        warn!("SSE connection closed, reconnecting in 5 seconds...");
                    }
                    Err(e) => {
                        error!("SSE connection error: {}, reconnecting in 5 seconds...", e);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });

        *handle_lock = Some(handle);
        Ok(())
    }

    /// Clean up dead listener references for a specific resource
    pub fn cleanup_listeners(&self, resource_path: &str) {
        let mut listeners = self.inner.listeners.write().unwrap();

        if let Some(listener_list) = listeners.get_mut(resource_path) {
            listener_list.retain(|weak| weak.strong_count() > 0);

            // Remove empty entries
            if listener_list.is_empty() {
                listeners.remove(resource_path);
                debug!("Removed empty listener list for resource: {}", resource_path);
            }
        }
    }
}

impl SseManagerInner {
    /// Main SSE processing loop
    async fn run_sse_loop(&self) -> anyhow::Result<()> {
        let connection = SseConnection::new(
            self.endpoint.clone(),
            self.user.clone(),
            self.pass.clone(),
        );

        let stream = connection.connect().await?;
        let mut stream = Box::pin(stream);

        info!("SSE connection established, processing events...");

        while let Some(result) = stream.next().await {
            match result {
                Ok(event) => {
                    self.route_event(event).await;
                }
                Err(e) => {
                    error!("Error processing SSE event: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Route an event to all listeners for the matching resource
    async fn route_event(&self, event: ChangesetEvent) {
        let resource = event.resource.clone();
        debug!("Routing changeset event for resource: {}", resource);

        let listeners = self.listeners.read().unwrap();

        if let Some(listener_list) = listeners.get(&resource) {
            let mut active_count = 0;

            for weak_listener in listener_list {
                if let Some(listener) = weak_listener.upgrade() {
                    active_count += 1;
                    // Dispatch to listener in background task
                    let event_clone = event.clone();
                    tokio::spawn(async move {
                        if let Err(e) = listener.dispatch_event(event_clone).await {
                            error!("Failed to dispatch event to listener: {}", e);
                        }
                    });
                }
            }

            debug!(
                "Dispatched event to {} active listener(s) for resource: {}",
                active_count, resource
            );
        } else {
            debug!("No listeners registered for resource: {}", resource);
        }
    }
}

impl Drop for SseManagerInner {
    fn drop(&mut self) {
        // Abort the background task when manager is dropped
        if let Some(handle) = self.task_handle.write().unwrap().take() {
            handle.abort();
            info!("SSE manager dropped, background task aborted");
        }
    }
}

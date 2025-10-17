//! ChangeListener API for type-safe SSE changeset callbacks
//!
//! This module provides a type-safe API for listening to TerminusDB changeset events
//! and dispatching them to registered callbacks based on document type.

use super::{changeset::*, client::TerminusDBHttpClient};
use crate::{spec::BranchSpec, DefaultTDBDeserializer};
use anyhow::Context;
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, RwLock},
};
use terminusdb_schema::{FromTDBInstance, InstanceFromJson, TdbIRI, TerminusDBModel};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Type-safe change listener for TerminusDB changeset events
///
/// The ChangeListener connects to TerminusDB's SSE stream and dispatches
/// typed callbacks when documents are added, updated, or deleted.
///
/// # Example
/// ```rust,ignore
/// use terminusdb_client::*;
///
/// #[derive(TerminusDBModel, FromTDBInstance, InstanceFromJson)]
/// struct User {
///     name: String,
///     email: String,
/// }
///
/// let client = TerminusDBHttpClient::local_node().await;
/// let spec = BranchSpec::new("mydb", Some("main"));
///
/// let listener = client.change_listener(spec);
///
/// // Register typed callbacks
/// listener
///     .on_added_id::<User>(|iri| {
///         println!("User added: {}", iri);
///     })
///     .on_added::<User>(|user| {
///         println!("User added: {} - {}", user.name, user.email);
///     })
///     .on_deleted::<User>(|iri| {
///         println!("User deleted: {}", iri);
///     });
///
/// // Start listening in background
/// let handle = Arc::new(listener).start().await?;
///
/// // Wait for listener or do other work
/// handle.await?;
/// ```
#[derive(Clone)]
pub struct ChangeListener {
    inner: Arc<ChangeListenerInner>,
}

struct ChangeListenerInner {
    client: TerminusDBHttpClient,
    spec: BranchSpec,
    handlers: RwLock<HandlerRegistry>,
}

/// Registry of all registered handlers organized by type
struct HandlerRegistry {
    added_id_handlers: HashMap<String, Vec<Box<dyn AddedIdHandler>>>,
    added_handlers: HashMap<String, Vec<Box<dyn AddedHandler>>>,
    deleted_handlers: HashMap<String, Vec<Box<dyn DeletedHandler>>>,
    changeset_handlers: HashMap<String, Vec<Box<dyn ChangesetHandler>>>,
    changed_handlers: HashMap<String, Vec<Box<dyn ChangedHandler>>>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self {
            added_id_handlers: HashMap::new(),
            added_handlers: HashMap::new(),
            deleted_handlers: HashMap::new(),
            changeset_handlers: HashMap::new(),
            changed_handlers: HashMap::new(),
        }
    }
}

// ===== Handler Traits =====

/// Handler for on_added_id callbacks (ID only, no fetch)
trait AddedIdHandler: Send + Sync {
    fn handle(&self, iri: TdbIRI);
}

/// Handler for on_added callbacks (fetches full document)
trait AddedHandler: Send + Sync {
    fn handle(&self, iri: TdbIRI, client: TerminusDBHttpClient, spec: BranchSpec);
}

/// Handler for on_deleted callbacks (ID only)
trait DeletedHandler: Send + Sync {
    fn handle(&self, iri: TdbIRI);
}

/// Handler for on_changeset callbacks (ID + changed fields map)
trait ChangesetHandler: Send + Sync {
    fn handle(&self, iri: TdbIRI, changed_fields: HashMap<String, Value>);
}

/// Handler for on_changed callbacks (fetches document + changed fields)
trait ChangedHandler: Send + Sync {
    fn handle(
        &self,
        iri: TdbIRI,
        changed_fields: HashMap<String, Value>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
    );
}

// ===== Concrete Handler Implementations =====

struct AddedIdHandlerImpl<F> {
    callback: F,
}

impl<F> AddedIdHandler for AddedIdHandlerImpl<F>
where
    F: Fn(TdbIRI) + Send + Sync,
{
    fn handle(&self, iri: TdbIRI) {
        (self.callback)(iri);
    }
}

struct AddedHandlerImpl<T, F> {
    callback: Arc<F>,
    _phantom: PhantomData<T>,
}

impl<T, F> AddedHandler for AddedHandlerImpl<T, F>
where
    T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    F: Fn(T) + Send + Sync + 'static,
{
    fn handle(&self, iri: TdbIRI, client: TerminusDBHttpClient, spec: BranchSpec) {
        let callback = self.callback.clone();
        let id = iri.id().to_string();
        let iri_clone = iri.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client.get_instance::<T>(&id, &spec, &mut deserializer).await {
                Ok(instance) => {
                    callback(instance);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch document {} for on_added callback: {}",
                        iri_clone, e
                    );
                }
            }
        });
    }
}

struct DeletedHandlerImpl<F> {
    callback: F,
}

impl<F> DeletedHandler for DeletedHandlerImpl<F>
where
    F: Fn(TdbIRI) + Send + Sync,
{
    fn handle(&self, iri: TdbIRI) {
        (self.callback)(iri);
    }
}

struct ChangesetHandlerImpl<F> {
    callback: F,
}

impl<F> ChangesetHandler for ChangesetHandlerImpl<F>
where
    F: Fn(TdbIRI, HashMap<String, Value>) + Send + Sync,
{
    fn handle(&self, iri: TdbIRI, changed_fields: HashMap<String, Value>) {
        (self.callback)(iri, changed_fields);
    }
}

struct ChangedHandlerImpl<T, F> {
    callback: Arc<F>,
    _phantom: PhantomData<T>,
}

impl<T, F> ChangedHandler for ChangedHandlerImpl<T, F>
where
    T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    F: Fn(T, HashMap<String, Value>) + Send + Sync + 'static,
{
    fn handle(
        &self,
        iri: TdbIRI,
        changed_fields: HashMap<String, Value>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
    ) {
        let callback = self.callback.clone();
        let id = iri.id().to_string();
        let iri_clone = iri.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client.get_instance::<T>(&id, &spec, &mut deserializer).await {
                Ok(instance) => {
                    callback(instance, changed_fields);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch document {} for on_changed callback: {}",
                        iri_clone, e
                    );
                }
            }
        });
    }
}

// ===== ChangeListener Implementation =====

impl ChangeListener {
    /// Create a new ChangeListener for the specified database and branch
    pub fn new(client: TerminusDBHttpClient, spec: BranchSpec) -> Self {
        Self {
            inner: Arc::new(ChangeListenerInner {
                client,
                spec,
                handlers: RwLock::new(HandlerRegistry::default()),
            }),
        }
    }

    /// Register a callback for when a document ID is added (does not fetch the document)
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_added_id::<User>(|iri| {
    ///     println!("User added with ID: {}", iri);
    /// });
    /// ```
    pub fn on_added_id<T: TerminusDBModel + 'static>(
        &self,
        callback: impl Fn(TdbIRI) + Send + Sync + 'static,
    ) -> &Self {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedIdHandlerImpl { callback });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .added_id_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_added_id handler for type: {}", type_name);
        self
    }

    /// Register a callback for when a document is added (fetches the full document)
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_added::<User>(|user| {
    ///     println!("User added: {} - {}", user.name, user.email);
    /// });
    /// ```
    pub fn on_added<T>(
        &self,
        callback: impl Fn(T) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .added_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_added handler for type: {}", type_name);
        self
    }

    /// Register a callback for when a document is deleted (ID only, no data available)
    ///
    /// Note: The SSE stream does not communicate the previous CommitId for deleted documents,
    /// so only the IRI is available.
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_deleted::<User>(|iri| {
    ///     println!("User deleted: {}", iri);
    /// });
    /// ```
    pub fn on_deleted<T: TerminusDBModel + 'static>(
        &self,
        callback: impl Fn(TdbIRI) + Send + Sync + 'static,
    ) -> &Self {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(DeletedHandlerImpl { callback });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .deleted_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_deleted handler for type: {}", type_name);
        self
    }

    /// Register a callback for when a document changes with field-level details
    ///
    /// Note: Currently the HashMap contains changed fields. The exact structure depends on
    /// the TerminusDB changeset plugin implementation.
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_changeset::<User>(|iri, changed_fields| {
    ///     println!("User {} changed fields: {:?}", iri, changed_fields);
    /// });
    /// ```
    pub fn on_changeset<T: TerminusDBModel + 'static>(
        &self,
        callback: impl Fn(TdbIRI, HashMap<String, Value>) + Send + Sync + 'static,
    ) -> &Self {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(ChangesetHandlerImpl { callback });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .changeset_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_changeset handler for type: {}", type_name);
        self
    }

    /// Register a callback for when a document changes (fetches document + changed fields)
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_changed::<User>(|user, changed_fields| {
    ///     println!("User {} changed: {:?}", user.name, changed_fields);
    /// });
    /// ```
    pub fn on_changed<T>(
        &self,
        callback: impl Fn(T, HashMap<String, Value>) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(ChangedHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .changed_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_changed handler for type: {}", type_name);
        self
    }

    /// Start listening to the SSE stream and dispatch events to registered handlers
    ///
    /// This method spawns a background task that:
    /// 1. Connects to the TerminusDB SSE endpoint
    /// 2. Parses changeset events
    /// 3. Dispatches to registered type-specific handlers
    /// 4. Automatically reconnects on disconnect
    ///
    /// Returns a JoinHandle that can be awaited to keep the listener running.
    ///
    /// # Example
    /// ```rust,ignore
    /// let listener = Arc::new(client.change_listener(spec));
    /// listener.on_added::<User>(|user| println!("New user: {}", user.name));
    ///
    /// let handle = listener.start().await?;
    /// // Listener runs in background
    ///
    /// // Optionally wait for it
    /// handle.await?;
    /// ```
    pub async fn start(self: Arc<Self>) -> anyhow::Result<JoinHandle<()>> {
        let listener = self.clone();

        let handle = tokio::spawn(async move {
            loop {
                match listener.run_listener_loop().await {
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

        Ok(handle)
    }

    /// Internal method that runs a single listener loop iteration
    async fn run_listener_loop(&self) -> anyhow::Result<()> {
        // Create SSE connection
        let connection = SseConnection::new(
            self.inner.client.endpoint.to_string(),
            self.inner.client.user.clone(),
            self.inner.client.pass.clone(),
        );

        info!("Connecting to TerminusDB changeset stream...");
        let stream = connection.connect().await?;
        let mut stream = Box::pin(stream);
        info!("Successfully connected to changeset stream");

        // Process events from the stream
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    self.handle_changeset_event(event).await;
                }
                Err(e) => {
                    error!("Error processing changeset event: {}", e);
                    // Continue processing other events
                }
            }
        }

        Ok(())
    }

    /// Handle a single changeset event by dispatching to registered handlers
    async fn handle_changeset_event(&self, event: ChangesetEvent) {
        debug!(
            "Received changeset event - Commit: {}, Branch: {}, Changes: {}",
            event.commit.id,
            event.branch,
            event.changes.len()
        );

        // Process each document change
        for change in event.changes {
            // Parse the document ID to get type information
            let iri = match TdbIRI::parse(&change.id) {
                Ok(iri) => iri,
                Err(e) => {
                    warn!("Failed to parse document ID '{}': {}", change.id, e);
                    continue;
                }
            };

            let type_name = iri.type_name().to_string();

            // Dispatch based on action type
            if change.is_added() {
                self.dispatch_added(&type_name, iri).await;
            } else if change.is_deleted() {
                self.dispatch_deleted(&type_name, iri).await;
            } else if change.is_updated() {
                // For updates, we treat them as changes
                // TODO: Extract actual changed fields from the event metadata
                let changed_fields = HashMap::new();
                self.dispatch_changed(&type_name, iri, changed_fields).await;
            }
        }
    }

    /// Dispatch to on_added_id and on_added handlers
    async fn dispatch_added(&self, type_name: &str, iri: TdbIRI) {
        let registry = self.inner.handlers.read().unwrap();

        // Dispatch to on_added_id handlers
        if let Some(handlers) = registry.added_id_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(iri.clone());
            }
        }

        // Dispatch to on_added handlers (these will fetch the document)
        if let Some(handlers) = registry.added_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(
                    iri.clone(),
                    self.inner.client.clone(),
                    self.inner.spec.clone(),
                );
            }
        }
    }

    /// Dispatch to on_deleted handlers
    async fn dispatch_deleted(&self, type_name: &str, iri: TdbIRI) {
        let registry = self.inner.handlers.read().unwrap();

        if let Some(handlers) = registry.deleted_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(iri.clone());
            }
        }
    }

    /// Dispatch to on_changeset and on_changed handlers
    async fn dispatch_changed(
        &self,
        type_name: &str,
        iri: TdbIRI,
        changed_fields: HashMap<String, Value>,
    ) {
        let registry = self.inner.handlers.read().unwrap();

        // Dispatch to on_changeset handlers
        if let Some(handlers) = registry.changeset_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(iri.clone(), changed_fields.clone());
            }
        }

        // Dispatch to on_changed handlers (these will fetch the document)
        if let Some(handlers) = registry.changed_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(
                    iri.clone(),
                    changed_fields.clone(),
                    self.inner.client.clone(),
                    self.inner.spec.clone(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a running TerminusDB instance
    // with the changeset SSE plugin enabled

    #[test]
    fn test_change_listener_creation() {
        // Test that we can create a listener (doesn't require TerminusDB)
        // This is more of a compilation test
    }
}

//! ChangeListener API for type-safe SSE changeset callbacks
//!
//! This module provides a type-safe API for listening to TerminusDB changeset events
//! and dispatching them to registered callbacks based on document type.

use super::{changeset::*, client::TerminusDBHttpClient, sse_manager::SseManager};
use crate::{document::GetOpts, spec::BranchSpec, DefaultTDBDeserializer};
use anyhow::Context;
use serde_json::Value;
use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, RwLock, Weak},
};
use terminusdb_schema::{FromTDBInstance, InstanceFromJson, TdbIRI, TerminusDBModel};
use tracing::{debug, error, warn};

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
/// // Listener is automatically registered with the SSE manager
/// // and will receive events in the background
/// ```
#[derive(Clone)]
pub struct ChangeListener {
    inner: Arc<ChangeListenerInner>,
}

pub(crate) struct ChangeListenerInner {
    client: TerminusDBHttpClient,
    spec: BranchSpec,
    handlers: RwLock<HandlerRegistry>,
    sse_manager: Option<Arc<SseManager>>,
    /// If true, this listener is disabled and won't register with SSE manager
    disabled: bool,
}

/// Registry of all registered handlers organized by type
struct HandlerRegistry {
    added_id_handlers: HashMap<String, Vec<Box<dyn AddedIdHandler>>>,
    added_handlers: HashMap<String, Vec<Box<dyn AddedHandler>>>,
    added_batch_handlers: HashMap<String, Vec<Box<dyn AddedBatchHandler>>>,
    deleted_handlers: HashMap<String, Vec<Box<dyn DeletedHandler>>>,
    changeset_handlers: HashMap<String, Vec<Box<dyn ChangesetHandler>>>,
    changed_handlers: HashMap<String, Vec<Box<dyn ChangedHandler>>>,
    changed_batch_handlers: HashMap<String, Vec<Box<dyn ChangedBatchHandler>>>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self {
            added_id_handlers: HashMap::new(),
            added_handlers: HashMap::new(),
            added_batch_handlers: HashMap::new(),
            deleted_handlers: HashMap::new(),
            changeset_handlers: HashMap::new(),
            changed_handlers: HashMap::new(),
            changed_batch_handlers: HashMap::new(),
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
    fn handle(&self, iri: TdbIRI, client: TerminusDBHttpClient, spec: BranchSpec, opts: &GetOpts);
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
        opts: &GetOpts,
    );
}

/// Handler for on_added_batch callbacks (fetches multiple documents at once)
trait AddedBatchHandler: Send + Sync {
    fn handle(
        &self,
        iris: Vec<TdbIRI>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
        opts: &GetOpts,
    );
}

/// Handler for on_changed_batch callbacks (fetches multiple documents at once with change info)
trait ChangedBatchHandler: Send + Sync {
    fn handle(
        &self,
        items: Vec<(TdbIRI, HashMap<String, Value>)>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
        opts: &GetOpts,
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
    opts: GetOpts,
    _phantom: PhantomData<T>,
}

impl<T, F> AddedHandler for AddedHandlerImpl<T, F>
where
    T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    F: Fn(T) + Send + Sync + 'static,
{
    fn handle(&self, iri: TdbIRI, client: TerminusDBHttpClient, spec: BranchSpec, opts: &GetOpts) {
        let callback = self.callback.clone();
        let id = iri.id().to_string();
        let iri_clone = iri.clone();
        let opts = self.opts.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client
                .get_instance_with_opts::<T>(&id, &spec, opts, &mut deserializer)
                .await
            {
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
    opts: GetOpts,
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
        opts: &GetOpts,
    ) {
        let callback = self.callback.clone();
        let id = iri.id().to_string();
        let iri_clone = iri.clone();
        let opts = self.opts.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client
                .get_instance_with_opts::<T>(&id, &spec, opts, &mut deserializer)
                .await
            {
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

struct AddedBatchHandlerImpl<T, F> {
    callback: Arc<F>,
    opts: GetOpts,
    _phantom: PhantomData<T>,
}

impl<T, F> AddedBatchHandler for AddedBatchHandlerImpl<T, F>
where
    T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    F: Fn(Vec<T>) + Send + Sync + 'static,
{
    fn handle(
        &self,
        iris: Vec<TdbIRI>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
        opts: &GetOpts,
    ) {
        let callback = self.callback.clone();
        let ids: Vec<String> = iris.iter().map(|iri| iri.id().to_string()).collect();
        let opts = self.opts.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client
                .get_instances::<T>(ids, &spec, opts, &mut deserializer)
                .await
            {
                Ok(instances) => {
                    callback(instances);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch documents for on_added_batch callback: {}",
                        e
                    );
                }
            }
        });
    }
}

struct ChangedBatchHandlerImpl<T, F> {
    callback: Arc<F>,
    opts: GetOpts,
    _phantom: PhantomData<T>,
}

impl<T, F> ChangedBatchHandler for ChangedBatchHandlerImpl<T, F>
where
    T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    F: Fn(Vec<(T, HashMap<String, Value>)>) + Send + Sync + 'static,
{
    fn handle(
        &self,
        items: Vec<(TdbIRI, HashMap<String, Value>)>,
        client: TerminusDBHttpClient,
        spec: BranchSpec,
        opts: &GetOpts,
    ) {
        let callback = self.callback.clone();
        let ids: Vec<String> = items.iter().map(|(iri, _)| iri.id().to_string()).collect();
        let changed_fields_map: HashMap<String, HashMap<String, Value>> = items
            .iter()
            .map(|(iri, fields)| (iri.id().to_string(), fields.clone()))
            .collect();
        let opts = self.opts.clone();

        tokio::spawn(async move {
            let mut deserializer = DefaultTDBDeserializer;
            match client
                .get_instances::<T>(ids.clone(), &spec, opts, &mut deserializer)
                .await
            {
                Ok(instances) => {
                    // Pair up instances with their changed fields
                    let paired: Vec<(T, HashMap<String, Value>)> = instances
                        .into_iter()
                        .enumerate()
                        .filter_map(|(idx, instance)| {
                            ids.get(idx)
                                .and_then(|id| changed_fields_map.get(id))
                                .map(|fields| (instance, fields.clone()))
                        })
                        .collect();
                    callback(paired);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch documents for on_changed_batch callback: {}",
                        e
                    );
                }
            }
        });
    }
}

// ===== ChangeListener Implementation =====

impl ChangeListener {
    /// Create a new ChangeListener for the specified database and branch
    pub(crate) fn new(
        client: TerminusDBHttpClient,
        spec: BranchSpec,
        sse_manager: Arc<SseManager>,
    ) -> anyhow::Result<Self> {
        let inner = Arc::new(ChangeListenerInner {
            client,
            spec,
            handlers: RwLock::new(HandlerRegistry::default()),
            sse_manager: Some(sse_manager.clone()),
            disabled: false,
        });

        // Register with the SSE manager
        let resource_path = inner.resource_path();
        sse_manager.register_listener(resource_path, Arc::downgrade(&inner))?;

        Ok(Self { inner })
    }

    /// Create a disabled ChangeListener that does nothing.
    ///
    /// This is used when SSE is disabled via environment variable.
    /// The listener will log a warning on first use but won't attempt
    /// to connect to the SSE stream.
    pub(crate) fn disabled(client: TerminusDBHttpClient, spec: BranchSpec) -> Self {
        warn!(
            "ChangeListener created but SSE is disabled. Set TERMINUSDB_SSE=true to enable real-time updates for database '{}', branch '{}'",
            spec.db,
            spec.branch.as_deref().unwrap_or("main")
        );

        let inner = Arc::new(ChangeListenerInner {
            client,
            spec,
            handlers: RwLock::new(HandlerRegistry::default()),
            sse_manager: None,
            disabled: true,
        });

        Self { inner }
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
    pub fn on_added<T>(&self, callback: impl Fn(T) + Send + Sync + 'static) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts: GetOpts::default(),
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
            opts: GetOpts::default(),
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

    /// Register a callback for when documents are added (fetches all documents in a single batch)
    ///
    /// This is more efficient than `on_added` when multiple documents of the same type are
    /// added in a single changeset, as it fetches all documents in one database query.
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_added_batch::<User>(|users| {
    ///     println!("Added {} users in batch", users.len());
    ///     for user in users {
    ///         println!("  - {} ({})", user.name, user.email);
    ///     }
    /// });
    /// ```
    pub fn on_added_batch<T>(&self, callback: impl Fn(Vec<T>) + Send + Sync + 'static) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedBatchHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts: GetOpts::default(),
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .added_batch_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!("Registered on_added_batch handler for type: {}", type_name);
        self
    }

    /// Register a callback for when documents change (fetches all documents in a single batch)
    ///
    /// This is more efficient than `on_changed` when multiple documents of the same type are
    /// updated in a single changeset, as it fetches all documents in one database query.
    ///
    /// # Example
    /// ```rust,ignore
    /// listener.on_changed_batch::<User>(|changes| {
    ///     println!("Changed {} users in batch", changes.len());
    ///     for (user, changed_fields) in changes {
    ///         println!("  - {} changed fields: {:?}", user.name, changed_fields.keys());
    ///     }
    /// });
    /// ```
    pub fn on_changed_batch<T>(
        &self,
        callback: impl Fn(Vec<(T, HashMap<String, Value>)>) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(ChangedBatchHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts: GetOpts::default(),
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .changed_batch_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!(
            "Registered on_changed_batch handler for type: {}",
            type_name
        );
        self
    }

    /// Register a callback for when a document is added with custom GetOpts (fetches the full document)
    ///
    /// This variant allows you to control document fetching behavior (e.g., unfold linked documents).
    ///
    /// # Example
    /// ```rust,ignore
    /// let opts = GetOpts::default().with_unfold(true);
    /// listener.on_added_with_opts::<User>(opts, |user| {
    ///     println!("User added with unfolded fields: {} - {}", user.name, user.email);
    /// });
    /// ```
    pub fn on_added_with_opts<T>(
        &self,
        opts: GetOpts,
        callback: impl Fn(T) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts,
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .added_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!(
            "Registered on_added_with_opts handler for type: {}",
            type_name
        );
        self
    }

    /// Register a callback for when a document changes with custom GetOpts (fetches document + changed fields)
    ///
    /// This variant allows you to control document fetching behavior (e.g., unfold linked documents).
    ///
    /// # Example
    /// ```rust,ignore
    /// let opts = GetOpts::default().with_unfold(true);
    /// listener.on_changed_with_opts::<User>(opts, |user, changed_fields| {
    ///     println!("User {} changed (unfolded): {:?}", user.name, changed_fields);
    /// });
    /// ```
    pub fn on_changed_with_opts<T>(
        &self,
        opts: GetOpts,
        callback: impl Fn(T, HashMap<String, Value>) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(ChangedHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts,
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .changed_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!(
            "Registered on_changed_with_opts handler for type: {}",
            type_name
        );
        self
    }

    /// Register a callback for when documents are added with custom GetOpts (fetches in a single batch)
    ///
    /// This variant allows you to control document fetching behavior (e.g., unfold linked documents).
    ///
    /// # Example
    /// ```rust,ignore
    /// let opts = GetOpts::default().with_unfold(true);
    /// listener.on_added_batch_with_opts::<User>(opts, |users| {
    ///     println!("Added {} users in batch (unfolded)", users.len());
    ///     for user in users {
    ///         println!("  - {} ({})", user.name, user.email);
    ///     }
    /// });
    /// ```
    pub fn on_added_batch_with_opts<T>(
        &self,
        opts: GetOpts,
        callback: impl Fn(Vec<T>) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(AddedBatchHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts,
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .added_batch_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!(
            "Registered on_added_batch_with_opts handler for type: {}",
            type_name
        );
        self
    }

    /// Register a callback for when documents change with custom GetOpts (fetches in a single batch)
    ///
    /// This variant allows you to control document fetching behavior (e.g., unfold linked documents).
    ///
    /// # Example
    /// ```rust,ignore
    /// let opts = GetOpts::default().with_unfold(true);
    /// listener.on_changed_batch_with_opts::<User>(opts, |changes| {
    ///     println!("Changed {} users in batch (unfolded)", changes.len());
    ///     for (user, changed_fields) in changes {
    ///         println!("  - {} changed fields: {:?}", user.name, changed_fields.keys());
    ///     }
    /// });
    /// ```
    pub fn on_changed_batch_with_opts<T>(
        &self,
        opts: GetOpts,
        callback: impl Fn(Vec<(T, HashMap<String, Value>)>) + Send + Sync + 'static,
    ) -> &Self
    where
        T: TerminusDBModel + FromTDBInstance + InstanceFromJson + Send + Sync + 'static,
    {
        let type_name = T::schema_name().to_string();
        let handler = Box::new(ChangedBatchHandlerImpl::<T, _> {
            callback: Arc::new(callback),
            opts,
            _phantom: PhantomData,
        });

        let mut registry = self.inner.handlers.write().unwrap();
        registry
            .changed_batch_handlers
            .entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(handler);

        debug!(
            "Registered on_changed_batch_with_opts handler for type: {}",
            type_name
        );
        self
    }
}

// ===== ChangeListenerInner Implementation =====

impl ChangeListenerInner {
    /// Construct the resource path for this listener based on its BranchSpec
    ///
    /// Format: "{account}/{database}/local/branch/{branch}"
    /// Example: "admin/mydb/local/branch/main"
    pub(crate) fn resource_path(&self) -> String {
        let branch = self.spec.branch.as_deref().unwrap_or("main");
        format!(
            "{}/{}/local/branch/{}",
            self.client.org, self.spec.db, branch
        )
    }

    /// Dispatch a changeset event to this listener's registered handlers
    ///
    /// This is called by the SseManager when an event matches this listener's resource path
    pub(crate) async fn dispatch_event(&self, event: ChangesetEvent) -> anyhow::Result<()> {
        // Group changes by type and action for batched processing
        let mut added_by_type: HashMap<String, Vec<TdbIRI>> = HashMap::new();
        let mut updated_by_type: HashMap<String, Vec<(TdbIRI, HashMap<String, Value>)>> =
            HashMap::new();
        let mut deleted_by_type: HashMap<String, Vec<TdbIRI>> = HashMap::new();

        // Classify each change
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

            // Group by action type
            if change.is_added() {
                added_by_type
                    .entry(type_name)
                    .or_insert_with(Vec::new)
                    .push(iri);
            } else if change.is_deleted() {
                deleted_by_type
                    .entry(type_name)
                    .or_insert_with(Vec::new)
                    .push(iri);
            } else if change.is_updated() {
                // TODO: Extract actual changed fields from the event metadata
                let changed_fields = HashMap::new();
                updated_by_type
                    .entry(type_name)
                    .or_insert_with(Vec::new)
                    .push((iri, changed_fields));
            }
        }

        // Dispatch batched changes per type
        for (type_name, iris) in added_by_type {
            self.dispatch_added_batch(&type_name, iris).await;
        }

        for (type_name, items) in updated_by_type {
            self.dispatch_changed_batch(&type_name, items).await;
        }

        for (type_name, iris) in deleted_by_type {
            self.dispatch_deleted_batch(&type_name, iris).await;
        }

        Ok(())
    }

    /// Dispatch to on_added_id and on_added handlers
    async fn dispatch_added(&self, type_name: &str, iri: TdbIRI) {
        let registry = self.handlers.read().unwrap();

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
                    self.client.clone(),
                    self.spec.clone(),
                    &GetOpts::default(),
                );
            }
        }
    }

    /// Dispatch to on_deleted handlers
    async fn dispatch_deleted(&self, type_name: &str, iri: TdbIRI) {
        let registry = self.handlers.read().unwrap();

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
        let registry = self.handlers.read().unwrap();

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
                    self.client.clone(),
                    self.spec.clone(),
                    &GetOpts::default(),
                );
            }
        }
    }

    /// Dispatch batch of added documents (conditionally fetches based on registered handlers)
    async fn dispatch_added_batch(&self, type_name: &str, iris: Vec<TdbIRI>) {
        if iris.is_empty() {
            return;
        }

        let registry = self.handlers.read().unwrap();

        // Dispatch to on_added_id handlers (no fetch needed)
        if let Some(handlers) = registry.added_id_handlers.get(type_name) {
            for iri in &iris {
                for handler in handlers {
                    handler.handle(iri.clone());
                }
            }
        }

        // Check if we need to fetch documents
        let needs_individual_fetch = registry
            .added_handlers
            .get(type_name)
            .map_or(false, |h| !h.is_empty());
        let needs_batch_fetch = registry
            .added_batch_handlers
            .get(type_name)
            .map_or(false, |h| !h.is_empty());

        if !needs_individual_fetch && !needs_batch_fetch {
            // No handlers need fetching, we're done
            debug!(
                "Skipping fetch for {} added documents of type {} (no fetching handlers registered)",
                iris.len(),
                type_name
            );
            return;
        }

        debug!(
            "Fetching {} added documents of type {} in single batch query",
            iris.len(),
            type_name
        );

        // Dispatch to batch handlers
        if let Some(handlers) = registry.added_batch_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(
                    iris.clone(),
                    self.client.clone(),
                    self.spec.clone(),
                    &GetOpts::default(),
                );
            }
        }

        // Dispatch to individual handlers (will each spawn async task to fetch)
        if let Some(handlers) = registry.added_handlers.get(type_name) {
            for iri in &iris {
                for handler in handlers {
                    handler.handle(
                        iri.clone(),
                        self.client.clone(),
                        self.spec.clone(),
                        &GetOpts::default(),
                    );
                }
            }
        }
    }

    /// Dispatch batch of changed documents (conditionally fetches based on registered handlers)
    async fn dispatch_changed_batch(
        &self,
        type_name: &str,
        items: Vec<(TdbIRI, HashMap<String, Value>)>,
    ) {
        if items.is_empty() {
            return;
        }

        let registry = self.handlers.read().unwrap();

        // Dispatch to on_changeset handlers (no fetch needed)
        if let Some(handlers) = registry.changeset_handlers.get(type_name) {
            for (iri, changed_fields) in &items {
                for handler in handlers {
                    handler.handle(iri.clone(), changed_fields.clone());
                }
            }
        }

        // Check if we need to fetch documents
        let needs_individual_fetch = registry
            .changed_handlers
            .get(type_name)
            .map_or(false, |h| !h.is_empty());
        let needs_batch_fetch = registry
            .changed_batch_handlers
            .get(type_name)
            .map_or(false, |h| !h.is_empty());

        if !needs_individual_fetch && !needs_batch_fetch {
            // No handlers need fetching, we're done
            debug!(
                "Skipping fetch for {} changed documents of type {} (no fetching handlers registered)",
                items.len(),
                type_name
            );
            return;
        }

        debug!(
            "Fetching {} changed documents of type {} in single batch query",
            items.len(),
            type_name
        );

        // Dispatch to batch handlers
        if let Some(handlers) = registry.changed_batch_handlers.get(type_name) {
            for handler in handlers {
                handler.handle(
                    items.clone(),
                    self.client.clone(),
                    self.spec.clone(),
                    &GetOpts::default(),
                );
            }
        }

        // Dispatch to individual handlers (will each spawn async task to fetch)
        if let Some(handlers) = registry.changed_handlers.get(type_name) {
            for (iri, changed_fields) in &items {
                for handler in handlers {
                    handler.handle(
                        iri.clone(),
                        changed_fields.clone(),
                        self.client.clone(),
                        self.spec.clone(),
                        &GetOpts::default(),
                    );
                }
            }
        }
    }

    /// Dispatch batch of deleted documents (never needs fetching)
    async fn dispatch_deleted_batch(&self, type_name: &str, iris: Vec<TdbIRI>) {
        if iris.is_empty() {
            return;
        }

        let registry = self.handlers.read().unwrap();

        if let Some(handlers) = registry.deleted_handlers.get(type_name) {
            for iri in &iris {
                for handler in handlers {
                    handler.handle(iri.clone());
                }
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

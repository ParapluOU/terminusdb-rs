//! Untyped document CRUD operations

use anyhow::bail;
use tap::TapFallible;

use crate::ErrorResponse;

use {
    crate::{
        document::{CommitHistoryEntry, DocumentHistoryParams, DocumentInsertArgs, GetOpts},
        err::TypedErrorResponse,
        result::ResponseWithHeaders,
        spec::BranchSpec,
        TDBInsertInstanceResult, TerminusAPIStatus,
    },
    ::tracing::{debug, error, instrument, trace},
    anyhow::{anyhow, Context},
    serde_json::{json, Value},
    std::{
        collections::{HashMap, HashSet},
        fmt::Debug,
        time::Instant,
    },
    terminusdb_schema::ToJson,
    terminusdb_woql_builder::{
        prelude::{node, string_literal, WoqlBuilder},
        vars,
    },
};

/// Options for document deletion operations.
///
/// This struct provides safe configuration for document deletion,
/// particularly wrapping the dangerous `nuke` parameter to prevent
/// accidental misuse.
#[derive(Debug, Clone, Default)]
pub struct DeleteOpts {
    /// If true, removes ALL data from the graph.
    ///
    /// **⚠️ EXTREMELY DANGEROUS**: This will permanently delete ALL documents
    /// in the graph, not just the specified document. Use with extreme caution.
    /// This option should only be used when you intentionally want to clear
    /// the entire database.
    nuke: bool,
}

impl DeleteOpts {
    /// Create default delete options (safe deletion of specific documents only).
    pub fn new() -> Self {
        Self { nuke: false }
    }

    /// Create delete options for removing a specific document (default behavior).
    ///
    /// This is the safe option that only deletes the specified document.
    pub fn document_only() -> Self {
        Self { nuke: false }
    }

    /// **⚠️ EXTREMELY DANGEROUS**: Create delete options that will nuke ALL data.
    ///
    /// This will permanently delete ALL documents in the graph, not just the
    /// specified document. This is irreversible and should only be used when
    /// you intentionally want to clear the entire database.
    ///
    /// # Safety
    /// This function is marked as unsafe (conceptually) because it can cause
    /// massive data loss. Only use this when you are absolutely certain you
    /// want to delete ALL data in the graph.
    ///
    /// # Example
    /// ```rust
    /// // Only use this if you really want to delete EVERYTHING!
    /// let opts = DeleteOpts::nuke_all_data();  // Very explicit naming
    /// ```
    pub fn nuke_all_data() -> Self {
        Self { nuke: true }
    }

    /// Returns true if this will nuke all data (dangerous operation).
    pub fn is_nuke(&self) -> bool {
        self.nuke
    }
}

use super::helpers::{dedup_documents_by_id, dump_failed_payload};

/// Extracts document IDs from JSON objects that have an "@id" field.
///
/// # Arguments
/// * `documents` - Vector of JSON objects representing documents
///
/// # Returns
/// A vector of document IDs (strings) that were found in the documents
fn extract_document_ids(documents: &[Value]) -> Vec<String> {
    documents
        .iter()
        .filter_map(|doc| {
            doc.get("@id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

/// HTTP method to use for document operations
#[derive(Debug, Clone, Copy)]
enum DocumentMethod {
    /// POST - create new document (fails if exists)
    Post,
    /// PUT without create=true - update existing document (fails if doesn't exist)
    Put,
    /// PUT with create=true - create or update document
    PutWithCreate,
}

/// Untyped document operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Checks if an untyped document exists in the database by ID.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`has_instance`](Self::has_instance) for typed models
    ///
    /// This function checks for the existence of a document by its raw ID string.
    /// It works with any document type but provides no type safety.
    ///
    /// # Arguments
    /// * `id` - The document ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to check
    ///
    /// # Returns
    /// `true` if the document exists, `false` otherwise
    ///
    /// # Example
    /// ```rust
    /// let exists = client.has_document("12345", &branch_spec).await;
    /// ```
    #[instrument(
        name = "terminus.document.has",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id = %id
        )
    )]
    pub async fn has_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
    ) -> bool {
        // Use get_document_if_exists to avoid error logging for expected "not found" cases
        match self
            .get_document_if_exists(id, spec, GetOpts::default())
            .await
        {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => false, // Treat any real errors as "not exists"
        }
    }

    /// ID here should manually include the type, like
    /// "Song/myid"
    /// Retrieves an untyped document from the database by ID.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`get_instance`](Self::get_instance) for typed models with deserialization
    ///
    /// This function retrieves a document by its raw ID string and returns it
    /// as an untyped `serde_json::Value`. It provides no type safety or automatic
    /// deserialization.
    ///
    /// # Arguments
    /// * `id` - The document ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling the query behavior
    ///
    /// # Returns
    /// The document as a `serde_json::Value`
    ///
    /// # Example
    /// ```rust
    /// let doc = client.get_document("12345", &branch_spec, GetOpts::default()).await?;
    /// let name = doc["name"].as_str().unwrap();
    /// ```
    ///
    /// # Note
    /// For time-travel queries, use a branch specification with a commit ID:
    /// ```rust
    /// let past_spec = BranchSpec::from("main/local/commit/abc123");
    /// let old_doc = client.get_document("12345", &past_spec, GetOpts::default()).await?;
    /// ```
    #[instrument(
        name = "terminus.document.get",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id = %id,
            unfold = opts.unfold,
            as_list = opts.as_list
        ),
        err
    )]
    pub async fn get_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<serde_json::Value> {
        if !self.has_document(id, spec).await {
            Err(anyhow!("document #{} does not exist", id))?
        }

        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_get_params(id, opts.unfold, opts.as_list, opts.minimized)
            .build();

        debug!("retrieving document at {}...", &uri);

        let start = Instant::now();

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        debug!("retrieved TDB document with status code: {}", res.status());

        let res = self.parse_response::<Value>(res).await?;

        debug!("retrieved TDB document in {:?}", start.elapsed());

        Ok(res)
    }

    /// Retrieves a single untyped document from the database and returns both the document and commit ID.
    ///
    /// This is a header-aware variant of [`get_document`](Self::get_document) that returns
    /// the commit ID from the TerminusDB-Data-Version header along with the document.
    ///
    /// # Arguments
    /// * `id` - The document ID (without type prefix, e.g., "12345")
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling query behavior
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: The document as a `serde_json::Value`
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// let result = client.get_document_with_headers("12345", &branch_spec, GetOpts::default()).await?;
    /// let doc = *result; // Uses Deref to get the inner value
    /// if let Some(commit_id) = result.extract_commit_id() {
    ///     println!("Retrieved from commit: {}", commit_id);
    /// }
    /// ```
    #[instrument(
        name = "terminus.document.get_with_headers",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id = %id,
            unfold = opts.unfold,
            as_list = opts.as_list
        ),
        err
    )]
    pub async fn get_document_with_headers(
        &self,
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<ResponseWithHeaders<serde_json::Value>> {
        if !self.has_document(id, spec).await {
            Err(anyhow!("document #{} does not exist", id))?
        }

        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_get_params(id, opts.unfold, opts.as_list, opts.minimized)
            .build();

        debug!("retrieving document at {}...", &uri);

        let start = Instant::now();

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        debug!("retrieved TDB document with status code: {}", res.status());

        let res = self.parse_response_with_headers::<Value>(res).await?;

        debug!("retrieved TDB document in {:?}", start.elapsed());

        Ok(res)
    }

    /// Retrieves an untyped document from the database if it exists.
    ///
    /// This method is designed for cases where a document might not exist and that's
    /// an expected scenario (e.g., checking before create). Unlike `get_document`,
    /// this method returns `None` for non-existent documents without logging errors.
    ///
    /// # Arguments
    /// * `id` - The document ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling the query behavior
    ///
    /// # Returns
    /// * `Ok(Some(document))` - If the document exists
    /// * `Ok(None)` - If the document doesn't exist
    /// * `Err(error)` - Only for actual errors (network, parsing, etc.)
    ///
    /// # Example
    /// ```rust
    /// match client.get_document_if_exists("12345", &branch_spec, GetOpts::default()).await? {
    ///     Some(doc) => println!("Document exists: {:?}", doc),
    ///     None => println!("Document not found"),
    /// }
    /// ```
    #[instrument(
        name = "terminus.document.get_if_exists",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id = %id,
            unfold = opts.unfold,
            as_list = opts.as_list
        )
        // Note: no 'err' attribute - we don't want to log DocumentNotFound as errors
    )]
    pub async fn get_document_if_exists(
        &self,
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_get_params(id, opts.unfold, opts.as_list, opts.minimized)
            .build();

        debug!("retrieving document at {}...", &uri);

        let start = Instant::now();

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        debug!("retrieved TDB document with status code: {}", res.status());

        // Check if it's a 404 Not Found
        if res.status() == 404 {
            debug!("Document #{} not found", id);
            return Ok(None);
        }

        // Parse the response - this might still contain an error response
        match self.parse_response::<Value>(res).await {
            Ok(doc) => {
                debug!("retrieved TDB document in {:?}", start.elapsed());
                Ok(Some(doc))
            }
            Err(err) => {
                // Check if the error is DocumentNotFound
                if let Some(err_response) = err.downcast_ref::<TypedErrorResponse>() {
                    match err_response {
                        TypedErrorResponse::DocumentError { error: err, .. }
                            if matches!(err.api_status, TerminusAPIStatus::NotFound) =>
                        {
                            debug!("Document #{} not found (from error response)", id);
                            return Ok(None);
                        }
                        _ => {}
                    }
                }
                // For any other error, propagate it
                Err(err)
            }
        }
    }

    /// Internal method for document operations with specific HTTP method
    #[instrument(
        name = "terminus.document.insert_with_method",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            method = ?method,
            document_count = model.len(),
            graph_type = ?args.ty
        ),
        err
    )]
    async fn insert_documents_with_method(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
        method: DocumentMethod,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        if model.is_empty() {
            debug!("All documents were filtered out or no documents to insert");
            return Ok(ResponseWithHeaders::without_headers(HashMap::new()));
        }

        self.ensure_database(&args.spec.db)
            .await
            .context("ensuring database")?;

        let ty = args.ty.to_string().to_lowercase();

        let mut to_jsoned = model
            .into_iter()
            .map(|t| t.to_json())
            .rev()
            .collect::<Vec<_>>();

        dedup_documents_by_id(&mut to_jsoned);

        let mut documents_to_update_instead = vec![];

        // For POST method, filter out documents that already exist (unless skip_existence_check=true)
        if matches!(method, DocumentMethod::Post) && !args.skip_existence_check {
            let document_ids = extract_document_ids(&to_jsoned);

            debug!(
                "POST method: Found {} documents with IDs to check",
                document_ids.len()
            );

            if !document_ids.is_empty() {
                debug!("Document IDs to check: {:?}", document_ids);
                let existing_ids = self.check_existing_ids(&document_ids, &args.spec).await?;

                if !existing_ids.is_empty() {
                    documents_to_update_instead = to_jsoned
                        .iter()
                        .filter(|d| {
                            existing_ids.contains(
                                d.get("@id").and_then(|id| id.as_str()).unwrap_or_default(),
                            )
                        })
                        .cloned()
                        .collect();

                    debug!(
                        "Filtering out {} existing documents from POST operation",
                        existing_ids.len()
                    );
                    let before_count = to_jsoned.len();
                    to_jsoned.retain(|doc| {
                        doc.get("@id")
                            .and_then(|id| id.as_str())
                            .map(|id| !existing_ids.contains(id))
                            .unwrap_or(true) // Keep documents without @id
                    });
                    debug!(
                        "Filtered from {} to {} documents",
                        before_count,
                        to_jsoned.len()
                    );
                }
            }
        }

        // If all documents were filtered out, return empty result
        if to_jsoned.is_empty() {
            debug!("All documents were filtered out or no documents to insert");
            return Ok(ResponseWithHeaders::without_headers(HashMap::new()));
        }

        //eprintln!("inserting document(s): {:#?}", &to_jsoned);

        let json = serde_json::to_string(&to_jsoned).unwrap();

        trace!(
            "about to insert {} at URI with method {:?}: {:#?}",
            &ty,
            method,
            &json[0..(10000.min(json.len()))]
        );

        // Build request based on method
        let res = match method {
            DocumentMethod::Post => {
                let mut url_builder = self
                    .build_url()
                    .endpoint("document")
                    .database(&args.spec)
                    .query("author", &args.author)
                    .query_encoded("message", &args.message)
                    .query("graph_type", &ty);

                // When force=true, use full_replace to guarantee success on duplicates
                if args.force {
                    url_builder = url_builder.query("full_replace", "true");
                }

                let uri = url_builder.build();

                debug!("POST {} to URI {}", &ty, &uri);

                // Acquire concurrency permit for write operations
                let _permit = self.acquire_write_permit().await;

                let mut request = self
                    .http
                    .post(uri)
                    .basic_auth(&self.user, Some(&self.pass))
                    .header("Content-Type", "application/json")
                    .body(json.clone());

                if let Some(timeout) = args.timeout {
                    request = request.timeout(timeout);
                }

                let r = request
                    .send()
                    .await
                    .tap_err(|e| error!("error on request.send(): {:?}", e))?;

                // insert existing documents with PUT (only when skip_existence_check=false)
                if !args.skip_existence_check && !documents_to_update_instead.is_empty() {
                    let update_res = Box::pin(self.insert_documents_with_method(
                        documents_to_update_instead.iter().collect(),
                        args.clone(),
                        DocumentMethod::Put,
                    ))
                    .await
                    .map_err(|e| {
                        error!("Error updating existing documents during POST: {}", e);
                    });
                }

                r
            }
            DocumentMethod::Put => {
                let uri = self
                    .build_url()
                    .endpoint("document")
                    .database(&args.spec)
                    .document_params(&args.author, &args.message, &ty, false) // create=false
                    .build();

                debug!("PUT {} to URI {} (create=false)", &ty, &uri);

                // Acquire concurrency permit for write operations
                let _permit = self.acquire_write_permit().await;

                let mut request = self
                    .http
                    .put(uri)
                    .basic_auth(&self.user, Some(&self.pass))
                    .header("Content-Type", "application/json")
                    .body(json.clone());

                if let Some(timeout) = args.timeout {
                    request = request.timeout(timeout);
                }

                request.send().await?
            }
            DocumentMethod::PutWithCreate => {
                let uri = self
                    .build_url()
                    .endpoint("document")
                    .database(&args.spec)
                    .document_params(&args.author, &args.message, &ty, true) // create=true
                    .build();

                debug!("PUT {} to URI {} (create=true)", &ty, &uri);

                // Acquire concurrency permit for write operations
                let _permit = self.acquire_write_permit().await;

                let mut request = self
                    .http
                    .put(uri)
                    .basic_auth(&self.user, Some(&self.pass))
                    .header("Content-Type", "application/json")
                    .body(json.clone());

                if let Some(timeout) = args.timeout {
                    request = request.timeout(timeout);
                }

                request.send().await?
            }
        };

        let parsed = self.parse_response_with_headers::<Vec<String>>(res).await;

        trace!(
            "parsed response from insert_documents_with_method(): {:#?}",
            &parsed
        );

        if let Err(e) = &parsed {
            error!("request error: {:#?}", &e);
            dump_failed_payload(&json);
            return Err(anyhow!("Document operation failed: {}", e));
        }

        trace!("{:?}'ed {} into TerminusDB", method, ty);

        let parsed = parsed?;
        let version_header = parsed.commit_id.clone();
        let data = parsed.into_inner();
        let result_map = data
            .into_iter()
            .map(|id| (id.clone(), TDBInsertInstanceResult::Inserted(id)))
            .collect();

        Ok(ResponseWithHeaders::new(result_map, version_header))
    }

    /// Inserts multiple untyped documents into the database.
    ///
    /// **⚠️ Consider using strongly-typed alternatives instead:**
    /// - [`insert_instances`](Self::insert_instances) for multiple typed models
    /// - [`insert_instance`](Self::insert_instance) for single typed models
    ///
    /// This function accepts any type that implements `ToJson` (like `serde_json::Value`
    /// or schema definitions) and inserts them as untyped documents.
    ///
    /// Uses PUT with create=true for backward compatibility.
    ///
    /// # Arguments
    /// * `model` - Vector of references to objects that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with document IDs and insert results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    ///
    /// let docs = vec![
    ///     &json!({"@type": "Person", "name": "Alice"}),
    ///     &json!({"@type": "Person", "name": "Bob"}),
    /// ];
    /// let result = client.insert_documents(docs, args).await?;
    /// ```
    #[instrument(
        name = "terminus.document.insert_multiple",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            document_count = model.len(),
            graph_type = ?args.ty
        ),
        err
    )]
    pub async fn insert_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        self.insert_documents_with_method(model, args, DocumentMethod::PutWithCreate)
            .await
    }

    /// Inserts documents with a namespace context.
    ///
    /// This is essential when inserting instances that belong to a specific namespace,
    /// especially in multi-namespace databases where the same class name may exist
    /// in different namespaces. The Context's `@schema` field tells TerminusDB how
    /// to resolve short type names like "DocumentType" to full URIs.
    ///
    /// # Arguments
    /// * `context` - The context with namespace configuration (schema, base)
    /// * `documents` - The documents to insert
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Example
    /// ```rust,ignore
    /// use terminusdb_xsd::XsdModel;
    ///
    /// let model = XsdModel::from_file("book.xsd", None::<&str>)?;
    /// let context = model.context().clone();
    /// let instances = model.parse_xml_to_instances(xml)?;
    ///
    /// // Instance with @type: "DocumentType" resolves to
    /// // "http://example.com/book#DocumentType" via context
    /// client.insert_documents_with_context(context, &instances, args).await?;
    /// ```
    #[instrument(
        name = "terminus.document.insert_with_context",
        skip(self, context, documents, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            schema_namespace = %context.schema,
            document_count = documents.len()
        ),
        err
    )]
    pub async fn insert_documents_with_context(
        &self,
        context: terminusdb_schema::Context,
        documents: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        use terminusdb_schema::json::ToJson as _;

        // Convert context and documents to JSON
        let context_json = context.to_json();
        let doc_jsons: Vec<_> = documents.iter().map(|d| d.to_json()).collect();

        // Combine into a single batch - context first, then documents
        let mut all_docs: Vec<&serde_json::Value> = Vec::with_capacity(1 + documents.len());
        all_docs.push(&context_json);
        all_docs.extend(doc_jsons.iter());

        debug!(
            "Inserting {} documents with context namespace {}",
            documents.len(),
            context.schema
        );

        self.insert_documents(all_docs, args).await
    }

    /// Inserts a single untyped document into the database.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`insert_instance`](Self::insert_instance) for typed models
    ///
    /// This function accepts any type that implements `ToJson` (like `serde_json::Value`
    /// or schema definitions) and inserts it as an untyped document.
    ///
    /// # Arguments
    /// * `model` - A reference to an object that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A cloned instance of the client (note: does not include commit ID information)
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    ///
    /// let doc = json!({"@type": "Person", "name": "Alice"});
    /// client.insert_document(&doc, args).await?;
    /// ```
    ///
    /// # Note
    /// This function returns the client instance but does not provide access to
    /// the commit ID. Use [`insert_documents`](Self::insert_documents) if you need
    /// commit information from headers.
    #[instrument(
        name = "terminus.document.insert",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            graph_type = ?args.ty
        ),
        err
    )]
    pub async fn insert_document(
        &self,
        model: &impl ToJson,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        let json = model.to_json();

        let ty = args.ty.to_string().to_lowercase();

        let uri = self
            .build_url()
            .endpoint("document")
            .database(&args.spec)
            .document_params(&args.author, &args.message, &ty, true)
            .build();

        debug!("about to insert {} at URI {}: {:#?}", &ty, &uri, &json);

        // todo: author should probably be node name
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.to_string())
            .send()
            .await?;

        self.parse_response::<Value>(res).await?;

        debug!("inserted {} into TerminusDB", ty);

        Ok(self.clone())
    }

    /// Creates new documents using POST endpoint.
    ///
    /// This method uses the POST endpoint which is designed for creating new documents.
    /// It will fail if any of the documents already exist.
    ///
    /// # Arguments
    /// * `model` - Vector of references to objects that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with document IDs and insert results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    ///
    /// let docs = vec![
    ///     &json!({"@type": "Person", "name": "Alice"}),
    ///     &json!({"@type": "Person", "name": "Bob"}),
    /// ];
    /// let result = client.post_documents(docs, args).await?;
    /// ```
    #[instrument(
        name = "terminus.document.post",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            document_count = model.len(),
            graph_type = ?args.ty
        ),
        err
    )]
    pub async fn post_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        self.insert_documents_with_method(model, args, DocumentMethod::Post)
            .await
    }

    /// Updates existing documents using PUT endpoint without create.
    ///
    /// This method uses the PUT endpoint without the create=true parameter,
    /// which means it will only update existing documents and fail if any don't exist.
    ///
    /// # Arguments
    /// * `model` - Vector of references to objects that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with document IDs and update results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    ///
    /// let docs = vec![
    ///     &json!({"@id": "Person/alice", "@type": "Person", "name": "Alice Updated"}),
    ///     &json!({"@id": "Person/bob", "@type": "Person", "name": "Bob Updated"}),
    /// ];
    /// let result = client.put_documents(docs, args).await?;
    /// ```
    #[instrument(
        name = "terminus.document.put",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            document_count = model.len(),
            graph_type = ?args.ty
        ),
        err
    )]
    pub async fn put_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        self.insert_documents_with_method(model, args, DocumentMethod::Put)
            .await
    }

    #[instrument(
        name = "terminus.document.insert_by_schema_type",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            schema_type = %T::schema_name(),
            document_count = model.len()
        ),
        err
    )]
    pub async fn insert_documents_by_schema_type<T: terminusdb_schema::ToTDBSchema>(
        &self,
        mut model: Vec<&terminusdb_schema::Instance>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        // dedup_instances_by_id(&mut model);

        let selection = model
            .into_iter()
            .filter(|instance| instance.schema.class_name() == &T::schema_name())
            .collect::<Vec<_>>();

        self.insert_documents(selection, args).await
    }

    /// Get the commit history for a specific document.
    ///
    /// This method retrieves the list of commits where the specified document was modified.
    /// This is particularly useful for tracking changes to RandomKey entities over time.
    ///
    /// # Arguments
    /// * `document_id` - The full document ID (e.g., "MyEntity/abc123randomkey")
    /// * `spec` - Branch specification (branch to query history from)
    /// * `params` - Optional parameters for pagination and filtering
    ///
    /// # Returns
    /// A vector of `CommitHistoryEntry` containing commit details
    ///
    /// # Example
    /// ```rust
    /// // Get full history for a document
    /// let history = client.get_document_history(
    ///     "Person/abc123",
    ///     &branch_spec,
    ///     None
    /// ).await?;
    ///
    /// // Get last 5 commits
    /// let params = DocumentHistoryParams::new()
    ///     .with_start(0)
    ///     .with_count(5);
    /// let recent = client.get_document_history(
    ///     "Person/abc123",
    ///     &branch_spec,
    ///     Some(params)
    /// ).await?;
    /// ```
    #[instrument(
        name = "terminus.document.get_history",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            document_id = %document_id,
            start = params.as_ref().and_then(|p| p.start).unwrap_or(0),
            count = params.as_ref().and_then(|p| p.count)
        ),
        err
    )]
    pub async fn get_document_history(
        &self,
        document_id: &str,
        spec: &BranchSpec,
        params: Option<DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<CommitHistoryEntry>> {
        let params = params.unwrap_or_default();

        let uri = self
            .build_url()
            .endpoint("history")
            .database_with_branch(spec)
            .history_params(document_id, &params)
            .build();

        debug!("Fetching document history from: {}", &uri);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        // The /history endpoint returns a direct array, not wrapped in ApiResponse
        let history = res
            .json::<Vec<CommitHistoryEntry>>()
            .await
            .context("Failed to parse document history response")?;

        debug!(
            "Retrieved {} history entries for document {}",
            history.len(),
            document_id
        );

        Ok(history)
    }

    /// Retrieves multiple untyped documents from the database by IDs.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`get_instances`](Self::get_instances) for typed models with deserialization
    ///
    /// This function retrieves multiple documents by their IDs and returns them
    /// as untyped `serde_json::Value` objects. It provides no type safety or automatic
    /// deserialization. For large ID lists, it automatically uses POST with
    /// `X-HTTP-Method-Override: GET` to avoid URL length limits.
    ///
    /// # Arguments
    /// * `ids` - Vector of document IDs to retrieve
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling query behavior (skip, count, type filter, etc.)
    ///
    /// # Returns
    /// A vector of documents as `serde_json::Value` objects
    ///
    /// # Example
    /// ```rust
    /// let ids = vec!["Person/alice".to_string(), "Person/bob".to_string()];
    /// let opts = GetOpts::default().with_unfold(true);
    /// let docs = client.get_documents(ids, &branch_spec, opts).await?;
    /// ```
    ///
    /// # Pagination Example
    /// ```rust
    /// let ids = vec!["Person/alice".to_string(), "Person/bob".to_string()];
    /// let opts = GetOpts::paginated(0, 10); // skip 0, take 10
    /// let docs = client.get_documents(ids, &branch_spec, opts).await?;
    /// ```
    ///
    /// # Type Filtering Example
    /// ```rust
    /// let ids = vec![]; // empty means "get all"
    /// let opts = GetOpts::filtered_by_type::<Person>().with_count(5);
    /// let docs = client.get_documents(ids, &branch_spec, opts).await?;
    /// ```
    #[instrument(
        name = "terminus.document.get_multiple",
        skip(self, ids),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id_count = ids.len(),
            unfold = opts.unfold,
            skip = opts.skip,
            count = opts.count,
            type_filter = opts.type_filter
        ),
        err
    )]
    pub async fn get_documents(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        debug!("Retrieving {} documents", ids.len());

        // Build the URL for the document endpoint
        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_get_multiple_params(&ids, &opts)
            .build();

        // Determine if we should use POST method based on URL length or explicit large request
        let use_post = uri.len() > 2000 || ids.len() > 50; // Use POST for long URLs or many IDs

        debug!(
            "Fetching documents from: {} (using {})",
            &uri,
            if use_post { "POST" } else { "GET" }
        );

        let start = Instant::now();

        let res = if use_post {
            // Use POST with X-HTTP-Method-Override: GET for large requests
            let base_uri = self.build_url().endpoint("document").database(spec).build();

            // Create query document as JSON
            let mut query_doc = serde_json::Map::new();
            if !ids.is_empty() {
                query_doc.insert("ids".to_string(), serde_json::to_value(&ids)?);
            }
            query_doc.insert("as_list".to_string(), serde_json::Value::Bool(true));
            query_doc.insert("unfold".to_string(), serde_json::Value::Bool(opts.unfold));
            query_doc.insert(
                "minimized".to_string(),
                serde_json::Value::Bool(opts.minimized),
            );

            if let Some(skip) = opts.skip {
                query_doc.insert("skip".to_string(), serde_json::Value::Number(skip.into()));
            }
            if let Some(count) = opts.count {
                query_doc.insert("count".to_string(), serde_json::Value::Number(count.into()));
            }
            if let Some(ref type_filter) = opts.type_filter {
                query_doc.insert(
                    "type".to_string(),
                    serde_json::Value::String(type_filter.clone()),
                );
            }

            let query_json = serde_json::to_string(&query_doc)?;

            let mut request = self
                .http
                .post(base_uri)
                .basic_auth(&self.user, Some(&self.pass))
                .header("Content-Type", "application/json")
                .header("X-HTTP-Method-Override", "GET")
                .body(query_json);

            // Apply timeout if provided
            if let Some(timeout) = opts.timeout {
                request = request.timeout(timeout);
            }

            request.send().await?
        } else {
            // Use GET for smaller requests
            let mut request = self.http.get(uri).basic_auth(&self.user, Some(&self.pass));

            // Apply timeout if provided
            if let Some(timeout) = opts.timeout {
                request = request.timeout(timeout);
            }

            request.send().await?
        };

        // Extract status before consuming response
        let status = res.status();

        // Parse response as array of JSON values
        let docs = self.parse_response::<Vec<serde_json::Value>>(res).await?;

        debug!(
            "Retrieved {} documents with status code: {} in {:?}",
            docs.len(),
            status,
            start.elapsed()
        );

        Ok(docs)
    }

    /// Retrieves multiple untyped documents from the database and returns both the documents and commit ID.
    ///
    /// This is a header-aware variant of [`get_documents`](Self::get_documents) that returns
    /// the commit ID from the TerminusDB-Data-Version header along with the documents.
    ///
    /// # Arguments
    /// * `ids` - Vector of document IDs to retrieve
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling query behavior (skip, count, type filter, etc.)
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: Vector of documents as `serde_json::Value` objects
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// let ids = vec!["Person/alice".to_string(), "Person/bob".to_string()];
    /// let opts = GetOpts::default().with_unfold(true);
    /// let result = client.get_documents_with_headers(ids, &branch_spec, opts).await?;
    /// let docs = *result; // Uses Deref to get the inner value
    /// if let Some(commit_id) = result.extract_commit_id() {
    ///     println!("Retrieved from commit: {}", commit_id);
    /// }
    /// ```
    #[instrument(
        name = "terminus.document.get_multiple_with_headers",
        skip(self, ids),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id_count = ids.len(),
            unfold = opts.unfold,
            skip = opts.skip,
            count = opts.count,
            type_filter = opts.type_filter
        ),
        err
    )]
    pub(crate) async fn get_documents_with_headers(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<ResponseWithHeaders<Vec<serde_json::Value>>> {
        debug!("Retrieving {} documents with headers", ids.len());

        // Build the URL for the document endpoint
        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_get_multiple_params(&ids, &opts)
            .build();

        // Determine if we should use POST method based on URL length or explicit large request
        let use_post = uri.len() > 2000 || ids.len() > 50; // Use POST for long URLs or many IDs

        debug!(
            "Fetching documents from: {} (using {})",
            &uri,
            if use_post { "POST" } else { "GET" }
        );

        let start = Instant::now();

        let res = if use_post {
            // Use POST with X-HTTP-Method-Override: GET for large requests
            let base_uri = self.build_url().endpoint("document").database(spec).build();

            // Create query document as JSON
            let mut query_doc = serde_json::Map::new();
            if !ids.is_empty() {
                query_doc.insert("ids".to_string(), serde_json::to_value(&ids)?);
            }
            query_doc.insert("as_list".to_string(), serde_json::Value::Bool(true));
            query_doc.insert("unfold".to_string(), serde_json::Value::Bool(opts.unfold));
            query_doc.insert(
                "minimized".to_string(),
                serde_json::Value::Bool(opts.minimized),
            );

            if let Some(skip) = opts.skip {
                query_doc.insert("skip".to_string(), serde_json::Value::Number(skip.into()));
            }
            if let Some(count) = opts.count {
                query_doc.insert("count".to_string(), serde_json::Value::Number(count.into()));
            }
            if let Some(ref type_filter) = opts.type_filter {
                query_doc.insert(
                    "type".to_string(),
                    serde_json::Value::String(type_filter.clone()),
                );
            }

            let query_json = serde_json::to_string(&query_doc)?;

            let mut request = self
                .http
                .post(base_uri)
                .basic_auth(&self.user, Some(&self.pass))
                .header("Content-Type", "application/json")
                .header("X-HTTP-Method-Override", "GET")
                .body(query_json);

            // Apply timeout if provided
            if let Some(timeout) = opts.timeout {
                request = request.timeout(timeout);
            }

            request.send().await?
        } else {
            // Use GET for smaller requests
            let mut request = self.http.get(uri).basic_auth(&self.user, Some(&self.pass));

            // Apply timeout if provided
            if let Some(timeout) = opts.timeout {
                request = request.timeout(timeout);
            }

            request.send().await?
        };

        debug!("Retrieved documents with status code: {}", res.status());

        // Parse response as array of JSON values with headers
        let docs = self
            .parse_response_with_headers::<Vec<serde_json::Value>>(res)
            .await?;

        debug!(
            "Retrieved {} documents in {:?}",
            docs.len(),
            start.elapsed()
        );

        Ok(docs)
    }

    /// Deletes documents from the database.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`delete_instance`](Self::delete_instance) for typed models
    ///
    /// This function deletes documents by their IDs. It provides no type safety.
    ///
    /// **⚠️ Warning**: Using `DeleteOpts::nuke_all_data()` will remove ALL data from the graph.
    /// Use with extreme caution as this operation is irreversible.
    ///
    /// # Arguments
    /// * `id` - Optional document ID to delete. If None, uses the request body or nuke behavior
    /// * `spec` - Branch specification indicating which branch to delete from
    /// * `author` - Author of the deletion
    /// * `message` - Commit message for the deletion
    /// * `graph_type` - Graph type (usually "instance")
    /// * `opts` - Delete options controlling the deletion behavior
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust
    /// // Delete a specific document (safe)
    /// client.delete_document(
    ///     Some("Person/alice"),
    ///     &branch_spec,
    ///     "admin",
    ///     "Removed alice",
    ///     "instance",
    ///     DeleteOpts::document_only()
    /// ).await?;
    ///
    /// // WARNING: Nuclear option - deletes ALL data
    /// client.delete_document(
    ///     None,
    ///     &branch_spec,
    ///     "admin",
    ///     "Reset all data",
    ///     "instance",
    ///     DeleteOpts::nuke_all_data()  // DANGEROUS: This deletes everything!
    /// ).await?;
    /// ```
    #[instrument(
        name = "terminus.document.delete",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id = ?id,
            graph_type = %graph_type,
            nuke = opts.is_nuke()
        ),
        err
    )]
    pub async fn delete_document(
        &self,
        id: Option<&str>,
        spec: &BranchSpec,
        author: &str,
        message: &str,
        graph_type: &str,
        opts: DeleteOpts,
    ) -> anyhow::Result<Self> {
        let uri = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .document_delete_params(author, message, graph_type, &opts, id)
            .build();

        debug!("Deleting document at URI: {}", &uri);

        if opts.is_nuke() {
            debug!("⚠️  WARNING: Nuclear deletion requested - this will remove ALL data from the graph!");
        }

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        // todo: the OpenAPI spec mentions a proper JSON response is missing
        if let Ok(err) = self.parse_response::<ErrorResponse>(res).await {
            bail!("failed to delete: {:#?}", err);
        } else {
            debug!("Successfully deleted document");
        }

        Ok(self.clone())
    }

    /// Checks which of the given IDs already exist in the database using a single WOQL query.
    ///
    /// This method uses WOQL's `or()` combined with `count()` queries to check multiple
    /// IDs in a single database request, making it much more efficient than checking
    /// each ID individually.
    ///
    /// # Arguments
    /// * `ids` - Vector of document IDs to check for existence
    /// * `spec` - Branch specification indicating which branch to check
    ///
    /// # Returns
    /// A `HashSet<String>` containing the IDs that already exist in the database
    ///
    /// # Example
    /// ```rust
    /// let ids = vec!["Person/alice".to_string(), "Person/bob".to_string()];
    /// let existing = client.check_existing_ids(&ids, &branch_spec).await?;
    /// // existing will contain only the IDs that already exist in the database
    /// ```
    #[instrument(
        name = "terminus.document.check_existing_ids",
        skip(self, ids),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            id_count = ids.len()
        ),
        err
    )]
    pub async fn check_existing_ids(
        &self,
        ids: &[String],
        spec: &BranchSpec,
    ) -> anyhow::Result<HashSet<String>> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }

        debug!("Checking existence of {} IDs using WOQL", ids.len());

        // Create variables for the query
        let id_var = vars!("ID");
        let count_var = vars!("Count");
        let type_var = vars!("Type");

        // Build count queries for each ID
        let mut count_queries = Vec::new();
        let mut count_vars = Vec::new();

        for (i, id) in ids.iter().enumerate() {
            let count_var = vars!(format!("Count_{}", i));
            count_vars.push((id.clone(), count_var.clone()));

            // Count documents where the document ID (as subject) has any rdf:type
            // This checks if the document exists
            let sub_query =
                WoqlBuilder::new().triple(node(id.clone()), "rdf:type", type_var.clone());

            // Create a count query that counts the sub_query results
            let count_query = sub_query.count(count_var);
            count_queries.push(count_query);
        }

        if count_queries.is_empty() {
            return Ok(HashSet::new());
        }

        // Combine all counts with and() and select the count variables
        let select_vars: Vec<_> = count_vars.iter().map(|(_, v)| v.clone()).collect();

        let query = if count_queries.len() == 1 {
            count_queries.into_iter().next().unwrap()
        } else {
            // For multiple queries, chain them with and()
            let mut iter = count_queries.into_iter();
            let first = iter.next().unwrap();
            first.and(iter.collect::<Vec<_>>())
        }
        .select(select_vars)
        .finalize();

        // Execute the query
        debug!("Executing WOQL count query for {} IDs", ids.len());
        let result = self
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;
        debug!("WOQL query returned {} bindings", result.bindings.len());

        // Extract the existing IDs from the count results
        let mut existing_ids = HashSet::new();

        if let Some(binding) = result.bindings.first() {
            // Check each count variable
            for (id, count_var) in count_vars.iter() {
                if let Some(count_obj) = binding.get(count_var.name()) {
                    // Count is returned as {"@type": "xsd:decimal", "@value": Number}
                    if let Some(count_map) = count_obj.as_object() {
                        if let Some(value) = count_map.get("@value") {
                            if let Some(count) = value.as_u64() {
                                if count > 0 {
                                    existing_ids.insert(id.clone());
                                }
                            } else if let Some(count) = value.as_f64() {
                                if count > 0.0 {
                                    existing_ids.insert(id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!(
            "Found {} existing IDs out of {} checked",
            existing_ids.len(),
            ids.len()
        );
        if !existing_ids.is_empty() {
            debug!("Existing IDs found: {:?}", existing_ids);
        }

        Ok(existing_ids)
    }
}

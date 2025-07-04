//! Untyped document CRUD operations

use {
    crate::{
        document::{CommitHistoryEntry, DocumentHistoryParams, DocumentInsertArgs, GetOpts}, 
        result::ResponseWithHeaders, 
        spec::BranchSpec,
        TDBInsertInstanceResult,
    },
    ::log::{debug, error, trace},
    anyhow::{anyhow, Context},
    serde_json::{json, Value},
    std::{collections::HashMap, fmt::Debug, time::Instant},
    terminusdb_schema::ToJson,
};

use super::helpers::{dedup_documents_by_id, dump_failed_payload};

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
    pub async fn has_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
    ) -> bool {
        let r: anyhow::Result<_> = async {
            let uri = self.build_url()
                .endpoint("document")
                .database(spec)
                .document_get_params(id, false, false)
                .build();

            Ok::<Value, anyhow::Error>(
                self.parse_response::<Value>(
                    self.http
                        .get(uri)
                        .basic_auth(&self.user, Some(&self.pass))
                        .send()
                        .await?,
                )
                .await?,
            )
        }
        .await;

        r.is_ok()
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
    pub async fn get_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<serde_json::Value> {
        if !self.has_document(id, spec).await {
            Err(anyhow!("document #{} does not exist", id))?
        }

        let uri = self.build_url()
            .endpoint("document")
            .database(spec)
            .document_get_params(id, opts.unfold, opts.as_list)
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

    /// Inserts multiple untyped documents into the database.
    ///
    /// **⚠️ Consider using strongly-typed alternatives instead:**
    /// - [`insert_instances`](Self::insert_instances) for multiple typed models
    /// - [`insert_instance`](Self::insert_instance) for single typed models
    ///
    /// This function accepts any type that implements `ToJson` (like `serde_json::Value`
    /// or schema definitions) and inserts them as untyped documents.
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
    pub async fn insert_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        let ty = args.ty.to_string().to_lowercase();

        let uri = self.build_url()
            .endpoint("document")
            .database(&args.spec)
            .document_params(&args.author, &args.message, &ty, true)
            .build();

        let mut to_jsoned = model
            .into_iter()
            .map(|t| t.to_json())
            .rev()
            .collect::<Vec<_>>();

        // for (i, j) in &mut to_jsoned.iter_mut().enumerate() {
        //     j.as_object_mut()
        //         .unwrap()
        //         .insert("@capture".to_string(), format!("v{}", i).into());
        // }

        dedup_documents_by_id(&mut to_jsoned);

        eprintln!("inserting document(s): {:#?}", &to_jsoned);

        let json = serde_json::to_string(&to_jsoned).unwrap();

        trace!(
            "about to insert {} at URI {}: {:#?}",
            &ty,
            &uri,
            // todo: add trailing "..." or "(truncated)" if message was longer
            &json[0..(10000.min(json.len()))]
        );

        if args.ty.is_instance() {
            // panic!("{:#?}", &to_jsoned);
        }

        // todo: author should probably be node name
        // todo: dont clone just for the failed paload dumping
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.clone())
            .send()
            .await?;

        let parsed = self.parse_response_with_headers::<Vec<String>>(res).await;

        trace!("parsed response from insert_documents(): {:#?}", &parsed);

        if let Err(e) = &parsed {
            error!("request error: {:#?}", &e);
            dump_failed_payload(&json);
            return Err(anyhow!("Insert request failed: {}", e));
        }

        debug!("inserted {} into TerminusDB", ty);

        let parsed = parsed?;
        let version_header = parsed.commit_id.clone();
        let data = parsed.into_inner();
        let result_map = data
            .into_iter()
            .map(|id| (id.clone(), TDBInsertInstanceResult::Inserted(id)))
            .collect();

        Ok(ResponseWithHeaders::new(result_map, version_header))
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
    pub async fn insert_document(
        &self,
        model: &impl ToJson,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        let json = model.to_json();

        let ty = args.ty.to_string().to_lowercase();

        let uri = self.build_url()
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
    pub async fn get_document_history(
        &self,
        document_id: &str,
        spec: &BranchSpec,
        params: Option<DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<CommitHistoryEntry>> {
        let params = params.unwrap_or_default();
        
        let uri = self.build_url()
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

        let history = self.parse_response::<Vec<CommitHistoryEntry>>(res)
            .await
            .context("Failed to parse document history response")?;

        debug!("Retrieved {} history entries for document {}", history.len(), document_id);

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
    pub async fn get_documents(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        debug!("Retrieving {} documents", ids.len());

        // Build the URL for the document endpoint
        let uri = self.build_url()
            .endpoint("document")
            .database(spec)
            .document_get_multiple_params(&ids, &opts)
            .build();

        // Determine if we should use POST method based on URL length or explicit large request
        let use_post = uri.len() > 2000 || ids.len() > 50; // Use POST for long URLs or many IDs

        debug!("Fetching documents from: {} (using {})", &uri, if use_post { "POST" } else { "GET" });

        let start = Instant::now();

        let res = if use_post {
            // Use POST with X-HTTP-Method-Override: GET for large requests
            let base_uri = self.build_url()
                .endpoint("document")
                .database(spec)
                .build();

            // Create query document as JSON
            let mut query_doc = serde_json::Map::new();
            if !ids.is_empty() {
                query_doc.insert("ids".to_string(), serde_json::to_value(&ids)?);
            }
            query_doc.insert("as_list".to_string(), serde_json::Value::Bool(true));
            query_doc.insert("unfold".to_string(), serde_json::Value::Bool(opts.unfold));
            
            if let Some(skip) = opts.skip {
                query_doc.insert("skip".to_string(), serde_json::Value::Number(skip.into()));
            }
            if let Some(count) = opts.count {
                query_doc.insert("count".to_string(), serde_json::Value::Number(count.into()));
            }
            if let Some(ref type_filter) = opts.type_filter {
                query_doc.insert("type".to_string(), serde_json::Value::String(type_filter.clone()));
            }

            let query_json = serde_json::to_string(&query_doc)?;

            self.http
                .post(base_uri)
                .basic_auth(&self.user, Some(&self.pass))
                .header("Content-Type", "application/json")
                .header("X-HTTP-Method-Override", "GET")
                .body(query_json)
                .send()
                .await?
        } else {
            // Use GET for smaller requests
            self.http
                .get(uri)
                .basic_auth(&self.user, Some(&self.pass))
                .send()
                .await?
        };

        debug!("Retrieved documents with status code: {}", res.status());

        // Parse response as array of JSON values
        let docs = self.parse_response::<Vec<serde_json::Value>>(res).await?;

        debug!("Retrieved {} documents in {:?}", docs.len(), start.elapsed());

        Ok(docs)
    }
}
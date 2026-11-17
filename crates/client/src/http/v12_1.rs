//! Endpoints and parameters introduced in TerminusDB 12.1.0-rc3:
//!
//! * `GET /api/document/<path>?counted=true` — an index-answered document count.
//! * `GET /api/changes/<path>?count=N` — the ids added/changed/deleted at a
//!   branch head or a specific commit.
//! * `GET /api/schema/<path>` — the class-frame endpoint, including its
//!   `format=jsonschema` mode.

use {
    crate::{
        document::{DocumentFormat, GetOpts},
        spec::BranchSpec,
    },
    ::tracing::{debug, instrument},
    anyhow::Context,
    serde::Deserialize,
};

/// Response to `GET /api/document/<path>?counted=true`.
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentCountResponse {
    /// Always `"api:DocumentCountResponse"`.
    #[serde(rename = "@type")]
    pub response_type: String,
    /// The number of matching documents.
    #[serde(rename = "api:count")]
    pub count: u64,
}

/// Response to `GET /api/changes/<path>`: the document ids touched by a commit.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChangesResponse {
    /// Ids of documents created by the commit.
    #[serde(default)]
    pub added: Vec<String>,
    /// Ids of documents modified by the commit.
    #[serde(default)]
    pub changed: Vec<String>,
    /// Ids of documents deleted by the commit.
    #[serde(default)]
    pub deleted: Vec<String>,
}

impl ChangesResponse {
    /// Whether the commit touched no documents at all.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.deleted.is_empty()
    }

    /// Total number of ids across all three categories.
    pub fn len(&self) -> usize {
        self.added.len() + self.changed.len() + self.deleted.len()
    }
}

/// Options for `GET /api/schema/<path>` (the class-frame endpoint).
#[derive(Debug, Clone)]
pub struct ClassFrameOptions {
    /// Compress ids against the database prefixes (server default `true`).
    pub compress_ids: Option<bool>,
    /// Expand abstract classes into their concrete descendants (server default
    /// `true`).
    pub expand_abstract: Option<bool>,
}

impl Default for ClassFrameOptions {
    fn default() -> Self {
        Self {
            compress_ids: None,
            expand_abstract: None,
        }
    }
}

impl super::client::TerminusDBHttpClient {
    /// Count the documents in a database (or of one class) through the
    /// *document API* with
    /// `counted=true`, which the server answers from the index instead of
    /// materialising the documents.
    ///
    /// `counted` is mutually exclusive with the `id`/`ids`/`query` selectors —
    /// the server rejects the combination with
    /// `api:CountedNotSupportedForSelector` — so only the `type` filter and the
    /// graph selection from `opts` are honoured here.
    ///
    /// (Named `document_count` to keep it apart from
    /// [`count_documents`](Self::count_documents), which counts through
    /// GraphQL's `_count` field.)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example(client: TerminusDBHttpClient, spec: BranchSpec) -> anyhow::Result<()> {
    /// let total = client.document_count(&spec, GetOpts::default()).await?;
    /// let people = client
    ///     .document_count(&spec, GetOpts::default().with_type_filter_string("Person"))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.document.count",
        skip(self),
        fields(db = %spec.db, branch = ?spec.branch, type_filter = ?opts.type_filter),
        err
    )]
    pub async fn document_count(&self, spec: &BranchSpec, opts: GetOpts) -> anyhow::Result<u64> {
        let mut url = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .query("counted", "true");
        if let Some(ref type_filter) = opts.type_filter {
            url = url.query("type", type_filter);
        }
        let uri = url.build();
        debug!("counting documents at {}", &uri);

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to count documents")?;

        let counted = self.parse_response::<DocumentCountResponse>(res).await?;
        Ok(counted.count)
    }

    /// The document ids added, changed and deleted by a commit
    /// (`GET /api/changes/<path>`).
    ///
    /// `spec` may name a branch — in which case the changes of the branch
    /// *head* commit are returned — or pin a commit, for that commit's
    /// changes. `count` caps the number of ids returned *per category*;
    /// `None` means unlimited (the server's `-1` default).
    ///
    /// Requires `meta_read_access` on the resource.
    #[instrument(
        name = "terminus.changes.get",
        skip(self),
        fields(db = %spec.db, branch = ?spec.branch, count = ?count),
        err
    )]
    pub async fn get_changes(
        &self,
        spec: &BranchSpec,
        count: Option<usize>,
    ) -> anyhow::Result<ChangesResponse> {
        let mut url = self.build_url().endpoint("changes").database(spec);
        if let Some(count) = count {
            url = url.query("count", &count.to_string());
        }
        let uri = url.build();
        debug!("fetching changes from {}", &uri);

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get changes")?;

        self.parse_response::<ChangesResponse>(res).await
    }

    /// The class frame for one class, or for every class when `class` is
    /// `None` (`GET /api/schema/<path>`).
    ///
    /// A frame describes a class as the UI/validation layer sees it: its
    /// properties with their ranges and type families, its documentation and
    /// its `@oneOf` choices.
    #[instrument(
        name = "terminus.schema.frame",
        skip(self),
        fields(db = %spec.db, branch = ?spec.branch, class = ?class),
        err
    )]
    pub async fn get_class_frame(
        &self,
        spec: &BranchSpec,
        class: Option<&str>,
        options: ClassFrameOptions,
    ) -> anyhow::Result<serde_json::Value> {
        self.schema_frame_request(spec, class, options, "frame")
            .await
    }

    /// The same classes rendered as JSON Schema (`format=jsonschema`,
    /// TerminusDB 12.1).
    #[instrument(
        name = "terminus.schema.jsonschema",
        skip(self),
        fields(db = %spec.db, branch = ?spec.branch, class = ?class),
        err
    )]
    pub async fn get_json_schema(
        &self,
        spec: &BranchSpec,
        class: Option<&str>,
        options: ClassFrameOptions,
    ) -> anyhow::Result<serde_json::Value> {
        self.schema_frame_request(spec, class, options, "jsonschema")
            .await
    }

    async fn schema_frame_request(
        &self,
        spec: &BranchSpec,
        class: Option<&str>,
        options: ClassFrameOptions,
        format: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let mut url = self
            .build_url()
            .endpoint("schema")
            .database(spec)
            .query("format", format);
        if let Some(class) = class {
            url = url.query("type", class);
        }
        if let Some(compress_ids) = options.compress_ids {
            url = url.query("compress_ids", &compress_ids.to_string());
        }
        if let Some(expand_abstract) = options.expand_abstract {
            url = url.query("expand_abstract", &expand_abstract.to_string());
        }
        let uri = url.build();
        debug!("fetching class frame from {}", &uri);

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get class frame")?;

        self.parse_response::<serde_json::Value>(res).await
    }

    /// Read documents in one of the *text* output formats
    /// (`format=markdown`, `format=embedding`) added in TerminusDB 12.1.
    ///
    /// Unlike `format=json`, these answer a `text/markdown` or `text/plain`
    /// body — one rendered block per document — so they cannot go through the
    /// JSON getters. `id` selects a single document; `None` renders every
    /// document, optionally narrowed by `type_filter`.
    ///
    /// `format=embedding` requires the class to declare `@metadata.embedding`
    /// with a `query` and a Handlebars `template`; without one the server
    /// answers `no_embedding_queries_defined` and this returns an error. Both
    /// formats are instance-graph only.
    #[instrument(
        name = "terminus.document.get_text",
        skip(self),
        fields(db = %spec.db, branch = ?spec.branch, id = ?id, format = format.as_str()),
        err
    )]
    pub async fn get_documents_text(
        &self,
        spec: &BranchSpec,
        id: Option<&str>,
        type_filter: Option<&str>,
        format: DocumentFormat,
    ) -> anyhow::Result<String> {
        let mut url = self
            .build_url()
            .endpoint("document")
            .database(spec)
            .query("format", format.as_str());
        if let Some(id) = id {
            url = url.query("id", id);
        }
        if let Some(type_filter) = type_filter {
            url = url.query("type", type_filter);
        }
        let uri = url.build();
        debug!("reading documents as {} from {}", format.as_str(), &uri);

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to read documents as text")?;

        let status = res.status();
        let body = res
            .text()
            .await
            .context("failed to read the text document response body")?;
        if !status.is_success() {
            anyhow::bail!(
                "{} read failed with status {}: {}",
                format.as_str(),
                status,
                body
            );
        }
        Ok(body)
    }

    /// One document rendered as Markdown (TerminusDB 12.1).
    pub async fn get_document_markdown(
        &self,
        id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        self.get_documents_text(spec, Some(id), None, DocumentFormat::Markdown)
            .await
    }

    /// One document rendered through its class's `@metadata.embedding`
    /// template (TerminusDB 12.1).
    pub async fn get_document_embedding(
        &self,
        id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        self.get_documents_text(spec, Some(id), None, DocumentFormat::Embedding)
            .await
    }

    /// Stream a document's history as newline-delimited JSON
    /// (`streaming=true`, TerminusDB 12.1).
    ///
    /// The non-streaming [`get_document_history`](Self::get_document_history)
    /// builds the whole array server-side before replying; with
    /// `streaming=true` the server answers `application/x-ndjson` chunked and
    /// writes one entry per line as it walks the commit chain, so a long
    /// history can be consumed incrementally.
    ///
    /// All the ordinary history parameters apply, including rc3's `diff`,
    /// `complete`, `before`/`after` and `graph_type`.
    pub async fn get_document_history_streaming(
        &self,
        document_id: &str,
        spec: &BranchSpec,
        params: Option<crate::document::DocumentHistoryParams>,
    ) -> anyhow::Result<impl futures_util::Stream<Item = anyhow::Result<crate::document::CommitHistoryEntry>>>
    {
        use futures_util::StreamExt;

        let params = params.unwrap_or_default();
        let uri = self
            .build_url()
            .endpoint("history")
            .database_with_branch(spec)
            .history_params(document_id, &params)
            .query("streaming", "true")
            .build();
        debug!("streaming document history from {}", &uri);

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to start the streaming history request")?;

        // A failure before streaming begins arrives as an ordinary error body.
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("streaming history failed ({}): {}", status, text);
        }

        let byte_stream = res.bytes_stream();

        Ok(async_stream::try_stream! {
            futures_util::pin_mut!(byte_stream);
            let mut buf: Vec<u8> = Vec::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.context("error reading history stream chunk")?;
                buf.extend_from_slice(&chunk);
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=pos).collect();
                    let line = &line[..line.len() - 1];
                    if line.iter().all(|b| b.is_ascii_whitespace()) {
                        continue;
                    }
                    let entry: crate::document::CommitHistoryEntry =
                        serde_json::from_slice(line)
                            .context("failed to parse a streaming history entry")?;
                    yield entry;
                }
            }

            // A final line without a trailing newline.
            if !buf.iter().all(|b| b.is_ascii_whitespace()) {
                let entry: crate::document::CommitHistoryEntry = serde_json::from_slice(&buf)
                    .context("failed to parse the final streaming history entry")?;
                yield entry;
            }
        })
    }
}

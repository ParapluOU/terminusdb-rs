use crate::TerminusDBModel;
use std::time::Duration;

/// Output format for a document read (`format=` on `GET /api/document`).
///
/// `jsonld`/`rdfxml`/`turtle` exist on the wire but are enterprise-gated, so
/// they are deliberately absent here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DocumentFormat {
    /// The default JSON document representation.
    #[default]
    Json,
    /// Render each document through its class's `@metadata.embedding`
    /// Handlebars `template` (TerminusDB 12.1). The class must define
    /// `@metadata.embedding` with `query` and `template`, or the server
    /// answers `no_embedding_query`. Instance graphs only.
    Embedding,
    /// Render each document as Markdown (TerminusDB 12.1).
    Markdown,
}

impl DocumentFormat {
    /// The wire value for the `format` query parameter.
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentFormat::Json => "json",
            DocumentFormat::Embedding => "embedding",
            DocumentFormat::Markdown => "markdown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GetOpts {
    pub unfold: bool,
    pub as_list: bool,
    /// Skip a certain number of documents (for pagination)
    pub skip: Option<usize>,
    /// Number of documents to retrieve (for pagination)
    pub count: Option<usize>,
    /// Filter documents by type (e.g., "Person")
    pub type_filter: Option<String>,
    /// Minimize the output (defaults to true for efficient data transfer)
    pub minimized: bool,
    /// TerminusDB 12 `raw_json`: read the document as unstructured
    /// `sys:JSONDocument`. Required to GET a document inserted with `raw_json`.
    pub raw_json: bool,
    /// Optional request timeout
    pub timeout: Option<Duration>,
    /// Output format (TerminusDB 12.1). `None` leaves the server default
    /// (`json`).
    pub format: Option<DocumentFormat>,
    /// TerminusDB 12.1 `identifiers` (server default `true`). Setting this to
    /// `false` strips the generated `@id`/`@type` annotations, so a document
    /// reads back in the lexical form it was submitted in.
    pub identifiers: Option<bool>,
    /// TerminusDB 12.1 `id_prefix`: list only the documents whose id starts
    /// with this prefix. Instance graphs only — the server answers
    /// `id_prefix_is_only_supported_for_instance_graphs` otherwise.
    pub id_prefix: Option<String>,
    /// TerminusDB 12.1 `raw_id` (server default `false`): return ids
    /// unexpanded/uncompressed, as stored.
    pub raw_id: Option<bool>,
    /// TerminusDB 12.1 `xsd`: the XSD canonicalisation version used when
    /// rendering literals (server default `"1.1"`). `"1.0"` is enterprise-only
    /// and errors on the community server.
    pub xsd: Option<String>,
}

impl Default for GetOpts {
    fn default() -> Self {
        Self {
            unfold: false,
            as_list: false,
            skip: None,
            count: None,
            type_filter: None,
            minimized: true,
            raw_json: false,
            timeout: None,
            format: None,
            identifiers: None,
            id_prefix: None,
            raw_id: None,
            xsd: None,
        }
    }
}

impl GetOpts {
    /// Create a new GetOpts with pagination settings
    pub fn paginated(skip: usize, count: usize) -> Self {
        Self {
            skip: Some(skip),
            count: Some(count),
            ..Default::default()
        }
    }

    /// Create a new GetOpts with type filtering
    pub fn filtered_by_type<T: TerminusDBModel>() -> Self {
        Self {
            type_filter: Some(T::to_schema().class_name().to_string()),
            ..Default::default()
        }
    }

    /// Set skip for chaining
    pub fn with_skip(mut self, skip: usize) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Set count for chaining
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set type filter for chaining
    pub fn with_type_filter<T: TerminusDBModel>(mut self) -> Self {
        self.type_filter = Some(T::to_schema().class_name().to_string());
        self
    }

    /// Set type filter for chaining using a string (for cases where the type is not known at compile time)
    pub fn with_type_filter_string(mut self, type_name: &str) -> Self {
        self.type_filter = Some(type_name.to_string());
        self
    }

    /// Set unfold for chaining
    pub fn with_unfold(mut self, unfold: bool) -> Self {
        self.unfold = unfold;
        self
    }

    /// Set as_list for chaining
    pub fn with_as_list(mut self, as_list: bool) -> Self {
        self.as_list = as_list;
        self
    }

    /// Set raw_json for chaining (read as unstructured sys:JSONDocument).
    pub fn with_raw_json(mut self, raw_json: bool) -> Self {
        self.raw_json = raw_json;
        self
    }

    /// Set minimized for chaining
    pub fn with_minimized(mut self, minimized: bool) -> Self {
        self.minimized = minimized;
        self
    }

    /// Set timeout for chaining
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the output format (TerminusDB 12.1).
    ///
    /// Note that `Markdown` and `Embedding` make the server answer a *text*
    /// body, which the JSON getters cannot parse — use
    /// `TerminusDBHttpClient::get_documents_text` (or its `get_document_markdown`
    /// / `get_document_embedding` shorthands) for those.
    pub fn with_format(mut self, format: DocumentFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Set `identifiers` (TerminusDB 12.1); `false` strips `@id`/`@type`.
    pub fn with_identifiers(mut self, identifiers: bool) -> Self {
        self.identifiers = Some(identifiers);
        self
    }

    /// Set `id_prefix` (TerminusDB 12.1): list only ids under this prefix.
    pub fn with_id_prefix(mut self, id_prefix: impl Into<String>) -> Self {
        self.id_prefix = Some(id_prefix.into());
        self
    }

    /// Set `raw_id` (TerminusDB 12.1): return ids as stored.
    pub fn with_raw_id(mut self, raw_id: bool) -> Self {
        self.raw_id = Some(raw_id);
        self
    }

    /// Set the XSD canonicalisation version (TerminusDB 12.1).
    pub fn with_xsd(mut self, xsd: impl Into<String>) -> Self {
        self.xsd = Some(xsd.into());
        self
    }

    /// Add the optional TerminusDB 12.1 read parameters to a document-read
    /// request *body*.
    ///
    /// The document reads that carry their selector in a JSON body (so that
    /// url-encoded lexical/hash ids survive) must pass these the same way —
    /// the server reads each parameter from the query string *or* the body, and
    /// a body request has no query string. The query-string equivalent is
    /// `UrlBuilder::document_get_optional_params`.
    pub fn apply_to_body(&self, body: &mut serde_json::Map<String, serde_json::Value>) {
        if self.raw_json {
            body.insert("raw_json".to_string(), serde_json::Value::Bool(true));
        }
        if let Some(identifiers) = self.identifiers {
            body.insert(
                "identifiers".to_string(),
                serde_json::Value::Bool(identifiers),
            );
        }
        if let Some(ref id_prefix) = self.id_prefix {
            body.insert(
                "id_prefix".to_string(),
                serde_json::Value::String(id_prefix.clone()),
            );
        }
        if let Some(raw_id) = self.raw_id {
            body.insert("raw_id".to_string(), serde_json::Value::Bool(raw_id));
        }
        if let Some(ref xsd) = self.xsd {
            body.insert("xsd".to_string(), serde_json::Value::String(xsd.clone()));
        }
    }
}

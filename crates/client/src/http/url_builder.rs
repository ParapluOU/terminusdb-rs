//! URL building utilities for TerminusDB API endpoints

use url::Url;

/// Centralized URL builder for TerminusDB API endpoints.
/// Eliminates duplication and provides consistent URL construction.
#[derive(Debug)]
pub struct UrlBuilder<'a> {
    endpoint: &'a Url,
    org: &'a str,
    parts: Vec<String>,
    query_params: Vec<(String, String)>,
}

impl<'a> UrlBuilder<'a> {
    pub fn new(endpoint: &'a Url, org: &'a str) -> Self {
        Self {
            endpoint,
            org,
            parts: Vec::new(),
            query_params: Vec::new(),
        }
    }

    /// Add an API endpoint type (db, document, woql, log, etc.)
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.parts.push(endpoint.to_string());
        self
    }

    /// Add a database path (handles commit, branch, and default paths)
    pub fn database(mut self, spec: &crate::spec::BranchSpec) -> Self {
        if let Some(commit_id) = spec.commit_id() {
            self.parts.push(format!(
                "{}/{}/local/commit/{}",
                self.org, spec.db, commit_id
            ));
        } else if let Some(branch) = &spec.branch {
            // Include branch in path when explicitly specified
            self.parts
                .push(format!("{}/{}/local/branch/{}", self.org, spec.db, branch));
        } else {
            // Default: just org/db (uses main branch)
            self.parts.push(format!("{}/{}", self.org, spec.db));
        }
        self
    }

    /// Add a database path for history endpoint which includes branch information
    pub fn database_with_branch(mut self, spec: &crate::spec::BranchSpec) -> Self {
        let branch = spec.branch.as_deref().unwrap_or("main");
        self.parts
            .push(format!("{}/{}/local/branch/{}", self.org, spec.db, branch));
        self
    }

    /// Add a simple database path for management operations
    pub fn simple_database(mut self, db: &str) -> Self {
        self.parts.push(format!("{}/{}", self.org, db));
        self
    }

    /// Add a query parameter
    pub fn query(mut self, key: &str, value: &str) -> Self {
        self.query_params.push((key.to_string(), value.to_string()));
        self
    }

    /// Add a query parameter with URL encoding
    pub fn query_encoded(mut self, key: &str, value: &str) -> Self {
        self.query_params
            .push((key.to_string(), urlencoding::encode(value).to_string()));
        self
    }

    /// Add multiple common document query parameters
    pub fn document_params(
        mut self,
        author: &str,
        message: &str,
        graph_type: &str,
        create: bool,
    ) -> Self {
        self.query_params.extend([
            ("author".to_string(), author.to_string()),
            (
                "message".to_string(),
                urlencoding::encode(message).to_string(),
            ),
            ("graph_type".to_string(), graph_type.to_string()),
            ("create".to_string(), create.to_string()),
        ]);
        self
    }

    /// Add document retrieval query parameters for single document
    pub fn document_get_params(
        mut self,
        id: &str,
        unfold: bool,
        as_list: bool,
        minimized: bool,
    ) -> Self {
        self.query_params.extend([
            ("id".to_string(), id.to_string()),
            ("unfold".to_string(), unfold.to_string()),
            ("as_list".to_string(), as_list.to_string()),
            ("minimized".to_string(), minimized.to_string()),
        ]);
        self
    }

    /// Append the document output `format` (TerminusDB 12.1), which the server
    /// only reads from the query string or the `Accept` header — never from a
    /// request body. Needed even on the body-POST read paths.
    pub fn document_format_param(mut self, opts: &crate::document::GetOpts) -> Self {
        if let Some(format) = opts.format {
            self.query_params
                .push(("format".to_string(), format.as_str().to_string()));
        }
        self
    }

    /// Append the optional TerminusDB 12.1 document-read parameters that are
    /// only sent when explicitly set (`raw_json`, `format`, `identifiers`,
    /// `id_prefix`, `raw_id`, `xsd`). Shared by every document GET path so the
    /// single- and multi-document reads cannot drift apart.
    pub fn document_get_optional_params(mut self, opts: &crate::document::GetOpts) -> Self {
        if opts.raw_json {
            self.query_params
                .push(("raw_json".to_string(), "true".to_string()));
        }
        if let Some(format) = opts.format {
            self.query_params
                .push(("format".to_string(), format.as_str().to_string()));
        }
        if let Some(identifiers) = opts.identifiers {
            self.query_params
                .push(("identifiers".to_string(), identifiers.to_string()));
        }
        if let Some(ref id_prefix) = opts.id_prefix {
            self.query_params
                .push(("id_prefix".to_string(), id_prefix.clone()));
        }
        if let Some(raw_id) = opts.raw_id {
            self.query_params
                .push(("raw_id".to_string(), raw_id.to_string()));
        }
        if let Some(ref xsd) = opts.xsd {
            self.query_params.push(("xsd".to_string(), xsd.clone()));
        }
        self
    }

    /// Add document retrieval query parameters for multiple documents
    pub fn document_get_multiple_params(
        mut self,
        ids: &[String],
        opts: &crate::document::GetOpts,
    ) -> Self {
        // Always set as_list to true for multiple documents to get proper JSON array response
        self.query_params
            .push(("as_list".to_string(), "true".to_string()));
        self.query_params
            .push(("unfold".to_string(), opts.unfold.to_string()));
        self.query_params
            .push(("minimized".to_string(), opts.minimized.to_string()));

        // Add the ids as a JSON array
        if !ids.is_empty() {
            let ids_json = serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string());
            self.query_params.push(("ids".to_string(), ids_json));
        }

        // Add pagination parameters
        if let Some(skip) = opts.skip {
            self.query_params
                .push(("skip".to_string(), skip.to_string()));
        }
        if let Some(count) = opts.count {
            self.query_params
                .push(("count".to_string(), count.to_string()));
        }

        // Add type filter
        if let Some(ref type_filter) = opts.type_filter {
            self.query_params
                .push(("type".to_string(), type_filter.clone()));
        }

        self
    }

    /// Add log query parameters
    pub fn log_params(mut self, start: usize, count: usize, verbose: bool) -> Self {
        self.query_params.extend([
            ("start".to_string(), start.to_string()),
            ("count".to_string(), count.to_string()),
            ("verbose".to_string(), verbose.to_string()),
        ]);
        self
    }

    /// Add history query parameters
    pub fn history_params(
        mut self,
        doc_id: &str,
        params: &crate::document::DocumentHistoryParams,
    ) -> Self {
        self.query_params
            .push(("id".to_string(), doc_id.to_string()));

        if let Some(start) = params.start {
            self.query_params
                .push(("start".to_string(), start.to_string()));
        }
        if let Some(count) = params.count {
            self.query_params
                .push(("count".to_string(), count.to_string()));
        }
        if let Some(updated) = params.updated {
            self.query_params
                .push(("updated".to_string(), updated.to_string()));
        }
        if let Some(created) = params.created {
            self.query_params
                .push(("created".to_string(), created.to_string()));
        }

        self
    }

    /// Add delete document query parameters
    pub fn document_delete_params(
        mut self,
        author: &str,
        message: &str,
        graph_type: &str,
        delete_opts: &crate::http::document::DeleteOpts,
        id: Option<&str>,
    ) -> Self {
        self.query_params.extend([
            ("author".to_string(), author.to_string()),
            (
                "message".to_string(),
                urlencoding::encode(message).to_string(),
            ),
            ("graph_type".to_string(), graph_type.to_string()),
            ("nuke".to_string(), delete_opts.is_nuke().to_string()),
        ]);

        if let Some(doc_id) = id {
            self.query_params
                .push(("id".to_string(), doc_id.to_string()));
        }

        self
    }

    /// Add a generic path segment (for endpoints like squash, reset, etc.)
    pub fn add_path(mut self, path: &str) -> Self {
        self.parts.push(path.to_string());
        self
    }

    /// Build the final URL string
    pub fn build(self) -> String {
        let mut url = format!("{}/{}", self.endpoint, self.parts.join("/"));

        if !self.query_params.is_empty() {
            let query_string = self
                .query_params
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query_string);
        }

        url
    }
}

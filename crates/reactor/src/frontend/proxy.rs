//! Byte-faithful passthrough.
//!
//! Two rules, both absolute:
//!
//! 1. **Bodies are bytes.** Nothing here parses, re-serializes or even looks at
//!    a body. v12 returns numbers at full rational precision and this workspace
//!    enables `serde_json/arbitrary_precision`; a round trip through `Value`
//!    would reorder keys and could lose digits. Invariant 1.
//! 2. **The client's `Authorization` is forwarded unchanged.** Reactor has its
//!    own credentials for capture and fills, and they must never be substituted
//!    for a client's — that would turn the proxy into an authorization bypass by
//!    construction, before any caching question even arises.

use bytes::Bytes;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, StatusCode};
use url::Url;

/// Headers that describe a single hop and must not be relayed.
///
/// Relaying `Transfer-Encoding` or `Content-Length` from a response we have
/// buffered (or re-chunked) is the classic proxy bug: the framing no longer
/// matches the bytes. `Connection` and friends are per-connection by
/// definition, and `Proxy-*` belong to the hop that generated them.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "content-length",
];

/// Remove hop-by-hop headers in place.
pub fn strip_hop_by_hop(headers: &mut HeaderMap) {
    // `Connection: x, y` nominates *other* headers as hop-by-hop. Read that
    // list BEFORE removing anything: `Connection` is itself hop-by-hop, so
    // stripping it first would discard the list and leak the headers the peer
    // meant to keep local to its hop.
    let nominated: Vec<HeaderName> = headers
        .get_all(http::header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|token| HeaderName::try_from(token.trim()).ok())
        .collect();
    for name in HOP_BY_HOP {
        headers.remove(*name);
    }
    for name in nominated {
        headers.remove(name);
    }
}

/// A request to send upstream.
#[derive(Clone, Debug)]
pub struct UpstreamRequest {
    /// The method.
    pub method: Method,
    /// Path with query string, as received.
    pub path_and_query: String,
    /// Headers, already stripped of hop-by-hop entries.
    pub headers: HeaderMap,
    /// The body.
    pub body: Bytes,
}

/// A buffered upstream response.
#[derive(Clone, Debug)]
pub struct UpstreamResponse {
    /// The status.
    pub status: StatusCode,
    /// Headers, already stripped.
    pub headers: HeaderMap,
    /// The body, verbatim.
    pub body: Bytes,
}

impl UpstreamResponse {
    /// The `TerminusDB-Data-Version` header, if present.
    pub fn data_version(&self) -> Option<String> {
        self.headers
            .get("terminusdb-data-version")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    /// The commit id inside `TerminusDB-Data-Version`.
    ///
    /// The header's format is `"branch:<commit>"`, so the commit is the part
    /// after the colon. A header without one yields `None` rather than the whole
    /// string: a bogus commit id would be worse than no commit id.
    pub fn commit_id(&self) -> Option<terminusdb_client::CommitId> {
        parse_data_version(self.data_version()?.as_str())
    }

    /// True for a 2xx status.
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// The `Content-Type`, if any.
    pub fn content_type(&self) -> Option<String> {
        self.headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }
}

/// Parse `"branch:<commit>"` into a commit id.
pub fn parse_data_version(value: &str) -> Option<terminusdb_client::CommitId> {
    let commit = value.rsplit(':').next()?.trim();
    if commit.is_empty() {
        return None;
    }
    Some(terminusdb_client::CommitId::new(commit))
}

/// The upstream HTTP client.
pub struct Proxy {
    http: reqwest::Client,
    upstream: Url,
}

impl Proxy {
    /// Build a proxy to an upstream base URL.
    pub fn new(upstream: Url, timeout: std::time::Duration) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            // No automatic decompression: we relay bytes, and a body we
            // transparently inflated would no longer match the
            // `Content-Encoding` we relay with it.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .build()?;
        Ok(Self { http, upstream })
    }

    /// The upstream base URL.
    pub fn upstream(&self) -> &Url {
        &self.upstream
    }

    /// Absolute upstream URL for a path-and-query.
    pub fn url_for(&self, path_and_query: &str) -> String {
        format!(
            "{}{}",
            self.upstream.as_str().trim_end_matches('/'),
            path_and_query
        )
    }

    /// Send a request and buffer the response.
    pub async fn send(&self, request: &UpstreamRequest) -> anyhow::Result<UpstreamResponse> {
        let response = self.start(request).await?;
        let status = response.status();
        let mut headers = response.headers().clone();
        strip_hop_by_hop(&mut headers);
        let body = response.bytes().await?;
        Ok(UpstreamResponse {
            status,
            headers,
            body,
        })
    }

    /// Send a request and return the streaming response, for bodies that must
    /// not be buffered — an SSE subscription never ends, and an NDJSON document
    /// list can be larger than memory.
    pub async fn start(&self, request: &UpstreamRequest) -> anyhow::Result<reqwest::Response> {
        let url = self.url_for(&request.path_and_query);
        let mut headers = request.headers.clone();
        strip_hop_by_hop(&mut headers);
        // The upstream host is ours, not the client's.
        headers.remove(http::header::HOST);
        Ok(self
            .http
            .request(request.method.clone(), url)
            .headers(headers)
            .body(request.body.clone())
            .send()
            .await?)
    }
}

/// Copy upstream response headers onto an outgoing response.
pub fn copy_response_headers(from: &HeaderMap, to: &mut HeaderMap) {
    for (name, value) in from {
        if HOP_BY_HOP.contains(&name.as_str()) {
            continue;
        }
        to.append(name.clone(), value.clone());
    }
}

/// Render a `HeaderValue` from a string, dropping anything unrepresentable
/// rather than failing the request.
pub fn header_value(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_of(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(
                HeaderName::try_from(*name).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn hop_by_hop_headers_are_stripped() {
        let mut headers = headers_of(&[
            ("connection", "close"),
            ("keep-alive", "timeout=5"),
            ("transfer-encoding", "chunked"),
            ("content-length", "17"),
            ("upgrade", "websocket"),
            ("te", "trailers"),
            ("trailer", "expires"),
            ("proxy-authorization", "Basic xyz"),
            ("proxy-connection", "keep-alive"),
            ("authorization", "Basic abc"),
            ("content-type", "application/json"),
        ]);
        strip_hop_by_hop(&mut headers);

        for gone in [
            "connection",
            "keep-alive",
            "transfer-encoding",
            "content-length",
            "upgrade",
            "te",
            "trailer",
            "proxy-authorization",
            "proxy-connection",
        ] {
            assert!(headers.get(gone).is_none(), "{gone} should be stripped");
        }
        assert_eq!(
            headers.get("authorization").unwrap(),
            "Basic abc",
            "the client's credentials are forwarded unchanged"
        );
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
    }

    #[test]
    fn headers_nominated_by_connection_are_also_stripped() {
        let mut headers = headers_of(&[
            ("connection", "x-custom-hop, keep-alive"),
            ("x-custom-hop", "secret"),
            ("x-keep", "fine"),
        ]);
        strip_hop_by_hop(&mut headers);
        assert!(headers.get("x-custom-hop").is_none());
        assert!(headers.get("connection").is_none());
        assert_eq!(headers.get("x-keep").unwrap(), "fine");
    }

    #[test]
    fn stripping_an_empty_map_is_fine() {
        let mut headers = HeaderMap::new();
        strip_hop_by_hop(&mut headers);
        assert!(headers.is_empty());
    }

    #[test]
    fn copying_response_headers_skips_hop_by_hop() {
        let from = headers_of(&[
            ("content-type", "application/json"),
            ("transfer-encoding", "chunked"),
            ("terminusdb-data-version", "branch:abc123"),
        ]);
        let mut to = HeaderMap::new();
        copy_response_headers(&from, &mut to);
        assert_eq!(to.get("content-type").unwrap(), "application/json");
        assert_eq!(to.get("terminusdb-data-version").unwrap(), "branch:abc123");
        assert!(to.get("transfer-encoding").is_none());
    }

    #[test]
    fn a_data_version_yields_its_commit() {
        assert_eq!(
            parse_data_version("branch:abc123").map(|c| c.as_str().to_owned()),
            Some("abc123".to_owned())
        );
        // No prefix: the whole value is the commit.
        assert_eq!(
            parse_data_version("abc123").map(|c| c.as_str().to_owned()),
            Some("abc123".to_owned())
        );
        // Several colons: the last segment wins.
        assert_eq!(
            parse_data_version("repo:branch:abc123").map(|c| c.as_str().to_owned()),
            Some("abc123".to_owned())
        );
    }

    #[test]
    fn an_empty_data_version_yields_nothing_rather_than_a_bogus_commit() {
        assert!(parse_data_version("").is_none());
        assert!(parse_data_version("branch:").is_none());
        assert!(parse_data_version("   ").is_none());
    }

    #[test]
    fn a_response_exposes_its_commit_and_content_type() {
        let response = UpstreamResponse {
            status: StatusCode::OK,
            headers: headers_of(&[
                ("terminusdb-data-version", "branch:deadbeef"),
                ("content-type", "application/json"),
            ]),
            body: Bytes::from_static(b"{}"),
        };
        assert!(response.is_success());
        assert_eq!(
            response.commit_id().map(|c| c.as_str().to_owned()),
            Some("deadbeef".to_owned())
        );
        assert_eq!(response.content_type().as_deref(), Some("application/json"));
        assert_eq!(response.data_version().as_deref(), Some("branch:deadbeef"));
    }

    #[test]
    fn a_response_without_a_data_version_has_no_commit() {
        let response = UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::new(),
        };
        assert!(response.commit_id().is_none());
        assert!(response.data_version().is_none());
    }

    #[test]
    fn upstream_urls_are_joined_without_doubling_slashes() {
        let proxy = Proxy::new(
            Url::parse("http://localhost:6363/").unwrap(),
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(
            proxy.url_for("/api/document/admin/db?id=x"),
            "http://localhost:6363/api/document/admin/db?id=x"
        );
    }
}

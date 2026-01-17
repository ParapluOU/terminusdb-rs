//! Response parsing utilities for the HTTP client

#[cfg(not(target_arch = "wasm32"))]
use reqwest::Response;

use {
    crate::{result::ResponseWithHeaders, ApiResponse},
    ::tracing::{instrument, trace},
    anyhow::{anyhow, Context},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    serde_json::{json, Value},
    std::fmt::Debug,
};

/// Extract JSON from a malformed response that contains embedded HTTP headers.
/// HTTP headers are separated from body by a blank line, so we look for
/// `\n\n{` or `\n\n[` patterns.
fn extract_json_from_malformed_response(text: &str) -> Option<&str> {
    // Look for blank line followed by JSON start (Unix-style \n\n)
    if let Some(pos) = text.find("\n\n{") {
        return Some(&text[pos + 2..]);
    }
    if let Some(pos) = text.find("\n\n[") {
        return Some(&text[pos + 2..]);
    }
    // Also handle \r\n\r\n (Windows-style)
    if let Some(pos) = text.find("\r\n\r\n{") {
        return Some(&text[pos + 4..]);
    }
    if let Some(pos) = text.find("\r\n\r\n[") {
        return Some(&text[pos + 4..]);
    }
    None
}

/// Check if response text is the TerminusDB "Still Loading" HTML page.
///
/// This page is returned when the server is still starting up and
/// synchronizing its backing store. It's not an error but a transient state.
fn is_server_loading_html(text: &str) -> bool {
    text.trim_start().starts_with("<!DOCTYPE html>") && text.contains("Still loading")
}

/// Response parsing methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.response.parse",
        skip(self, res),
        fields(
            response_type = std::any::type_name::<T>()
        ),
        err
    )]
    pub(crate) async fn parse_response<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<T> {
        use crate::err::ServerNotReadyError;

        // let full = res.bytes().await?;

        // let json = serde_json::from_slice::<serde_json::Value>(&full)
        //     .context("failed to parse response as JSON")
        //     .tap_err(|e| {
        //         tracing::error!("failed to parse response bytes as JSON ({:?}): {:?}", e, full);
        //     })?;

        let full = res.text().await.context("failed to parse response text")?;

        // Detect TerminusDB "Still Loading" page - server is starting up
        if is_server_loading_html(&full) {
            tracing::debug!("Server is still loading (synchronizing backing store)");
            return Err(ServerNotReadyError.into());
        }

        // Handle malformed responses where HTTP headers are embedded in the body.
        // When normal parsing fails, try to extract JSON after a blank line (header/body separator).
        // Note: #[instrument(..., err)] already logs errors, so no explicit error! call needed.
        let json = serde_json::from_str::<serde_json::Value>(&full)
            .or_else(|original_err| {
                extract_json_from_malformed_response(&full)
                    .and_then(|json_str| serde_json::from_str(json_str).ok())
                    .ok_or(original_err)
            })
            .context("failed to parse response as JSON")?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        // eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        // eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(r)
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.response.parse_with_headers",
        skip(self, res),
        fields(
            response_type = std::any::type_name::<T>()
        ),
        err
    )]
    pub(crate) async fn parse_response_with_headers<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<ResponseWithHeaders<T>> {
        // Extract the TerminusDB-Data-Version header before consuming the response
        let terminusdb_data_version = res
            .headers()
            .get("TerminusDB-Data-Version")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        trace!(
            "[TerminusDBHttpClient] TerminusDB-Data-Version header: {:?}",
            terminusdb_data_version
        );

        let full = res.text().await.context("failed to parse response text")?;

        // Detect TerminusDB "Still Loading" page - server is starting up
        if is_server_loading_html(&full) {
            tracing::debug!("Server is still loading (synchronizing backing store)");
            return Err(crate::err::ServerNotReadyError.into());
        }

        // Handle malformed responses where HTTP headers are embedded in the body.
        // When normal parsing fails, try to extract JSON after a blank line (header/body separator).
        let json = serde_json::from_str::<serde_json::Value>(&full)
            .or_else(|original_err| {
                extract_json_from_malformed_response(&full)
                    .and_then(|json_str| serde_json::from_str(json_str).ok())
                    .ok_or(original_err)
            })
            .context("failed to parse response as JSON")?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        // eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        // eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(ResponseWithHeaders::new_with_string(
                    r,
                    terminusdb_data_version,
                ))
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    /// Parse response without logging errors.
    ///
    /// Use this for health checks where errors are expected during startup.
    /// Same as `parse_response()` but without the `err` attribute on instrument
    /// and without explicit error logging.
    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.response.try_parse",
        skip(self, res),
        fields(
            response_type = std::any::type_name::<T>()
        )
    )]
    pub(crate) async fn try_parse_response<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<T> {
        use crate::err::ServerNotReadyError;

        let full = res.text().await.context("failed to parse response text")?;

        // Detect TerminusDB "Still Loading" page - server is starting up
        if is_server_loading_html(&full) {
            tracing::debug!("Server is still loading (synchronizing backing store)");
            return Err(ServerNotReadyError.into());
        }

        // Handle malformed responses where HTTP headers are embedded in the body.
        let json = serde_json::from_str::<serde_json::Value>(&full)
            .or_else(|original_err| {
                extract_json_from_malformed_response(&full)
                    .and_then(|json_str| serde_json::from_str(json_str).ok())
                    .ok_or(original_err)
            })
            .context("failed to parse response as JSON")?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(r)
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_malformed_response_with_headers() {
        let malformed = r#"Status: 404
Content-type: application/json; charset=UTF-8

{"@type":"api:GetDocumentErrorResponse","api:error":{"@type":"api:DocumentNotFound","api:document_id":"AwsDBUser/system"},"api:message":"Document not found: 'AwsDBUser/system'","api:request_id":"6a758206-ce9a-11f0-b5f2-121e63fbbd57","api:status":"api:not_found"}"#;

        let extracted = extract_json_from_malformed_response(malformed);
        assert!(extracted.is_some());

        let json_str = extracted.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();

        assert_eq!(parsed["@type"], "api:GetDocumentErrorResponse");
        assert_eq!(parsed["api:error"]["@type"], "api:DocumentNotFound");
        assert_eq!(
            parsed["api:error"]["api:document_id"],
            "AwsDBUser/system"
        );
    }

    #[test]
    fn test_extract_json_from_malformed_response_array() {
        let malformed = "Status: 200\nContent-type: application/json\n\n[{\"id\": 1}, {\"id\": 2}]";

        let extracted = extract_json_from_malformed_response(malformed);
        assert!(extracted.is_some());

        let json_str = extracted.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed[0]["id"], 1);
    }

    #[test]
    fn test_extract_json_from_malformed_response_windows_line_endings() {
        let malformed = "Status: 404\r\nContent-type: application/json\r\n\r\n{\"error\": true}";

        let extracted = extract_json_from_malformed_response(malformed);
        assert!(extracted.is_some());

        let json_str = extracted.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();

        assert_eq!(parsed["error"], true);
    }

    #[test]
    fn test_extract_json_from_valid_json_returns_none() {
        // Valid JSON without headers should return None (no extraction needed)
        let valid_json = r#"{"@type":"api:Response","api:status":"ok"}"#;

        let extracted = extract_json_from_malformed_response(valid_json);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_extract_json_from_garbage_returns_none() {
        let garbage = "this is not json and has no blank line separator";

        let extracted = extract_json_from_malformed_response(garbage);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_is_server_loading_html_detects_loading_page() {
        let loading_html = r#"<!DOCTYPE html>
<html>
<head>
<title>Still Loading</title>

<meta http-equiv="content-type" content="text/html; charset=UTF-8">

</head>
<body>

<h1>Still loading</h1>

<p>
TerminusDB is still synchronizing backing store</p>
</body>
</html>"#;

        assert!(is_server_loading_html(loading_html));
    }

    #[test]
    fn test_is_server_loading_html_rejects_json() {
        let json = r#"{"@type":"api:Response","api:status":"ok"}"#;
        assert!(!is_server_loading_html(json));
    }

    #[test]
    fn test_is_server_loading_html_rejects_other_html() {
        // HTML that isn't the loading page
        let other_html = "<!DOCTYPE html><html><body>Not loading</body></html>";
        assert!(!is_server_loading_html(other_html));
    }
}

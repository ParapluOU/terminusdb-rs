//! Response parsing utilities for the HTTP client

#[cfg(not(target_arch = "wasm32"))]
use {
    reqwest::Response,
};

use {
    crate::{ApiResponse, result::ResponseWithHeaders},
    ::log::trace,
    anyhow::{anyhow, Context},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    serde_json::{json, Value},
    std::fmt::Debug,
};

/// Response parsing methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) async fn parse_response<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<T> {
        let json = res.json::<serde_json::Value>().await?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(r)
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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

        trace!("[TerminusDBHttpClient] TerminusDB-Data-Version header: {:?}", terminusdb_data_version);

        let json = res.json::<serde_json::Value>().await?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(ResponseWithHeaders::new(r, terminusdb_data_version))
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }
}
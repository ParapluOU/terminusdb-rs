//! Advanced authentication for `TerminusDBHttpClient`.
//!
//! Additive design: the client defaults to Basic auth (existing behavior);
//! `with_bearer` / `with_api_key` override the `Authorization` header via the
//! `AuthApply` extension trait, which all request paths funnel through.
//!
//! Grounding: the DFRNT hub uses Personal Access Tokens
//! (TERMINUSDB_ACCESS_TOKEN) — see docs/terminusdb/get-your-api-key.md.

use reqwest::RequestBuilder;

use super::client::TerminusDBHttpClient;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthMethod {
    Basic { user: String, pass: String },
    Bearer(String),
    ApiKey(String),
}

/// Extension trait applying the client's auth method to a request builder.
/// `Basic` (or no override) reproduces the historical
/// `.basic_auth(user, Some(pass))` behavior exactly.
pub(crate) trait AuthApply {
    fn apply_auth(self, client: &TerminusDBHttpClient) -> RequestBuilder;
}

impl AuthApply for RequestBuilder {
    fn apply_auth(self, client: &TerminusDBHttpClient) -> RequestBuilder {
        match client.auth_override() {
            Some(AuthMethod::Bearer(token)) => self.bearer_auth(token),
            Some(AuthMethod::ApiKey(key)) => self.header("Authorization", format!("Apikey {key}")),
            Some(AuthMethod::Basic { user, pass }) => self.basic_auth(user, Some(pass)),
            None => self.basic_auth(&client.user, Some(&client.pass)),
        }
    }
}

// Advanced Authentication (Bearer/API-key) — unit tests.
// Additive design: Basic stays the default; with_bearer/with_api_key override
// the Authorization header.

use crate::http::auth::AuthApply;
use crate::http::client::TerminusDBHttpClient;

#[tokio::test]
async fn default_client_uses_basic_auth() {
    let client = TerminusDBHttpClient::new(
        url::Url::parse("http://localhost:6363").unwrap(),
        "admin",
        "root",
        "admin",
    )
    .await
    .unwrap();

    let request = client
        .http
        .get("http://localhost:6363/api/info")
        .apply_auth(&client)
        .build()
        .unwrap();
    let auth = request.headers().get("authorization").unwrap();
    assert_eq!(auth, "Basic YWRtaW46cm9vdA==");
}

#[tokio::test]
async fn bearer_override_sets_authorization_header() {
    let client = TerminusDBHttpClient::new(
        url::Url::parse("http://localhost:6363").unwrap(),
        "admin",
        "root",
        "admin",
    )
    .await
    .unwrap()
    .with_bearer("pat-123");

    let request = client
        .http
        .get("http://localhost:6363/api/info")
        .apply_auth(&client)
        .build()
        .unwrap();
    let auth = request.headers().get("authorization").unwrap();
    assert_eq!(auth, "Bearer pat-123");
}

#[tokio::test]
async fn api_key_override_sets_authorization_header() {
    let client = TerminusDBHttpClient::new(
        url::Url::parse("http://localhost:6363").unwrap(),
        "admin",
        "root",
        "admin",
    )
    .await
    .unwrap()
    .with_api_key("key-456");

    let request = client
        .http
        .get("http://localhost:6363/api/info")
        .apply_auth(&client)
        .build()
        .unwrap();
    let auth = request.headers().get("authorization").unwrap();
    assert_eq!(auth, "Apikey key-456");
}

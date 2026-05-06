//! Pure JSON-to-SDL renderer for GraphQL introspection responses.
//!
//! Wraps cynic-introspection's parse + `to_sdl()` pipeline so callers
//! don't have to depend on cynic directly — and so anything that already
//! holds an introspection envelope (built-time dump, runtime startup
//! introspection, mocked test data) can render to SDL without booting a
//! TerminusDB server. Boot-a-server helpers live behind the `live`
//! feature; this renderer is always available.

use serde_json::Value;

/// Convert a raw `__Schema` introspection envelope (the JSON `data`
/// field returned by `TerminusDBHttpClient::introspect_schema`) into
/// SDL text.
pub fn render_introspection_to_sdl(json: &Value) -> anyhow::Result<String> {
    let response: cynic_introspection::IntrospectionQuery =
        serde_json::from_value(json.clone())?;
    let schema = response.into_schema()?;
    Ok(schema.to_sdl())
}

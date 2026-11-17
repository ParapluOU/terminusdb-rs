//! GraphQL subscriptions over Server-Sent Events (TerminusDB 12.1).
//!
//! TerminusDB 12.1 implements the
//! [graphql-sse](https://github.com/enisdenjo/graphql-sse/blob/master/PROTOCOL.md)
//! protocol in *distinct connections* mode: a subscription is a `POST` to the
//! ordinary GraphQL endpoint with `Accept: text/event-stream`, and the server
//! holds the connection open and writes SSE events to it as commits land.
//!
//! The event sequence is a TerminusDB-specific `connected` event (so a client
//! can know when it is safe to make the change it wants to observe — otherwise
//! subscribing races the mutation), then a `next` event per matching commit,
//! then `complete`.
//!
//! Subscription fields are generated from the schema: `<Type>_added`,
//! `<Type>_updated` and `<Type>_deleted`, each accepting a `filter` argument
//! (parsed and schema-checked, but **not** applied by the community server in
//! 12.1.0-rc3 — see `register_subscription_parsed/8`, whose cohort key ignores
//! the filter — so a subscriber must still filter client-side), plus
//! `_ChangeSet`, which fires **once per commit** with list-valued
//! `<Class>_added`/`_changed`/`_deleted` fields and takes `include_children`
//! (default `true`, overridable per field). `_CommitMetadata { _id _timestamp
//! _datetime _change_type }` is available in both forms.
//!
//! This is unrelated to — and supersedes — the deprecated
//! [`sse_manager`](super::sse_manager) plugin client.

use {
    crate::{http::graphql::GraphQLError, GraphQLRequest},
    ::tracing::{debug, instrument},
    anyhow::Context,
    serde_json::Value,
};

/// One event from a GraphQL subscription stream.
///
/// `PartialEq` compares the error *messages* of an `Errors` event, which is
/// what tests care about (`GraphQLError` itself is not `PartialEq`).
#[derive(Debug, Clone)]
pub enum SubscriptionEvent {
    /// The server accepted the subscription and is now listening. A
    /// TerminusDB extension to graphql-sse, sent once before any `next`.
    ///
    /// Wait for this before making the change you want to observe; without it
    /// the subscription registration races the mutation.
    Connected,
    /// A subscription result, as the server sent it.
    ///
    /// rc3 wraps the selection in the usual GraphQL `data` envelope —
    /// `{"data": {"Person_added": {"_id": "Person/alice", ...}}}` — even though
    /// the protocol reference shows the selection unwrapped. Use
    /// [`SubscriptionEvent::payload`], which unwraps `data` when it is present,
    /// rather than reaching into the envelope yourself.
    Next(Value),
    /// A `next` event that carried GraphQL errors rather than data — how
    /// graphql-sse reports a validation failure (the HTTP status is still
    /// 200). A `Complete` follows.
    Errors(Vec<GraphQLError>),
    /// The stream is finished; the server closes the connection after this.
    Complete,
}

impl PartialEq for SubscriptionEvent {
    fn eq(&self, other: &Self) -> bool {
        use SubscriptionEvent::*;
        match (self, other) {
            (Connected, Connected) | (Complete, Complete) => true,
            (Next(a), Next(b)) => a == b,
            (Errors(a), Errors(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(x, y)| x.message == y.message)
            }
            _ => false,
        }
    }
}

impl SubscriptionEvent {
    /// The GraphQL data payload of a `next` event, with the `data` envelope
    /// unwrapped: for `subscription { Person_added { ... } }` this is the
    /// object holding `Person_added`.
    pub fn payload(&self) -> Option<&Value> {
        match self {
            SubscriptionEvent::Next(payload) => Some(payload.get("data").unwrap_or(payload)),
            _ => None,
        }
    }

    /// The whole `next` event body, envelope included.
    pub fn raw_payload(&self) -> Option<&Value> {
        match self {
            SubscriptionEvent::Next(payload) => Some(payload),
            _ => None,
        }
    }

    /// Whether this event ends the stream.
    pub fn is_complete(&self) -> bool {
        matches!(self, SubscriptionEvent::Complete)
    }
}

/// Parse one SSE event block (the lines between blank lines) into an event.
///
/// Returns `None` for a block with no `data:` line at all — a comment or
/// keepalive — which the SSE specification says to ignore.
pub(crate) fn parse_sse_block(block: &str) -> anyhow::Result<Option<SubscriptionEvent>> {
    let mut event_type = "message";
    let mut data: Option<String> = None;

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = rest.trim();
        } else if let Some(rest) = line.strip_prefix("data:") {
            // Exactly one optional leading space is part of the framing.
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            match data.as_mut() {
                // Multi-line data fields are joined with newlines.
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(rest);
                }
                None => data = Some(rest.to_string()),
            }
        }
    }

    let data = match data {
        None => return Ok(None),
        Some(data) => data,
    };

    match event_type {
        "connected" => Ok(Some(SubscriptionEvent::Connected)),
        "complete" => Ok(Some(SubscriptionEvent::Complete)),
        "next" | "message" => {
            if data.trim().is_empty() || data.trim() == "null" {
                // A `next` with no payload carries no information; treat an
                // empty-bodied event as a keepalive.
                return Ok(None);
            }
            let payload: Value = serde_json::from_str(&data)
                .with_context(|| format!("failed to parse a subscription payload: {data}"))?;
            if let Some(errors) = payload.get("errors").and_then(|e| e.as_array()) {
                let errors: Vec<GraphQLError> = serde_json::from_value(Value::Array(
                    errors.clone(),
                ))
                .with_context(|| format!("failed to parse subscription errors: {payload}"))?;
                return Ok(Some(SubscriptionEvent::Errors(errors)));
            }
            Ok(Some(SubscriptionEvent::Next(payload)))
        }
        // Unknown event types are ignored per the SSE specification.
        _ => Ok(None),
    }
}

impl super::client::TerminusDBHttpClient {
    /// Open a GraphQL subscription over SSE and stream its events
    /// (TerminusDB 12.1).
    ///
    /// The returned stream yields [`SubscriptionEvent::Connected`] first, then
    /// a [`SubscriptionEvent::Next`] per matching commit, and ends with
    /// [`SubscriptionEvent::Complete`]. Dropping the stream closes the
    /// connection, which is how a subscriber unsubscribes.
    ///
    /// A finite query or mutation sent here is executed and answered as a
    /// single `next` followed by `complete`, which is occasionally useful for a
    /// uniform SSE interface.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # use futures_util::StreamExt;
    /// # async fn example(client: TerminusDBHttpClient) -> anyhow::Result<()> {
    /// let mut events = Box::pin(
    ///     client
    ///         .subscribe_graphql(
    ///             "people",
    ///             None,
    ///             GraphQLRequest::new("subscription { Person_added { _id name } }"),
    ///         )
    ///         .await?,
    /// );
    /// while let Some(event) = events.next().await {
    ///     match event? {
    ///         SubscriptionEvent::Connected => println!("listening"),
    ///         SubscriptionEvent::Next(payload) => println!("change: {payload}"),
    ///         SubscriptionEvent::Errors(errors) => println!("errors: {errors:?}"),
    ///         SubscriptionEvent::Complete => break,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.graphql.subscribe",
        skip(self, request),
        fields(database = %database, branch = ?branch),
        err
    )]
    pub async fn subscribe_graphql(
        &self,
        database: &str,
        branch: Option<&str>,
        request: GraphQLRequest,
    ) -> anyhow::Result<impl futures_util::Stream<Item = anyhow::Result<SubscriptionEvent>>> {
        let url = self.build_graphql_subscription_url(database, branch);
        self.open_subscription(url, request).await
    }

    /// Subscribe to changes in the **system** database (`/api/graphql/_system`),
    /// e.g. `subscription { UserDatabase_added { _id name } }` to observe
    /// database lifecycle events. Requires admin authentication.
    pub async fn subscribe_graphql_system(
        &self,
        request: GraphQLRequest,
    ) -> anyhow::Result<impl futures_util::Stream<Item = anyhow::Result<SubscriptionEvent>>> {
        let url = format!(
            "{}/graphql/_system",
            self.endpoint.as_str().trim_end_matches('/')
        );
        self.open_subscription(url, request).await
    }

    /// The subscription endpoint URL: the ordinary GraphQL endpoint for the
    /// data product.
    fn build_graphql_subscription_url(&self, database: &str, branch: Option<&str>) -> String {
        format!(
            "{}/graphql/{}/{}/local/branch/{}",
            self.endpoint.as_str().trim_end_matches('/'),
            self.org,
            database,
            branch.unwrap_or("main")
        )
    }

    async fn open_subscription(
        &self,
        url: String,
        request: GraphQLRequest,
    ) -> anyhow::Result<impl futures_util::Stream<Item = anyhow::Result<SubscriptionEvent>>> {
        use futures_util::StreamExt;

        debug!("opening a GraphQL subscription at {}", &url);

        // Deliberately no concurrency permit and no timeout: a subscription is
        // a long-lived connection, so taking a read permit for its lifetime
        // would starve ordinary reads.
        let res = self
            .http
            .post(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request)
            .send()
            .await
            .context("failed to open the GraphQL subscription")?;

        // Validation errors arrive *through* the stream as a `next` event, so a
        // non-2xx here is a transport/auth failure rather than a bad query.
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("GraphQL subscription refused ({}): {}", status, text);
        }

        let byte_stream = res.bytes_stream();

        Ok(async_stream::try_stream! {
            futures_util::pin_mut!(byte_stream);
            let mut buf: Vec<u8> = Vec::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.context("error reading a subscription chunk")?;
                buf.extend_from_slice(&chunk);

                // SSE frames events as blocks separated by a blank line.
                while let Some(pos) = find_block_end(&buf) {
                    let block: Vec<u8> = buf.drain(..pos.1).collect();
                    let block = &block[..pos.0];
                    let block = std::str::from_utf8(block)
                        .context("a subscription event block was not valid UTF-8")?;
                    if let Some(event) = parse_sse_block(block)? {
                        let complete = event.is_complete();
                        yield event;
                        if complete {
                            return;
                        }
                    }
                }
            }

            // The connection closed without a `complete`; flush a trailing
            // block if the server wrote one.
            if !buf.is_empty() {
                let block = std::str::from_utf8(&buf)
                    .context("a subscription event block was not valid UTF-8")?;
                if let Some(event) = parse_sse_block(block)? {
                    yield event;
                }
            }
        })
    }
}

/// Locate the end of the first SSE event block in `buf`, as
/// `(block_len, consume_len)`: the block content length and how many bytes to
/// drain including the separator. Handles both `\n\n` and `\r\n\r\n`.
fn find_block_end(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, i + 2));
        }
        if i + 3 < buf.len()
            && buf[i] == b'\r'
            && buf[i + 1] == b'\n'
            && buf[i + 2] == b'\r'
            && buf[i + 3] == b'\n'
        {
            return Some((i, i + 4));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_connected_extension_event() {
        let event = parse_sse_block("event: connected\ndata: null").unwrap();
        assert_eq!(event, Some(SubscriptionEvent::Connected));
    }

    #[test]
    fn parses_a_next_payload() {
        let event =
            parse_sse_block("event: next\ndata: {\"Person_added\":{\"_id\":\"Person/alice\"}}")
                .unwrap()
                .unwrap();
        assert_eq!(
            event.payload().and_then(|p| p.get("Person_added")),
            Some(&serde_json::json!({"_id": "Person/alice"}))
        );
    }

    #[test]
    fn unwraps_the_data_envelope_the_server_actually_sends() {
        let event = parse_sse_block(
            "event: next\ndata: {\"data\":{\"Person_added\":{\"_id\":\"Person/alice\"}}}",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            event.payload().and_then(|p| p.get("Person_added")),
            Some(&serde_json::json!({"_id": "Person/alice"})),
            "payload() must see through the `data` envelope"
        );
        assert!(
            event.raw_payload().unwrap().get("data").is_some(),
            "raw_payload() keeps the envelope"
        );
    }

    #[test]
    fn parses_a_next_carrying_graphql_errors() {
        let event = parse_sse_block(
            "event: next\ndata: {\"errors\":[{\"message\":\"Failed to parse subscription query\"}]}",
        )
        .unwrap()
        .unwrap();
        match event {
            SubscriptionEvent::Errors(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].message.contains("Failed to parse"));
            }
            other => panic!("expected Errors, got {other:?}"),
        }
    }

    #[test]
    fn parses_an_empty_complete_event() {
        let event = parse_sse_block("event: complete\ndata:").unwrap();
        assert_eq!(event, Some(SubscriptionEvent::Complete));
    }

    #[test]
    fn ignores_blocks_without_data_and_unknown_event_types() {
        assert_eq!(parse_sse_block(": keepalive").unwrap(), None);
        assert_eq!(parse_sse_block("event: surprise\ndata: {}").unwrap(), None);
    }

    #[test]
    fn finds_block_boundaries_in_both_line_endings() {
        assert_eq!(find_block_end(b"event: next\ndata: 1\n\nrest"), Some((19, 21)));
        assert_eq!(
            find_block_end(b"event: next\r\ndata: 1\r\n\r\nrest"),
            Some((20, 24))
        );
        assert_eq!(find_block_end(b"event: next\ndata: 1"), None);
    }
}

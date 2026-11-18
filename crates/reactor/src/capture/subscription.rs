//! The `_ChangeSet` subscription driver.
//!
//! Low latency, and never trusted alone.
//!
//! # Why it emits `changes: None`
//!
//! The subscription payload *looks* like it could build a [`ChangeSet`]
//! directly, and that would save a round trip. It cannot, for a specific
//! reason: `_ChangeSet` reports **post-commit** state for added and changed
//! documents and **pre-commit** state only for deleted ones. So for a *changed*
//! document it cannot supply the before-state — which is precisely what the
//! vacated [`crate::term::Term::Index`] bucket and the losing
//! [`crate::term::Term::Backlink`] are derived from. A changeset built from the
//! payload would have to be `complete: false`, firing
//! [`crate::term::Term::Everything`] and invalidating the entire namespace on
//! every commit.
//!
//! So this driver does one job: it says "something happened at commit C", fast.
//! The resolver then does the exact work with pinned reads. The subscription
//! buys latency, not information.
//!
//! # Server notes (TerminusDB 12.1.0-rc3, community)
//!
//! * `next` payloads are wrapped in the usual GraphQL `data` envelope, which
//!   [`terminusdb_client::SubscriptionEvent::payload`] unwraps.
//! * Subscription `filter` arguments are **parsed but not applied**, so a
//!   filtered subscription still delivers everything. We do not pass filters.
//! * [`terminusdb_client::SubscriptionEvent::Connected`] arrives before any
//!   payload; a driver must wait for it, or the registration races the change it
//!   is meant to observe.

use std::sync::Arc;

use futures_util::stream::BoxStream;
use futures_util::StreamExt;

use crate::capture::{CommitEvent, CommitSource};
use crate::ids::Namespace;
use crate::schema_index::SchemaIndex;
use terminusdb_client::{CommitId, GraphQLRequest, SubscriptionEvent, TerminusDBHttpClient};

/// Subscribes to `_ChangeSet` over SSE.
pub struct ChangeSetSubscription {
    client: Arc<TerminusDBHttpClient>,
    schema: Arc<SchemaIndex>,
}

impl ChangeSetSubscription {
    /// Build a driver for the classes in `schema`.
    ///
    /// The query is generated from the class list, so it has to be regenerated
    /// and resubscribed when the schema changes — the supervisor does that by
    /// dropping this driver and building a new one.
    pub fn new(client: Arc<TerminusDBHttpClient>, schema: Arc<SchemaIndex>) -> Self {
        Self { client, schema }
    }
}

/// Build the `_ChangeSet` subscription query for a set of classes.
///
/// Only `_id` is selected per class: we need the *fact* of a change, not its
/// content (see the module docs). `_CommitMetadata` is what we actually read —
/// everything else is there because `_ChangeSet` requires at least one class
/// selection to be a meaningful subscription.
pub fn changeset_query(schema: &SchemaIndex) -> String {
    let mut query = String::from("subscription {\n  _ChangeSet {\n");
    for class in schema.classes() {
        // Enums and abstract classes have no instances to observe.
        if schema.is_enum(class) || schema.is_abstract(class) {
            continue;
        }
        let name = class.as_str();
        query.push_str(&format!(
            "    {name}_added {{ _id }}\n    {name}_changed {{ _id }}\n    {name}_deleted {{ _id }}\n"
        ));
    }
    query.push_str("    _CommitMetadata { _id _timestamp _change_type }\n  }\n}\n");
    query
}

/// Pull the commit id out of a `_ChangeSet` payload.
///
/// `_CommitMetadata._id` is a document id (`ValidCommit/<hash>` or similar), so
/// the last path segment is the commit hash. Returns `None` for a payload we
/// cannot read rather than guessing — a driver that invents a commit id would
/// send the applier on a log walk for a commit that does not exist.
fn commit_of(payload: &serde_json::Value) -> Option<CommitId> {
    let id = payload
        .get("_ChangeSet")
        .and_then(|cs| cs.get("_CommitMetadata"))
        .or_else(|| payload.get("_CommitMetadata"))
        .and_then(|meta| meta.get("_id"))
        .and_then(|id| id.as_str())?;
    super::commit_id_from_any(id)
}

impl CommitSource for ChangeSetSubscription {
    fn name(&self) -> &'static str {
        "subscription"
    }

    fn subscribe(
        &self,
        ns: &Namespace,
        _after: Option<CommitId>,
    ) -> BoxStream<'static, anyhow::Result<CommitEvent>> {
        let client = Arc::clone(&self.client);
        let ns = ns.clone();
        let query = changeset_query(&self.schema);

        async_stream::stream! {
            let request = GraphQLRequest::new(query);
            let stream = match client
                .subscribe_graphql(&ns.db, Some(&ns.branch), request)
                .await
            {
                Ok(stream) => stream,
                Err(err) => {
                    // Yield the error and end: the supervisor decides whether to
                    // retry, so reconnection policy lives in one place instead
                    // of being duplicated per driver.
                    yield Err(err);
                    return;
                }
            };
            let mut stream = Box::pin(stream);
            let mut connected = false;
            while let Some(event) = stream.next().await {
                match event {
                    Ok(SubscriptionEvent::Connected) => {
                        connected = true;
                        tracing::debug!(%ns, "_ChangeSet subscription connected");
                    }
                    Ok(SubscriptionEvent::Next(value)) => {
                        if !connected {
                            // A payload before `Connected` means we are not
                            // talking to the protocol we think we are.
                            tracing::warn!(%ns, "subscription payload before Connected");
                        }
                        let event = SubscriptionEvent::Next(value);
                        match event.payload().and_then(commit_of) {
                            Some(commit) => {
                                tracing::debug!(%ns, %commit, "_ChangeSet event");
                                yield Ok(CommitEvent::new(commit));
                            }
                            None => tracing::debug!(
                                %ns,
                                "_ChangeSet event without a readable commit id; ignoring"
                            ),
                        }
                    }
                    Ok(SubscriptionEvent::Errors(errors)) => {
                        let messages: Vec<String> =
                            errors.iter().map(|e| e.message.clone()).collect();
                        yield Err(anyhow::anyhow!(
                            "_ChangeSet subscription reported errors: {}",
                            messages.join("; ")
                        ));
                        return;
                    }
                    Ok(SubscriptionEvent::Complete) => {
                        tracing::debug!(%ns, "_ChangeSet subscription completed");
                        return;
                    }
                    Err(err) => {
                        yield Err(err);
                        return;
                    }
                }
            }
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::schema_fixture;
    use serde_json::json;

    #[test]
    fn the_query_covers_every_concrete_class() {
        let schema = schema_fixture();
        let query = changeset_query(&schema);
        for class in ["Person", "Employee", "Company", "Department", "Address"] {
            assert!(
                query.contains(&format!("{class}_added")),
                "{class} is missing from {query}"
            );
            assert!(query.contains(&format!("{class}_changed")));
            assert!(query.contains(&format!("{class}_deleted")));
        }
        assert!(query.contains("_CommitMetadata { _id _timestamp _change_type }"));
        assert!(query.starts_with("subscription {"));
    }

    #[test]
    fn the_query_skips_enums_and_abstract_classes() {
        let query = changeset_query(&schema_fixture());
        assert!(
            !query.contains("Colour_added"),
            "an enum has no instances to observe"
        );
        assert!(
            !query.contains("Agent_added"),
            "an abstract class has no instances of its own"
        );
    }

    #[test]
    fn an_empty_schema_still_yields_a_valid_subscription() {
        let query = changeset_query(&SchemaIndex::default());
        assert!(query.contains("_CommitMetadata"));
        assert!(query.starts_with("subscription {"));
    }

    #[test]
    fn the_commit_id_is_read_from_the_metadata() {
        let wrapped = json!({"_ChangeSet": {"_CommitMetadata": {"_id": "ValidCommit/abc123"}}});
        assert_eq!(commit_of(&wrapped), Some(CommitId::new("abc123")));

        let bare = json!({"_CommitMetadata": {"_id": "abc123"}});
        assert_eq!(commit_of(&bare), Some(CommitId::new("abc123")));
    }

    #[test]
    fn an_unreadable_payload_yields_no_commit_rather_than_a_guess() {
        for payload in [
            json!({}),
            json!({"_ChangeSet": {}}),
            json!({"_ChangeSet": {"_CommitMetadata": {}}}),
            json!({"_ChangeSet": {"_CommitMetadata": {"_id": ""}}}),
            json!({"_ChangeSet": {"_CommitMetadata": {"_id": 7}}}),
            json!("nonsense"),
        ] {
            assert_eq!(commit_of(&payload), None, "{payload}");
        }
    }
}

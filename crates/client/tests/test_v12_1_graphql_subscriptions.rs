#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Live verification of TerminusDB 12.1 GraphQL subscriptions over SSE.
//!
//! Each test opens a subscription, waits for the server's `connected` event
//! (without which subscribing races the mutation), makes a change, and asserts
//! on the `next` event that arrives. Every wait is bounded by a timeout so a
//! protocol regression fails the test instead of hanging it.

use std::time::Duration;

use anyhow::Result;
use futures_util::{Stream, StreamExt};
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
}

fn person(id: &str, name: &str) -> Result<Person> {
    Ok(Person {
        id: EntityIDFor::new(id)?,
        name: name.to_string(),
    })
}

/// How long to wait for an event before calling it a failure.
const EVENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Pull the next event, failing rather than hanging.
async fn next_event<S>(events: &mut std::pin::Pin<Box<S>>) -> Result<SubscriptionEvent>
where
    S: Stream<Item = Result<SubscriptionEvent>> + ?Sized,
{
    let event = timeout(EVENT_TIMEOUT, events.next())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for a subscription event"))?
        .ok_or_else(|| anyhow::anyhow!("the subscription stream ended unexpectedly"))?;
    event
}

/// Wait for the `connected` extension event, which tells us the subscription
/// is registered and it is safe to make the change we want to observe.
async fn await_connected<S>(events: &mut std::pin::Pin<Box<S>>) -> Result<()>
where
    S: Stream<Item = Result<SubscriptionEvent>> + ?Sized,
{
    let event = next_event(events).await?;
    assert_eq!(
        event,
        SubscriptionEvent::Connected,
        "the first event must be the `connected` extension event"
    );
    Ok(())
}

/// A per-document `<Type>_added` subscription delivers an event for an insert.
#[tokio::test]
async fn subscription_delivers_a_document_insert() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person,), _, _, _>("v12_1_sub_added", |client, spec| async move {
            let mut events = Box::pin(
                client
                    .subscribe_graphql(
                        &spec.db,
                        spec.branch.as_deref(),
                        GraphQLRequest::new("subscription { Person_added { _id name } }"),
                    )
                    .await?,
            );
            await_connected(&mut events).await?;

            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&person("alice", "Alice")?, args).await?;

            let event = next_event(&mut events).await?;
            println!("Person_added -> {event:?}");
            let payload = event
                .payload()
                .ok_or_else(|| anyhow::anyhow!("expected a `next` event, got {event:?}"))?;
            let added = payload
                .get("Person_added")
                .ok_or_else(|| anyhow::anyhow!("no Person_added field in {payload}"))?;
            assert_eq!(added.get("name").and_then(|v| v.as_str()), Some("Alice"));
            assert_eq!(
                added.get("_id").and_then(|v| v.as_str()),
                Some("Person/alice")
            );
            Ok(())
        })
        .await
}

/// The `filter` argument is accepted and schema-checked — but in 12.1.0-rc3's
/// community server it does **not** actually suppress non-matching events.
///
/// `parse_subscription_query` extracts the filter into `filter_canonical_json`
/// and `register_sse_subscription_parsed/9` passes it on, but
/// `register_subscription_parsed/8` takes it as `_FilterJson` and keys the
/// broadcast cohort on (descriptor, class, operation, selection hash, mode)
/// only. This test pins the behaviour we actually get, so that a future server
/// release which starts honouring the filter fails here loudly instead of
/// silently changing what subscribers receive.
#[tokio::test]
async fn subscription_filter_is_accepted_but_not_yet_applied() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person,), _, _, _>("v12_1_sub_filter", |client, spec| async move {
            let mut events = Box::pin(
                client
                    .subscribe_graphql(
                        &spec.db,
                        spec.branch.as_deref(),
                        GraphQLRequest::new(
                            "subscription { Person_added(filter: {name: {eq: \"Bob\"}}) { _id name } }",
                        ),
                    )
                    .await?,
            );
            // The subscription is accepted: an unknown filter field would be a
            // validation error delivered as a `next` with errors instead.
            await_connected(&mut events).await?;

            let args = DocumentInsertArgs::from(spec.clone());
            client
                .insert_instance(&person("alice", "Alice")?, args.clone())
                .await?;

            let event = next_event(&mut events).await?;
            println!("filtered Person_added -> {event:?}");
            let payload = event.payload().expect("expected a `next` event");
            assert_eq!(
                payload
                    .get("Person_added")
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str()),
                Some("Alice"),
                "rc3 community ignores the subscription filter, so Alice still \
                 arrives for a `name eq Bob` filter: {payload}"
            );
            Ok(())
        })
        .await
}

/// `_ChangeSet` fires once per commit, with list-valued per-class fields and
/// commit metadata.
#[tokio::test]
async fn changeset_subscription_batches_a_commit() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person,), _, _, _>("v12_1_sub_changeset", |client, spec| async move {
            let mut events = Box::pin(
                client
                    .subscribe_graphql(
                        &spec.db,
                        spec.branch.as_deref(),
                        GraphQLRequest::new(
                            "subscription { _ChangeSet { \
                               Person_added { _id name } \
                               Person_changed { _id name } \
                               Person_deleted { _id name } \
                               _CommitMetadata { _id _timestamp _datetime _change_type } \
                             } }",
                        ),
                    )
                    .await?,
            );
            await_connected(&mut events).await?;

            // Two documents in ONE commit: a _ChangeSet event fires once with
            // both, where per-document subscriptions would fire twice.
            let args = DocumentInsertArgs::from(spec.clone());
            client
                .insert_instances(
                    vec![person("alice", "Alice")?, person("bob", "Bob")?],
                    args.clone(),
                )
                .await?;

            let event = next_event(&mut events).await?;
            println!("_ChangeSet -> {event:?}");
            let payload = event.payload().expect("expected a `next` event");
            let changeset = payload
                .get("_ChangeSet")
                .ok_or_else(|| anyhow::anyhow!("no _ChangeSet in {payload}"))?;

            let added = changeset
                .get("Person_added")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("Person_added is not a list: {changeset}"))?;
            let mut names: Vec<&str> = added
                .iter()
                .filter_map(|d| d.get("name").and_then(|v| v.as_str()))
                .collect();
            names.sort();
            assert_eq!(
                names,
                vec!["Alice", "Bob"],
                "one commit inserting two documents should batch into one event: {changeset}"
            );

            // Commit metadata rides along; a _ChangeSet's change type is the
            // whole commit, not a per-document add/change/delete.
            let metadata = changeset
                .get("_CommitMetadata")
                .ok_or_else(|| anyhow::anyhow!("no _CommitMetadata in {changeset}"))?;
            println!("_CommitMetadata -> {metadata}");
            assert!(
                metadata.get("_id").is_some(),
                "commit metadata should carry the commit id: {metadata}"
            );
            assert_eq!(
                metadata.get("_change_type").and_then(|v| v.as_str()),
                Some("commit"),
                "a _ChangeSet event's change type is `commit`: {metadata}"
            );
            assert!(metadata.get("_timestamp").is_some());
            assert!(metadata.get("_datetime").is_some());
            Ok(())
        })
        .await
}

/// A deletion is reported, and `_ChangeSet` resolves the deleted document's
/// fields from the pre-commit layer rather than returning just an id.
#[tokio::test]
async fn changeset_resolves_deleted_documents() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person,), _, _, _>("v12_1_sub_deleted", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            client
                .insert_instance(&person("carol", "Carol")?, args.clone())
                .await?;

            let mut events = Box::pin(
                client
                    .subscribe_graphql(
                        &spec.db,
                        spec.branch.as_deref(),
                        GraphQLRequest::new(
                            "subscription { _ChangeSet { Person_deleted { _id name } } }",
                        ),
                    )
                    .await?,
            );
            await_connected(&mut events).await?;

            client
                .delete_document(
                    Some("Person/carol"),
                    &spec,
                    "test",
                    "delete carol",
                    "instance",
                    Default::default(),
                )
                .await?;

            let event = next_event(&mut events).await?;
            println!("_ChangeSet (delete) -> {event:?}");
            let payload = event.payload().expect("expected a `next` event");
            let deleted = payload
                .get("_ChangeSet")
                .and_then(|c| c.get("Person_deleted"))
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("no Person_deleted list in {payload}"))?;
            assert_eq!(deleted.len(), 1, "one document was deleted: {payload}");
            assert_eq!(
                deleted[0].get("name").and_then(|v| v.as_str()),
                Some("Carol"),
                "a deleted document resolves from the pre-commit layer, so its \
                 fields are available: {payload}"
            );
            Ok(())
        })
        .await
}

/// An invalid subscription query is reported *through* the stream as a `next`
/// event carrying GraphQL errors, followed by `complete` — not as an HTTP 4xx.
#[tokio::test]
async fn invalid_subscription_reports_errors_in_the_stream() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person,), _, _, _>("v12_1_sub_invalid", |client, spec| async move {
            let mut events = Box::pin(
                client
                    .subscribe_graphql(
                        &spec.db,
                        spec.branch.as_deref(),
                        GraphQLRequest::new("subscription { this is not valid graphql"),
                    )
                    .await?,
            );

            // The server may or may not emit `connected` before refusing; walk
            // the stream until an error or completion turns up.
            let mut saw_errors = false;
            let mut saw_complete = false;
            for _ in 0..4 {
                match next_event(&mut events).await {
                    Ok(SubscriptionEvent::Errors(errors)) => {
                        println!("subscription errors -> {errors:?}");
                        assert!(!errors.is_empty());
                        saw_errors = true;
                    }
                    Ok(SubscriptionEvent::Complete) => {
                        saw_complete = true;
                        break;
                    }
                    Ok(other) => println!("ignoring {other:?}"),
                    // The stream ending is an acceptable way to signal the
                    // refusal, as long as we saw the errors first.
                    Err(e) if saw_errors => {
                        println!("stream ended after the errors: {e}");
                        saw_complete = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            assert!(
                saw_errors,
                "a validation failure must be reported as a `next` event with errors"
            );
            assert!(saw_complete, "and the stream must then complete");
            Ok(())
        })
        .await
}

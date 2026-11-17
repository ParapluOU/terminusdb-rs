#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Live verification of the remaining rc3 client surface:
//!
//! * the history parameters we lacked — `diff`, `complete`, `before`/`after`,
//!   `fast`, `graph_type` — and `streaming=true` (NDJSON).
//! * `unfold` on `POST /api/diff`.
//! * typed `Insert`/`Delete` patch operations for `POST /api/patch`, including
//!   `@capture`/`@ref`.

use anyhow::Result;
use futures_util::StreamExt;
use serde_json::json;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct City {
    id: EntityIDFor<Self>,
    name: String,
    population: i32,
}

/// An `@unfoldable` target, so a diff can meaningfully unfold it.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", unfoldable = true)]
struct Address {
    id: EntityIDFor<Self>,
    street: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    address: Ref<Address>,
}

fn city(id: &str, name: &str, population: i32) -> Result<City> {
    Ok(City {
        id: EntityIDFor::new(id)?,
        name: name.to_string(),
        population,
    })
}

/// The branch path `POST /api/patch` and `POST /api/diff` want.
fn branch_path(client: &TerminusDBHttpClient, spec: &BranchSpec) -> String {
    format!(
        "{}/{}/local/branch/{}",
        client.org(),
        spec.db,
        spec.branch.as_deref().unwrap_or("main")
    )
}

/// `diff=true` adds a per-commit diff to each history entry; `complete=true`
/// walks the whole commit chain; `before`/`after` filter by time.
#[tokio::test]
async fn history_diff_complete_and_time_filters() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(City,), _, _, _>("v12_1_history", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&city("w", "Warsaw", 1)?, args.clone()).await?;
            client.insert_instance(&city("w", "Warsaw", 2)?, args.clone()).await?;

            // Plain history: two commits touched the document.
            let plain = client
                .get_document_history("City/w", &spec, None)
                .await?;
            println!("plain history: {} entries", plain.len());
            assert!(plain.len() >= 2, "expected at least two commits: {plain:?}");
            assert!(
                plain.iter().all(|e| e.diff.is_none()),
                "without diff=true no entry should carry a diff"
            );

            // diff=true enriches the entries with the per-commit diff.
            let with_diff = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_diff(true)),
                )
                .await?;
            let diffs: Vec<_> = with_diff.iter().filter_map(|e| e.diff.as_ref()).collect();
            println!("history with diff=true: {diffs:#?}");
            assert!(
                !diffs.is_empty(),
                "diff=true should attach at least one diff: {with_diff:?}"
            );
            let rendered = serde_json::to_string(&diffs)?;
            assert!(
                rendered.contains("population") || rendered.contains("SwapValue"),
                "the diff should describe the population change: {rendered}"
            );

            // complete=true must not lose any entries.
            let complete = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_complete(true)),
                )
                .await?;
            println!("complete history: {} entries", complete.len());
            assert!(complete.len() >= plain.len());

            // fast=true is the enterprise-accelerated path: the community
            // server has no `fast_document_history_entries` plugin, so it
            // answers `fast_history_not_available` rather than falling back.
            let fast = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_fast(true)),
                )
                .await;
            println!("fast history -> {fast:?}");
            assert!(
                fast.is_err(),
                "fast=true is enterprise-only and must not silently succeed: {fast:?}"
            );

            // graph_type is accepted and selects the graph to walk.
            let instance = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_graph_type(DocumentType::Instance)),
                )
                .await?;
            assert!(!instance.is_empty());

            // after = now -> nothing; before = now -> everything.
            let now = chrono::Utc::now().to_rfc3339();
            let after_now = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_after(now.clone())),
                )
                .await?;
            println!("history after now: {} entries", after_now.len());
            assert!(
                after_now.is_empty(),
                "no commit can be newer than now: {after_now:?}"
            );
            let before_now = client
                .get_document_history(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_before(now)),
                )
                .await?;
            assert_eq!(
                before_now.len(),
                plain.len(),
                "every commit is older than now"
            );
            Ok(())
        })
        .await
}

/// `streaming=true` answers NDJSON; the streamed entries must match the
/// collected ones.
#[tokio::test]
async fn history_streaming_matches_the_collected_history() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(City,), _, _, _>("v12_1_history_stream", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for population in 1..=3 {
                client
                    .insert_instance(&city("w", "Warsaw", population)?, args.clone())
                    .await?;
            }

            let collected = client.get_document_history("City/w", &spec, None).await?;

            let stream = client
                .get_document_history_streaming("City/w", &spec, None)
                .await?;
            let streamed: Vec<_> = stream.collect::<Vec<_>>().await;
            let streamed: Vec<_> = streamed.into_iter().collect::<Result<Vec<_>>>()?;
            println!(
                "streamed {} entries, collected {}",
                streamed.len(),
                collected.len()
            );
            assert_eq!(
                streamed.len(),
                collected.len(),
                "the NDJSON stream must yield the same number of entries"
            );
            let streamed_ids: Vec<String> =
                streamed.iter().map(|e| e.identifier.to_string()).collect();
            let collected_ids: Vec<String> =
                collected.iter().map(|e| e.identifier.to_string()).collect();
            assert_eq!(streamed_ids, collected_ids, "and the same commits, in order");

            // The streaming path honours diff=true too.
            let stream = client
                .get_document_history_streaming(
                    "City/w",
                    &spec,
                    Some(DocumentHistoryParams::new().with_diff(true)),
                )
                .await?;
            let streamed: Vec<_> = stream.collect::<Vec<_>>().await;
            let streamed: Vec<_> = streamed.into_iter().collect::<Result<Vec<_>>>()?;
            assert!(
                streamed.iter().any(|e| e.diff.is_some()),
                "streaming with diff=true should carry diffs: {streamed:?}"
            );
            Ok(())
        })
        .await
}

/// `unfold` on `POST /api/diff`: with the default (`true`) a changed
/// unfoldable link is reported as a nested diff; with `false` the documents are
/// compared as stored.
#[tokio::test]
async fn diff_unfold_parameter() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Address, Person), _, _, _>(
            "v12_1_diff_unfold",
            |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_instance(
                        &Address {
                            id: EntityIDFor::new("home")?,
                            street: "First Street".to_string(),
                        },
                        args.clone(),
                    )
                    .await?;
                let before = client
                    .insert_instance(
                        &Person {
                            id: EntityIDFor::new("jane")?,
                            name: "Jane".to_string(),
                            address: Ref::from(EntityIDFor::<Address>::new("home")?),
                        },
                        args.clone(),
                    )
                    .await?
                    .commit_id
                    .clone()
                    .expect("insert returned no data version");

                // Change the linked (unfoldable) address.
                let after = client
                    .insert_instance(
                        &Address {
                            id: EntityIDFor::new("home")?,
                            street: "Second Street".to_string(),
                        },
                        args.clone(),
                    )
                    .await?
                    .commit_id
                    .clone()
                    .expect("insert returned no data version");

                let unfolded = client
                    .diff_data_versions(
                        &spec,
                        &before,
                        &after,
                        Some("Person/jane"),
                        DiffOptions {
                            unfold: Some(true),
                            ..Default::default()
                        },
                    )
                    .await?;
                println!("diff unfold=true -> {unfolded}");

                let stored = client
                    .diff_data_versions(
                        &spec,
                        &before,
                        &after,
                        Some("Person/jane"),
                        DiffOptions {
                            unfold: Some(false),
                            ..Default::default()
                        },
                    )
                    .await?;
                println!("diff unfold=false -> {stored}");

                // The parameter has to change the answer: unfolding reaches the
                // address's changed street, comparing as stored does not.
                let unfolded_text = serde_json::to_string(&unfolded)?;
                let stored_text = serde_json::to_string(&stored)?;
                assert!(
                    unfolded_text.contains("Second Street"),
                    "unfold=true should surface the changed street: {unfolded_text}"
                );
                assert_ne!(
                    unfolded_text, stored_text,
                    "unfold must actually change the diff"
                );
                Ok(())
            },
        )
        .await
}

/// Typed `Insert`/`Delete`/`SwapValue` patch operations, and `@capture`/`@ref`
/// for documents that reference each other within one patch.
#[tokio::test]
async fn typed_patch_operations() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(City,), _, _, _>("v12_1_patch", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            client
                .insert_instance(&city("warsaw", "Warsaw", 1_800_000)?, args.clone())
                .await?;
            let path = branch_path(&client, &spec);

            // Insert a document and delete another in one patch.
            client
                .patch_typed(
                    &path,
                    &[
                        PatchOperation::insert(json!({
                            "@id": "City/Leipzig",
                            "@type": "City",
                            // `City` has an `id` field of its own (EntityIDFor),
                            // which is a required schema property, so a raw
                            // patch document has to carry it.
                            "id": "City/Leipzig",
                            "name": "Leipzig",
                            "population": 600_000,
                        })),
                        PatchOperation::delete("City/warsaw"),
                    ],
                    "test",
                    "insert Leipzig, delete Warsaw",
                )
                .await?;

            let leipzig = client
                .get_document("City/Leipzig", &spec, GetOpts::default())
                .await?;
            println!("inserted via patch: {leipzig}");
            assert_eq!(
                leipzig.get("name").and_then(|v| v.as_str()),
                Some("Leipzig")
            );
            assert!(
                client
                    .get_document_if_exists("City/warsaw", &spec, GetOpts::default())
                    .await?
                    .is_none(),
                "the Delete operation should have removed Warsaw"
            );

            // SwapValue replaces one field, and the `@before` value must match.
            client
                .patch_typed(
                    &path,
                    &[PatchOperation::swap_value(
                        "City/Leipzig",
                        "name",
                        json!("Leipzig"),
                        json!("Leipzig (Saxony)"),
                    )],
                    "test",
                    "rename Leipzig",
                )
                .await?;
            let renamed = client
                .get_document("City/Leipzig", &spec, GetOpts::default())
                .await?;
            println!("after SwapValue: {renamed}");
            assert_eq!(
                renamed.get("name").and_then(|v| v.as_str()),
                Some("Leipzig (Saxony)")
            );

            // A SwapValue whose @before is stale must be refused.
            let conflict = client
                .patch_typed(
                    &path,
                    &[PatchOperation::swap_value(
                        "City/Leipzig",
                        "name",
                        json!("Not The Current Value"),
                        json!("Nope"),
                    )],
                    "test",
                    "stale swap",
                )
                .await;
            println!("stale SwapValue -> {conflict:?}");
            assert!(
                conflict.is_err(),
                "a SwapValue with a stale @before must conflict, got {conflict:?}"
            );

            // @capture: insert under a capture name instead of an @id, so the
            // server mints the id.
            let captured = client
                .patch_typed(
                    &path,
                    &[PatchOperation::insert_captured(
                        "Dresden",
                        json!({
                            "@type": "City",
                            "id": "City/Dresden",
                            "name": "Dresden",
                            "population": 560_000,
                        }),
                    )],
                    "test",
                    "insert Dresden under a capture",
                )
                .await?;
            println!("captured insert -> {captured}");
            let cities = client
                .get_documents(
                    vec![],
                    &spec,
                    GetOpts::default().with_type_filter::<City>(),
                )
                .await?;
            let names: Vec<&str> = cities
                .iter()
                .filter_map(|c| c.get("name").and_then(|v| v.as_str()))
                .collect();
            println!("cities now: {names:?}");
            assert!(
                names.contains(&"Dresden"),
                "the captured insert should have created Dresden: {names:?}"
            );
            Ok(())
        })
        .await
}

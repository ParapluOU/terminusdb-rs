#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Live verification of the endpoints and parameters added in TerminusDB
//! 12.1.0-rc3:
//!
//! * `GET /api/document?counted=true` → `document_count`
//! * the new document-read params `identifiers`, `id_prefix`, `raw_id`, `xsd`
//! * `format=markdown` (and the `format=embedding` precondition)
//! * `GET /api/changes/<path>?count=N` → `get_changes`
//! * `GET /api/schema/<path>` → `get_class_frame` / `get_json_schema`
//! * GraphQL's index-answered unfiltered `_count`

// Reads documents as raw JSON on purpose: these tests assert what the *server*
// stored and returned, which is the one question a typed getter cannot answer.
#![allow(deprecated)]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Member {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Widget {
    id: EntityIDFor<Self>,
    label: String,
}

fn member(id: &str, name: &str, age: i32) -> Result<Member> {
    Ok(Member {
        id: EntityIDFor::new(id)?,
        name: name.to_string(),
        age,
    })
}

/// `counted=true` answers a `DocumentCountResponse` for the whole database and
/// for a single class.
#[tokio::test]
async fn counted_returns_a_document_count() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member, Widget), _, _, _>("v12_1_counted", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for m in [
                member("a", "A", 10)?,
                member("b", "B", 20)?,
                member("c", "C", 30)?,
            ] {
                client.insert_instance(&m, args.clone()).await?;
            }
            client
                .insert_instance(
                    &Widget {
                        id: EntityIDFor::new("w1")?,
                        label: "Widget one".to_string(),
                    },
                    args.clone(),
                )
                .await?;

            let members = client
                .document_count(&spec, GetOpts::default().with_type_filter::<Member>())
                .await?;
            println!("counted Member -> {members}");
            assert_eq!(members, 3);

            let widgets = client
                .document_count(&spec, GetOpts::default().with_type_filter::<Widget>())
                .await?;
            assert_eq!(widgets, 1);

            // Unfiltered: every document in the instance graph.
            let total = client.document_count(&spec, GetOpts::default()).await?;
            println!("counted all -> {total}");
            assert_eq!(total, 4, "three members plus one widget");
            Ok(())
        })
        .await
}

/// `id_prefix` narrows a listing to ids under a prefix; `identifiers=false`
/// strips the generated `@id`/`@type`; `raw_id` and `xsd` are accepted.
#[tokio::test]
async fn new_document_list_params_are_honoured() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member,), _, _, _>("v12_1_list_params", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for m in [
                member("alpha-1", "Alpha One", 10)?,
                member("alpha-2", "Alpha Two", 20)?,
                member("beta-1", "Beta One", 30)?,
            ] {
                client.insert_instance(&m, args.clone()).await?;
            }

            // id_prefix: only the two alpha members.
            let docs = client
                .get_documents(
                    vec![],
                    &spec,
                    GetOpts::default()
                        .with_type_filter::<Member>()
                        .with_id_prefix("Member/alpha"),
                )
                .await?;
            let names: Vec<String> = docs
                .iter()
                .filter_map(|d| d.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect();
            println!("id_prefix=Member/alpha -> {names:?}");
            assert_eq!(
                docs.len(),
                2,
                "id_prefix should select only alpha-*: {docs:?}"
            );

            // identifiers=false strips @id/@type from the response.
            let stripped = client
                .get_document(
                    "Member/alpha-1",
                    &spec,
                    GetOpts::default().with_identifiers(false),
                )
                .await?;
            println!("identifiers=false -> {stripped}");
            assert!(
                stripped.get("@id").is_none() && stripped.get("@type").is_none(),
                "identifiers=false must strip @id/@type: {stripped}"
            );
            assert_eq!(
                stripped.get("name").and_then(|v| v.as_str()),
                Some("Alpha One"),
                "the payload itself must survive: {stripped}"
            );

            // raw_id and the default xsd canonicalisation version are accepted
            // and still return the document.
            let raw = client
                .get_document(
                    "Member/alpha-1",
                    &spec,
                    GetOpts::default().with_raw_id(true).with_xsd("1.1"),
                )
                .await?;
            println!("raw_id=true, xsd=1.1 -> {raw}");
            assert_eq!(raw.get("name").and_then(|v| v.as_str()), Some("Alpha One"));
            Ok(())
        })
        .await
}

/// `format=markdown` renders documents as Markdown text; `format=embedding`
/// requires the class to declare `@metadata.embedding` and otherwise errors.
#[tokio::test]
async fn document_output_formats() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member,), _, _, _>("v12_1_formats", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            client
                .insert_instance(&member("a", "Alice", 30)?, args)
                .await?;

            // Markdown comes back as a text/markdown body, not JSON.
            let md = client.get_document_markdown("Member/a", &spec).await?;
            println!("format=markdown ->\n{md}");
            assert!(
                md.contains("Alice"),
                "the markdown rendering should contain the document's data: {md}"
            );
            assert!(
                md.contains('#') || md.contains('*') || md.contains('-'),
                "the rendering should actually look like markdown: {md}"
            );

            // Every document of a type, in one markdown body.
            let all = client
                .get_documents_text(&spec, None, Some("Member"), DocumentFormat::Markdown)
                .await?;
            assert!(all.contains("Alice"), "type-wide markdown: {all}");

            // Member declares no @metadata.embedding, so the embedding format
            // must be refused rather than silently falling back to JSON.
            let err = client.get_document_embedding("Member/a", &spec).await;
            println!("format=embedding without @metadata.embedding -> {err:?}");
            assert!(
                err.is_err(),
                "format=embedding must error without an embedding query, got {err:?}"
            );
            Ok(())
        })
        .await
}

/// `GET /api/changes/<path>` reports the ids added/changed/deleted by a commit.
#[tokio::test]
async fn changes_reports_the_ids_touched_by_a_commit() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member,), _, _, _>("v12_1_changes", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());

            // One commit per insert, so the branch head's changes are precisely
            // the second insert.
            client
                .insert_instance(&member("first", "First", 10)?, args.clone())
                .await?;
            client
                .insert_instance(&member("second", "Second", 20)?, args.clone())
                .await?;

            let changes = client.get_changes(&spec, None).await?;
            println!("changes at head: {changes:?}");
            // The server answers with *expanded* IRIs, not the compressed
            // `Type/id` form the document API takes.
            assert_eq!(
                changes.added,
                vec!["terminusdb:///data/Member/second".to_string()],
                "the head commit added exactly the second member"
            );
            assert!(changes.deleted.is_empty());

            // An update is reported under `changed`, not `added`.
            client
                .insert_instance(&member("first", "First Renamed", 11)?, args.clone())
                .await?;
            let changes = client.get_changes(&spec, None).await?;
            println!("changes after the update: {changes:?}");
            assert_eq!(
                changes.changed,
                vec!["terminusdb:///data/Member/first".to_string()],
                "updating a document reports it as changed: {changes:?}"
            );

            // A delete is reported under `deleted`.
            client
                .delete_document(
                    Some("Member/second"),
                    &spec,
                    "test",
                    "delete the second member",
                    "instance",
                    Default::default(),
                )
                .await?;
            let changes = client.get_changes(&spec, None).await?;
            println!("changes after the delete: {changes:?}");
            assert_eq!(
                changes.deleted,
                vec!["terminusdb:///data/Member/second".to_string()]
            );

            // `count` caps the ids returned per category.
            let capped = client.get_changes(&spec, Some(0)).await?;
            println!("changes with count=0: {capped:?}");
            assert!(
                capped.len() <= changes.len(),
                "count must not increase the number of ids returned"
            );
            Ok(())
        })
        .await
}

/// The class-frame endpoint, in both its `frame` and `jsonschema` modes.
#[tokio::test]
async fn class_frame_and_json_schema() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member,), _, _, _>("v12_1_frame", |client, spec| async move {
            let frame = client
                .get_class_frame(&spec, Some("Member"), ClassFrameOptions::default())
                .await?;
            println!("Member frame: {frame}");
            assert!(
                frame.get("name").is_some() && frame.get("age").is_some(),
                "the frame should describe both properties: {frame}"
            );

            // Without a class, the endpoint frames every class in the schema.
            let all = client
                .get_class_frame(&spec, None, ClassFrameOptions::default())
                .await?;
            println!("all frames: {all}");
            assert!(
                all.get("Member").is_some(),
                "the all-classes frame should contain Member: {all}"
            );

            // format=jsonschema renders the same class as JSON Schema.
            let json_schema = client
                .get_json_schema(&spec, Some("Member"), ClassFrameOptions::default())
                .await?;
            println!("Member json schema: {json_schema}");
            let rendered = serde_json::to_string(&json_schema)?;
            assert!(
                rendered.contains("properties"),
                "a JSON Schema rendering should have a `properties` block: {rendered}"
            );
            Ok(())
        })
        .await
}

/// TerminusDB 12.1 answers an *unfiltered* GraphQL `_count` from the index.
#[tokio::test]
async fn graphql_unfiltered_count_is_answered() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Member,), _, _, _>("v12_1_gql_count", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for m in [
                member("a", "A", 10)?,
                member("b", "B", 20)?,
                member("c", "C", 30)?,
            ] {
                client.insert_instance(&m, args.clone()).await?;
            }

            let count = client
                .count_documents(&spec.db, spec.branch.as_deref(), "Member", "{}")
                .await?;
            println!("GraphQL _count(Member: {{}}) -> {count}");
            assert_eq!(count, 3);

            // A filtered count still works and agrees with the document API.
            let filtered = client
                .count_documents(
                    &spec.db,
                    spec.branch.as_deref(),
                    "Member",
                    "{name: {eq: \"A\"}}",
                )
                .await?;
            println!("GraphQL filtered _count -> {filtered}");
            assert_eq!(filtered, 1);

            let via_document_api = client
                .document_count(&spec, GetOpts::default().with_type_filter::<Member>())
                .await?;
            assert_eq!(
                via_document_api as usize, count,
                "the document API and GraphQL must agree on the count"
            );
            Ok(())
        })
        .await
}

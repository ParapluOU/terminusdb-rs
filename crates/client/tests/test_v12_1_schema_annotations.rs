//! Live verification of the schema annotations added in TerminusDB 12.1.0-rc3
//! against a real server:
//!
//! * `@key: {"@type": "UuidV7"}` — time-ordered UUID keys (`#[tdb(key = "uuid_v7")]`).
//! * `@shared` — a regular document, referenceable by many, cascade-deleted
//!   with its last referrer (`#[tdb(shared = true)]`).
//! * field-level `@unfold` — unfold *this* link on retrieval even when the
//!   target class is not `@unfoldable` (`#[tdb(unfold)]`).
//!
//! The schema-shape half of each test reads the authored schema back from the
//! server, which is the only way to know the server accepted the annotation
//! rather than quietly ignoring it.

#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// ---------------------------------------------------------------------------
// @key UuidV7
// ---------------------------------------------------------------------------

/// A UuidV7-keyed document with no id field: the id is minted (client-side, as
/// for `Random`) as `<base>/<uuid-v7>`.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "uuid_v7")]
struct Note {
    text: String,
}

/// A UuidV7-keyed document that supplies its own `@id`. UuidV7 accepts an
/// externally supplied id (unlike the deterministic key strategies), which is
/// why the id field may be an `EntityIDFor<Self>` rather than a `ServerIDFor`.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "uuid_v7", id_field = "id")]
struct Memo {
    id: EntityIDFor<Self>,
    text: String,
}

// ---------------------------------------------------------------------------
// @shared
// ---------------------------------------------------------------------------

/// `@shared`: its own IRI, linkable from any number of documents, but
/// cascade-deleted when the last referrer goes.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", shared = true)]
struct Tag {
    id: EntityIDFor<Self>,
    label: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Post {
    id: EntityIDFor<Self>,
    title: String,
    tag: Ref<Tag>,
}

// ---------------------------------------------------------------------------
// field-level @unfold
// ---------------------------------------------------------------------------

/// Deliberately NOT `@unfoldable`: whether a link to it unfolds must therefore
/// come from the *property*, not the class.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Reviewer {
    id: EntityIDFor<Self>,
    name: String,
}

/// `lead` carries field-level `@unfold`; `backup` is the control and must stay
/// a bare id reference.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Project {
    id: EntityIDFor<Self>,
    name: String,
    #[tdb(unfold)]
    lead: Option<Ref<Reviewer>>,
    backup: Option<Ref<Reviewer>>,
}

/// Find an authored schema document by its `@id`.
async fn schema_doc(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    id: &str,
) -> Result<serde_json::Value> {
    let docs = client.get_schema_documents(spec).await?;
    docs.into_iter()
        .find(|d| d.get("@id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| anyhow::anyhow!("no schema document with @id {id}"))
}

/// A `@key: {"@type": "UuidV7"}` class is accepted, and an instance without an
/// explicit id gets a UUIDv7 id under the class base.
#[tokio::test]
async fn uuid_v7_key_mints_a_v7_id() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Note,), _, _, _>("v12_1_uuid_v7_key", |client, spec| async move {
            // The server kept the annotation, i.e. it knows the UuidV7 strategy.
            let schema = schema_doc(&client, &spec, "Note").await?;
            println!("Note schema: {schema}");
            assert_eq!(
                schema.get("@key").and_then(|k| k.get("@type")),
                Some(&serde_json::Value::String("UuidV7".to_string())),
                "@key did not round-trip as UuidV7: {schema}"
            );

            let args = DocumentInsertArgs::from(spec.clone());
            let result = client
                .insert_instance(
                    &Note {
                        text: "first".to_string(),
                    },
                    args.clone(),
                )
                .await?;
            let id = result.root_id.clone();
            println!("UuidV7-keyed insert -> {id}");
            assert!(id.contains("Note/"), "id is not under the class base: {id}");
            let uuid = id.rsplit('/').next().unwrap();
            assert_eq!(uuid.len(), 36, "id is not a hyphenated UUID: {uuid}");
            assert_eq!(
                uuid.chars().nth(14),
                Some('7'),
                "id is not a v7 UUID (version nibble): {uuid}"
            );

            // v7 ids are time-ordered: a later insert sorts after an earlier one.
            let second = client
                .insert_instance(
                    &Note {
                        text: "second".to_string(),
                    },
                    args,
                )
                .await?;
            let second_uuid = second.root_id.rsplit('/').next().unwrap().to_string();
            assert_ne!(uuid, second_uuid.as_str(), "UuidV7 minted a duplicate id");
            assert!(
                uuid < second_uuid.as_str(),
                "v7 ids should sort by creation time: {uuid} vs {second_uuid}"
            );
            Ok(())
        })
        .await
}

/// A UuidV7 class also accepts an externally supplied `@id`.
#[tokio::test]
async fn uuid_v7_key_accepts_an_external_id() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Memo,), _, _, _>("v12_1_uuid_v7_ext", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            let memo = Memo {
                id: EntityIDFor::new("my-own-id")?,
                text: "hello".to_string(),
            };
            client.insert_instance(&memo, args).await?;

            let mut de = DefaultTDBDeserializer;
            let read: Memo = client.get_instance("my-own-id", &spec, &mut de).await?;
            assert_eq!(read.text, "hello");
            assert_eq!(read.id.id(), "my-own-id");
            Ok(())
        })
        .await
}

/// `@shared` serialises as `"@shared": []` and the server cascade-deletes the
/// shared document when its last referrer is deleted.
#[tokio::test]
async fn shared_class_is_cascade_deleted_with_its_last_referrer() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Tag, Post), _, _, _>("v12_1_shared", |client, spec| async move {
            // The annotation survived the round trip through the server.
            let schema = schema_doc(&client, &spec, "Tag").await?;
            println!("Tag schema: {schema}");
            assert_eq!(
                schema.get("@shared"),
                Some(&serde_json::Value::Array(vec![])),
                "@shared did not round-trip as []: {schema}"
            );

            let args = DocumentInsertArgs::from(spec.clone());
            let tag = Tag {
                id: EntityIDFor::new("rust")?,
                label: "Rust".to_string(),
            };
            client.insert_instance(&tag, args.clone()).await?;
            for (id, title) in [("one", "First"), ("two", "Second")] {
                client
                    .insert_instance(
                        &Post {
                            id: EntityIDFor::new(id)?,
                            title: title.to_string(),
                            tag: Ref::from(EntityIDFor::<Tag>::new("rust")?),
                        },
                        args.clone(),
                    )
                    .await?;
            }

            // Two referrers: deleting one leaves the shared tag in place.
            client
                .delete_document(
                    Some("Post/one"),
                    &spec,
                    "test",
                    "delete the first referrer",
                    "instance",
                    Default::default(),
                )
                .await?;
            let still_there = client
                .get_document_if_exists("Tag/rust", &spec, GetOpts::default())
                .await?;
            assert!(
                still_there.is_some(),
                "a @shared document must survive while another referrer remains"
            );

            // Deleting the last referrer cascade-deletes the shared document.
            client
                .delete_document(
                    Some("Post/two"),
                    &spec,
                    "test",
                    "delete the last referrer",
                    "instance",
                    Default::default(),
                )
                .await?;
            let gone = client
                .get_document_if_exists("Tag/rust", &spec, GetOpts::default())
                .await?;
            println!("after deleting the last referrer, Tag/rust = {gone:?}");
            assert!(
                gone.is_none(),
                "a @shared document must be cascade-deleted with its last referrer, got {gone:?}"
            );
            Ok(())
        })
        .await
}

/// Field-level `@unfold` unfolds the annotated link even though the target
/// class is not `@unfoldable`; a sibling link to the same class does not.
#[tokio::test]
async fn field_level_unfold_expands_only_the_annotated_property() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Reviewer, Project), _, _, _>(
            "v12_1_field_unfold",
            |client, spec| async move {
                // The flag lives inside the property's type-family object.
                let schema = schema_doc(&client, &spec, "Project").await?;
                println!("Project schema: {schema}");
                assert_eq!(
                    schema.get("lead").and_then(|f| f.get("@unfold")),
                    Some(&serde_json::Value::Bool(true)),
                    "lead lost its field-level @unfold: {schema}"
                );
                assert!(
                    schema
                        .get("backup")
                        .and_then(|f| f.get("@unfold"))
                        .is_none(),
                    "backup must not carry @unfold: {schema}"
                );

                let args = DocumentInsertArgs::from(spec.clone());
                for (id, name) in [("ann", "Ann"), ("bert", "Bert")] {
                    client
                        .insert_instance(
                            &Reviewer {
                                id: EntityIDFor::new(id)?,
                                name: name.to_string(),
                            },
                            args.clone(),
                        )
                        .await?;
                }
                client
                    .insert_instance(
                        &Project {
                            id: EntityIDFor::new("apollo")?,
                            name: "Apollo".to_string(),
                            lead: Some(Ref::from(EntityIDFor::<Reviewer>::new("ann")?)),
                            backup: Some(Ref::from(EntityIDFor::<Reviewer>::new("bert")?)),
                        },
                        args,
                    )
                    .await?;

                let mut de = DefaultTDBDeserializer;
                let project: Project = client
                    .get_instance_unfolded("apollo", &spec, &mut de)
                    .await?;

                let lead = project.lead.as_ref().expect("apollo has a lead");
                assert!(
                    lead.is_loaded(),
                    "field-level @unfold should have unfolded `lead` inline"
                );
                assert_eq!(lead.get_expect().name, "Ann");

                let backup = project.backup.as_ref().expect("apollo has a backup");
                assert!(
                    !backup.is_loaded(),
                    "`backup` has no @unfold and Reviewer is not @unfoldable, so it must stay a reference"
                );
                assert_eq!(backup.id().id(), "bert");
                Ok(())
            },
        )
        .await
}

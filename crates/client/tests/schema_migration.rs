// Schema migration (CreateClassProperty with default) against a real server.

#![recursion_limit = "512"]

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{DocumentInsertArgs, MigrationOperation, MigrationOptions};
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct MigProbe {
    id: EntityIDFor<Self>,
    name: String,
}

#[tokio::test]
async fn create_class_property_migrates_existing_instances() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_db_schema::<(MigProbe,), _, _, _>("mig_probe", |client, spec| async move {
            let doc = MigProbe {
                id: EntityIDFor::random(),
                name: "before-migration".to_string(),
            };
            client
                .insert(&doc, DocumentInsertArgs::from(spec.clone()))
                .await?;

            // Add mandatory property "age" with a default filling existing rows
            let resp = client
                .migrate_schema(
                    &spec,
                    "tester",
                    "add age with default",
                    vec![MigrationOperation::CreateClassProperty {
                        class: "MigProbe".to_string(),
                        property: "age".to_string(),
                        property_type: serde_json::json!("xsd:integer"),
                        default: Some(serde_json::json!(0)),
                    }],
                    MigrationOptions::default(),
                )
                .await?;
            assert!(
                resp.status.contains("success"),
                "migration failed: {resp:?}"
            );

            // Raw read: existing instance now carries the new property
            let raw = client
                .get_document(
                    &format!("MigProbe/{}", doc.id.id()),
                    &spec,
                    terminusdb_client::GetOpts::default(),
                )
                .await?;
            assert_eq!(raw["age"], serde_json::json!(0), "default not applied: {raw}");
            assert_eq!(raw["name"], serde_json::json!("before-migration"));
            Ok(())
        })
        .await
}

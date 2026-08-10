// Merge strategies (Rebase / Apply) verified against a real server:
// feature work on a merge branch lands on main with both documents.

#![recursion_limit = "512"]

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{DefaultTDBDeserializer, DocumentInsertArgs, MergeBranchOptions, MergeStrategy};
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct MergeProbe {
    id: EntityIDFor<Self>,
    name: String,
}

async fn merge_work(
    server: &TerminusDBServer,
    strategy: MergeStrategy,
    feature_name: &str,
) -> anyhow::Result<()> {
    server
        .with_db_schema::<(MergeProbe,), _, _, _>("merge_probe", |client, spec| async move {
            let base = MergeProbe {
                id: EntityIDFor::random(),
                name: "base".to_string(),
            };
            client
                .insert(&base, DocumentInsertArgs::from(spec.clone()))
                .await?;

            let feature_id = client
                .with_merge_branch(
                    &spec,
                    MergeBranchOptions {
                        author: "tester".to_string(),
                        strategy,
                        ..Default::default()
                    },
                    |branch| async move {
                        let feature = MergeProbe {
                            id: EntityIDFor::random(),
                            name: feature_name.to_string(),
                        };
                        branch
                            .save_instance(&feature, DocumentInsertArgs::default())
                            .await?;
                        Ok(feature.id.id().to_string())
                    },
                )
                .await?;

            // Main now contains both the base doc and the feature doc
            let mut deser = DefaultTDBDeserializer;
            let got: MergeProbe = client
                .get::<MergeProbe>(&feature_id, &spec, &mut deser)
                .await?;
            assert_eq!(got.name, feature_name, "feature doc missing on main");
            let base: MergeProbe = client
                .get::<MergeProbe>(&base.id.id(), &spec, &mut deser)
                .await?;
            assert_eq!(base.name, "base", "base doc missing on main");
            Ok(())
        })
        .await
}

#[tokio::test]
async fn merge_rebase_lands_feature_on_main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    merge_work(&server, MergeStrategy::Rebase, "rebase-feature").await
}

#[tokio::test]
async fn merge_apply_lands_feature_on_main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    merge_work(&server, MergeStrategy::Apply, "apply-feature").await
}

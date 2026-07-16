#![recursion_limit = "256"]
//! Live test of the TerminusDB 12 schema-migration API client against a 12.1
//! server: seed a schema, dry-run a migration, then apply it and confirm the
//! new property is queryable.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde_json::json;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Product {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[tokio::test]
    async fn test_v12_schema_migration_add_property() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Product,), _, _, _>("v12_migration", |client, spec| async move {
                // Seed one instance.
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_instance(
                        &Product {
                            id: EntityIDFor::new("Widget").unwrap(),
                            name: "Widget".to_string(),
                        },
                        args,
                    )
                    .await?;

                // Add an optional `sku` property (a weakening — no default needed).
                let ops = vec![MigrationOperation::CreateClassProperty {
                    class: "Product".to_string(),
                    property: "sku".to_string(),
                    property_type: json!({ "@type": "Optional", "@class": "xsd:string" }),
                    default: None,
                }];

                // Dry-run first: validates without committing.
                let dry = client
                    .migrate_schema(
                        &spec,
                        "tester",
                        "preview add sku",
                        ops.clone(),
                        MigrationOptions::dry_run(),
                    )
                    .await?;
                assert!(dry.is_success(), "dry-run should succeed: {dry:?}");

                // Apply for real.
                let applied = client
                    .migrate_schema(
                        &spec,
                        "tester",
                        "add sku to Product",
                        ops,
                        MigrationOptions::default(),
                    )
                    .await?;
                assert!(applied.is_success(), "migration should succeed: {applied:?}");

                // The new property exists in the schema now.
                let schema_docs = client.get_schema_documents(&spec).await?;
                let product = schema_docs
                    .iter()
                    .find(|d| d.get("@id").and_then(|v| v.as_str()) == Some("Product"))
                    .expect("Product class should be in the schema");
                assert!(
                    product.get("sku").is_some(),
                    "Product schema should have 'sku' after migration: {product:?}"
                );

                Ok(())
            })
            .await
    }
}

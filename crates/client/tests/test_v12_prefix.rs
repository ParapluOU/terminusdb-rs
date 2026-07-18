#![recursion_limit = "256"]
//! Live test of the TerminusDB 12 prefix management API client against a 12.1
//! server: add → get → update → upsert → delete a context prefix.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;

    #[tokio::test]
    async fn test_v12_prefix_crud() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_prefix", |client, spec| async move {
                // Add a new prefix.
                let added = client
                    .add_prefix(&spec, "company", "http://example.org/company/")
                    .await?;
                assert!(added.is_success(), "add_prefix failed: {added:?}");

                // GET resolves it.
                let uri = client.get_prefix(&spec, "company").await?;
                assert_eq!(uri, "http://example.org/company/");

                // Adding the same prefix again fails (POST is create-only).
                assert!(
                    client
                        .add_prefix(&spec, "company", "http://other/")
                        .await
                        .is_err(),
                    "adding an existing prefix should fail"
                );

                // Update it to a new IRI.
                client
                    .update_prefix(&spec, "company", "http://example.org/co/")
                    .await?;
                assert_eq!(
                    client.get_prefix(&spec, "company").await?,
                    "http://example.org/co/"
                );

                // Upsert a brand-new prefix (create-or-update).
                client
                    .upsert_prefix(&spec, "dept", "http://example.org/dept/")
                    .await?;
                assert_eq!(
                    client.get_prefix(&spec, "dept").await?,
                    "http://example.org/dept/"
                );

                // Delete and confirm it's gone.
                client.delete_prefix(&spec, "company").await?;
                assert!(
                    client.get_prefix(&spec, "company").await.is_err(),
                    "deleted prefix should no longer resolve"
                );

                Ok(())
            })
            .await
    }
}

#![recursion_limit = "256"]
//! Live test of the TerminusDB 12 document-API params on the client: `raw_json`
//! (insert unstructured `sys:JSONDocument` with no schema check) round-trips
//! against a 12.1 server.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde_json::json;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;

    #[tokio::test]
    async fn test_v12_raw_json_insert() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_raw_json", |client, spec| async move {
                // An arbitrary, schema-free document. raw_json stores it as a
                // sys:JSONDocument without a schema definition.
                let raw = json!({
                    "@id": "JSONDocument/gadget",
                    "name": "Gadget",
                    "tags": ["a", "b", "c"],
                    "nested": { "count": 3, "ratio": 0.5 }
                });

                let args = DocumentInsertArgs::from(spec.clone()).with_raw_json(true);
                client.post_documents(vec![&raw], args).await?;

                // Read it back (raw docs require raw_json=true on GET as well).
                let got = client
                    .get_document(
                        "JSONDocument/gadget",
                        &spec,
                        GetOpts::default().with_raw_json(true),
                    )
                    .await?;

                eprintln!("raw_json read-back: {}", serde_json::to_string_pretty(&got)?);
                // Top-level fields round-trip through the unstructured store.
                assert_eq!(got.get("name").and_then(|v| v.as_str()), Some("Gadget"));
                assert!(got.get("tags").is_some(), "tags array should survive");
                assert!(got.get("nested").is_some(), "nested object should survive");
                Ok(())
            })
            .await
    }
}

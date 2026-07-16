#![recursion_limit = "256"]
//! Live test of TerminusDB 12 WOQL streaming mode: insert several documents,
//! stream a triple query, and collect the Binding records as they arrive.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;
    use terminusdb_woql2::prelude::{NodeValue, Query, Triple, Value as WoqlValue};

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Person {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[tokio::test]
    async fn test_v12_woql_streaming() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Person,), _, _, _>("v12_streaming", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                for (id, name) in [("alice", "Alice"), ("bob", "Bob"), ("carol", "Carol")] {
                    client
                        .insert_instance(
                            &Person {
                                id: EntityIDFor::new(id).unwrap(),
                                name: name.to_string(),
                            },
                            args.clone(),
                        )
                        .await?;
                }

                // Stream: every (Person, name) pair.
                let query = Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".to_string()),
                    predicate: NodeValue::Node("@schema:name".to_string()),
                    object: WoqlValue::Variable("Name".to_string()),
                    graph: None,
                });

                let stream = client.query_stream(Some(spec.clone()), query).await?;
                futures_util::pin_mut!(stream);

                let mut names = Vec::new();
                while let Some(binding) = stream.next().await {
                    let binding = binding?;
                    if let Some(name) = binding.get("Name") {
                        // Bindings are typed values: {"@type":"xsd:string","@value":"Alice"}
                        let n = name
                            .get("@value")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        names.push(n);
                    }
                }
                names.sort();
                assert_eq!(names, vec!["Alice", "Bob", "Carol"], "streamed all 3 bindings");
                Ok(())
            })
            .await
    }
}

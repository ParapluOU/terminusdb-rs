//! End-to-end: compile XPath -> WOQL and run it against a real TerminusDB
//! server, proving `document("...")/submodel/@prop` returns the stored value.
//!
//! Uses `TerminusDBServer` (per CLAUDE.md) so it runs in parallel with no
//! `#[ignore]` needed.
#![recursion_limit = "256"]

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_woql2::prelude::*;
    use terminusdb_xpath::compile;

    /// A value-carrying subdocument reached via an object property.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(subdocument = true)]
    struct SubModel {
        prop: String,
    }

    /// The root document: `submodel` is an object-property hop to `SubModel`.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct MyModel {
        id: EntityIDFor<Self>,
        submodel: SubModel,
    }

    /// Pull a plain string out of a WOQL binding value (node IRI or literal).
    fn binding_str(v: &serde_json::Value) -> Option<String> {
        if let Some(s) = v.as_str() {
            return Some(s.to_string());
        }
        for key in ["@value", "@id", "node"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                return Some(s.to_string());
            }
        }
        None
    }

    async fn run_query(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        query: Query,
    ) -> anyhow::Result<Vec<HashMap<String, serde_json::Value>>> {
        let res = client
            .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), query)
            .await?;
        Ok(res.bindings)
    }

    #[tokio::test]
    async fn xpath_navigates_document_to_value_property() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        // One known instance: MyModel/root1 -> submodel -> prop="hello-xpath".
        let inst = MyModel {
            id: EntityIDFor::new("root1")?,
            submodel: SubModel {
                prop: "hello-xpath".to_string(),
            },
        };
        server
            // with_db_seed inserts the MyModel schema tree (incl. the
            // SubModel subdoc) and the instance.
            .with_db_seed("test_xpath_e2e", vec![inst], |client, spec| async move {
                // --- 1. Relative path: `submodel/@prop` needs no subject IRI. ---
                let compiled = compile("submodel/@prop").expect("compile relative xpath");
                let bindings = run_query(&client, &spec, compiled.query.clone()).await?;
                let value = bindings
                    .iter()
                    .find_map(|b| b.get(&compiled.result_var).and_then(binding_str))
                    .expect("relative xpath should bind the @prop value");
                assert_eq!(value, "hello-xpath", "relative path result");

                // --- 2. Discover the document's WOQL subject IRI. ---
                let discover = Query::Select(Select {
                    variables: vec!["S".to_string()],
                    query: Box::new(Query::Triple(Triple {
                        subject: NodeValue::Variable("S".to_string()),
                        predicate: NodeValue::Node("rdf:type".to_string()),
                        object: Value::Node("@schema:MyModel".to_string()),
                        graph: Some(GraphType::Instance),
                    })),
                });
                let subject_iri = run_query(&client, &spec, discover)
                    .await?
                    .iter()
                    .find_map(|b| b.get("S").and_then(binding_str))
                    .expect("should find the MyModel document IRI");

                // --- 3. The flagship: `document("<iri>")/submodel/@prop`. ---
                let xpath = format!(r#"document("{subject_iri}")/submodel/@prop"#);
                let compiled = compile(&xpath).expect("compile document() xpath");
                let bindings = run_query(&client, &spec, compiled.query.clone()).await?;
                let value = bindings
                    .iter()
                    .find_map(|b| b.get(&compiled.result_var).and_then(binding_str))
                    .unwrap_or_else(|| {
                        panic!("document() xpath returned no @prop binding; bindings={bindings:?}")
                    });
                assert_eq!(value, "hello-xpath", "document() path result");

                Ok(())
            })
            .await
    }
}

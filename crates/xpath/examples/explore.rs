//! Interactive-ish WOQL exploration harness — the iteration loop for developing
//! this crate against a real embedded TerminusDB.
//!
//! It boots an in-memory `TerminusDBServer`, loads a tiny schema + data, then for
//! each XPath expression prints:
//!   1. the compiled WOQL (DSL + JSON-LD) via `terminusdb_xpath::explain`, and
//!   2. the live result rows — or the server's error.
//!
//! This makes WOQL *observable*: when a query returns nothing or errors, you can
//! see exactly what was sent. Add expressions to `PROBES`, then:
//!
//!     cargo run -p terminusdb-xpath --example explore
//!
//! (Run from anywhere in the workspace.)
#![recursion_limit = "256"]

use std::collections::HashMap;

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql2::prelude::*;
use terminusdb_xpath::{compile, explain};

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true)]
struct Address {
    city: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
    address: Address,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("xpath_explore", |client, spec| async move {
            // --- seed a tiny dataset ---
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_schemas::<(Person,)>(args.clone()).await?;
            for (id, name, age, city) in [
                ("jane", "Jane", 30, "Berlin"),
                ("john", "John", 25, "Paris"),
            ] {
                client
                    .insert_instance(
                        &Person {
                            id: EntityIDFor::new(id).unwrap(),
                            name: name.to_string(),
                            age,
                            address: Address {
                                city: city.to_string(),
                            },
                        },
                        args.clone(),
                    )
                    .await?;
            }

            // Discover Jane's real subject IRI (don't assume the id format).
            let jane_iri = discover_iri_by_name(&client, &spec, "Jane").await?;

            // --- probes: (description, xpath) ---
            let probes: Vec<(&str, String)> = vec![
                ("value property on a document", format!(r#"document("{jane_iri}")/@name"#)),
                ("object hop then value", format!(r#"document("{jane_iri}")/address/@city"#)),
                ("relative: all names", "@name".to_string()),
                ("relative: hop + predicate filter", r#"address[@city = "Berlin"]/@city"#.to_string()),
                ("numeric predicate", r#"address[@city = "Paris"]/@city"#.to_string()),
                ("silently-empty (bad subject)", r#"document("Person/nobody")/@name"#.to_string()),
            ];

            for (desc, expr) in &probes {
                probe(&client, &spec, desc, expr).await;
            }

            Ok(())
        })
        .await
}

/// Compile, explain, and run one XPath expression, printing a full report.
async fn probe(client: &TerminusDBHttpClient, spec: &BranchSpec, desc: &str, expr: &str) {
    println!("\n\n========================= {desc}");
    match explain(expr) {
        Ok(ex) => println!("{}", ex.report()),
        Err(e) => {
            println!("COMPILE ERROR: {e}");
            return;
        }
    }
    let compiled = compile(expr).expect("already explained");
    match client
        .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), compiled.query)
        .await
    {
        Ok(res) => {
            let rows: Vec<_> = res
                .bindings
                .iter()
                .filter_map(|b| b.get(&compiled.result_var))
                .collect();
            println!("--- {} result row(s) for ?{} ---", rows.len(), compiled.result_var);
            for row in rows {
                println!("  {row}");
            }
            if res.bindings.is_empty() {
                println!("  (no bindings — the query ran but matched nothing)");
            }
        }
        Err(e) => println!("SERVER ERROR: {e}"),
    }
}

/// Find a document's subject IRI by its `name` value.
async fn discover_iri_by_name(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    name: &str,
) -> anyhow::Result<String> {
    let q = Query::Select(Select {
        variables: vec!["S".to_string()],
        query: Box::new(Query::Triple(Triple {
            subject: NodeValue::Variable("S".to_string()),
            predicate: NodeValue::Node("@schema:name".to_string()),
            object: Value::Data(XSDAnySimpleType::String(name.to_string())),
            graph: Some(GraphType::Instance),
        })),
    });
    let res = client
        .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), q)
        .await?;
    res.bindings
        .iter()
        .find_map(|b| {
            b.get("S").and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.get("@id").and_then(|x| x.as_str()).map(str::to_string))
            })
        })
        .ok_or_else(|| anyhow::anyhow!("no document with name={name}"))
}

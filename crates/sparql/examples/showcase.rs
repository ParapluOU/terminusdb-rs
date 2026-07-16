//! # SPARQL → WOQL proof of concept, live against TerminusDB
//!
//! Compiles SPARQL `SELECT` queries into WOQL and runs them against a real
//! embedded TerminusDB, printing the compiled WOQL (DSL) alongside the results.
//!
//! | SPARQL                          | TerminusDB / WOQL                       |
//! |---------------------------------|-----------------------------------------|
//! | `?p a schema:Person`            | `triple(?p, rdf:type, @schema:Person)`  |
//! | `?p schema:name ?n`             | `triple(?p, @schema:name, ?n)`          |
//! | `{ A } UNION { B }`             | `or(A, B)`                              |
//! | `OPTIONAL { ... }`              | `opt(...)`                              |
//! | `FILTER(?a > 26)`               | `greater(?a, 26)`                       |
//! | `ORDER BY DESC(?a) LIMIT n`     | `limit(n, ... order_by([desc(?a)] ...))`|
//!
//! Run with:
//!     cargo run -p terminusdb-sparql --example showcase --features client
#![recursion_limit = "256"]

use std::collections::HashMap;

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, FromTuple, TerminusDBModel};
use terminusdb_sparql::explain;

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, FromTuple)]
#[tdb(id_field = "id")]
struct Company {
    id: EntityIDFor<Self>,
    name: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, FromTuple)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
    /// Optional → a nullable property (absent = OPTIONAL won't bind).
    nickname: Option<String>,
    /// A real graph edge to a Company (`Ref<T>`), so `?p schema:employer ?c`
    /// followed by `?c schema:name ?cn` is a genuine join across the edge.
    employer: Ref<Company>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Person, Company), _, _, _>("sparql_showcase", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for c in Company::from_tuples([("acme", "Acme Corp"), ("globex", "Globex")]) {
                client.insert_instance(&c, args.clone()).await?;
            }
            for p in Person::from_tuples([
                ("jane", "Jane", 30, Some("Janey"), "acme"),
                ("john", "John", 25, None::<&str>, "acme"),
                ("mary", "Mary", 40, None::<&str>, "globex"),
                ("zoe", "Zoe", 35, Some("Z"), "globex"),
            ]) {
                client.insert_instance(&p, args.clone()).await?;
            }

            banner("SPARQL → WOQL, live against TerminusDB");
            println!("  database `{}`\n", spec.db);

            let steps: &[(&str, &str)] = &[
                (
                    "Basic graph pattern",
                    "SELECT ?name WHERE { ?p a s:Person . ?p s:name ?name }",
                ),
                (
                    "FILTER with a numeric range",
                    "SELECT ?name ?age WHERE { ?p a s:Person . ?p s:name ?name . ?p s:age ?age \
                     . FILTER(?age > 26 && ?age < 40) }",
                ),
                (
                    "OPTIONAL (nickname may be absent)",
                    "SELECT ?name ?nick WHERE { ?p a s:Person . ?p s:name ?name . \
                     OPTIONAL { ?p s:nickname ?nick } }",
                ),
                (
                    "UNION of two filtered patterns",
                    "SELECT ?name WHERE { { ?p s:name ?name . FILTER(?name = \"Jane\") } \
                     UNION { ?p s:name ?name . FILTER(?name = \"Mary\") } }",
                ),
                (
                    "ORDER BY + LIMIT",
                    "SELECT ?name ?age WHERE { ?p a s:Person . ?p s:name ?name . ?p s:age ?age } \
                     ORDER BY DESC(?age) LIMIT 3",
                ),
                (
                    "DISTINCT across an object-property join",
                    "SELECT DISTINCT ?cn WHERE { ?p s:employer ?c . ?c s:name ?cn }",
                ),
                (
                    "Join + filter on the joined entity",
                    "SELECT ?name ?cn WHERE { ?p s:name ?name . ?p s:employer ?c . \
                     ?c s:name ?cn . FILTER(?cn = \"Acme Corp\") }",
                ),
            ];

            for (i, (desc, body)) in steps.iter().enumerate() {
                run_step(&client, &spec, i + 1, desc, body).await?;
            }

            banner("Unsupported constructs fail loudly");
            let bad = "PREFIX s: <http://terminusdb.com/schema#> \
                       SELECT (COUNT(?p) AS ?c) WHERE { ?p a s:Person }";
            println!("  SPARQL\n    SELECT (COUNT(?p) AS ?c) ...\n");
            match terminusdb_sparql::compile(bad) {
                Ok(_) => println!("  (unexpectedly compiled)"),
                Err(e) => println!("  → rejected: {e}"),
            }
            println!();
            Ok(())
        })
        .await
}

const PREFIX: &str = "PREFIX s: <http://terminusdb.com/schema#>";

async fn run_step(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    n: usize,
    desc: &str,
    body: &str,
) -> anyhow::Result<()> {
    let sparql = format!("{PREFIX} {body}");
    println!("\n{}", "─".repeat(78));
    println!(" {n}. {desc}");
    println!("{}", "─".repeat(78));
    println!("\n  SPARQL\n    {}", body.split_whitespace().collect::<Vec<_>>().join(" "));

    let ex = explain(&sparql).map_err(|e| anyhow::anyhow!("compile `{sparql}`: {e}"))?;
    println!("\n  Compiled WOQL (DSL)\n    {}", ex.dsl);

    let compiled = terminusdb_sparql::compile(&sparql)?;
    let res = client
        .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), compiled.query.clone())
        .await?;
    println!(
        "\n  Result ({} row{})",
        res.bindings.len(),
        if res.bindings.len() == 1 { "" } else { "s" }
    );
    print_table(&compiled.variables, &res.bindings);
    Ok(())
}

fn print_table(vars: &[String], rows: &[HashMap<String, serde_json::Value>]) {
    if rows.is_empty() {
        println!("    (no rows)");
        return;
    }
    println!("    {}", vars.join("  |  "));
    for row in rows {
        let cells: Vec<String> = vars
            .iter()
            .map(|v| row.get(v).map(scalar).unwrap_or_else(|| "—".to_string()))
            .collect();
        println!("    {}", cells.join("  |  "));
    }
}

fn scalar(v: &serde_json::Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(val) = v.get("@value") {
        return val.as_str().map(str::to_string).unwrap_or_else(|| val.to_string());
    }
    if let Some(id) = v.get("@id").and_then(|x| x.as_str()) {
        return id.to_string();
    }
    v.to_string()
}

fn banner(title: &str) {
    println!("\n{}", "═".repeat(78));
    println!(" {title}");
    println!("{}", "═".repeat(78));
}

//! # XPath → WOQL proof of concept
//!
//! A guided walkthrough for a TerminusDB maintainer. It boots an in-memory
//! TerminusDB, defines a small document graph, seeds data, then runs a series of
//! XPath expressions. For **each** expression it prints:
//!
//! 1. the **XPath** query,
//! 2. the **generated WOQL** — both the readable DSL and the exact JSON-LD that
//!    is POSTed to `/api/woql`,
//! 3. the **live result** returned by TerminusDB.
//!
//! Run it:
//! ```text
//! cargo run -p terminusdb-xpath --example showcase
//! ```
//!
//! ## The idea
//!
//! XPath is a path language for trees; TerminusDB is a document/graph store.
//! The compiler maps XPath navigation onto WOQL graph patterns:
//!
//! | XPath                     | TerminusDB meaning                              | WOQL                                   |
//! | ------------------------- | ----------------------------------------------- | -------------------------------------- |
//! | `document("Employee/1")`  | start from a document (subject IRI)             | subject node                           |
//! | child step `employer`     | follow an **object property** (an edge/link)    | `triple(S, @schema:employer, O)`       |
//! | attribute step `@name`    | read a **value property** (a literal)           | `triple(S, @schema:name, Lit)`         |
//! | `a/b/c`                    | chained hops (a join on shared variables)       | `and(triple…, triple…, triple…)`       |
//! | `//city`                  | reach `city` via **any** chain of edges         | `path(S, star(<any>) . @schema:city)`  |
//! | `[@founded > 1990]`        | a predicate/filter on the step's node           | extra `triple` + `greater(…)`          |
//! | the final step            | the projected result                            | `select([$last], …)`                   |
//!
//! Unqualified names get the `@schema:` prefix that TerminusDB uses for
//! user-defined properties. The same lowering works whether an object-property
//! edge points to a subdocument (as below) or to another top-level document —
//! it is a graph edge either way.

#![recursion_limit = "256"]

use std::collections::HashMap;

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, FromTuple, TerminusDBModel};
use terminusdb_xpath::{compile, explain, Explanation};

// ===========================================================================
// The document models  —  Employee ──employer──▶ Company ──headquarters──▶ Address
// ===========================================================================

/// A postal address. A **subdocument**: stored inline in its owner, but still a
/// node in the graph reachable through the owner's object-property edge. Both
/// fields are value properties (literals).
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, FromTuple)]
#[tdb(subdocument = true)]
struct Address {
    city: String,
    country: String,
}

/// A company. Also a subdocument here (embedded in the employee).
/// - `name`, `founded` are **value properties**,
/// - `headquarters` is an **object property** edge to an `Address`.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, FromTuple)]
#[tdb(subdocument = true)]
struct Company {
    name: String,
    founded: i32,
    headquarters: Address,
}

/// An employee: a top-level document (has its own IRI, e.g. `Employee/alice`).
/// - `name`, `salary` are **value properties**,
/// - `employer` is an **object property** edge to a `Company`.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, FromTuple)]
#[tdb(id_field = "id")]
struct Employee {
    id: EntityIDFor<Self>,
    name: String,
    salary: i32,
    employer: Company,
}

// ===========================================================================
// Walkthrough
// ===========================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    // The seed data. `Company`/`Address` are subdocuments, so each `Employee`
    // carries them inline. `FromTuple` builds each model from a tuple (nested
    // models included), and `from_tuples` builds the whole `Vec`.
    let company = |name: &str, founded: i32, city: &str, country: &str| {
        Company::from((name, founded, Address::from((city, country))))
    };
    let employees = Employee::from_tuples([
        ("alice", "Alice", 95000, company("Acme Corp", 1999, "London", "UK")),
        ("bob", "Bob", 80000, company("Globex", 2010, "Berlin", "DE")),
        ("carol", "Carol", 120000, company("Acme Corp", 1999, "London", "UK")),
    ]);

    // with_db_seed inserts the Employee schema tree (Company + Address are
    // pulled in) AND the instances — no hand-written seed step needed.
    server
        .with_db_seed("xpath_showcase", employees, |client, spec| async move {
            banner("XPath → WOQL   ·   proof of concept");
            println!(
                "Graph:  Employee ──employer──▶ Company ──headquarters──▶ Address\n\
                 Data:   Alice  (salary 95000)  @ Acme Corp (founded 1999, HQ London / UK)\n\
                 \x20       Bob    (salary 80000)  @ Globex    (founded 2010, HQ Berlin / DE)\n\
                 \x20       Carol  (salary 120000) @ Acme Corp (founded 1999, HQ London / UK)"
            );

            // Ids via the typed helper — never hand-format IRIs.
            let alice = EntityIDFor::<Employee>::new("alice")?;
            let carol = EntityIDFor::<Employee>::new("carol")?;
            let doc = |id: &EntityIDFor<Employee>, rest: &str| {
                format!(r#"document("{}"){rest}"#, id.typed())
            };

            // Each case deserializes the result into a concrete Rust type (note
            // the `i64` rows and the final `Employee` model) and asserts it — the
            // walkthrough both shows and verifies exactly what comes out.

            present::<String>(&client, &spec, 1,
                "Value property — `@attr` reads a literal.",
                &doc(&alice, "/@name"), vec!["Alice".into()]).await?;

            present::<i64>(&client, &spec, 2,
                "Numeric value property — deserialized as `i64`, not a string.",
                &doc(&alice, "/@salary"), vec![95000]).await?;

            present::<String>(&client, &spec, 3,
                "Object-property hop — a child step follows an edge to another node.",
                &doc(&alice, "/employer/@name"), vec!["Acme Corp".into()]).await?;

            present::<String>(&client, &spec, 4,
                "Multi-hop — employer, then into its headquarters, then read the city.",
                &doc(&alice, "/employer/headquarters/@city"), vec!["London".into()]).await?;

            present::<String>(&client, &spec, 5,
                "`//` descendant — reach `headquarters` via ANY chain of edges.",
                &doc(&alice, "//headquarters/@city"), vec!["London".into()]).await?;

            present::<i64>(&client, &spec, 6,
                "Predicate (numeric filter) on a hopped node; return the founding year.",
                &doc(&carol, "/employer[@founded > 1990]/@founded"), vec![1999]).await?;

            present::<String>(&client, &spec, 7,
                "Predicate (nested relative path) — filter by the HQ's country.",
                &doc(&alice, r#"/employer[headquarters/@country = "UK"]/@name"#),
                vec!["Acme Corp".into()]).await?;

            present::<String>(&client, &spec, 8,
                "Relative path (no `document()`) — scans ALL documents for employers' names.",
                "employer/@name", vec!["Acme Corp".into(), "Globex".into()]).await?;

            // A node result read back as its full typed model — nested
            // subdocuments and all. The XPath filters out the models for you.
            let employees = present_model::<Employee>(&client, &spec, 9,
                "Bare `document(...)` selects the node — read it back as a full model.",
                &doc(&alice, "")).await?;
            assert_eq!(employees.len(), 1);
            assert_eq!(employees[0].employer.headquarters.city, "London");
            println!();

            Ok(())
        })
        .await
}

/// A literal WOQL binding value, e.g. `{"@type": "xsd:integer", "@value": 95000}`.
/// Typing the HashMap value as this (rather than `serde_json::Value`) lets serde
/// pull the `@value` straight into `T` — no manual unwrapping.
#[derive(serde::Deserialize, Debug)]
struct Literal<T> {
    #[serde(rename = "@value")]
    value: T,
}

/// Run an XPath whose result is a scalar and deserialize each into `T`.
/// Result type is `HashMap<String, Literal<T>>` — fully typed, no `serde_json::Value`.
async fn run_typed<T: serde::de::DeserializeOwned + std::fmt::Debug>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    xpath: &str,
) -> anyhow::Result<Vec<T>> {
    let compiled = compile(xpath)?;
    let res = client
        .query::<HashMap<String, Literal<T>>>(Some(spec.clone()), compiled.query)
        .await?;
    Ok(res
        .bindings
        .into_iter()
        .filter_map(|mut b| b.remove(&compiled.result_var).map(|cell| cell.value))
        .collect())
}

/// Run an XPath whose result is a node and read each back as a full model `M`.
/// Result type is `HashMap<String, M>` — the model deserializes directly from the
/// `read_document` JSON (no manual `from_json`).
async fn read_models<M: serde::de::DeserializeOwned + std::fmt::Debug>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    xpath: &str,
) -> anyhow::Result<Vec<M>> {
    let compiled = compile(xpath)?;
    // Wrap the query so the selected node is read as a document into `Doc`.
    let query = compiled.read_documents_query("Doc");
    let res = client
        .query::<HashMap<String, M>>(Some(spec.clone()), query)
        .await?;
    Ok(res.bindings.into_iter().filter_map(|mut b| b.remove("Doc")).collect())
}

/// Print the case header + XPath + the generated WOQL (DSL and JSON-LD).
fn print_header(n: usize, description: &str, xpath: &str, ex: &Explanation) {
    println!("\n{}", "─".repeat(78));
    println!(" {n}. {description}");
    println!("{}", "─".repeat(78));
    println!("\n  XPath\n    {xpath}");
    println!("\n  Generated WOQL (DSL)\n    {}", ex.dsl);
    println!(
        "\n  Generated WOQL (JSON-LD, POSTed to /api/woql)\n{}",
        indent(&serde_json::to_string_pretty(&ex.json).unwrap(), "    ")
    );
}

/// A scalar case: deserialize the result set into `Vec<T>`, show it, assert it.
async fn present<T>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    n: usize,
    description: &str,
    xpath: &str,
    mut expected: Vec<T>,
) -> anyhow::Result<()>
where
    T: serde::de::DeserializeOwned + std::fmt::Debug + Ord,
{
    let ex = explain(xpath).map_err(|e| anyhow::anyhow!("compile `{xpath}`: {e}"))?;
    print_header(n, description, xpath, &ex);

    // XPath yields a node-set, so present distinct values.
    let mut got: Vec<T> = run_typed(client, spec, xpath).await?;
    got.sort();
    got.dedup();
    expected.sort();
    expected.dedup();

    println!(
        "\n  Result — typed Vec<{}>\n    {got:?}    ← asserted == {expected:?}",
        short_type::<T>(),
    );
    assert_eq!(got, expected, "xpath `{xpath}`");
    Ok(())
}

/// A node case: read each result node back as a full model `M` and show it.
async fn present_model<M>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    n: usize,
    description: &str,
    xpath: &str,
) -> anyhow::Result<Vec<M>>
where
    M: serde::de::DeserializeOwned + std::fmt::Debug,
{
    let ex = explain(xpath).map_err(|e| anyhow::anyhow!("compile `{xpath}`: {e}"))?;
    print_header(n, description, xpath, &ex);

    let got: Vec<M> = read_models(client, spec, xpath).await?;
    println!(
        "\n  Result — typed Vec<{}> (via read_document)\n{}",
        short_type::<M>(),
        indent(&format!("{got:#?}"), "    "),
    );
    Ok(got)
}

/// Last segment of a type name (`showcase::Employee` → `Employee`).
fn short_type<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

// ===========================================================================
// Seed + helpers
// ===========================================================================

/// Construct an `Employee` with a typed id.
fn banner(title: &str) {
    println!("\n{}", "═".repeat(78));
    println!(" {title}");
    println!("{}", "═".repeat(78));
}

fn indent(s: &str, pad: &str) -> String {
    s.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

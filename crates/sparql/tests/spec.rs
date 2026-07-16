//! Live SPARQL→WOQL spec suite.
//!
//! Every case runs against a real embedded TerminusDB. All cases share ONE
//! per-process server (`TerminusDBServer::test_instance()`); each test gets its
//! OWN isolated tmp database (`with_db_schema`), so they run in parallel without
//! interfering. Nothing here is proven until the compiled WOQL is executed and
//! returns the expected bindings — that is the whole point of this file.
#![recursion_limit = "256"]

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::future::Future;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, FromTuple, TerminusDBModel};

    // --- schema -------------------------------------------------------------

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
        /// Optional → absent for some people; exercises OPTIONAL.
        nickname: Option<String>,
        /// A real graph edge to a Company (`Ref<T>` = traversable link), so
        /// `?p s:employer ?c . ?c s:name ?cn` is a genuine join across an edge.
        employer: Ref<Company>,
    }

    // --- harness ------------------------------------------------------------

    /// Seed a fresh isolated db with companies + people, then run the closure.
    async fn with_dataset<F, Fut>(f: F) -> anyhow::Result<()>
    where
        F: FnOnce(TerminusDBHttpClient, BranchSpec) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Person, Company), _, _, _>("sparql_spec", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                // Companies first (people link to them).
                for c in Company::from_tuples([("acme", "Acme"), ("globex", "Globex")]) {
                    client.insert_instance(&c, args.clone()).await?;
                }
                // (id, name, age, nickname, employer-id)
                for p in Person::from_tuples([
                    ("jane", "Jane", 30, Some("Janey"), "acme"),
                    ("john", "John", 25, None::<&str>, "acme"),
                    ("mary", "Mary", 40, None::<&str>, "globex"),
                ]) {
                    client.insert_instance(&p, args.clone()).await?;
                }
                f(client, spec).await
            })
            .await
    }

    const PREFIX: &str = "PREFIX s: <http://terminusdb.com/schema#>";

    /// Compile `sparql`, run it, and return the (sorted) string values bound to
    /// `var` across all solutions.
    async fn run(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        sparql: &str,
        var: &str,
    ) -> anyhow::Result<Vec<String>> {
        let compiled = terminusdb_sparql::compile(sparql)
            .map_err(|e| anyhow::anyhow!("compile `{sparql}`: {e}"))?;
        let res = client
            .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), compiled.query.clone())
            .await
            .map_err(|e| anyhow::anyhow!("run `{sparql}`: {e}"))?;
        let mut got: Vec<String> = res
            .bindings
            .iter()
            .filter_map(|b| b.get(var).and_then(binding_scalar))
            .collect();
        got.sort();
        Ok(got)
    }

    /// Assert (order-insensitive) that `var`'s bindings equal `expected`.
    async fn assert_sparql(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        sparql: &str,
        var: &str,
        expected: &[&str],
    ) -> anyhow::Result<()> {
        let got = run(client, spec, sparql, var).await?;
        let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(got, want, "sparql `{sparql}` (var ?{var})");
        Ok(())
    }

    /// Extract a scalar (literal or node IRI) from a WOQL binding value.
    fn binding_scalar(v: &serde_json::Value) -> Option<String> {
        if let Some(s) = v.as_str() {
            return Some(s.to_string());
        }
        match v.get("@value") {
            Some(serde_json::Value::String(s)) => return Some(s.clone()),
            Some(other) if !other.is_null() => return Some(other.to_string()),
            _ => {}
        }
        v.get("@id").and_then(|x| x.as_str()).map(str::to_string)
    }

    // --- cases --------------------------------------------------------------

    #[tokio::test]
    async fn basic_graph_pattern() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!("{PREFIX} SELECT ?name WHERE {{ ?p a s:Person . ?p s:name ?name }}");
            assert_sparql(&client, &spec, &q, "name", &["Jane", "John", "Mary"]).await
        })
        .await
    }

    #[tokio::test]
    async fn filter_numeric_comparison() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!(
                "{PREFIX} SELECT ?name WHERE {{ ?p s:name ?name . ?p s:age ?age . FILTER(?age > 26) }}"
            );
            assert_sparql(&client, &spec, &q, "name", &["Jane", "Mary"]).await
        })
        .await
    }

    #[tokio::test]
    async fn filter_conjunction_range() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!(
                "{PREFIX} SELECT ?name WHERE {{ ?p s:name ?name . ?p s:age ?age . \
                 FILTER(?age > 26 && ?age < 40) }}"
            );
            assert_sparql(&client, &spec, &q, "name", &["Jane"]).await
        })
        .await
    }

    #[tokio::test]
    async fn filter_string_equality() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!(
                "{PREFIX} SELECT ?age WHERE {{ ?p s:name ?name . ?p s:age ?age . \
                 FILTER(?name = \"Mary\") }}"
            );
            assert_sparql(&client, &spec, &q, "age", &["40"]).await
        })
        .await
    }

    #[tokio::test]
    async fn optional_binds_when_present() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            // Everyone appears (3 names); only Jane has a nickname. The
            // `a s:Person` guard keeps Company subjects (which also have a
            // `name`) out of the result — a triple pattern with no type
            // constraint matches every subject bearing that predicate.
            let q = format!(
                "{PREFIX} SELECT ?name ?nick WHERE {{ ?p a s:Person . ?p s:name ?name . \
                 OPTIONAL {{ ?p s:nickname ?nick }} }}"
            );
            assert_sparql(&client, &spec, &q, "name", &["Jane", "John", "Mary"]).await?;
            assert_sparql(&client, &spec, &q, "nick", &["Janey"]).await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn union_of_filters() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!(
                "{PREFIX} SELECT ?name WHERE {{ \
                 {{ ?p s:name ?name . FILTER(?name = \"Jane\") }} \
                 UNION \
                 {{ ?p s:name ?name . FILTER(?name = \"Mary\") }} }}"
            );
            assert_sparql(&client, &spec, &q, "name", &["Jane", "Mary"]).await
        })
        .await
    }

    #[tokio::test]
    async fn order_by_desc_with_limit() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            // Oldest two, descending: Mary (40), Jane (30).
            let q = format!(
                "{PREFIX} SELECT ?name WHERE {{ ?p s:name ?name . ?p s:age ?age }} \
                 ORDER BY DESC(?age) LIMIT 2"
            );
            // ORDER BY matters here, so check the exact sequence (not sorted).
            let compiled = terminusdb_sparql::compile(&q).unwrap();
            let res = client
                .query::<HashMap<String, serde_json::Value>>(
                    Some(spec.clone()),
                    compiled.query.clone(),
                )
                .await?;
            let names: Vec<String> = res
                .bindings
                .iter()
                .filter_map(|b| b.get("name").and_then(binding_scalar))
                .collect();
            assert_eq!(names, vec!["Mary".to_string(), "Jane".to_string()]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn distinct_collapses_duplicates() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            // Two people at Acme, one at Globex → 2 distinct employer names.
            let q = format!(
                "{PREFIX} SELECT DISTINCT ?cn WHERE {{ ?p s:employer ?c . ?c s:name ?cn }}"
            );
            assert_sparql(&client, &spec, &q, "cn", &["Acme", "Globex"]).await?;
            // Without DISTINCT the Acme row appears twice (bag semantics).
            let bag = format!("{PREFIX} SELECT ?cn WHERE {{ ?p s:employer ?c . ?c s:name ?cn }}");
            let got = run(&client, &spec, &bag, "cn").await?;
            assert_eq!(got, vec!["Acme", "Acme", "Globex"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn join_across_object_property() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            // Follow the employer edge, then filter on the company's name.
            let q = format!(
                "{PREFIX} SELECT ?name WHERE {{ \
                 ?p s:name ?name . ?p s:employer ?c . ?c s:name ?cn . \
                 FILTER(?cn = \"Acme\") }}"
            );
            assert_sparql(&client, &spec, &q, "name", &["Jane", "John"]).await
        })
        .await
    }

    #[tokio::test]
    async fn select_star_binds_all_variables() -> anyhow::Result<()> {
        with_dataset(|client, spec| async move {
            let q = format!("{PREFIX} SELECT * WHERE {{ ?p s:name ?name . ?p s:age ?age }}");
            let compiled = terminusdb_sparql::compile(&q).unwrap();
            let res = client
                .query::<HashMap<String, serde_json::Value>>(
                    Some(spec.clone()),
                    compiled.query.clone(),
                )
                .await?;
            assert_eq!(res.bindings.len(), 3, "one row per person");
            // Every row binds the subject, the name, and the age.
            for b in &res.bindings {
                assert!(b.contains_key("p"));
                assert!(b.contains_key("name"));
                assert!(b.contains_key("age"));
            }
            Ok(())
        })
        .await
    }
}

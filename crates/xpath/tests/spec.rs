//! Live XPath→WOQL spec suite.
//!
//! Every case runs against a real embedded TerminusDB. All cases in this file
//! share ONE per-process server (`TerminusDBServer::test_instance()`); each test
//! gets its OWN isolated tmp database (`with_tmp_db`), so they run in parallel
//! without interfering. This is the loop for growing XPath coverage: add a case,
//! run it, and if it errors or returns nothing, use `terminusdb_xpath::explain`
//! or `examples/explore.rs` to see the compiled WOQL.
#![recursion_limit = "256"]

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::future::Future;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    // --- schema -------------------------------------------------------------

    /// A value-bearing subdocument reached via an object property.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(subdocument = true)]
    struct Address {
        city: String,
        zip: i32,
    }

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Person {
        id: EntityIDFor<Self>,
        name: String,
        age: i32,
        address: Address,
    }

    /// Typed ids for the seeded people (never hand-format IRIs — go through
    /// `EntityIDFor` / `TdbIRI`).
    struct Ids {
        jane: EntityIDFor<Person>,
        john: EntityIDFor<Person>,
    }

    // --- harness ------------------------------------------------------------

    /// Seed a fresh isolated db with two people and hand the test its client,
    /// spec, and the discovered document IRIs.
    async fn with_dataset<F, Fut>(f: F) -> anyhow::Result<()>
    where
        F: FnOnce(TerminusDBHttpClient, BranchSpec, Ids) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let server = TerminusDBServer::test_instance().await?;
        // with_db_seed inserts the Person schema tree (incl. the Address
        // subdoc) and the instances.
        let people = vec![
            person("jane", "Jane", 30, "Berlin", 10115)?,
            person("john", "John", 25, "Paris", 75001)?,
        ];
        server
            .with_db_seed("xpath_spec", people, |client, spec| async move {
                let ids = Ids {
                    jane: EntityIDFor::new("jane")?,
                    john: EntityIDFor::new("john")?,
                };
                f(client, spec, ids).await
            })
            .await
    }

    fn person(id: &str, name: &str, age: i32, city: &str, zip: i32) -> anyhow::Result<Person> {
        Ok(Person {
            id: EntityIDFor::new(id)?,
            name: name.to_string(),
            age,
            address: Address {
                city: city.to_string(),
                zip,
            },
        })
    }

    /// Compile `xpath`, run it, and assert the (order-insensitive) result values.
    async fn assert_xpath(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        xpath: &str,
        expected: &[&str],
    ) -> anyhow::Result<()> {
        let compiled = terminusdb_xpath::compile(xpath)
            .map_err(|e| anyhow::anyhow!("compile `{xpath}`: {e}"))?;
        let res = client
            .query::<HashMap<String, serde_json::Value>>(Some(spec.clone()), compiled.query.clone())
            .await
            .map_err(|e| anyhow::anyhow!("run `{xpath}`: {e}"))?;

        let mut got: Vec<String> = res
            .bindings
            .iter()
            .filter_map(|b| b.get(&compiled.result_var).and_then(binding_scalar))
            .collect();
        got.sort();
        let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(got, want, "xpath `{xpath}`");
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
    async fn value_and_object_steps() -> anyhow::Result<()> {
        with_dataset(|client, spec, ids| async move {
            // @attr = value property (literal)
            assert_xpath(&client, &spec, &fmt("{jane}/@name", &ids), &["Jane"]).await?;
            // numeric value
            assert_xpath(&client, &spec, &fmt("{jane}/@age", &ids), &["30"]).await?;
            // child step = object-property hop, then value
            assert_xpath(&client, &spec, &fmt("{jane}/address/@city", &ids), &["Berlin"]).await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn relative_paths_scan_all_documents() -> anyhow::Result<()> {
        with_dataset(|client, spec, _ids| async move {
            assert_xpath(&client, &spec, "@name", &["Jane", "John"]).await?;
            assert_xpath(&client, &spec, "address/@city", &["Berlin", "Paris"]).await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn predicates() -> anyhow::Result<()> {
        with_dataset(|client, spec, _ids| async move {
            // equality predicate (string)
            assert_xpath(&client, &spec, r#"address[@city = "Berlin"]/@zip"#, &["10115"]).await?;
            // comparison predicate (int)
            assert_xpath(&client, &spec, "address[@zip > 70000]/@city", &["Paris"]).await?;
            // existence predicate
            assert_xpath(&client, &spec, "address[@city]/@city", &["Berlin", "Paris"]).await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn document_accepts_short_id_and_full_iri() -> anyhow::Result<()> {
        with_dataset(|client, spec, ids| async move {
            // Short id ("Person/jane") and full IRI ("terminusdb:///data/Person/jane")
            // — both taken from the typed id, never hand-formatted.
            let short = ids.jane.typed().to_string();
            let full = ids.jane.iri_string();
            assert_xpath(&client, &spec, &format!(r#"document("{short}")/@name"#), &["Jane"])
                .await?;
            assert_xpath(&client, &spec, &format!(r#"document("{full}")/@name"#), &["Jane"])
                .await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn document_matches_relative() -> anyhow::Result<()> {
        with_dataset(|client, spec, ids| async move {
            assert_xpath(&client, &spec, &fmt("{john}/address/@city", &ids), &["Paris"]).await?;
            Ok(())
        })
        .await
    }

    /// The type-safe builder (no strings) runs against the DB. `doc(id)` takes
    /// the typed `EntityIDFor<Person>` directly.
    #[tokio::test]
    async fn typed_builder() -> anyhow::Result<()> {
        use terminusdb_xpath::builder::{attr, doc};
        with_dataset(|client, spec, ids| async move {
            // doc(jane)/address[@city = "Berlin"]/@zip
            let compiled = doc(ids.jane.clone())
                .child("address")
                .filter(attr("city").eq("Berlin"))
                .attr("zip")
                .compile()?;
            let res = client
                .query::<HashMap<String, serde_json::Value>>(
                    Some(spec.clone()),
                    compiled.query.clone(),
                )
                .await?;
            let got: Vec<String> = res
                .bindings
                .iter()
                .filter_map(|b| b.get(&compiled.result_var).and_then(binding_scalar))
                .collect();
            assert_eq!(got, vec!["10115".to_string()]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn descendant() -> anyhow::Result<()> {
        with_dataset(|client, spec, ids| async move {
            // `//address` reaches the address node via a star over any predicate
            // (an omitted path predicate), then `/@city` reads its value.
            assert_xpath(&client, &spec, &fmt("{jane}//address/@city", &ids), &["Berlin"]).await?;
            Ok(())
        })
        .await
    }

    /// Substitute `{jane}` / `{john}` in a template with `document("<id>")`,
    /// using the typed short id from `EntityIDFor` (not a hand-built IRI).
    fn fmt(template: &str, ids: &Ids) -> String {
        template
            .replace("{jane}", &format!(r#"document("{}")"#, ids.jane.typed()))
            .replace("{john}", &format!(r#"document("{}")"#, ids.john.typed()))
    }
}

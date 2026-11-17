#![recursion_limit = "256"]
//! Live verification of the WOQL operations added in TerminusDB 12.1.0-rc3
//! against a real server: `TripleCount` (both its resource and its new pattern
//! form), `TripleRangeCount`, `TripleRelation` and the `UuidV7` idgen, plus
//! `Typecast`'s new `prefixes` property.
//!
//! These construct the woql2 AST directly (the builder is deprecated). Offline
//! JSON-shape assertions prove nothing about these classes — the point is that
//! the server accepts the serialization and answers with the right bindings.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::Value;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance, XSDAnySimpleType};
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_woql2::compare::Typecast;
    use terminusdb_woql2::misc::{TripleCount, UuidV7};
    use terminusdb_woql2::prelude::{DataValue, NodeValue, Query, Value as WoqlValue};
    use terminusdb_woql2::triple::{AllenRelation, TripleRangeCount, TripleRelation};

    /// A plain document with a numeric property, so range counts have something
    /// ordered to count.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Member {
        id: EntityIDFor<Self>,
        name: String,
        age: i32,
    }

    /// A document with an `xsd:dateTime` property: `TripleRelation` compares
    /// the object against a reference instant/interval, so the object has to be
    /// a date-ish value (the server coerces an instant to a point interval).
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Event {
        id: EntityIDFor<Self>,
        when: DateTime<Utc>,
    }

    fn event(id: &str, y: i32, m: u32, d: u32) -> anyhow::Result<Event> {
        Ok(Event {
            id: EntityIDFor::new(id)?,
            when: Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap(),
        })
    }

    fn instant(y: i32, m: u32, d: u32) -> DataValue {
        DataValue::Data(XSDAnySimpleType::DateTime(
            Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap(),
        ))
    }

    fn member(id: &str, name: &str, age: i32) -> anyhow::Result<Member> {
        Ok(Member {
            id: EntityIDFor::new(id)?,
            name: name.to_string(),
            age,
        })
    }

    fn var(name: &str) -> DataValue {
        DataValue::Variable(name.to_string())
    }
    fn node_var(name: &str) -> NodeValue {
        NodeValue::Variable(name.to_string())
    }
    fn node(iri: &str) -> NodeValue {
        NodeValue::Node(iri.to_string())
    }
    fn val_int(n: i64) -> WoqlValue {
        WoqlValue::Data(XSDAnySimpleType::Integer(n))
    }

    async fn run(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        query: Query,
    ) -> anyhow::Result<Vec<HashMap<String, Value>>> {
        let res: WOQLResult<HashMap<String, Value>> =
            client.query(Some(spec.clone()), query).await?;
        Ok(res.bindings)
    }

    /// Pull an integer out of a WOQL binding, whatever numeric shape it took.
    fn binding_i64(v: &Value) -> Option<i64> {
        if let Some(n) = v.as_i64() {
            return Some(n);
        }
        if let Some(s) = v.as_str() {
            return s.parse().ok();
        }
        let inner = v.get("@value")?;
        inner
            .as_i64()
            .or_else(|| inner.as_str().and_then(|s| s.parse().ok()))
            .or_else(|| inner.as_f64().map(|f| f as i64))
    }

    /// Pull a plain string out of a WOQL binding value.
    fn binding_str(v: &Value) -> Option<String> {
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

    /// `TripleCount`'s new pattern form: count the triples matching a partial
    /// (subject, predicate, object) pattern rather than a whole resource.
    #[tokio::test]
    async fn test_v12_1_triple_count_pattern() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let members = vec![
            member("alice", "Alice", 30)?,
            member("bob", "Bob", 40)?,
            member("carol", "Carol", 50)?,
        ];
        server
            .with_db_seed("v12_1_triple_count", members, |client, spec| async move {
                // Three members, so three `name` edges.
                let b = run(
                    &client,
                    &spec,
                    Query::TripleCount(TripleCount::pattern(
                        TripleCount::wildcard_node("S"),
                        node("@schema:name"),
                        TripleCount::wildcard_value("O"),
                        var("C"),
                    )),
                )
                .await?;
                assert!(
                    !b.is_empty(),
                    "TripleCount pattern form returned no bindings"
                );
                let count =
                    binding_i64(b[0].get("C").expect("no C binding")).expect("C was not numeric");
                println!("TripleCount(_, @schema:name, _) -> {count}");
                assert_eq!(count, 3, "expected one name edge per seeded member");

                // Pinning the object narrows it to exactly one triple.
                let b = run(
                    &client,
                    &spec,
                    Query::TripleCount(TripleCount::pattern(
                        TripleCount::wildcard_node("S"),
                        node("@schema:age"),
                        val_int(40),
                        var("C"),
                    )),
                )
                .await?;
                let count = binding_i64(b[0].get("C").expect("no C binding")).unwrap();
                println!("TripleCount(_, @schema:age, 40) -> {count}");
                assert_eq!(count, 1, "exactly one member is aged 40");

                // A pattern that matches nothing still answers, with zero.
                let b = run(
                    &client,
                    &spec,
                    Query::TripleCount(TripleCount::pattern(
                        TripleCount::wildcard_node("S"),
                        node("@schema:age"),
                        val_int(999),
                        var("C"),
                    )),
                )
                .await?;
                let count = binding_i64(b[0].get("C").expect("no C binding")).unwrap();
                assert_eq!(count, 0, "no member is aged 999");
                Ok(())
            })
            .await
    }

    /// `TripleCount`'s original resource form must keep working now that
    /// `resource` is optional.
    #[tokio::test]
    async fn test_v12_1_triple_count_resource() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let members = vec![member("alice", "Alice", 30)?, member("bob", "Bob", 40)?];
        server
            .with_db_seed("v12_1_tc_resource", members, |client, spec| async move {
                let resource = format!("{}/{}", client.org(), spec.db);
                let b = run(
                    &client,
                    &spec,
                    Query::TripleCount(TripleCount::for_resource(resource.clone(), var("C"))),
                )
                .await?;
                assert!(
                    !b.is_empty(),
                    "TripleCount resource form returned no bindings"
                );
                let count =
                    binding_i64(b[0].get("C").expect("no C binding")).expect("C was not numeric");
                println!("TripleCount({resource}) -> {count} edges");
                assert!(count > 0, "a seeded database has edges");
                Ok(())
            })
            .await
    }

    /// `TripleRangeCount`: index-answered count of the triples whose object
    /// falls in the half-open range [low, high).
    #[tokio::test]
    async fn test_v12_1_triple_range_count() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let members = vec![
            member("a", "A", 10)?,
            member("b", "B", 20)?,
            member("c", "C", 30)?,
            member("d", "D", 40)?,
        ];
        server
            .with_db_seed("v12_1_range_count", members, |client, spec| async move {
                // [20, 40) covers ages 20 and 30 -> 2. Half-open, so 40 is out.
                let b = run(
                    &client,
                    &spec,
                    Query::TripleRangeCount(TripleRangeCount {
                        subject: node_var("S"),
                        predicate: node("@schema:age"),
                        low: val_int(20),
                        high: val_int(40),
                        count: var("C"),
                        graph: None,
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "TripleRangeCount returned no bindings");
                let count =
                    binding_i64(b[0].get("C").expect("no C binding")).expect("C was not numeric");
                println!("TripleRangeCount(@schema:age, [20,40)) -> {count}");
                assert_eq!(count, 2, "ages 20 and 30 are in [20,40); 40 is excluded");

                // The whole span.
                let b = run(
                    &client,
                    &spec,
                    Query::TripleRangeCount(TripleRangeCount {
                        subject: node_var("S"),
                        predicate: node("@schema:age"),
                        low: val_int(0),
                        high: val_int(100),
                        count: var("C"),
                        graph: None,
                    }),
                )
                .await?;
                let count = binding_i64(b[0].get("C").expect("no C binding")).unwrap();
                assert_eq!(count, 4, "all four ages are in [0,100)");
                Ok(())
            })
            .await
    }

    /// `TripleRelation`: match a triple whose object stands in a given Allen
    /// relation to a reference instant.
    ///
    /// The server compiles this to `woql_interval_relation_typed/3`, so the
    /// object and the reference value must be date-ish (an `xsd:dateTime`
    /// instant is coerced to a point interval). Integers do not match — that is
    /// what `IntervalRelation`'s numeric siblings are for.
    #[tokio::test]
    async fn test_v12_1_triple_relation() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let events = vec![
            event("jan", 2025, 1, 1)?,
            event("apr", 2025, 4, 1)?,
            event("jul", 2025, 7, 1)?,
        ];
        server
            .with_db_seed("v12_1_triple_relation", events, |client, spec| async move {
                // Reference instant: 2025-04-01, which the server coerces to
                // the point interval [Apr1, Apr1).
                //
                // `before` is strict, but `after` against a *point* reference is
                // deliberately on-or-after in 12.1 ("as-of" semantics: the seam
                // instant belongs to the interval it starts — see
                // check_interval_relation/5 in woql_compile.pl), so the April
                // event matches `after` as well as `equals`.
                for (relation, expected) in [
                    (AllenRelation::Before, vec!["2025-01-01"]),
                    (AllenRelation::After, vec!["2025-04-01", "2025-07-01"]),
                ] {
                    let q = Query::TripleRelation(TripleRelation {
                        subject: node_var("S"),
                        predicate: node("@schema:when"),
                        object: WoqlValue::Variable("O".to_string()),
                        relation: relation.into(),
                        value: instant(2025, 4, 1),
                    });
                    let b = run(&client, &spec, q).await?;
                    let mut objects: Vec<String> = b
                        .iter()
                        .filter_map(|m| m.get("O").and_then(binding_str))
                        .collect();
                    objects.sort();
                    println!("TripleRelation(when {relation} 2025-04-01) -> {objects:?}");
                    assert_eq!(
                        objects.len(),
                        expected.len(),
                        "wrong number of {relation} matches: {objects:?}"
                    );
                    for (got, want) in objects.iter().zip(expected.iter()) {
                        assert!(
                            got.starts_with(want),
                            "expected a {relation} match at {want}, got {got}"
                        );
                    }
                }

                // `equals` against the reference instant finds the April event.
                let q = Query::TripleRelation(TripleRelation {
                    subject: node_var("S"),
                    predicate: node("@schema:when"),
                    object: WoqlValue::Variable("O".to_string()),
                    relation: AllenRelation::Equals.into(),
                    value: instant(2025, 4, 1),
                });
                let b = run(&client, &spec, q).await?;
                let objects: Vec<String> = b
                    .iter()
                    .filter_map(|m| m.get("O").and_then(binding_str))
                    .collect();
                println!("TripleRelation(when equals 2025-04-01) -> {objects:?}");
                assert_eq!(
                    objects.len(),
                    1,
                    "exactly one event is at the reference instant"
                );
                assert!(objects[0].starts_with("2025-04-01"));
                Ok(())
            })
            .await
    }

    /// `UuidV7`: the idgen predicate must mint a UUIDv7-shaped IRI under the
    /// supplied base. Two calls must not collide, and v7 IDs sort by time.
    #[tokio::test]
    async fn test_v12_1_uuid_v7_idgen() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_1_uuid_v7", |client, spec| async move {
                let mut minted = Vec::new();
                for _ in 0..2 {
                    let b = run(
                        &client,
                        &spec,
                        Query::UuidV7(UuidV7 {
                            base: DataValue::Data(XSDAnySimpleType::String(
                                "terminusdb:///data/Thing/".to_string(),
                            )),
                            uri: NodeValue::Variable("U".to_string()),
                        }),
                    )
                    .await?;
                    assert!(!b.is_empty(), "UuidV7 returned no bindings");
                    let u = b[0].get("U").expect("no U binding");
                    let iri = u
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| {
                            u.get("@id")
                                .or_else(|| u.get("node"))
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_else(|| panic!("unexpected UuidV7 binding shape: {u}"));
                    println!("UuidV7 -> {iri}");
                    // The server answers with the IRI compressed against the
                    // database prefixes, so the full base is not echoed back —
                    // only its trailing path segment.
                    assert!(
                        iri.contains("Thing/"),
                        "minted IRI is not under the supplied base: {iri}"
                    );
                    let uuid = iri.rsplit('/').next().unwrap();
                    assert_eq!(uuid.len(), 36, "not a hyphenated UUID: {uuid}");
                    // Version nibble: the 15th hex digit of a UUIDv7 is '7'.
                    assert_eq!(
                        uuid.chars().nth(14),
                        Some('7'),
                        "not a v7 UUID (version nibble): {uuid}"
                    );
                    minted.push(uuid.to_string());
                }
                assert_ne!(minted[0], minted[1], "UuidV7 minted a duplicate");
                Ok(())
            })
            .await
    }

    /// `Typecast` keeps working with rc3's new `prefixes` property present
    /// (`None`) and the cast itself still produces the right value.
    #[tokio::test]
    async fn test_v12_1_typecast_prefixes() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_1_typecast", |client, spec| async move {
                let b = run(
                    &client,
                    &spec,
                    Query::Typecast(Typecast {
                        value: WoqlValue::Data(XSDAnySimpleType::String("42".to_string())),
                        type_uri: node("xsd:integer"),
                        result_value: WoqlValue::Variable("R".to_string()),
                        prefixes: None,
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "Typecast returned no bindings");
                let r = b[0].get("R").expect("no R binding");
                println!("Typecast(\"42\" -> xsd:integer) = {r}");
                assert_eq!(binding_i64(r), Some(42));
                Ok(())
            })
            .await
    }
}

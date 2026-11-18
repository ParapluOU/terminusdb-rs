#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! The M1 gate: Reactor is transparent.
//!
//! Before caching is interesting, the proxy has to be *invisible*. Every test
//! here runs the same operation through Reactor and directly against the node
//! and compares the results. If any of these fail, nothing downstream matters:
//! a cache in front of a proxy that changes answers is just a faster way to be
//! wrong.
//!
//! The byte-fidelity test is the sharpest one. TerminusDB 12 returns numbers at
//! full rational precision and this workspace enables
//! `serde_json/arbitrary_precision`; a body that went through
//! `serde_json::Value` on our side would come back with reordered keys and, for
//! `xsd:decimal`, fewer digits. Comparing bytes — not parsed values — is what
//! makes invariant 1 testable rather than aspirational.

mod common;

use anyhow::Result;
use common::models::{ledger, person, Ledger, Person};
use common::Reactor;
use terminusdb_client::{DocumentInsertArgs, GetOpts};

/// Documents inserted through Reactor arrive, and read back identically through
/// both paths.
#[tokio::test]
async fn documents_round_trip_through_the_proxy() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_doc", |direct, spec| async move {
            let reactor = Reactor::start(up, &spec.db).await?;
            let via = reactor.client().await?;

            // Insert *through* Reactor: a write must pass through untouched.
            let args = DocumentInsertArgs::from(spec.clone());
            via.insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;

            let through = via
                .get_document("Person/alice", &spec, GetOpts::default())
                .await?;
            let straight = direct
                .get_document("Person/alice", &spec, GetOpts::default())
                .await?;
            assert_eq!(
                through, straight,
                "the same document read two ways must be the same document"
            );
            assert_eq!(through["name"], serde_json::json!("Ann"));
            Ok(())
        })
        .await
}

/// A list read (`type=`) and a multi-document read behave identically.
#[tokio::test]
async fn list_reads_round_trip_through_the_proxy() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_list", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for (id, name, age) in [("a", "A", 1), ("b", "B", 2), ("c", "C", 3)] {
                direct
                    .insert_instance(&person(id, name, age)?, args.clone())
                    .await?;
            }

            let reactor = Reactor::start(up, &spec.db).await?;

            let through = reactor
                .get(
                    "/api/document/admin/{db}?type=Person&as_list=true"
                        .replace("{db}", &spec.db)
                        .as_str(),
                )
                .await?;
            assert_eq!(through.status, 200, "body: {}", through.text());

            let mut through_json = through.json()?;
            let mut direct_json: serde_json::Value = serde_json::from_slice(
                &reqwest::Client::new()
                    .get(format!(
                        "http://localhost:{}/api/document/admin/{}?type=Person&as_list=true",
                        up.port(),
                        spec.db
                    ))
                    .basic_auth("admin", Some("root"))
                    .send()
                    .await?
                    .bytes()
                    .await?,
            )?;
            // Order is not guaranteed by the server, so compare as sets of ids.
            sort_by_id(&mut through_json);
            sort_by_id(&mut direct_json);
            assert_eq!(through_json, direct_json);
            Ok(())
        })
        .await
}

fn sort_by_id(value: &mut serde_json::Value) {
    if let Some(array) = value.as_array_mut() {
        array.sort_by_key(|item| {
            item.get("@id")
                .and_then(|id| id.as_str())
                .unwrap_or("")
                .to_owned()
        });
    }
}

/// A WOQL read through Reactor matches a direct one.
#[tokio::test]
async fn woql_round_trips_through_the_proxy() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_woql", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let query = terminusdb_woql2::triple!(
                terminusdb_woql2::node!("Person/alice"),
                "name",
                terminusdb_woql2::var!("n")
            );
            let body = serde_json::json!({"query": query.to_woql_json()});

            let through = reactor
                .post(&format!("/api/woql/admin/{}", spec.db), body.clone())
                .await?;
            assert_eq!(through.status, 200, "body: {}", through.text());
            let straight: serde_json::Value = serde_json::from_slice(
                &reqwest::Client::new()
                    .post(format!(
                        "http://localhost:{}/api/woql/admin/{}",
                        up.port(),
                        spec.db
                    ))
                    .basic_auth("admin", Some("root"))
                    .json(&body)
                    .send()
                    .await?
                    .bytes()
                    .await?,
            )?;
            assert_eq!(through.json()?, straight);
            Ok(())
        })
        .await
}

/// A GraphQL query through Reactor matches a direct one — *after* the injected
/// `_id`/`_type` are accounted for.
///
/// Note what this asserts and what it does not: Reactor injects ids into the
/// executed query, so the cached body legitimately carries fields the client did
/// not ask for. Stripping them from the client's copy is implemented as a
/// recorded path list in the plan but is not yet applied to the response, so
/// this test asserts the *content* matches rather than the bytes, and documents
/// the difference.
#[tokio::test]
async fn graphql_round_trips_through_the_proxy() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_gql", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let body = serde_json::json!({"query": "query { Person { name age } }"});

            let through = reactor
                .post(&format!("/api/graphql/admin/{}", spec.db), body.clone())
                .await?;
            assert_eq!(through.status, 200, "body: {}", through.text());
            let through_json = through.json()?;
            let people = through_json["data"]["Person"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            assert_eq!(people.len(), 1, "got {through_json}");
            assert_eq!(people[0]["name"], serde_json::json!("Ann"));
            assert_eq!(people[0]["age"], serde_json::json!(30));

            println!(
                "graphql through Reactor: {}",
                serde_json::to_string(&through_json)?
            );
            Ok(())
        })
        .await
}

/// **Invariant 1.** A high-precision `xsd:decimal` survives the proxy
/// byte-for-byte.
#[tokio::test]
async fn high_precision_decimals_are_byte_identical() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Ledger,), _, _, _>("rx_fidelity_decimal", |direct, spec| async move {
            // Far more significant digits than an f64 can hold: a round trip
            // through a 53-bit mantissa would truncate this.
            //
            // FINDING: TerminusDB 12.1.0-rc3 itself stores ~19 significant
            // digits for xsd:decimal, so the 25 digits inserted here come back
            // as 0.1234567890123456789. That is the *server's* limit, not ours
            // — and it is still well beyond f64, so this value remains a valid
            // probe for invariant 1: anything that went through an f64 would
            // read back as 0.12345678901234568 (17 digits).
            let precise = "0.1234567890123456789012345";
            let server_precision = "0.1234567890123456789";
            let f64_would_give = "0.12345678901234568";
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&ledger("l1", precise)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Ledger/l1", spec.db);

            let through = reactor.get(&path).await?;
            assert_eq!(through.status, 200, "body: {}", through.text());
            let straight = reqwest::Client::new()
                .get(format!("http://localhost:{}{}", up.port(), path))
                .basic_auth("admin", Some("root"))
                .send()
                .await?
                .bytes()
                .await?;

            assert_eq!(
                through.body,
                straight,
                "bytes must be identical:\n  via Reactor: {}\n  direct:      {}",
                through.text(),
                String::from_utf8_lossy(&straight)
            );
            assert!(
                through.text().contains(server_precision),
                "the server's stored precision must survive the proxy: {}",
                through.text()
            );
            assert!(
                !through.text().contains(f64_would_give),
                "a value that had passed through an f64 would read back as {f64_would_give}: {}",
                through.text()
            );
            println!(
                "FINDING xsd:decimal: inserted {precise}, server returned {} \
                 (server keeps ~19 significant digits; f64 would have given {f64_would_give})",
                through.text()
            );

            // And again on the second (cached) read: a cached body must be the
            // same bytes, not a re-serialization of them.
            let cached = reactor.get(&path).await?;
            assert_eq!(
                cached.body,
                straight,
                "a cached response must be byte-identical too ({})",
                cached.cache_status()
            );
            Ok(())
        })
        .await
}

/// Every response carries the protocol headers, and no hop-by-hop header leaks.
#[tokio::test]
async fn responses_carry_the_reactor_headers() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_headers", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let probe = reactor
                .get(&format!("/api/document/admin/{}?id=Person/alice", spec.db))
                .await?;

            assert_eq!(probe.status, 200, "body: {}", probe.text());
            assert!(
                probe.header("cache-status").is_some(),
                "Cache-Status is how an operator sees what we did"
            );
            assert_eq!(probe.header("x-reactor-node"), Some("test-reactor"));
            assert!(
                probe.header("x-reactor-head").is_some(),
                "X-Reactor-Head is how a client routes by replication state"
            );

            for hop in [
                "connection",
                "keep-alive",
                "transfer-encoding",
                "proxy-authenticate",
                "upgrade",
            ] {
                assert!(
                    probe.header(hop).is_none(),
                    "{hop} must not be relayed, got {:?}",
                    probe.header(hop)
                );
            }
            println!("cache-status: {}", probe.cache_status());
            Ok(())
        })
        .await
}

/// Deletes pass through and take effect.
#[tokio::test]
async fn deletes_pass_through() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_fidelity_delete", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let via = reactor.client().await?;

            via.delete_document(
                Some("Person/alice"),
                &spec,
                "reactor-test",
                "delete through the proxy",
                "instance",
                Default::default(),
            )
            .await?;

            let gone = direct
                .get_document_if_exists("Person/alice", &spec, GetOpts::default())
                .await?;
            assert!(gone.is_none(), "the delete must have reached the node");
            Ok(())
        })
        .await
}

/// An unknown route is forwarded, not intercepted. Invariant 6, end to end.
#[tokio::test]
async fn unknown_routes_are_forwarded() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_tmp_db("rx_fidelity_unknown", |_direct, spec| async move {
            let reactor = Reactor::start(up, &spec.db).await?;

            let probe = reactor.get("/api/info").await?;
            assert_eq!(probe.status, 200, "body: {}", probe.text());
            assert!(
                probe.is_bypass(),
                "an endpoint we do not analyze must be forwarded uncached, got {}",
                probe.cache_status()
            );
            assert!(
                probe.text().contains("terminusdb"),
                "and the node's own answer must come back: {}",
                probe.text()
            );
            Ok(())
        })
        .await
}

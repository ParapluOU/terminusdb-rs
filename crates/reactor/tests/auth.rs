#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! The cache must not leak.
//!
//! Cache keys deliberately contain no user: TerminusDB's access control is per
//! *resource*, so one cached copy is correct for everyone who may read that
//! resource. That shortcut is only sound if a hit is gated on the caller
//! actually having that access — otherwise anyone who can reach the proxy can
//! read whatever another user warmed.
//!
//! These are security assertions. A failure here is a blocker, not a flake.

mod common;

use anyhow::Result;
use common::models::{person, Person};
use common::Reactor;
use terminusdb_client::DocumentInsertArgs;

/// Populate the cache as `admin`, then ask again with wrong credentials: the
/// cached body must not come back.
#[tokio::test]
async fn wrong_credentials_cannot_read_a_populated_cache() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_auth_leak", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);

            // Warm it as a legitimate user.
            let warm = reactor.get(&path).await?;
            assert_eq!(warm.status, 200, "body: {}", warm.text());
            let hit = reactor.get(&path).await?;
            assert!(
                hit.is_hit(),
                "precondition: cached, got {}",
                hit.cache_status()
            );
            let secret = hit.text();
            assert!(secret.contains("Ann"));

            // Now a caller with the wrong password.
            let intruder = reactor
                .get_as(&path, Some(("admin", "definitely-not-the-password")))
                .await?;
            println!(
                "wrong credentials -> status {} cache-status {}",
                intruder.status,
                intruder.cache_status()
            );
            assert!(
                !intruder.status.eq(&200) || !intruder.text().contains("Ann"),
                "a cache hit must never be served to a caller who cannot read the \
                 namespace. Got status {} body {}",
                intruder.status,
                intruder.text()
            );
            assert!(
                intruder.status == 401 || intruder.status == 403,
                "upstream's own refusal should be relayed; got {} ({})",
                intruder.status,
                intruder.text()
            );

            // And an unknown user.
            let unknown = reactor.get_as(&path, Some(("nobody", "nothing"))).await?;
            assert!(
                !unknown.text().contains("Ann"),
                "got {} with status {}",
                unknown.text(),
                unknown.status
            );

            // The legitimate user is unaffected.
            let still = reactor.get(&path).await?;
            assert!(still.text().contains("Ann"));
            Ok(())
        })
        .await
}

/// An anonymous request is forwarded for upstream to judge, never answered from
/// cache.
#[tokio::test]
async fn an_anonymous_request_is_not_answered_from_cache() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_auth_anon", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit(), "precondition: cached");

            let anonymous = reactor.get_as(&path, None).await?;
            println!(
                "anonymous -> status {} cache-status {}",
                anonymous.status,
                anonymous.cache_status()
            );
            assert!(
                !anonymous.is_hit(),
                "with no credentials there is nothing to authorize, so upstream must \
                 decide; got {}",
                anonymous.cache_status()
            );
            Ok(())
        })
        .await
}

/// A write with wrong credentials is refused by upstream and does not disturb
/// the cache.
#[tokio::test]
async fn a_refused_write_does_not_poison_the_cache() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_auth_write", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/document/admin/{}?id=Person/alice", spec.db);
            reactor.get(&path).await?;
            assert!(reactor.get(&path).await?.is_hit());

            // An unauthenticated write attempt.
            let refused = reactor
                .http
                .post(reactor.url(&format!(
                    "/api/document/admin/{}?author=x&message=y&graph_type=instance",
                    spec.db
                )))
                .basic_auth("admin", Some("wrong"))
                .json(&serde_json::json!({"@type": "Person", "name": "Evil", "age": 1}))
                .send()
                .await?;
            println!("refused write -> status {}", refused.status());
            assert!(
                !refused.status().is_success(),
                "upstream must refuse this write"
            );

            // The cache is untouched and still correct.
            let after = reactor.get(&path).await?;
            assert!(
                after.text().contains("Ann"),
                "got {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

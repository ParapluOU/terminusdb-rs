// Collaboration: clone / push / pull convergence between two real servers.

#![recursion_limit = "512"]

use terminusdb_bin::{start_server, ServerOptions};
use terminusdb_client::{BranchSpec, DefaultTDBDeserializer, DocumentInsertArgs};
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct CollabProbe {
    id: EntityIDFor<Self>,
    name: String,
}

/// v12 access-control: grant the built-in Admin Role (clone/fetch/push)
/// to admin on an organization scope. The client's create_role/grant_role
/// currently target stale endpoints (/api/role, /api/user/../role), so this
/// test drives the documented /api/capabilities API directly.
async fn grant_admin_remote(port: u16) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "operation": "grant",
        "scope_type": "organization",
        "scope": "admin",
        "user": "admin",
        "roles": ["Admin Role"]
    });
    let resp = reqwest::Client::new()
        .post(format!("http://localhost:{port}/api/capabilities"))
        .basic_auth("admin", Some("root"))
        .json(&body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "capability grant failed: {}",
        resp.text().await?
    );
    Ok(())
}

async fn server() -> anyhow::Result<terminusdb_bin::TerminusDBServer> {
    Ok(start_server(ServerOptions {
        memory: true,
        quiet: true,
        test_mode: true,
        ..Default::default()
    })
    .await?)
}

#[tokio::test]
async fn clone_push_pull_converges_between_two_servers() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "terminusdb_client=info".into()),
        )
        .try_init();
    let a = server().await?;
    let b = server().await?;
    let ca = a.client().await?;
    let cb = b.client().await?;
    let db = "collab_probe";
    // remote_url format: scheme://host:port/{org}/{db} (the server builds
    // its own pack endpoint from this path)
    let remote_a = format!("http://localhost:{}/admin/{}", a.port(), db);
    let remote_b = format!("http://localhost:{}/admin/{}", b.port(), db);
    let auth = Some(("admin", "root"));
    let db_path = format!("admin/{db}");

    // Source and target must know admin as a remote-capable user
    grant_admin_remote(a.port()).await?;
    grant_admin_remote(b.port()).await?;

    // Seed server A
    ca.ensure_database(db).await?;
    let spec = BranchSpec::from(db);
    ca.schema::<CollabProbe>(DocumentInsertArgs::from(spec.clone()))
        .await?;
    let p1 = CollabProbe {
        id: EntityIDFor::random(),
        name: "from-a".to_string(),
    };
    ca.insert(&p1, DocumentInsertArgs::from(spec.clone())).await?;

    // Clone A -> B, verify data present on B
    cb.clone_repository("admin", db, &remote_a, Some("cloned"), Some("clone from a"), auth, None)
        .await?;
    let mut deser = DefaultTDBDeserializer;
    let got: CollabProbe = cb
        .get::<CollabProbe>(&p1.id.id(), &BranchSpec::from(db), &mut deser)
        .await?;
    assert_eq!(got.name, "from-a");

    // New commit on A; register B on A as a named remote, push by name
    let p2 = CollabProbe {
        id: EntityIDFor::random(),
        name: "from-a-2".to_string(),
    };
    ca.insert(&p2, DocumentInsertArgs::from(spec.clone())).await?;
    ca.add_remote(&db_path, "peer", &remote_b).await?;
    // establish the peer remote's head, then push by name
    ca.fetch(
        &format!("{db_path}/peer/_commits"),
        "peer",
        None,
        auth,
        None,
    )
    .await?;
    ca.push(&db_path, "peer", Some("main"), auth, None).await?;
    // B's clone registered the source as "origin"; pull by name
    cb.pull(&db_path, "origin", Some("main"), auth, None).await?;
    let got2: CollabProbe = cb
        .get::<CollabProbe>(&p2.id.id(), &BranchSpec::from(db), &mut deser)
        .await?;
    assert_eq!(got2.name, "from-a-2");

    Ok(())
}

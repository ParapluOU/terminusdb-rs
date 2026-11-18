#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Spikes: open questions from the design doc, turned into executable
//! knowledge.
//!
//! Each test answers **one** question about how TerminusDB 12.1.0-rc3 actually
//! behaves, asserts the behaviour it finds, and prints the finding so the test
//! output is the record. Where a spike contradicts an assumption in the
//! implementation, the test says so rather than being bent to pass: a
//! failing-but-accurate spike is worth more than a passing fiction.

mod common;

use anyhow::Result;
use common::models::{person, resident, Person, Resident};
use terminusdb_client::{DocumentInsertArgs, GetOpts, LogOpts};

/// **Q: what does `/api/changes` report, pinned at a commit?**
///
/// The whole resolver is built on this call: it is how a commit becomes a set
/// of document ids, which become dependency terms. If it reports nothing, the
/// cache never invalidates anything.
#[tokio::test]
async fn spike_what_does_api_changes_report() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_spike_changes", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());

            // Commit 1: create.
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args.clone())
                .await?;
            let after_create = direct.get_latest_commit_id(&spec).await?;

            // Commit 2: update.
            direct
                .insert_instance(&person("alice", "Anne", 31)?, args.clone())
                .await?;
            let after_update = direct.get_latest_commit_id(&spec).await?;

            let mut findings = Vec::new();
            for (label, commit) in [("create", &after_create), ("update", &after_update)] {
                let pinned = spec_at(&spec, commit);
                let changes = direct.get_changes(&pinned, None).await;
                match &changes {
                    Ok(changes) => findings.push(format!(
                        "{label} commit {commit}: added={:?} changed={:?} deleted={:?}",
                        changes.added, changes.changed, changes.deleted
                    )),
                    Err(err) => findings.push(format!("{label} commit {commit}: ERROR {err:#}")),
                }

                // And against the branch head, for comparison.
                let branch = direct.get_changes(&spec, None).await;
                if let Ok(branch) = branch {
                    findings.push(format!(
                        "  (branch head: added={:?} changed={:?} deleted={:?})",
                        branch.added, branch.changed, branch.deleted
                    ));
                }
            }
            println!("FINDING /api/changes:\n  {}", findings.join("\n  "));

            let version = up.version().await.unwrap_or_else(|_| "unknown".into());
            let supported = up.supports_changes().await;
            println!("  server version {version}; /api/changes present: {supported}");

            let pinned = spec_at(&spec, &after_update);
            let changes = direct.get_changes(&pinned, None).await?;
            let named: Vec<&String> = changes
                .added
                .iter()
                .chain(changes.changed.iter())
                .chain(changes.deleted.iter())
                .collect();

            if supported {
                assert!(
                    named.iter().any(|id| id.contains("alice")),
                    "the resolver depends on this naming the changed document; got {named:?}"
                );
                println!("  => the resolver can derive exact per-document terms on this node.");
            } else {
                // The important part of the finding, and the reason the
                // resolver now treats an empty list as *unknown*.
                assert!(
                    named.is_empty(),
                    "unexpected: /api/changes is absent yet returned ids {named:?}"
                );
                println!(
                    "  => FINDING: /api/changes is a 12.1 endpoint and is ABSENT on {version}. \
                     It answers 404, and `ChangesResponse` defaults every field, so the client \
                     silently yields three empty lists. The resolver therefore treats an empty \
                     list as unknown and fires Everything; Reactor stays sound, but \
                     per-document invalidation precision is unavailable on this node."
                );
            }
            Ok(())
        })
        .await
}

/// Build a commit-pinned spec for a branch spec.
fn spec_at(
    spec: &terminusdb_client::BranchSpec,
    commit: &terminusdb_client::CommitId,
) -> terminusdb_client::BranchSpec {
    let mut pinned = spec.clone();
    pinned.ref_commit = Some(commit.clone());
    pinned
}

/// **Q: do read endpoints return `TerminusDB-Data-Version`?**
///
/// The fill path prefers the response's data-version over the commit it asked
/// for, and refuses to cache a response it cannot place at a known commit. So
/// which endpoints supply one decides how much is cacheable at all.
#[tokio::test]
async fn spike_which_reads_return_a_data_version() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_spike_dataversion", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;

            let http = reqwest::Client::new();
            let base = format!("http://localhost:{}", up.port());
            let mut findings = Vec::new();

            let doc = http
                .get(format!(
                    "{base}/api/document/admin/{}?id=Person/alice",
                    spec.db
                ))
                .basic_auth("admin", Some("root"))
                .send()
                .await?;
            findings.push(format!(
                "document GET  -> {:?}",
                doc.headers()
                    .get("terminusdb-data-version")
                    .and_then(|v| v.to_str().ok())
            ));

            let gql = http
                .post(format!("{base}/api/graphql/admin/{}", spec.db))
                .basic_auth("admin", Some("root"))
                .json(&serde_json::json!({"query": "query { Person { name } }"}))
                .send()
                .await?;
            findings.push(format!(
                "graphql POST  -> {:?}",
                gql.headers()
                    .get("terminusdb-data-version")
                    .and_then(|v| v.to_str().ok())
            ));

            let query = terminusdb_woql2::triple!(
                terminusdb_woql2::node!("Person/alice"),
                "name",
                terminusdb_woql2::var!("n")
            );
            let woql = http
                .post(format!("{base}/api/woql/admin/{}", spec.db))
                .basic_auth("admin", Some("root"))
                .json(&serde_json::json!({"query": query.to_woql_json()}))
                .send()
                .await?;
            findings.push(format!(
                "woql POST     -> {:?}",
                woql.headers()
                    .get("terminusdb-data-version")
                    .and_then(|v| v.to_str().ok())
            ));

            println!(
                "FINDING TerminusDB-Data-Version on reads:\n  {}",
                findings.join("\n  ")
            );
            Ok(())
        })
        .await
}

/// **Q: does a commit-pinned GraphQL path work?**
///
/// Our fill path pins by URL. If GraphQL rejects `/local/commit/<id>` then
/// GraphQL responses can only be placed at a commit via their data-version
/// header, and if they have neither they cannot be cached at all. The code
/// handles every outcome; this records which one is real.
#[tokio::test]
async fn spike_is_graphql_pinnable_by_commit() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_spike_gqlpin", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 30)?, args)
                .await?;
            let head = direct.get_latest_commit_id(&spec).await?;

            let http = reqwest::Client::new();
            let response = http
                .post(format!(
                    "http://localhost:{}/api/graphql/admin/{}/local/commit/{}",
                    up.port(),
                    spec.db,
                    head
                ))
                .basic_auth("admin", Some("root"))
                .json(&serde_json::json!({"query": "query { Person { name } }"}))
                .send()
                .await?;
            let status = response.status();
            let body = response.text().await?;
            println!(
                "FINDING graphql at /local/commit/<id>: status {status}, body {}",
                body.chars().take(240).collect::<String>()
            );

            if status.is_success() && body.contains("Ann") {
                println!("  => GraphQL IS pinnable by commit; the fill path can pin it by URL.");
            } else {
                println!(
                    "  => GraphQL is NOT pinnable by commit; GraphQL fills rely on the \
                     data-version header, and are not cached without one."
                );
            }
            Ok(())
        })
        .await
}

/// **Q (design doc S3): who owns a subdocument change?**
///
/// A subdocument has no independent identity, so a change to it must be
/// attributed to *something*. Whether `/api/changes` reports the parent, the
/// subdocument, or both decides what dependency terms a subdocument tree needs
/// — currently guesswork in the analyzer.
#[tokio::test]
async fn spike_who_owns_a_subdocument_change() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Resident,), _, _, _>("rx_spike_subdoc", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&resident("r1", "Ann", "Berlin")?, args.clone())
                .await?;

            // Change ONLY the subdocument's field.
            direct
                .insert_instance(&resident("r1", "Ann", "Paris")?, args)
                .await?;
            let commit = direct.get_latest_commit_id(&spec).await?;

            let changes = direct.get_changes(&spec_at(&spec, &commit), None).await?;
            println!(
                "FINDING subdocument change attribution: added={:?} changed={:?} deleted={:?}",
                changes.added, changes.changed, changes.deleted
            );

            let all: Vec<&String> = changes
                .added
                .iter()
                .chain(changes.changed.iter())
                .chain(changes.deleted.iter())
                .collect();
            let mentions_parent = all.iter().any(|id| id.contains("Resident"));
            let mentions_child = all.iter().any(|id| id.contains("Address"));
            println!(
                "  => parent mentioned: {mentions_parent}; subdocument mentioned: {mentions_child}"
            );
            if mentions_parent {
                println!(
                    "  => a Doc(parent) dependency is invalidated by a subdocument change, \
                     which is what the analyzer assumes."
                );
            } else {
                println!(
                    "  => WARNING: the parent is NOT reported, so a Doc(parent) dependency \
                     would NOT be invalidated by a subdocument change. The analyzer would \
                     need to add the subdocument's class to the read set."
                );
            }
            let supported = up.supports_changes().await;
            if supported {
                assert!(
                    !all.is_empty(),
                    "a subdocument change must be reported as *something*"
                );
            } else {
                println!(
                    "  => INCONCLUSIVE: /api/changes is absent on this node \
                     (version {}), so subdocument attribution could not be \
                     determined. Re-run against 12.1 to answer design-doc S3.",
                    up.version().await.unwrap_or_else(|_| "unknown".into())
                );
            }
            Ok(())
        })
        .await
}

/// **Q: is `LogEntry.migration` actually populated for a schema change?**
///
/// It is the resolver's primary `schema_changed` detector. If it stays empty
/// for a real schema commit, that detector has a hole and the fallback (a
/// changed id that looks like a schema document) is carrying the whole load.
#[tokio::test]
async fn spike_does_a_schema_commit_record_a_migration() -> Result<()> {
    let server = common::upstream().await?;
    let _up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_spike_schema", |direct, spec| async move {
            // A second schema insertion is a schema commit.
            let args = DocumentInsertArgs::from(spec.clone());
            direct.insert_schemas::<(Resident,)>(args).await?;
            let commit = direct.get_latest_commit_id(&spec).await?;

            let entries = direct
                .log(
                    &spec_at(&spec, &commit),
                    LogOpts {
                        count: Some(1),
                        ..LogOpts::default()
                    },
                )
                .await?;
            let entry = entries.first().expect("a log entry for the schema commit");
            println!(
                "FINDING schema commit log entry: identifier={} migration_len={} message={:?}",
                entry.identifier,
                entry.migration.len(),
                entry.message
            );

            let changes = direct.get_changes(&spec_at(&spec, &commit), None).await?;
            println!(
                "  changes for the schema commit: added={:?} changed={:?} deleted={:?}",
                changes.added, changes.changed, changes.deleted
            );

            if entry.migration.is_empty() {
                println!(
                    "  => GAP CONFIRMED: `LogEntry.migration` is empty for a schema commit, so \
                     that detector does not fire. `schema_changed` then depends entirely on \
                     recognizing a changed id as a schema document."
                );
            } else {
                println!("  => `LogEntry.migration` is populated; the primary detector works.");
            }
            Ok(())
        })
        .await
}

/// **Q (design doc S5): does a `_ChangeSet` subscription survive many classes?**
///
/// The subscription query is generated from the class list, so a wide schema
/// makes a large query. This measures the size and checks registration still
/// works, as a cheap proxy for the real scaling question.
#[tokio::test]
async fn spike_changeset_query_size_for_a_wide_schema() -> Result<()> {
    use terminusdb_format::schema_doc::{RawClass, RawProperty, RawSchema};
    use terminusdb_reactor::capture::subscription::changeset_query;
    use terminusdb_reactor::SchemaIndex;

    // 50 classes, as a wide-schema stand-in.
    let classes: Vec<RawClass> = (0..50)
        .map(|i| RawClass {
            id: format!("Class{i}"),
            is_abstract: false,
            is_subdocument: false,
            inherits: Vec::new(),
            properties: vec![RawProperty {
                name: "name".to_owned(),
                class: "xsd:string".to_owned(),
                family: None,
                force_nullable: false,
            }],
        })
        .collect();
    let schema = SchemaIndex::from_raw(RawSchema {
        classes,
        enums: Vec::new(),
    });
    let query = changeset_query(&schema);

    println!(
        "FINDING _ChangeSet query for 50 classes: {} bytes, {} lines",
        query.len(),
        query.lines().count()
    );
    assert!(query.contains("Class0_added") && query.contains("Class49_deleted"));
    assert!(
        query.len() < 64 * 1024,
        "a subscription query that large would be a problem in itself: {} bytes",
        query.len()
    );

    // And it must still be a single well-formed subscription.
    assert_eq!(query.matches("subscription {").count(), 1);
    assert!(query.contains("_CommitMetadata"));
    Ok(())
}

/// **Q: does a document read reflect `unfold=true` for linked documents?**
///
/// Establishes whether an unfolded read embeds other documents' content, which
/// is why such a read needs response-id augmentation rather than a single
/// `Doc` dependency.
#[tokio::test]
async fn spike_what_does_unfold_embed() -> Result<()> {
    let server = common::upstream().await?;
    let _up = &server;
    server
        .with_schema::<(Resident,), _, _, _>("rx_spike_unfold", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&resident("r1", "Ann", "Berlin")?, args)
                .await?;

            let folded = direct
                .get_document("Resident/r1", &spec, GetOpts::default())
                .await?;
            let unfolded = direct
                .get_document("Resident/r1", &spec, GetOpts::default().with_unfold(true))
                .await?;
            println!(
                "FINDING unfold:\n  folded:   {}\n  unfolded: {}",
                serde_json::to_string(&folded)?,
                serde_json::to_string(&unfolded)?
            );
            Ok(())
        })
        .await
}

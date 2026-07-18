//! Database-level commands: optimize, squash, database CRUD, and deploy.

use anyhow::Result;
use serde_json::json;
use terminusdb_client::TerminusDBHttpClient;
use url::Url;

pub(crate) async fn run_optimize(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    meta: bool,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = if meta {
        format!("{}/{}/_meta", org, database)
    } else {
        format!("{}/{}/local/branch/{}", org, database, branch)
    };

    let result = client.optimize(&path, None).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_squash(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    author: String,
    message: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client.squash(&path, &author, &message, None).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_squash_and_reset(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    author: String,
    message: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client.squash_and_reset(&path, &author, &message).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_database_create(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    label: Option<String>,
    comment: Option<String>,
    schema: bool,
) -> Result<()> {
    let label_str = label.as_deref().unwrap_or(&database);
    let comment_str = comment.as_deref().unwrap_or("");

    // Create HTTP client for direct API call
    let http_client = reqwest::Client::new();
    let api_url = format!("{}/api/db/{}/{}", host, org, database);

    let body = json!({
        "label": label_str,
        "comment": comment_str,
        "public": false,
        "schema": schema
    });

    let res = http_client
        .post(&api_url)
        .basic_auth(&user, Some(&password))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("Failed to create database: {}", error_text);
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_database_info(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
) -> Result<()> {
    let http_client = reqwest::Client::new();
    let api_url = format!("{}/api/db/{}/{}", host, org, database);

    let res = http_client
        .get(&api_url)
        .basic_auth(&user, Some(&password))
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await?;
        anyhow::bail!(
            "Failed to get database info (status {}): {}",
            status,
            error_text
        );
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_database_list(
    host: String,
    user: String,
    password: String,
    org: String,
) -> Result<()> {
    let http_client = reqwest::Client::new();
    let api_url = format!("{}/api/db/{}", host, org);

    let res = http_client
        .get(&api_url)
        .basic_auth(&user, Some(&password))
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await?;
        anyhow::bail!(
            "Failed to list databases (status {}): {}",
            status,
            error_text
        );
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_database_delete(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    force: bool,
) -> Result<()> {
    if !force {
        eprintln!("About to delete database: {}/{}", org, database);
        eprintln!("This action cannot be undone!");
        eprint!("Type 'yes' to confirm: ");
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        if input.trim() != "yes" {
            eprintln!("Deletion cancelled.");
            return Ok(());
        }
    }

    let http_client = reqwest::Client::new();
    let api_url = format!("{}/api/db/{}/{}", host, org, database);

    let res = http_client
        .delete(&api_url)
        .basic_auth(&user, Some(&password))
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await?;
        anyhow::bail!(
            "Failed to delete database (status {}): {}",
            status,
            error_text
        );
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_database_log(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    limit: usize,
) -> Result<()> {
    let http_client = reqwest::Client::new();
    let api_url = format!("{}/api/log/{}/{}", host, org, database);

    let res = http_client
        .get(&api_url)
        .basic_auth(&user, Some(&password))
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await?;
        anyhow::bail!(
            "Failed to get commit log (status {}): {}",
            status,
            error_text
        );
    }

    let mut result: Vec<serde_json::Value> = res.json().await?;

    // Limit the number of commits shown
    if result.len() > limit {
        result.truncate(limit);
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_deploy(
    source_host: String,
    source_user: String,
    source_password: String,
    source_org: String,
    source_db: String,
    source_branch: String,
    target_host: String,
    target_user: String,
    target_password: String,
    target_org: String,
    target_db: String,
    target_label: Option<String>,
    target_comment: Option<String>,
    skip_create: bool,
) -> Result<()> {
    eprintln!(
        "Starting deployment from {}:{}/{} to {}:{}/{}",
        source_host, source_org, source_db, target_host, target_org, target_db
    );

    // Step 1: Clone source database to target using clone_repository
    eprintln!("\n[1/1] Cloning source database to target...");

    let target_url = Url::parse(&target_host)?;
    let target_client =
        TerminusDBHttpClient::new(target_url, &target_user, &target_password, &target_org).await?;

    let source_remote_url = format!("{}/{}/{}", source_host, source_org, source_db);

    let label = target_label.as_deref();
    let comment = target_comment.as_deref();

    target_client
        .clone_repository(
            &target_org,
            &target_db,
            &source_remote_url,
            label,
            comment,
            Some((&source_user, &source_password)),
            None, // timeout
        )
        .await?;

    eprintln!("✓ Successfully cloned database");

    eprintln!("\n🎉 Deployment completed successfully!");
    eprintln!(
        "   Source: {}:{}/{} (branch: {})",
        source_host, source_org, source_db, source_branch
    );
    eprintln!("   Target: {}:{}/{}", target_host, target_org, target_db);

    Ok(())
}

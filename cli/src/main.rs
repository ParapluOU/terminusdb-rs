mod formatter;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use formatter::{ColorMode, OutputFormat};
use futures_util::stream::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use terminusdb_client::TerminusDBHttpClient;
use tracing::{debug, error, info, warn};
use url::Url;

#[derive(Parser)]
#[command(name = "tdb")]
#[command(about = "TerminusDB CLI - Command line interface for TerminusDB operations", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream changeset events from TerminusDB SSE endpoint
    Changestream {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG", default_value = "admin")]
        org: String,

        /// Database name to monitor
        #[arg(long, env = "TERMINUSDB_DB")]
        database: Option<String>,

        /// Branch name to monitor
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Output format: json, compact, or pretty (default)
        #[arg(long, default_value = "pretty")]
        format: String,

        /// Color output: auto (default), always, or never
        #[arg(long, default_value = "auto")]
        color: String,
    },

    /// Remote repository management commands
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },

    /// Clone a remote repository to create a new database
    Clone {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization to create database in
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Name for the new database
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// URL of the remote repository to clone
        #[arg(long)]
        remote_url: String,

        /// Optional label for the database
        #[arg(long)]
        label: Option<String>,

        /// Optional comment for the database
        #[arg(long)]
        comment: Option<String>,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Fetch changes from a remote repository
    Fetch {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Name of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Remote branch name
        #[arg(long, default_value = "main")]
        remote_branch: String,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Pull changes from a remote repository (fetch + merge)
    Pull {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// URL of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Optional remote branch name
        #[arg(long)]
        remote_branch: Option<String>,

        /// Author for the merge commit
        #[arg(long)]
        author: String,

        /// Message for the merge commit
        #[arg(long)]
        message: String,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Push changes to a remote repository
    Push {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// URL of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Optional remote branch name
        #[arg(long)]
        remote_branch: Option<String>,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Optimize a database graph (branch or metadata)
    Optimize {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name (ignored if --meta is used)
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Optimize the metadata graph instead of a branch
        #[arg(long)]
        meta: bool,
    },

    /// Deploy a database from source to target (reverse branch cloning)
    Deploy {
        /// Source TerminusDB server URL
        #[arg(long)]
        source_host: String,

        /// Source username
        #[arg(long, default_value = "admin")]
        source_user: String,

        /// Source password
        #[arg(long, default_value = "root")]
        source_password: String,

        /// Source organization
        #[arg(long)]
        source_org: String,

        /// Source database
        #[arg(long)]
        source_db: String,

        /// Source branch (default: main)
        #[arg(long, default_value = "main")]
        source_branch: String,

        /// Target TerminusDB server URL
        #[arg(long)]
        target_host: String,

        /// Target username
        #[arg(long, default_value = "admin")]
        target_user: String,

        /// Target password
        #[arg(long, default_value = "root")]
        target_password: String,

        /// Target organization
        #[arg(long)]
        target_org: String,

        /// Target database
        #[arg(long)]
        target_db: String,

        /// Optional label for target database
        #[arg(long)]
        target_label: Option<String>,

        /// Optional comment for target database
        #[arg(long)]
        target_comment: Option<String>,

        /// Skip creating target database (use if it already exists)
        #[arg(long)]
        skip_create: bool,
    },

    /// Database management commands
    Database {
        #[command(subcommand)]
        command: DatabaseCommands,
    },
}

#[derive(Subcommand)]
enum DatabaseCommands {
    /// Create a new database
    Create {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Optional label for the database
        #[arg(long)]
        label: Option<String>,

        /// Optional comment/description for the database
        #[arg(long)]
        comment: Option<String>,

        /// Create with schema graph (default: true)
        #[arg(long, default_value = "true")]
        schema: bool,
    },

    /// Get information about a database
    Info {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,
    },

    /// List all databases in an organization
    List {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,
    },

    /// Delete a database
    Delete {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Get commit log for a database
    Log {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Limit number of commits to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum RemoteCommands {
    /// Add a new remote repository
    Add {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name for the remote (e.g., "origin")
        #[arg(long)]
        name: String,

        /// URL of the remote repository
        #[arg(long)]
        url: String,
    },

    /// List all remotes for a database
    List {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,
    },

    /// Get information about a specific remote
    Get {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote
        #[arg(long)]
        name: String,
    },

    /// Update a remote repository URL
    Update {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote
        #[arg(long)]
        name: String,

        /// New URL for the remote repository
        #[arg(long)]
        url: String,
    },

    /// Delete a remote repository
    Delete {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote to delete
        #[arg(long)]
        name: String,
    },
}

/// SSE event data from TerminusDB changeset plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangesetEvent {
    /// Resource path, e.g., "admin/dev/local/branch/main"
    resource: String,
    /// Branch name, e.g., "main"
    branch: String,
    /// Commit information
    commit: ChangesetCommitInfo,
    /// Metadata about changes
    metadata: MetadataInfo,
    /// List of document changes
    changes: Vec<DocumentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangesetCommitInfo {
    id: String,
    author: String,
    message: String,
    timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataInfo {
    inserts_count: u64,
    deletes_count: u64,
    documents_added: u64,
    documents_deleted: u64,
    documents_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentChange {
    id: String,
    action: String,
}

/// Helper function to parse remote authentication string
fn parse_remote_auth(auth_str: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = auth_str.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Remote auth must be in format 'username:password'");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

async fn run_remote_add(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
    url: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.add_remote(&path, &name, &url).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_remote_list(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let remotes = client.list_remotes(&path).await?;

    println!("{}", serde_json::to_string_pretty(&remotes)?);
    Ok(())
}

async fn run_remote_get(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let remote = client.get_remote(&path, &name).await?;

    println!("{}", serde_json::to_string_pretty(&remote)?);
    Ok(())
}

async fn run_remote_update(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
    url: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.update_remote(&path, &name, &url).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_remote_delete(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.delete_remote(&path, &name).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_clone(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    remote_url: String,
    label: Option<String>,
    comment: Option<String>,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth.as_ref().map(|s| parse_remote_auth(s)).transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let result = client.clone_repository(
        &org,
        &database,
        &remote_url,
        label.as_deref(),
        comment.as_deref(),
        auth,
    ).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_fetch(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: String,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth.as_ref().map(|s| parse_remote_auth(s)).transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client.fetch(&path, &remote_url, Some(&remote_branch), auth).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_pull(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: Option<String>,
    author: String,
    message: String,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth.as_ref().map(|s| parse_remote_auth(s)).transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client.pull(
        &path,
        &remote_url,
        remote_branch.as_deref(),
        &author,
        &message,
        auth,
    ).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_push(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: Option<String>,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth.as_ref().map(|s| parse_remote_auth(s)).transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client.push(&path, &remote_url, remote_branch.as_deref(), auth).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_optimize(
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

    let result = client.optimize(&path).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_database_create(
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

async fn run_database_info(
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
        anyhow::bail!("Failed to get database info (status {}): {}", status, error_text);
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_database_list(
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
        anyhow::bail!("Failed to list databases (status {}): {}", status, error_text);
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_database_delete(
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
        anyhow::bail!("Failed to delete database (status {}): {}", status, error_text);
    }

    let result: serde_json::Value = res.json().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_database_log(
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
        anyhow::bail!("Failed to get commit log (status {}): {}", status, error_text);
    }

    let mut result: Vec<serde_json::Value> = res.json().await?;

    // Limit the number of commits shown
    if result.len() > limit {
        result.truncate(limit);
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn run_deploy(
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
    eprintln!("Starting deployment from {}:{}/{} to {}:{}/{}",
        source_host, source_org, source_db, target_host, target_org, target_db);

    // Step 1: Clone source database to target using clone_repository
    eprintln!("\n[1/1] Cloning source database to target...");

    let target_url = Url::parse(&target_host)?;
    let target_client = TerminusDBHttpClient::new(target_url, &target_user, &target_password, &target_org).await?;

    let source_remote_url = format!("{}/{}/{}", source_host, source_org, source_db);

    let label = target_label.as_deref();
    let comment = target_comment.as_deref();

    target_client.clone_repository(
        &target_org,
        &target_db,
        &source_remote_url,
        label,
        comment,
        Some((&source_user, &source_password))
    ).await?;

    eprintln!("✓ Successfully cloned database");

    eprintln!("\n🎉 Deployment completed successfully!");
    eprintln!("   Source: {}:{}/{} (branch: {})", source_host, source_org, source_db, source_branch);
    eprintln!("   Target: {}:{}/{}", target_host, target_org, target_db);

    Ok(())
}

async fn run_changestream(
    host: String,
    user: String,
    password: String,
    org: String,
    database: Option<String>,
    branch: String,
    format: String,
    color: String,
) -> Result<()> {
    // Validate database is provided
    let db = database.context("Database name is required. Provide via --database or TERMINUSDB_DB env var")?;

    // Parse the host URL
    let url = Url::parse(&host)
        .context(format!("Invalid TerminusDB host URL: {}", host))?;

    info!("Connecting to TerminusDB at {}", url);
    info!("Monitoring database: {} (branch: {})", db, branch);

    // Create the client to verify connection
    let client = TerminusDBHttpClient::new(url.clone(), &user, &password, &org)
        .await
        .context("Failed to create TerminusDB client")?;

    // Verify connection
    if !client.is_running().await {
        anyhow::bail!("TerminusDB server is not running at {}", host);
    }

    info!("Connected successfully to TerminusDB");

    // Get the SSE endpoint URL from the client (includes /api prefix)
    let sse_url = client.get_sse_url();

    eprintln!("Streaming changesets from {}/{} (branch: {}). Press Ctrl+C to stop.", org, db, branch);
    info!("SSE endpoint URL: {}", sse_url);
    info!("Authenticating as: {}", user);
    info!("Organization: {}", org);

    // Expected resource path for filtering
    let resource_path = format!("{}/{}/local/branch/{}", org, db, branch);
    info!("Filtering events for resource: {}", resource_path);

    // Parse output options
    let output_format = OutputFormat::from_str(&format);
    let color_mode = ColorMode::from_str(&color);
    let colorize = color_mode.should_colorize();

    // Build HTTP client
    let http_client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    info!("Creating SSE request to: {}", sse_url);
    info!("Request headers: Accept: text/event-stream, Authorization: Basic ***");

    // Create authenticated request
    let request = http_client
        .get(&sse_url)
        .basic_auth(&user, Some(&password))
        .header("Accept", "text/event-stream");

    info!("Attempting to establish SSE connection...");

    // Create EventSource from the request
    let mut event_source = EventSource::new(request)
        .map_err(|e| anyhow!("Failed to create EventSource: {}. Check that the TerminusDB server has the changeset plugin enabled and is accessible at {}", e, sse_url))?;

    info!("SSE connection established, waiting for events...");

    // Process events from the stream
    loop {
        tokio::select! {
            // Handle Ctrl+C
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal, exiting...");
                break;
            }

            // Process SSE events
            event_result = event_source.next() => {
                match event_result {
                    Some(Ok(Event::Open)) => {
                        info!("SSE connection opened successfully!");
                    }
                    Some(Ok(Event::Message(message))) => {
                        // Only process changeset events
                        if message.event == "changeset" {
                            match serde_json::from_str::<ChangesetEvent>(&message.data) {
                                Ok(changeset_event) => {
                                    // Filter events by resource path
                                    if changeset_event.resource == resource_path {
                                        // Format and print the event
                                        match output_format {
                                            OutputFormat::Compact => {
                                                // Compact one-line format
                                                let metadata_str = formatter::format_metadata(
                                                    changeset_event.metadata.documents_added,
                                                    changeset_event.metadata.documents_updated,
                                                    changeset_event.metadata.documents_deleted,
                                                    colorize,
                                                );
                                                println!("{} | {} | {} | {}",
                                                    changeset_event.commit.id,
                                                    changeset_event.commit.author,
                                                    changeset_event.commit.message,
                                                    metadata_str
                                                );
                                            }
                                            OutputFormat::Json => {
                                                // JSON format
                                                let output = json!({
                                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                                    "resource": changeset_event.resource,
                                                    "branch": changeset_event.branch,
                                                    "commit": {
                                                        "id": changeset_event.commit.id,
                                                        "author": changeset_event.commit.author,
                                                        "message": changeset_event.commit.message,
                                                        "timestamp": changeset_event.commit.timestamp,
                                                    },
                                                    "metadata": {
                                                        "inserts_count": changeset_event.metadata.inserts_count,
                                                        "deletes_count": changeset_event.metadata.deletes_count,
                                                        "documents_added": changeset_event.metadata.documents_added,
                                                        "documents_deleted": changeset_event.metadata.documents_deleted,
                                                        "documents_updated": changeset_event.metadata.documents_updated,
                                                    },
                                                    "changes": changeset_event.changes,
                                                });

                                                println!("{}", serde_json::to_string(&output)?);
                                            }
                                            OutputFormat::Pretty => {
                                                // Pretty formatted output with colors
                                                let header = formatter::format_commit_header(
                                                    &changeset_event.commit.id,
                                                    &changeset_event.commit.author,
                                                    &changeset_event.commit.message,
                                                    changeset_event.commit.timestamp,
                                                    colorize,
                                                );
                                                println!("\n{}", header);

                                                let metadata_str = formatter::format_metadata(
                                                    changeset_event.metadata.documents_added,
                                                    changeset_event.metadata.documents_updated,
                                                    changeset_event.metadata.documents_deleted,
                                                    colorize,
                                                );
                                                println!("Changes: {}\n", metadata_str);

                                                // Print each document change
                                                for change in &changeset_event.changes {
                                                    // For updated documents, we currently don't have field-level changes
                                                    // from the SSE event. In future, this could be enhanced.
                                                    let change_output = formatter::format_document_change(
                                                        &change.id,
                                                        &change.action,
                                                        None, // changed_fields would go here if available
                                                        colorize,
                                                    );
                                                    println!("{}", change_output);
                                                }
                                                println!(); // Extra newline between events
                                            }
                                        }
                                    } else {
                                        debug!("Ignoring event for different resource: {}", changeset_event.resource);
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse changeset event: {}", e);
                                    debug!("Raw event data: {}", message.data);
                                }
                            }
                        } else if message.event.is_empty() {
                            // Heartbeat or comment
                            debug!("Received SSE heartbeat");
                        } else {
                            debug!("Received unknown event type: {}", message.event);
                        }
                    }
                    Some(Err(e)) => {
                        error!("SSE stream error: {}", e);
                        error!("Failed URL: {}", sse_url);
                        error!("This could mean:");
                        error!("  1. The TerminusDB server doesn't have the changeset plugin enabled");
                        error!("  2. The /changesets/stream endpoint is not available");
                        error!("  3. Authentication failed (check credentials)");
                        error!("  4. Network connectivity issues");
                        return Err(anyhow!("SSE stream error: {}. See logs above for details.", e));
                    }
                    None => {
                        warn!("SSE stream ended");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (logs to stderr, keeping stdout clean for data)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Changestream {
            host,
            user,
            password,
            org,
            database,
            branch,
            format,
            color,
        } => {
            run_changestream(host, user, password, org, database, branch, format, color).await
        }
        Commands::Remote { command } => match command {
            RemoteCommands::Add {
                host,
                user,
                password,
                org,
                database,
                name,
                url,
            } => run_remote_add(host, user, password, org, database, name, url).await,
            RemoteCommands::List {
                host,
                user,
                password,
                org,
                database,
            } => run_remote_list(host, user, password, org, database).await,
            RemoteCommands::Get {
                host,
                user,
                password,
                org,
                database,
                name,
            } => run_remote_get(host, user, password, org, database, name).await,
            RemoteCommands::Update {
                host,
                user,
                password,
                org,
                database,
                name,
                url,
            } => run_remote_update(host, user, password, org, database, name, url).await,
            RemoteCommands::Delete {
                host,
                user,
                password,
                org,
                database,
                name,
            } => run_remote_delete(host, user, password, org, database, name).await,
        },
        Commands::Clone {
            host,
            user,
            password,
            org,
            database,
            remote_url,
            label,
            comment,
            remote_auth,
        } => {
            run_clone(
                host,
                user,
                password,
                org,
                database,
                remote_url,
                label,
                comment,
                remote_auth,
            )
            .await
        }
        Commands::Fetch {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            remote_auth,
        } => {
            run_fetch(host, user, password, org, database, branch, remote_url, remote_branch, remote_auth).await
        }
        Commands::Pull {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            author,
            message,
            remote_auth,
        } => {
            run_pull(
                host,
                user,
                password,
                org,
                database,
                branch,
                remote_url,
                remote_branch,
                author,
                message,
                remote_auth,
            )
            .await
        }
        Commands::Push {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            remote_auth,
        } => {
            run_push(
                host,
                user,
                password,
                org,
                database,
                branch,
                remote_url,
                remote_branch,
                remote_auth,
            )
            .await
        }
        Commands::Optimize {
            host,
            user,
            password,
            org,
            database,
            branch,
            meta,
        } => run_optimize(host, user, password, org, database, branch, meta).await,
        Commands::Deploy {
            source_host,
            source_user,
            source_password,
            source_org,
            source_db,
            source_branch,
            target_host,
            target_user,
            target_password,
            target_org,
            target_db,
            target_label,
            target_comment,
            skip_create,
        } => {
            run_deploy(
                source_host,
                source_user,
                source_password,
                source_org,
                source_db,
                source_branch,
                target_host,
                target_user,
                target_password,
                target_org,
                target_db,
                target_label,
                target_comment,
                skip_create,
            )
            .await
        }
        Commands::Database { command } => match command {
            DatabaseCommands::Create {
                host,
                user,
                password,
                org,
                database,
                label,
                comment,
                schema,
            } => run_database_create(host, user, password, org, database, label, comment, schema).await,
            DatabaseCommands::Info {
                host,
                user,
                password,
                org,
                database,
            } => run_database_info(host, user, password, org, database).await,
            DatabaseCommands::List {
                host,
                user,
                password,
                org,
            } => run_database_list(host, user, password, org).await,
            DatabaseCommands::Delete {
                host,
                user,
                password,
                org,
                database,
                force,
            } => run_database_delete(host, user, password, org, database, force).await,
            DatabaseCommands::Log {
                host,
                user,
                password,
                org,
                database,
                limit,
            } => run_database_log(host, user, password, org, database, limit).await,
        },
    }
}

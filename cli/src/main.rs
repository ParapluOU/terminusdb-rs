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
    }
}

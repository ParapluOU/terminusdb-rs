//! `tdb-reactor`: a reactive cache and reverse proxy for TerminusDB 12.
//!
//! Startup order matters and is not arbitrary:
//!
//! 1. configuration, validated — startup is the one place we fail loudly;
//! 2. the engine and one **writer task per namespace**, before capture, so no
//!    message can be sent to a mailbox nobody is draining;
//! 3. capture, which establishes each namespace's first watermark (until it
//!    does, reads simply bypass — there is no position to pin a fill at);
//! 4. the listener, last.
//!
//! Shutdown runs in reverse: stop accepting, signal the writers, join.

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use terminusdb_reactor::capture::{CaptureHint, Supervisor};
use terminusdb_reactor::config::{Cli, Config};
use terminusdb_reactor::engine::Engine;
use terminusdb_reactor::frontend::{self, AppState};
use terminusdb_reactor::store::{FsCasStore, MemoryStore, TieredStore};
use terminusdb_reactor::FragmentStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Arc::new(cli.resolve()?);
    init_tracing(&config);

    tracing::info!(
        node = %config.node.id,
        listen = %config.node.listen,
        upstream = %config.upstream.url,
        namespaces = config.namespaces.len(),
        "starting tdb-reactor"
    );
    if config.namespaces.is_empty() {
        tracing::warn!(
            "no namespace is configured: every request will be proxied and nothing cached"
        );
    }

    // ---- the fragment store
    let hot = MemoryStore::new(config.cache.memory_budget_mb * 1024 * 1024);
    let warm = match &config.cache.dir {
        Some(dir) => {
            let store = FsCasStore::open(dir)
                .with_context(|| format!("opening the warm tier at {}", dir.display()))?;
            tracing::info!(dir = %dir.display(), bytes = store.size(), "warm fragment tier open");
            Some(store)
        }
        None => None,
    };
    let store: Arc<dyn FragmentStore> = Arc::new(TieredStore::new(hot, warm));

    // ---- the engine, and a writer per namespace *before* capture starts
    let engine = Arc::new(Engine::new(config.engine_config(), store));
    let mut writers = Vec::new();
    for ns in config.namespaces() {
        let writer = engine
            .take_writer(&ns)
            .with_context(|| format!("a writer for {ns} was already taken"))?;
        writers.push(tokio::spawn(writer.run()));
    }

    // ---- the upstream client, for capture and fills only
    let upstream = url::Url::parse(&config.upstream.url)?;
    let client = Arc::new(
        terminusdb_client::TerminusDBHttpClient::new_with_timeout(
            upstream,
            &config.upstream.user,
            &config.upstream.pass,
            &config.upstream.org,
            Some(config.upstream.request_timeout()),
        )
        .await
        .context("connecting to the local TerminusDB node")?,
    );

    // ---- capture
    let (hints, hint_rx) = tokio::sync::mpsc::unbounded_channel::<CaptureHint>();
    let supervisor = Arc::new(Supervisor::new(
        Arc::clone(&engine),
        Arc::clone(&client),
        Arc::clone(&config),
    ));
    let capture = tokio::spawn(Arc::clone(&supervisor).run(hint_rx));

    // ---- the front end
    let proxy = Arc::new(frontend::build_proxy(&config)?);
    let gate = Arc::new(frontend::build_gate(&config)?);
    let state =
        AppState::new(Arc::clone(&config), Arc::clone(&engine), proxy, gate, hints).into_shared();
    let app = frontend::router(state);

    let addr = config.listen_addr()?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving")?;

    // ---- shutdown, in reverse
    tracing::info!("shutting down");
    capture.abort();
    // Tells every writer loop to finish; `Engine::drop` would do it too, but
    // being explicit means the writers stop before we stop waiting for them.
    engine.shutdown();
    for writer in writers {
        let _ = writer.await;
    }
    tracing::info!("stopped");
    Ok(())
}

/// Install the tracing subscriber.
///
/// `RUST_LOG` wins over the configured level, because when you are debugging a
/// running node you should not have to edit a file to raise the log level.
fn init_tracing(config: &Config) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.observability.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

/// Resolve on SIGINT or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            // Without a handler we simply never resolve on SIGTERM; ctrl-c
            // still works, and the process can still be killed.
            Err(err) => {
                tracing::warn!(error = %err, "could not install a SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("SIGINT received"),
        _ = terminate => tracing::info!("SIGTERM received"),
    }
}

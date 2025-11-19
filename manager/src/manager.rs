use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::process::{Child, Command};
use std::sync::Arc;
use terminusdb_bin::extract_binary;
use terminusdb_client::TerminusDBHttpClient;
use url::Url;

/// Manages the local TerminusDB instance lifecycle
#[derive(Clone)]
pub struct TerminusDBManager {
    inner: Arc<RwLock<ManagerInner>>,
}

struct ManagerInner {
    /// Spawned process handle
    process: Option<Child>,
    /// Port the instance is running on
    port: u16,
    /// Password for the local instance
    password: String,
}

impl TerminusDBManager {
    /// Create a new manager and start the local TerminusDB instance
    pub async fn new() -> Result<Self> {
        let password = std::env::var("TERMINUSDB_ADMIN_PASS")
            .or_else(|_| std::env::var("TERMINUSDB_PASS"))
            .unwrap_or_else(|_| "root".to_string());

        let manager = Self {
            inner: Arc::new(RwLock::new(ManagerInner {
                process: None,
                port: 0,
                password,
            })),
        };

        manager.ensure_running().await?;
        Ok(manager)
    }

    /// Ensure the local TerminusDB instance is running, start it if not
    pub async fn ensure_running(&self) -> Result<()> {
        // Check if already running (in a separate scope to drop lock)
        {
            let mut inner = self.inner.write();
            if let Some(ref mut child) = inner.process {
                if let Ok(None) = child.try_wait() {
                    // Process is still running
                    return Ok(());
                }
            }
        }

        // TerminusDB uses port 6363 by default (not configurable via CLI)
        let port = 6363;

        tracing::info!("Starting local TerminusDB instance on port {}", port);

        // Extract the TerminusDB binary
        let binary_path = extract_binary()
            .context("Failed to extract TerminusDB binary")?;

        // Get password for spawn command
        let password = self.inner.read().password.clone();

        // Spawn TerminusDB process in background
        // Use the 'serve' command with memory mode for development
        let child = Command::new(&binary_path)
            .args(&[
                "serve",
                "--memory",
                &password,
            ])
            .spawn()
            .context("Failed to spawn TerminusDB process")?;

        // Store the process handle (in a separate scope to drop lock before await)
        {
            let mut inner = self.inner.write();
            inner.process = Some(child);
            inner.port = port;
        }

        // Wait a moment for the server to start
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Verify it's accessible
        self.ping().await?;

        tracing::info!("Local TerminusDB instance started successfully on port {}", port);
        Ok(())
    }

    /// Get the port the local instance is running on
    pub fn port(&self) -> u16 {
        self.inner.read().port
    }

    /// Get the password for the local instance
    pub fn password(&self) -> String {
        self.inner.read().password.clone()
    }

    /// Check if the local instance is running
    pub fn is_running(&self) -> bool {
        let inner = self.inner.read();
        if let Some(ref process) = inner.process {
            // Check if process is alive (this is a heuristic, not perfect)
            matches!(std::process::Command::new("ps")
                .arg("-p")
                .arg(process.id().to_string())
                .output(), Ok(output) if output.status.success())
        } else {
            false
        }
    }

    /// Get a client for the local instance
    pub async fn client(&self) -> Result<TerminusDBHttpClient> {
        let (port, password) = {
            let inner = self.inner.read();
            (inner.port, inner.password.clone())
        };

        let url = Url::parse(&format!("http://localhost:{}", port))?;
        TerminusDBHttpClient::new(url, "admin", &password, "admin").await
    }

    /// Ping the local instance to verify it's accessible
    async fn ping(&self) -> Result<()> {
        let client = self.client().await?;
        client.info().await.context("Failed to ping local TerminusDB instance")?;
        Ok(())
    }

    /// Stop the local TerminusDB instance
    pub fn stop(&self) -> Result<()> {
        let mut inner = self.inner.write();

        if let Some(mut child) = inner.process.take() {
            tracing::info!("Stopping local TerminusDB instance");

            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                if let Err(e) = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM) {
                    tracing::warn!("Failed to send SIGTERM: {}", e);
                }
            }

            // Wait a moment for graceful shutdown
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Force kill if still running
            if child.try_wait()?.is_none() {
                tracing::warn!("Forcing termination of TerminusDB process");
                child.kill()?;
            }

            child.wait()?;
            tracing::info!("Local TerminusDB instance stopped");
        }

        Ok(())
    }
}

impl Drop for ManagerInner {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

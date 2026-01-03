//! Concurrency limiting infrastructure for TerminusDB HTTP client
//!
//! This module provides optional concurrency limiting for HTTP requests to TerminusDB servers.
//! Concurrency limiters are keyed per host and shared across all client instances connecting
//! to the same host, ensuring coordinated concurrency control even with multiple clients.
//!
//! ## Features
//!
//! - **Separate READ/WRITE limits**: Different concurrency limits for GET vs POST/PUT/DELETE operations
//! - **Per-host keying**: Each TerminusDB host can have its own concurrency limits
//! - **Global sharing**: Multiple client instances to the same host share concurrency limiters
//! - **Environment variable configuration**: Can be configured via `TERMINUSDB_CONCURRENCY_LIMIT_READ`
//!   and `TERMINUSDB_CONCURRENCY_LIMIT_WRITE` environment variables
//! - **Optional**: Concurrency limiting is opt-in and has zero overhead when disabled
//!
//! ## Usage
//!
//! ### Via Environment Variables (Automatic)
//!
//! ```bash
//! export TERMINUSDB_CONCURRENCY_LIMIT_READ=10   # Max 10 concurrent reads
//! export TERMINUSDB_CONCURRENCY_LIMIT_WRITE=5   # Max 5 concurrent writes
//! ```
//!
//! ```rust,ignore
//! // Concurrency limits automatically applied from environment
//! let client = TerminusDBHttpClient::new(url, "admin", "root", "admin").await?;
//! ```
//!
//! ### Via Code (Manual)
//!
//! ```rust,ignore
//! use terminusdb_client::http::ConcurrencyLimitConfig;
//!
//! let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
//!     .await?
//!     .with_concurrency_limit(ConcurrencyLimitConfig {
//!         max_concurrent_reads: Some(10),
//!         max_concurrent_writes: Some(5),
//!     });
//! ```

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::debug;

/// Configuration for concurrency limiting
///
/// Both fields are optional. If `None`, no concurrency limiting is applied for that operation type.
#[derive(Debug, Clone)]
pub struct ConcurrencyLimitConfig {
    /// Maximum number of concurrent read (GET) requests
    pub max_concurrent_reads: Option<usize>,
    /// Maximum number of concurrent write (POST/PUT/DELETE) requests
    pub max_concurrent_writes: Option<usize>,
}

impl ConcurrencyLimitConfig {
    /// Create concurrency limit config from environment variables
    ///
    /// Reads the following environment variables:
    /// - `TERMINUSDB_CONCURRENCY_LIMIT_READ`: Max concurrent read operations (GET requests)
    /// - `TERMINUSDB_CONCURRENCY_LIMIT_WRITE`: Max concurrent write operations (POST/PUT/DELETE requests)
    ///
    /// Returns `None` if neither environment variable is set.
    ///
    /// # Example
    ///
    /// ```bash
    /// export TERMINUSDB_CONCURRENCY_LIMIT_READ=10
    /// export TERMINUSDB_CONCURRENCY_LIMIT_WRITE=5
    /// ```
    ///
    /// ```rust
    /// # use terminusdb_client::http::concurrency_limiter::ConcurrencyLimitConfig;
    /// // Will read from environment
    /// if let Some(config) = ConcurrencyLimitConfig::from_env() {
    ///     println!("Concurrency limits configured from environment");
    /// }
    /// ```
    pub fn from_env() -> Option<Self> {
        let read = std::env::var("TERMINUSDB_CONCURRENCY_LIMIT_READ")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());

        let write = std::env::var("TERMINUSDB_CONCURRENCY_LIMIT_WRITE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());

        // Only create config if at least one limit is set
        if read.is_some() || write.is_some() {
            debug!(
                "Concurrency limiting configured from environment: read={:?}, write={:?}",
                read, write
            );
            Some(ConcurrencyLimitConfig {
                max_concurrent_reads: read,
                max_concurrent_writes: write,
            })
        } else {
            None
        }
    }

    /// Create a new concurrency limit config with the given limits
    ///
    /// # Example
    ///
    /// ```rust
    /// # use terminusdb_client::http::concurrency_limiter::ConcurrencyLimitConfig;
    /// // Limit to 10 concurrent reads and 5 concurrent writes
    /// let config = ConcurrencyLimitConfig::new(Some(10), Some(5));
    ///
    /// // Only limit reads
    /// let config = ConcurrencyLimitConfig::new(Some(10), None);
    ///
    /// // Only limit writes
    /// let config = ConcurrencyLimitConfig::new(None, Some(5));
    /// ```
    pub fn new(max_concurrent_reads: Option<usize>, max_concurrent_writes: Option<usize>) -> Self {
        Self {
            max_concurrent_reads,
            max_concurrent_writes,
        }
    }
}

/// Shared semaphore instance
type SharedSemaphore = Arc<Semaphore>;

/// Global registry of semaphores keyed by host
///
/// This ensures that multiple client instances connecting to the same host
/// share the same semaphores, providing coordinated concurrency limiting across
/// all clients.
static GLOBAL_SEMAPHORES: Lazy<DashMap<String, (Option<SharedSemaphore>, Option<SharedSemaphore>)>> =
    Lazy::new(DashMap::new);

/// Get or create semaphores for a given host
///
/// This function looks up existing semaphores for the host in the global registry.
/// If none exist, it creates new ones based on the provided configuration and stores
/// them in the registry.
///
/// Multiple calls with the same host will return the same semaphore instances,
/// ensuring coordination across all clients connecting to that host.
///
/// # Arguments
///
/// * `host` - The hostname/endpoint to create semaphores for
/// * `config` - The concurrency limit configuration specifying max concurrent operations
///
/// # Returns
///
/// A tuple of `(read_semaphore, write_semaphore)`, where each is `Option<Arc<Semaphore>>`.
/// Returns `None` for a semaphore if that operation type has no limit configured.
///
/// # Example
///
/// ```rust,ignore
/// let config = ConcurrencyLimitConfig {
///     max_concurrent_reads: Some(10),
///     max_concurrent_writes: Some(5),
/// };
///
/// let (read_sem, write_sem) = get_or_create_semaphores("localhost", &config);
/// ```
pub fn get_or_create_semaphores(
    host: &str,
    config: &ConcurrencyLimitConfig,
) -> (Option<SharedSemaphore>, Option<SharedSemaphore>) {
    debug!("Getting or creating semaphores for host: {}", host);

    GLOBAL_SEMAPHORES
        .entry(host.to_string())
        .or_insert_with(|| {
            debug!(
                "Creating new semaphores for host {}: read={:?} concurrent, write={:?} concurrent",
                host, config.max_concurrent_reads, config.max_concurrent_writes
            );

            let read = config
                .max_concurrent_reads
                .filter(|&n| n > 0)
                .map(|n| Arc::new(Semaphore::new(n)));

            let write = config
                .max_concurrent_writes
                .filter(|&n| n > 0)
                .map(|n| Arc::new(Semaphore::new(n)));

            (read, write)
        })
        .value()
        .clone()
}

/// Clear all semaphores from the global registry
///
/// This is primarily useful for testing. In production, semaphores typically
/// persist for the lifetime of the application.
///
/// # Example
///
/// ```rust
/// # use terminusdb_client::http::concurrency_limiter;
/// concurrency_limiter::clear_all_semaphores();
/// ```
pub fn clear_all_semaphores() {
    GLOBAL_SEMAPHORES.clear();
    debug!("Cleared all global semaphores");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_limit_config_new() {
        let config = ConcurrencyLimitConfig::new(Some(10), Some(5));
        assert_eq!(config.max_concurrent_reads, Some(10));
        assert_eq!(config.max_concurrent_writes, Some(5));
    }

    #[test]
    fn test_concurrency_limit_config_partial() {
        let config = ConcurrencyLimitConfig::new(Some(10), None);
        assert_eq!(config.max_concurrent_reads, Some(10));
        assert_eq!(config.max_concurrent_writes, None);
    }

    #[test]
    fn test_get_or_create_semaphores() {
        clear_all_semaphores();

        let config = ConcurrencyLimitConfig::new(Some(10), Some(5));
        let (read1, write1) = get_or_create_semaphores("test-host", &config);

        assert!(read1.is_some());
        assert!(write1.is_some());

        // Get again - should return same instances
        let (read2, write2) = get_or_create_semaphores("test-host", &config);

        assert!(Arc::ptr_eq(&read1.unwrap(), &read2.unwrap()));
        assert!(Arc::ptr_eq(&write1.unwrap(), &write2.unwrap()));
    }

    #[test]
    fn test_different_hosts_different_semaphores() {
        clear_all_semaphores();

        let config = ConcurrencyLimitConfig::new(Some(10), Some(5));
        let (read1, _) = get_or_create_semaphores("host1", &config);
        let (read2, _) = get_or_create_semaphores("host2", &config);

        assert!(!Arc::ptr_eq(&read1.unwrap(), &read2.unwrap()));
    }

    #[test]
    fn test_zero_concurrency_limit_returns_none() {
        clear_all_semaphores();

        let config = ConcurrencyLimitConfig::new(Some(0), Some(0));
        // Use unique host name to avoid test pollution from parallel tests
        let (read, write) = get_or_create_semaphores("test-host-zero-limit", &config);

        assert!(read.is_none());
        assert!(write.is_none());
    }
}

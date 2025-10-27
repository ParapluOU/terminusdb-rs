//! Rate limiting infrastructure for TerminusDB HTTP client
//!
//! This module provides optional rate limiting for HTTP requests to TerminusDB servers.
//! Rate limiters are keyed per host and shared across all client instances connecting
//! to the same host, ensuring coordinated rate limiting even with multiple clients.
//!
//! ## Features
//!
//! - **Separate READ/WRITE limits**: Different rate limits for GET vs POST/PUT/DELETE operations
//! - **Per-host keying**: Each TerminusDB host can have its own rate limits
//! - **Global sharing**: Multiple client instances to the same host share rate limiters
//! - **Environment variable configuration**: Can be configured via `TERMINUSDB_RATE_LIMIT_READ`
//!   and `TERMINUSDB_RATE_LIMIT_WRITE` environment variables
//! - **Optional**: Rate limiting is opt-in and has zero overhead when disabled
//!
//! ## Usage
//!
//! ### Via Environment Variables (Automatic)
//!
//! ```bash
//! export TERMINUSDB_RATE_LIMIT_READ=10   # 10 reads/second
//! export TERMINUSDB_RATE_LIMIT_WRITE=5   # 5 writes/second
//! ```
//!
//! ```rust,ignore
//! // Rate limits automatically applied from environment
//! let client = TerminusDBHttpClient::new(url, "admin", "root", "admin").await?;
//! ```
//!
//! ### Via Code (Manual)
//!
//! ```rust,ignore
//! use terminusdb_client::http::RateLimitConfig;
//!
//! let client = TerminusDBHttpClient::new(url, "admin", "root", "admin")
//!     .await?
//!     .with_rate_limit(RateLimitConfig {
//!         read_requests_per_second: Some(10),
//!         write_requests_per_second: Some(5),
//!     });
//! ```

use dashmap::DashMap;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use once_cell::sync::Lazy;
use std::num::NonZeroU32;
use std::sync::Arc;
use tracing::debug;

/// Configuration for rate limiting
///
/// Both fields are optional. If `None`, no rate limiting is applied for that operation type.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of read (GET) requests per second
    pub read_requests_per_second: Option<u32>,
    /// Maximum number of write (POST/PUT/DELETE) requests per second
    pub write_requests_per_second: Option<u32>,
}

impl RateLimitConfig {
    /// Create rate limit config from environment variables
    ///
    /// Reads the following environment variables:
    /// - `TERMINUSDB_RATE_LIMIT_READ`: Read operations per second (GET requests)
    /// - `TERMINUSDB_RATE_LIMIT_WRITE`: Write operations per second (POST/PUT/DELETE requests)
    ///
    /// Returns `None` if neither environment variable is set.
    ///
    /// # Example
    ///
    /// ```bash
    /// export TERMINUSDB_RATE_LIMIT_READ=10
    /// export TERMINUSDB_RATE_LIMIT_WRITE=5
    /// ```
    ///
    /// ```rust
    /// # use terminusdb_client::http::rate_limiter::RateLimitConfig;
    /// // Will read from environment
    /// if let Some(config) = RateLimitConfig::from_env() {
    ///     println!("Rate limits configured from environment");
    /// }
    /// ```
    pub fn from_env() -> Option<Self> {
        let read = std::env::var("TERMINUSDB_RATE_LIMIT_READ")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        let write = std::env::var("TERMINUSDB_RATE_LIMIT_WRITE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        // Only create config if at least one limit is set
        if read.is_some() || write.is_some() {
            debug!(
                "Rate limiting configured from environment: read={:?}, write={:?}",
                read, write
            );
            Some(RateLimitConfig {
                read_requests_per_second: read,
                write_requests_per_second: write,
            })
        } else {
            None
        }
    }

    /// Create a new rate limit config with the given limits
    ///
    /// # Example
    ///
    /// ```rust
    /// # use terminusdb_client::http::rate_limiter::RateLimitConfig;
    /// // Limit to 10 reads/sec and 5 writes/sec
    /// let config = RateLimitConfig::new(Some(10), Some(5));
    ///
    /// // Only limit reads
    /// let config = RateLimitConfig::new(Some(10), None);
    ///
    /// // Only limit writes
    /// let config = RateLimitConfig::new(None, Some(5));
    /// ```
    pub fn new(
        read_requests_per_second: Option<u32>,
        write_requests_per_second: Option<u32>,
    ) -> Self {
        Self {
            read_requests_per_second,
            write_requests_per_second,
        }
    }
}

/// Shared rate limiter instance
type SharedRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

/// Global registry of rate limiters keyed by host
///
/// This ensures that multiple client instances connecting to the same host
/// share the same rate limiters, providing coordinated rate limiting across
/// all clients.
static GLOBAL_RATE_LIMITERS: Lazy<
    DashMap<String, (Option<SharedRateLimiter>, Option<SharedRateLimiter>)>,
> = Lazy::new(DashMap::new);

/// Get or create rate limiters for a given host
///
/// This function looks up existing rate limiters for the host in the global registry.
/// If none exist, it creates new ones based on the provided configuration and stores
/// them in the registry.
///
/// Multiple calls with the same host will return the same rate limiter instances,
/// ensuring coordination across all clients connecting to that host.
///
/// # Arguments
///
/// * `host` - The hostname/endpoint to create rate limiters for
/// * `config` - The rate limit configuration specifying requests per second
///
/// # Returns
///
/// A tuple of `(read_rate_limiter, write_rate_limiter)`, where each is `Option<Arc<RateLimiter>>`.
/// Returns `None` for a limiter if that operation type has no limit configured.
///
/// # Example
///
/// ```rust,ignore
/// let config = RateLimitConfig {
///     read_requests_per_second: Some(10),
///     write_requests_per_second: Some(5),
/// };
///
/// let (read_limiter, write_limiter) = get_or_create_rate_limiters("localhost", &config);
/// ```
pub fn get_or_create_rate_limiters(
    host: &str,
    config: &RateLimitConfig,
) -> (Option<SharedRateLimiter>, Option<SharedRateLimiter>) {
    debug!("Getting or creating rate limiters for host: {}", host);

    GLOBAL_RATE_LIMITERS
        .entry(host.to_string())
        .or_insert_with(|| {
            debug!(
                "Creating new rate limiters for host {}: read={:?} req/s, write={:?} req/s",
                host, config.read_requests_per_second, config.write_requests_per_second
            );

            let read = config.read_requests_per_second.and_then(|rps| {
                NonZeroU32::new(rps).map(|rps| {
                    let quota = Quota::per_second(rps);
                    Arc::new(RateLimiter::direct(quota))
                })
            });

            let write = config.write_requests_per_second.and_then(|rps| {
                NonZeroU32::new(rps).map(|rps| {
                    let quota = Quota::per_second(rps);
                    Arc::new(RateLimiter::direct(quota))
                })
            });

            (read, write)
        })
        .value()
        .clone()
}

/// Clear all rate limiters from the global registry
///
/// This is primarily useful for testing. In production, rate limiters typically
/// persist for the lifetime of the application.
///
/// # Example
///
/// ```rust
/// # use terminusdb_client::http::rate_limiter;
/// rate_limiter::clear_all_rate_limiters();
/// ```
pub fn clear_all_rate_limiters() {
    GLOBAL_RATE_LIMITERS.clear();
    debug!("Cleared all global rate limiters");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_new() {
        let config = RateLimitConfig::new(Some(10), Some(5));
        assert_eq!(config.read_requests_per_second, Some(10));
        assert_eq!(config.write_requests_per_second, Some(5));
    }

    #[test]
    fn test_rate_limit_config_partial() {
        let config = RateLimitConfig::new(Some(10), None);
        assert_eq!(config.read_requests_per_second, Some(10));
        assert_eq!(config.write_requests_per_second, None);
    }

    #[test]
    fn test_get_or_create_rate_limiters() {
        clear_all_rate_limiters();

        let config = RateLimitConfig::new(Some(10), Some(5));
        let (read1, write1) = get_or_create_rate_limiters("test-host", &config);

        assert!(read1.is_some());
        assert!(write1.is_some());

        // Get again - should return same instances
        let (read2, write2) = get_or_create_rate_limiters("test-host", &config);

        assert!(Arc::ptr_eq(&read1.unwrap(), &read2.unwrap()));
        assert!(Arc::ptr_eq(&write1.unwrap(), &write2.unwrap()));
    }

    #[test]
    fn test_different_hosts_different_limiters() {
        clear_all_rate_limiters();

        let config = RateLimitConfig::new(Some(10), Some(5));
        let (read1, _) = get_or_create_rate_limiters("host1", &config);
        let (read2, _) = get_or_create_rate_limiters("host2", &config);

        assert!(!Arc::ptr_eq(&read1.unwrap(), &read2.unwrap()));
    }

    #[test]
    fn test_zero_rate_limit_returns_none() {
        clear_all_rate_limiters();

        let config = RateLimitConfig::new(Some(0), Some(0));
        let (read, write) = get_or_create_rate_limiters("test-host", &config);

        assert!(read.is_none());
        assert!(write.is_none());
    }
}

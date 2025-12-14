//! Global client singleton for ORM operations.
//!
//! Provides a set-once global client that can be accessed from anywhere in the application.

use std::sync::OnceLock;
use terminusdb_client::TerminusDBHttpClient;

/// Global ORM client singleton
static GLOBAL_CLIENT: OnceLock<TerminusDBHttpClient> = OnceLock::new();

/// Error returned when attempting to initialize the global client twice
#[derive(Debug, Clone)]
pub struct ClientAlreadyInitializedError {
    pub message: String,
}

impl std::fmt::Display for ClientAlreadyInitializedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ClientAlreadyInitializedError {}

/// ORM client management.
///
/// Provides methods to initialize and access a global TerminusDB client singleton.
///
/// # Example
/// ```ignore
/// use terminusdb_orm::OrmClient;
/// use terminusdb_client::TerminusDBHttpClient;
///
/// // Initialize once at application startup
/// let client = TerminusDBHttpClient::local_node().await?;
/// OrmClient::init(client)?;
///
/// // Access from anywhere
/// let client = OrmClient::get();
/// ```
pub struct OrmClient;

impl OrmClient {
    /// Initialize the global ORM client.
    ///
    /// Returns `Ok(())` on first call, `Err(ClientAlreadyInitializedError)` on subsequent calls.
    ///
    /// # Thread Safety
    /// This method is thread-safe. If multiple threads race to initialize,
    /// exactly one will succeed and others will receive an error.
    pub fn init(client: TerminusDBHttpClient) -> Result<(), ClientAlreadyInitializedError> {
        GLOBAL_CLIENT
            .set(client)
            .map_err(|_| ClientAlreadyInitializedError {
                message: "ORM client has already been initialized. \
                     OrmClient::init() can only be called once per process."
                    .to_string(),
            })
    }

    /// Initialize the global ORM client, panicking if already set.
    ///
    /// Use this in application startup where failure to initialize is unrecoverable.
    pub fn init_or_panic(client: TerminusDBHttpClient) {
        Self::init(client).expect("ORM client already initialized")
    }

    /// Get a reference to the global client.
    ///
    /// # Panics
    /// Panics if `OrmClient::init()` has not been called.
    pub fn get() -> &'static TerminusDBHttpClient {
        GLOBAL_CLIENT.get().expect(
            "ORM client not initialized. Call OrmClient::init() before using ORM features.",
        )
    }

    /// Try to get a reference to the global client.
    ///
    /// Returns `None` if the client has not been initialized.
    pub fn try_get() -> Option<&'static TerminusDBHttpClient> {
        GLOBAL_CLIENT.get()
    }

    /// Check if the global client has been initialized.
    pub fn is_initialized() -> bool {
        GLOBAL_CLIENT.get().is_some()
    }
}

/// Trait for obtaining a client reference.
///
/// This allows methods to accept either the global client or an explicit client,
/// useful for testing or multi-tenant scenarios.
pub trait ClientProvider {
    fn client(&self) -> &TerminusDBHttpClient;
}

/// Uses the global client singleton.
pub struct GlobalClient;

impl ClientProvider for GlobalClient {
    fn client(&self) -> &TerminusDBHttpClient {
        OrmClient::get()
    }
}

impl ClientProvider for &TerminusDBHttpClient {
    fn client(&self) -> &TerminusDBHttpClient {
        self
    }
}

impl ClientProvider for TerminusDBHttpClient {
    fn client(&self) -> &TerminusDBHttpClient {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_not_initialized() {
        // In a fresh test, global may or may not be set depending on test order
        // Just test that try_get doesn't panic
        let _ = OrmClient::try_get();
    }
}

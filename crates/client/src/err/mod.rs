mod adapter;
mod tdb;

pub use {adapter::*, tdb::*};

/// Error returned when TerminusDB server is still starting up.
///
/// This occurs when the server returns the "Still Loading" HTML page
/// instead of JSON, indicating it's still synchronizing the backing store.
#[derive(Debug)]
pub struct ServerNotReadyError;

impl std::fmt::Display for ServerNotReadyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TerminusDB server is still loading (synchronizing backing store)"
        )
    }
}

impl std::error::Error for ServerNotReadyError {}

//! TerminusDB Manager - Web-based management interface for TerminusDB instances
//!
//! This crate provides a Rocket web application with an Elm frontend for managing
//! multiple TerminusDB instances across environments.

pub mod api;
pub mod manager;
pub mod models;
pub mod poller;
pub mod state;

pub use manager::TerminusDBManager;
pub use models::{NodeConfig, NodeStatus, RemoteInfo};
pub use state::AppState;

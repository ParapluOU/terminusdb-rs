//! # terminusdb
//!
//! Unified re-exports for the TerminusDB Rust client.
//!
//! This crate provides a single import point for the most commonly used types
//! from the TerminusDB client ecosystem. Instead of importing from multiple
//! crates, you can simply use:
//!
//! ```rust,ignore
//! use terminusdb::{TerminusDBHttpClient, BranchSpec, Schema, Instance};
//! ```
//!
//! ## Re-exported Types
//!
//! ### Client Types (from `terminusdb-client`)
//! - [`TerminusDBHttpClient`] - Main HTTP client for API communication
//! - [`BranchSpec`] - Database/branch/commit specification
//! - [`BranchClient`] - Branch-specific operations
//! - [`CommitId`] - Strongly typed commit identifier
//! - [`TerminusDBResult`] - Result type alias for operations
//! - [`TerminusAPIStatus`] - API response status enum
//!
//! ### Schema Types (from `terminusdb-schema`)
//! - [`Schema`] - Schema definition type
//! - [`Instance`] - Instance data type
//! - [`EntityIDFor`] - Strongly typed entity ID
//! - [`TdbLazy`] - Lazy loading support
//!
//! ### Path Types (from `terminusdb-types`)
//! - [`DatabasePath`] - Organization/database path
//! - [`ResourcePath`] - Full resource path
//! - [`DatabaseName`] - Validated database name
//!
//! ### Server Types (from `terminusdb-bin`)
//! - [`TerminusDBServer`] - Embedded server handle for testing

// =============================================================================
// Client Types (terminusdb-client)
// =============================================================================

/// Main HTTP client for TerminusDB API communication
pub use terminusdb_client::TerminusDBHttpClient;

/// Database/branch specification for targeting operations
pub use terminusdb_client::BranchSpec;

/// Branch-specific client operations
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::BranchClient;

/// Strongly typed commit identifier
pub use terminusdb_client::CommitId;

/// Result type alias for TerminusDB operations
pub use terminusdb_client::TerminusDBResult;

/// API response status enum
pub use terminusdb_client::TerminusAPIStatus;

/// Operations enum (Insert, Replace, Delete, Get)
pub use terminusdb_client::TerminusDBOperation;

/// Structured result for instance insertion
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::InsertInstanceResult;

/// Response wrapper with commit ID header
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::ResponseWithHeaders;

/// Delete operation options
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::DeleteOpts;

/// Get operation options
pub use terminusdb_client::GetOpts;

/// Type-safe change listener for SSE events
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::ChangeListener;

/// Changeset event types
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::{ChangesetEvent, ChangesetCommitInfo, DocumentChange, MetadataInfo};

/// Insert result enum (Inserted or AlreadyExists)
#[cfg(not(target_arch = "wasm32"))]
pub use terminusdb_client::TDBInsertInstanceResult;

/// Commit log types
pub use terminusdb_client::{CommitLogEntry, CommitLogIterator, LogEntry, LogOpts};

/// Deserialization traits
pub use terminusdb_client::{DefaultTDBDeserializer, TDBInstanceDeserializer};

/// Instance conversion trait
pub use terminusdb_schema::IntoBoxedTDBInstances;

/// Adapter error type
pub use terminusdb_client::TerminusDBAdapterError;

// =============================================================================
// Schema Types (terminusdb-schema)
// =============================================================================

/// Schema definition type
pub use terminusdb_schema::Schema;

/// Instance data type
pub use terminusdb_schema::Instance;

/// Instance property value wrapper
pub use terminusdb_schema::InstanceProperty;

/// Strongly typed entity ID
pub use terminusdb_schema::EntityIDFor;

/// Model marker trait - implement this for your domain types
pub use terminusdb_schema::TerminusDBModel;

/// Schema generation trait
pub use terminusdb_schema::ToTDBSchema;

/// Instance serialization trait
pub use terminusdb_schema::ToTDBInstance;

/// Instance deserialization trait
pub use terminusdb_schema::FromTDBInstance;

/// Lazy loading support for entity references
pub use terminusdb_schema::TdbLazy;

/// IRI parsing and manipulation
pub use terminusdb_schema::TdbIRI;

/// Enum serialization trait
pub use terminusdb_schema::TDBEnum;

/// JSON conversion traits
pub use terminusdb_schema::{ToJson, InstanceFromJson};

/// Primitive type support
pub use terminusdb_schema::{Primitive, PrimitiveValue};

/// Schema class mapping traits
pub use terminusdb_schema::{ToSchemaClass, ToMaybeTDBSchema};

/// Instance property conversion traits
pub use terminusdb_schema::{ToInstanceProperty, FromInstanceProperty};

/// Multiple schema generation
pub use terminusdb_schema::ToTDBSchemas;

/// Convenience macro for generating schema vectors
pub use terminusdb_schema::schemas;

// =============================================================================
// Path Types (terminusdb-types)
// =============================================================================

/// Organization/database path (e.g., "admin/mydb")
pub use terminusdb_types::DatabasePath;

/// Full resource path (e.g., "admin/mydb/local/branch/main")
pub use terminusdb_types::ResourcePath;

/// Validated database name
pub use terminusdb_types::DatabaseName;

/// Location type (Local or Remote)
pub use terminusdb_types::Location;

/// Resource type (Branch, Commit, Meta, etc.)
pub use terminusdb_types::ResourceType;

/// Path parsing errors
pub use terminusdb_types::PathError;

// =============================================================================
// Server Types (terminusdb-bin)
// =============================================================================

/// Embedded TerminusDB server handle for testing
pub use terminusdb_bin::TerminusDBServer;

/// Server startup options
pub use terminusdb_bin::ServerOptions;

/// Start a server with options
pub use terminusdb_bin::start_server;

/// Run code with a temporary server
pub use terminusdb_bin::with_server;

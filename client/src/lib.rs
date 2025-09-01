#![feature(map_first_last)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![feature(try_blocks)]
#![feature(trait_alias)]
#![feature(let_chains)]
#![allow(warnings)]

use enum_variant_macros::FromVariants;

use terminusdb_schema::*;
use terminusdb_schema_derive::*;
use terminusdb_woql2::prelude::*;

pub use {deserialize::*, document::*, err::*, info::*, r#trait::*, result::*, spec::*};

#[cfg(not(target_arch = "wasm32"))]
pub use http::*;


pub mod deserialize;
mod document;
mod endpoint;
pub mod err;
#[cfg(not(target_arch = "wasm32"))]
pub mod http;
pub mod info;
#[cfg(not(target_arch = "wasm32"))]
mod log;
mod query;
pub mod result;
mod spec;
mod r#trait;
#[cfg(not(target_arch = "wasm32"))]
pub mod debug;
pub use query::*;

use serde::{Deserialize, Serialize};
use std::convert::{From, Into};

#[macro_use]
extern crate custom_derive;
#[macro_use]
extern crate enum_derive;
#[macro_use]
extern crate exec_time;

// todo: move to config crate
pub const DEFAULT_SCHEMA_STRING: &str = "http://parture.org/schema/woql#";
// pub const DEFAULT_BASE_STRING: &str = "parture://";

// todo: define in store/impl/config crate?
const DEFAULT_DB_NAME: &str = "scores";

pub enum TerminusDBOperation {
    Insert,
    Replace,
    Delete,
    Get,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TerminusAPIStatus {
    #[serde(rename(deserialize = "api:success"))]
    Success,
    #[serde(rename(deserialize = "api:failure"))]
    Failure,
    #[serde(rename(deserialize = "api:not_found"))]
    NotFound,
    #[serde(rename(deserialize = "api:server_error"))]
    ServerError,
}

pub type TerminusDBResult<T> = Result<T, TerminusDBAdapterError>;

/// A strongly typed commit identifier.
///
/// This newtype wrapper ensures type safety when working with commit IDs
/// throughout the TerminusDB client API.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitId(String);

impl CommitId {
    /// Create a new CommitId from a string
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the commit ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the CommitId and return the inner String
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for CommitId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CommitId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for CommitId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for CommitId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default)]
pub struct CommitMeta {
    author: Option<String>,
    message: Option<String>,
}

#[test]
fn it_compiles() {
    //
}

pub use self::document::GetOpts;
pub use self::log::{CommitLogEntry, CommitLogIterator, EntityIterator, LogEntry, LogOpts};

// Re-export streams trait for convenience
pub use futures_util::Stream;

#![feature(map_first_last)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![feature(try_blocks)]
#![allow(warnings)]

use enum_variant_macros::FromVariants;

use terminusdb_schema::*;
use terminusdb_schema_derive::*;
use terminusdb_woql2::prelude::*;

pub use {deserialize::*, document::*, err::*, info::*, r#trait::*, result::*, spec::*};

#[cfg(not(target_arch = "wasm32"))]
pub use http::*;

#[cfg(not(target_arch = "wasm32"))]
pub use log::*;

pub mod deserialize;
mod document;
mod endpoint;
pub mod err;
#[cfg(not(target_arch = "wasm32"))]
mod http;
pub mod info;
#[cfg(not(target_arch = "wasm32"))]
mod log;
pub mod result;
mod spec;
mod r#trait;
mod query;
pub use query::*;

use serde::{Deserialize, Serialize};

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

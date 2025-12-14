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

// Import for primitive value conversion  
use terminusdb_schema::{Primitive, PrimitiveValue, ToSchemaClass, ToMaybeTDBSchema, FromInstanceProperty, InstanceProperty, ToInstanceProperty, Schema, STRING};
use terminusdb_schema::json::InstancePropertyFromJson;

pub use {deserialize::*, document::*, err::*, info::*, r#trait::*, result::*, spec::*, versioned_id::*};

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
pub mod versioned_id;
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

// Implement ToSchemaClass to mark CommitId as a string primitive
impl ToSchemaClass for CommitId {
    fn to_class() -> String {
        STRING.to_string()
    }
}

// Mark CommitId as a primitive type
impl Primitive for CommitId {}

// Implement ToMaybeTDBSchema (default impl for primitives)
impl ToMaybeTDBSchema for CommitId {}

// Implement conversion to PrimitiveValue
impl From<CommitId> for PrimitiveValue {
    fn from(commit_id: CommitId) -> Self {
        PrimitiveValue::String(commit_id.0)
    }
}

// Implement ToInstanceProperty with generic parent type
impl<Parent> ToInstanceProperty<Parent> for CommitId {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(self.into())
    }
}

// Implement FromInstanceProperty to enable deserialization
impl FromInstanceProperty for CommitId {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => Ok(CommitId::new(s.clone())),
            _ => anyhow::bail!("Expected string for CommitId, got {:?}", prop),
        }
    }
}

// Implement InstancePropertyFromJson for JSON deserialization
impl<Parent> InstancePropertyFromJson<Parent> for CommitId {
    fn property_from_json(json: serde_json::Value) -> anyhow::Result<InstanceProperty> {
        match json {
            serde_json::Value::String(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(s))),
            _ => anyhow::bail!("Expected string for CommitId, got {:?}", json),
        }
    }
}

#[derive(Default)]
pub struct CommitMeta {
    author: Option<String>,
    message: Option<String>,
}

#[test]
fn test_commit_id_conversions() {
    // Test creating a CommitId
    let commit_id = CommitId::new("abc123");
    assert_eq!(commit_id.as_str(), "abc123");
    
    // Test conversion to PrimitiveValue
    let primitive_value: PrimitiveValue = commit_id.clone().into();
    assert!(matches!(primitive_value, PrimitiveValue::String(s) if s == "abc123"));
    
    // Test ToInstanceProperty
    let instance_prop = <CommitId as ToInstanceProperty<()>>::to_property(commit_id.clone(), "commit", &Schema::empty_class("Test"));
    assert!(matches!(instance_prop, InstanceProperty::Primitive(PrimitiveValue::String(s)) if s == "abc123"));
    
    // Test FromInstanceProperty
    let prop = InstanceProperty::Primitive(PrimitiveValue::String("def456".to_string()));
    let restored = CommitId::from_property(&prop).unwrap();
    assert_eq!(restored.as_str(), "def456");
    
    // Test InstancePropertyFromJson
    let json_val = serde_json::Value::String("xyz789".to_string());
    let prop_from_json = <CommitId as InstancePropertyFromJson<()>>::property_from_json(json_val).unwrap();
    assert!(matches!(prop_from_json, InstanceProperty::Primitive(PrimitiveValue::String(s)) if s == "xyz789"));
}

pub use self::document::GetOpts;
pub use self::log::{CommitLogEntry, CommitLogIterator, EntityIterator, LogEntry, LogOpts};

// Re-export streams trait for convenience
pub use futures_util::Stream;

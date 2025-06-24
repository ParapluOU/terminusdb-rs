#![feature(map_first_last)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![feature(associated_type_bounds)]
#![feature(let_chains)]
#![feature(negative_impls, with_negative_coherence)]
#![feature(auto_traits)]
#![allow(warnings)]

// pub use error::*;
// pub use normalization::*;
pub use context::*;
pub use document::*;
pub use fvec::*;
pub use instance::*;
pub use primitive::*;
pub use schema::*;
pub use xsdtype::*;
pub mod json;
pub use id::*;
pub use json::{InstanceFromJson, ToJson};
pub use lazy::*;
pub use marker::*;
pub use model::*;
pub use pred::*;
pub use r#impl::map::HashMapStringEntry;
pub use ty::*;

// Re-export the helper function for deserializing complex types
pub use instance::prop::{from_tdb_instance_property, MaybeFromTDBInstance};

mod r#impl;

mod context;
mod document;
mod fvec;
mod id;
mod instance;
mod lazy;
mod marker;
mod model;
mod pred;
mod primitive;
mod schema;
#[cfg(test)]
mod test;
mod ty;
pub mod util;
mod xsdtype;

// todo: move to config crate
pub const DEFAULT_SCHEMA_STRING: &str = "http://parture.org/schema/woql#";
pub const DEFAULT_BASE_STRING: &str = "parture://";

pub type Field = String;

pub type URI = String;

pub type ID = String;

pub type Unit = ();

#[test]
fn test_compile() {}

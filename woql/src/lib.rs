#![feature(map_first_last)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![allow(warnings)]

// #[macro_use]
// use serde_json;

mod query;
// mod result;
// mod error;
pub mod woql;

// pub use error::*;
pub use query::*;
// pub use result::*;
pub use woql::*;

use enum_variant_macros::FromVariants;
// use parture_reflection2_simple::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::*;
use serde::{Deserialize, Serialize};

#[macro_use]
extern crate custom_derive;
#[macro_use]
extern crate enum_derive;
#[macro_use]
extern crate exec_time;
// extern crate serde;
// #[macro_use]
// extern crate serde_derive;

#[derive(Serialize, Deserialize, Debug)]
pub enum TerminusAPIStatus {
    #[serde(rename(deserialize = "api:success"))]
    Success,
    #[serde(rename(deserialize = "api:failure"))]
    Failure,
}

#[test]
fn it_compiles() {}

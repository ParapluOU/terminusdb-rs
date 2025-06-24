use crate::*;
use terminusdb_schema::SchemaTypeEnum;
use serde::Serialize;

#[derive(Clone, /*TerminusDBSchema, */ FromVariants, Serialize, Debug, Hash)]
pub enum Indicator {
    name(String),
    index(usize),
}

use serde::{Deserialize, Serialize};
use typestate::*;

/// The @type of the object. At the schema level, this is one of: Enum, Class, TaggedUnion and Unit.
#[derive(Eq, PartialEq, Debug)]
pub enum ClassType {
    /// value-tyoe enum without tags. Like Color {Red, Blue, Green}
    Enum,
    /// struct
    Class,
    /// tagged enum like Rust: Value {Tag1(Value), Tag2(Value), ...}
    TaggedUnion,
    /// empty thing, or nothing, or null. In JSON codified as []
    Unit,
}

make_type_param! {
    SchemaType(SchemaTypeI) => SchemaTypeClass, SchemaTypeOneOfClass, SchemaTypeEnum, SchemaTypeTaggedUnion
}

pub trait SchemaTypeI: Default + Into<SchemaType> {}

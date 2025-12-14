use crate::{EntityIDFor, FromTDBInstance, InstanceFromJson, ToTDBInstance, ToSchemaClass, ToTDBSchema};

pub trait TerminusDBModel:
    ToTDBInstance + FromTDBInstance + InstanceFromJson + ToSchemaClass + Clone + std::fmt::Debug
{
    fn instance_id(&self) -> Option<EntityIDFor<Self>> {
        match self.to_instance(None).gen_id() {
            None => None,
            Some(id) => EntityIDFor::new_unchecked(&id).unwrap().into(),
        }
    }
}

impl<T> TerminusDBModel for T where
    T: ToTDBInstance + FromTDBInstance + InstanceFromJson + ToSchemaClass + Clone + std::fmt::Debug
{
}

/// Marker trait for regular Class types (structs and simple enums).
/// Auto-implemented by the derive macro for all types EXCEPT TaggedUnions.
/// This is the complement of TaggedUnion - a type is either a Class or a TaggedUnion, not both.
pub trait Class: ToSchemaClass {}

/// Marker trait for TaggedUnion types.
/// Auto-implemented by the derive macro for enums with tagged variants.
pub trait TaggedUnion: ToTDBSchema {}

/// Marker trait for types that are variants of a TaggedUnion.
/// Auto-implemented by the derive macro for variant structs.
/// A type can implement this for multiple unions if it's used as a variant in multiple TaggedUnions.
pub trait TaggedUnionVariant<Union: TaggedUnion>: ToTDBSchema {}

// Enforce mutual exclusivity between Class and TaggedUnion
impl<T: TaggedUnion> !Class for T {}
impl<T: Class> !TaggedUnion for T {}

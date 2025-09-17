#![feature(map_first_last)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![feature(associated_type_bounds)]
#![feature(let_chains)]
#![feature(negative_impls, with_negative_coherence)]
#![feature(auto_traits)]
#![feature(trait_alias)]
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
pub use iri::TdbIRI;
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
mod iri;
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

/// Convenience macro for generating a vector of schemas from multiple types.
///
/// This macro takes a list of types that implement `ToTDBSchema` and generates
/// a vector containing their schema definitions. This is useful for bulk schema
/// insertion operations.
///
/// # Examples
/// ```rust,ignore
/// use terminusdb_schema::{schemas, ToTDBSchema};
///
/// #[derive(TerminusDBModel)]
/// struct Person { name: String, age: i32 }
///
/// #[derive(TerminusDBModel)]
/// struct Company { name: String, employees: Vec<Person> }
///
/// // Generate schemas for multiple types
/// let schema_vec = schemas!(Person, Company);
///
/// // Use with client operations
/// client.insert_documents(schema_vec, args).await?;
/// ```
#[macro_export]
macro_rules! schemas {
    ($($schema_type:ty),+ $(,)?) => {
        {
            use $crate::ToTDBSchema;
            vec![
                $(
                    <$schema_type as ToTDBSchema>::to_schema(),
                )+
            ]
        }
    };
    () => {
        vec![]
    };
}

/// Trait for converting tuples of types into vectors of schemas.
///
/// This trait is implemented for tuples up to 20 elements, where each element
/// must implement `ToTDBSchema`. It provides a type-safe way to generate
/// multiple schemas at compile time.
pub trait ToTDBSchemas {
    /// Convert the tuple of types into a vector of their schemas.
    fn to_schemas() -> Vec<crate::Schema>;
}

// Macro to generate ToTDBSchemas implementations for tuples
macro_rules! impl_to_tdb_schemas_for_tuple {
    // Base case for single element tuple
    ($T:ident) => {
        impl<$T> ToTDBSchemas for ($T,)
        where
            $T: ToTDBSchema,
        {
            fn to_schemas() -> Vec<crate::Schema> {
                $T::to_schema_tree()
            }
        }
    };

    // Recursive case for multiple elements
    ($($T:ident),+) => {
        impl<$($T),+> ToTDBSchemas for ($($T,)+)
        where
            $($T: ToTDBSchema,)+
        {
            fn to_schemas() -> Vec<crate::Schema> {
                let mut schemas = Vec::new();
                $(schemas.extend($T::to_schema_tree());)+
                schemas
            }
        }
    };
}

// Generate implementations for tuples up to size 20
impl_to_tdb_schemas_for_tuple!(T1);
impl_to_tdb_schemas_for_tuple!(T1, T2);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_to_tdb_schemas_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_to_tdb_schemas_for_tuple!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
);
impl_to_tdb_schemas_for_tuple!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17
);
impl_to_tdb_schemas_for_tuple!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18
);
impl_to_tdb_schemas_for_tuple!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19
);
impl_to_tdb_schemas_for_tuple!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20
);

#[test]
fn test_compile() {}

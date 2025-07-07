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
/// This trait is implemented for tuples up to 16 elements, where each element
/// must implement `ToTDBSchema`. It provides a type-safe way to generate
/// multiple schemas at compile time.
pub trait ToTDBSchemas {
    /// Convert the tuple of types into a vector of their schemas.
    fn to_schemas() -> Vec<crate::Schema>;
}

// Implement ToTDBSchemas for tuples of various sizes
impl<T1> ToTDBSchemas for (T1,)
where
    T1: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        T1::to_schema_tree()
    }
}

impl<T1, T2> ToTDBSchemas for (T1, T2)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas
    }
}

impl<T1, T2, T3> ToTDBSchemas for (T1, T2, T3)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
    T3: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas.extend(T3::to_schema_tree());
        schemas
    }
}

impl<T1, T2, T3, T4> ToTDBSchemas for (T1, T2, T3, T4)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
    T3: ToTDBSchema,
    T4: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas.extend(T3::to_schema_tree());
        schemas.extend(T4::to_schema_tree());
        schemas
    }
}

impl<T1, T2, T3, T4, T5> ToTDBSchemas for (T1, T2, T3, T4, T5)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
    T3: ToTDBSchema,
    T4: ToTDBSchema,
    T5: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas.extend(T3::to_schema_tree());
        schemas.extend(T4::to_schema_tree());
        schemas.extend(T5::to_schema_tree());
        schemas
    }
}

impl<T1, T2, T3, T4, T5, T6> ToTDBSchemas for (T1, T2, T3, T4, T5, T6)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
    T3: ToTDBSchema,
    T4: ToTDBSchema,
    T5: ToTDBSchema,
    T6: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas.extend(T3::to_schema_tree());
        schemas.extend(T4::to_schema_tree());
        schemas.extend(T5::to_schema_tree());
        schemas.extend(T6::to_schema_tree());
        schemas
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8> ToTDBSchemas for (T1, T2, T3, T4, T5, T6, T7, T8)
where
    T1: ToTDBSchema,
    T2: ToTDBSchema,
    T3: ToTDBSchema,
    T4: ToTDBSchema,
    T5: ToTDBSchema,
    T6: ToTDBSchema,
    T7: ToTDBSchema,
    T8: ToTDBSchema,
{
    fn to_schemas() -> Vec<crate::Schema> {
        let mut schemas = Vec::new();
        schemas.extend(T1::to_schema_tree());
        schemas.extend(T2::to_schema_tree());
        schemas.extend(T3::to_schema_tree());
        schemas.extend(T4::to_schema_tree());
        schemas.extend(T5::to_schema_tree());
        schemas.extend(T6::to_schema_tree());
        schemas.extend(T7::to_schema_tree());
        schemas.extend(T8::to_schema_tree());
        schemas
    }
}

#[test]
fn test_compile() {}

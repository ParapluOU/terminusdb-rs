pub mod map;

// Special types implementations
pub mod btreemap;
pub mod datetime;
pub mod hashmap;
pub mod uuid;

mod r#box;
mod generic;
mod int;
mod opt;
mod phantom;
mod set;
mod str;
mod value;
mod vec;

use std::collections::HashSet;

use crate::*;
pub use {
    btreemap::*, datetime::*, generic::*, hashmap::*, int::*, map::*, opt::*, phantom::*, set::*, str::*,
    uuid::*, value::*, vec::*,
};

macro_rules! impl_ref_tdb_schema {
    ($ref:path) => {
        impl <T: ToTDBSchema> crate::ToTDBSchema for $ref {
            fn to_schema() -> Schema {
                T::to_schema()
            }

            fn to_schema_tree() -> Vec<Schema> {
                T::to_schema_tree()
            }

            fn to_schema_tree_mut(collection: &mut HashSet<crate::Schema>) {
                let schema = <$ref as ToTDBSchema>::to_schema();
                let class_name = schema.class_name().clone();

                // Check if we already have a schema with this class name
                if !collection.iter().any(|s: &Schema| s.class_name() == &class_name) {
                    collection.insert(schema);

                    // Process the inner type
                    T::to_schema_tree_mut(collection);
                }
            }
        }
    };

    (container => $ref:path) => {
        impl <T: ToMaybeTDBSchema> ToMaybeTDBSchema for $ref {
            fn to_schema() -> Option<crate::Schema> {
                T::to_schema()
            }

            fn to_schema_tree() -> Vec<Schema> {
                T::to_schema_tree()
            }
        }
    };

    ([ $($ref:path),* ]) => {
        $(
            impl_ref_tdb_schema!($ref);
        )*
    };

    (container => [ $($ref:path),* ]) => {
        $(
            impl_ref_tdb_schema!(container => $ref);
        )*
    };
}

impl_ref_tdb_schema!([Box<T>]);

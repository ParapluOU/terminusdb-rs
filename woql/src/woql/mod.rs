// todo: Column

// todo: Source

// todo: FormatType

// todo: QueryResource

// todo: get

pub mod control;
mod convert;
mod count;
mod data;
mod data_add;
mod data_added;
mod dicttpl;
mod doc_del;
mod doc_ins;
mod doc_read;
mod doc_upd;
mod dot;
mod fieldvalpair;
mod indicator;
mod key;
mod link;
mod link_add;
mod link_added;
mod link_delete;
mod link_deleted;
mod list;
mod logic;
mod math;
mod node;
mod opt;
mod path;
mod pred;
mod query;
mod simpletype;
mod size;
mod str;
mod test;
mod triple;
mod triple_add;
mod triple_added;
mod triple_count;
mod triple_delete;
mod triple_deleted;
mod r#typeof;
mod val;
mod val_data;
mod val_node;
mod var;

use crate::*;
use enum_variant_macros::FromVariants;
use itertools::Itertools;
use std::collections::BTreeSet;

pub use self::str::*;
pub use control::*;
pub use convert::*;
pub use count::*;
pub use data::*;
pub use data_add::*;
pub use data_added::*;
pub use dicttpl::*;
pub use doc_del::*;
pub use doc_ins::*;
pub use doc_read::*;
pub use doc_upd::*;
pub use dot::*;
pub use fieldvalpair::*;
pub use indicator::*;
pub use key::*;
pub use link::*;
pub use link_add::*;
pub use link_added::*;
pub use link_delete::*;
pub use link_deleted::*;
pub use list::*;
pub use logic::*;
pub use math::*;
pub use node::*;
pub use opt::*;
pub use path::*;
pub use pred::*;
pub use query::*;
pub use r#typeof::*;
pub use simpletype::*;
pub use size::*;
pub use triple::*;
pub use triple_add::*;
pub use triple_added::*;
pub use triple_count::*;
pub use triple_delete::*;
pub use triple_deleted::*;
pub use val::*;
pub use val_data::*;
pub use val_node::*;
pub use var::*;

//
pub type TargetGraphType = Option<GraphType>;

impl ToRESTQuery for Option<GraphType> {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.unwrap_or_default().to_string().into()
    }
}

impl<T: ToRESTQuery> ToRESTQuery for Vec<T> {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.iter()
            .map(ToRESTQuery::to_rest_query_json)
            .collect::<Vec<_>>()
            .into()
    }
}

impl<T: ToRESTQuery> ToRESTQuery for BTreeSet<T> {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.iter()
            .map(ToRESTQuery::to_rest_query_json)
            .collect::<Vec<_>>()
            .into()
    }
}

impl<T: ToRESTQuery> ToRESTQuery for Option<T> {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.as_ref()
            .map(ToRESTQuery::to_rest_query_json)
            .unwrap_or(serde_json::Value::Null)
    }
}

// export!(
//     node,
//     pred,
//     var,
//     simpletype,
//     query,
//     dicttpl,
//     fieldvalpair,
//     val, val_node, val_data,
//     triple, triple_add, triple_delete, triple_deleted, triple_count,
//     link, link_add, link_added,
//     data, data_add, data_added,
//     doc_read, doc_ins, doc_del,
//     indicator,
//     math,
//     opt,
//     key,
//     str,
//     logic,
//     count,
//     convert,
//     path,
//     dot,
//     size,
//     r#typeof,
//     list,
//     control
// );

macro_rules! export {
    ($($name:ident),*) => {
        // mod $name;
        pub use $name::*;
    };
}

#[macro_export]
macro_rules! ast_struct {
    // enum
    (
        $name:ident {
            $(
                $(#[doc = $doc:literal])?
                $field:ident($type:ty)$(,)?
            )*
        }
    ) => {
        #[derive(enum_variant_macros::FromVariants, Clone, Debug)]
        pub enum $name {
            $(
                $(#[doc = $doc])?
                $field($type),
            )*
        }

        // derive to_rest_query impl
        impl_to_rest_query!($name => {
            $($field($type)),*
        });
    };

    // delegated transparent enum, like for abstracts
    (
        @transparent
        $name:ident {
            $(
                $(#[doc = $doc:literal])?
                $field:ident($type:ty)$(,)?
            )*
        }
    ) => {
        #[derive(enum_variant_macros::FromVariants, Clone, Debug)]
        pub enum $name {
            $(
                $(#[doc = $doc])?
                $field(Box<$type>),
            )*
        }

        // From impls for Box variants
        $(
            ast_struct!(frombox: $name => $type);
        )*

        impl ToRESTQuery for $name {
            fn to_rest_query_json(&self) -> serde_json::Value {
                match self {
                    $(
                        $name::$field(value) => {
                            value.to_rest_query_json()
                        }
                    )*
                }
            }
        }
    };

    // struct with ident type
    (
        $name:ident $(as $constructor:ident)? {
            $(
                $(#[doc = $doc:expr])?
                $(pub)? $field:ident: $type:tt
            ),*
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            $(
                $(#[doc = $doc])?
                pub $field: $type
            ),*
        }

        // derive to_rest_query impl
        // todo: add variant to match enum variant
        impl_to_rest_query!($name => {
            $($field: $type),*
        });

        // constructor function
        constructor_ast!($name $(as $constructor)? {
            $($field: $type),*
        });
    };

    (frombox: $name:ident => $type:ty) => {
        impl std::convert::From<$type> for $name {
            fn from(t: $type) -> Self {
                Box::new(t).into() // derived FromVariants impl
            }
        }
    };

    (frombox: $name:ident => $type:ty) => {
        // no from wrapper
    };

    (doc: $doc:literal) => {
        $doc
    };

    (doc: ) => {
        "todo: docs"
    }
}

#[macro_export]
macro_rules! constructor_ast {
    // variant with ident type
    ($T:ident as $fnname:ident {
        $($field:ident: $field_ty:ident),*
    }) => {
        pub fn $fnname(
            $( $field: constructor_ast!{signature_field_type: $field_ty} ),*
        ) -> $T {
            $T {
                $( $field: constructor_ast!{struct_body_field: $field + $field_ty} ),*
            }
        }
    };

    // variant with ty type
    ($T:ident as $fnname:ident {
        $($field:ident: $field_ty:ty),*
    }) => {
        pub fn $fnname(
            $( $field: constructor_ast!{signature_field_type: $field_ty} ),*
        ) -> $T {
            $T {
                $( $field: constructor_ast!{struct_body_field: $field + $field_ty} ),*
            }
        }
    };

    // empty dummy for when constructor name is not passed
    ($T:ident {
        $($field:ident: $field_ty:ident),*
    }) => {};

    // empty dummy for when constructor name is not passed
    ($T:ident {
        $($field:ident: $field_ty:ty),*
    }) => {};

    // when a signature field is a Vec
    (signature_field_type: Vec<$field_sub_ty:ty>) => {
        Vec<impl std::convert::Into<$field_sub_ty>>
    };

    // when a signature field is a Vec
    (signature_field_type: Vec<$field_sub_ty:ident>) => {
        Vec<impl std::convert::Into<$field_sub_ty>>
    };

    // for other field types
    (signature_field_type: $field_ty:ty) => {
        impl std::convert::Into<$field_ty>
    };

    // for other field types
    (signature_field_type: $field_ty:ident) => {
        impl std::convert::Into<$field_ty>
    };

    // when a field is a Vec
    (struct_body_field: $field:ident + Vec<$field_sub_ty:ty>) => {
        $field.into_iter().map(std::convert::Into::into).collect()
    };

    // when a field is a Vec
    (struct_body_field: $field:ident + Vec<$field_sub_ty:ident>) => {
        $field.into_iter().map(std::convert::Into::into).collect()
    };

    // for other field types
    (struct_body_field: $field:ident + $field_ty:ty) => {
        $field.into()
    };

    // for other field types
    (struct_body_field: $field:ident + $field_ty:ident) => {
        $field.into()
    };
}

#[macro_export]
macro_rules! impl_newtype_derive {
    ($name:ident => $T:ty) => {
        impl std::ops::Deref for $name {
            type Target = $T;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

#[macro_export]
macro_rules! newtype {
    ({
        name: $name:ident,
        type: $typ:ident,
        schemaclass: $class:ident
    }) => {
        #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name($typ);

        // impl_newtype_derive!($name => $typ);

        impl std::convert::Into<$typ> for $name {
            fn into(self) -> $typ {
                self.0
            }
        }

        impl ToSchemaClass for $name {
            fn to_class() -> &'static str {
                terminusdb_schema::$class
            }
        }

        impl<Parent> ToInstanceProperty<Parent> for $name {
            fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
                InstanceProperty::Primitive(PrimitiveValue::$typ(self.0.clone()))
            }
        }
    };
}

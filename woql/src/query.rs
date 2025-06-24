//
// INTERFACES
//

use enum_variant_macros::*;
use itertools::Itertools;
use terminusdb_schema::{
    Property, Instance, InstanceProperty, PrimitiveValue, Schema,
    ToInstanceProperty, ToJson, ToMaybeTDBSchema, ToSchemaClass, ToSchemaProperty, ToTDBInstance,
    ToTDBSchema, XSDAnySimpleType, BOOL, DATE, DECIMAL, FLOAT, HEX_BINARY, UNSIGNED_INT,
};
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::{Query, TypedVariable, Variable};
use terminusdb_schema_derive::*;

pub enum QueryTargetType {
    CliAst,
    WoqlXml,
}

/// whether a structure can be parsed to a representation that is understood by the CLI
pub trait ToCLIQueryAST {
    // todo: use associated constant?
    fn array_separator(&self) -> &str {
        ","
    }

    fn to_ast(&self) -> String;
}

impl<T: ToCLIQueryAST> ToCLIQueryAST for Vec<T> {
    fn to_ast(&self) -> String {
        self.iter()
            .map(|el| el.to_ast())
            .join(self.array_separator())
    }
}

/// whether a structure can be parsed to a representation that is understood by the REST query endpoint
pub trait ToRESTQuery {
    fn to_rest_query(&self) -> String {
        serde_json::to_string(&self.to_rest_query_json()).unwrap()
    }

    fn to_rest_query_json(&self) -> serde_json::Value;
}

impl ToRESTQuery for usize {
    fn to_rest_query_json(&self) -> serde_json::Value {
        (*self).into()
    }
}

impl ToRESTQuery for String {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.clone().into()
    }
}

/// switchboard trait
pub trait ToSerializedQuery: ToRESTQuery + ToCLIQueryAST {
    fn serialize_as(&self, typ: QueryTargetType) -> String {
        match typ {
            QueryTargetType::CliAst => self.to_ast(),
            QueryTargetType::WoqlXml => self.to_rest_query(),
        }
    }

    fn as_json(&self, typ: QueryTargetType) -> serde_json::Value {
        serde_json::from_str(&self.serialize_as(typ)).unwrap()
    }
}

// default impl
impl<T: ToRESTQuery + ToCLIQueryAST> ToSerializedQuery for T {}

#[macro_export]
macro_rules! impl_to_rest_query {
    // for struct
    ($T:ty => {
        $($field:ident: $type:tt),*
    }) => {
        impl ToRESTQuery for $T {
            fn to_rest_query_json(&self) -> serde_json::Value {
                let mut map = serde_json::Map::new();
                map.insert("@type".to_string(), std::convert::Into::into(stringify!($T)));

                $(
                    map.insert(
                        stringify!($field).replace("r#", "").to_string(),
                        impl_to_rest_query!(self.$field: $type)
                    );
                )*

                serde_json::Value::Object(map)
            }
        }
    };

    // for enum
    ($T:ty => {
        $($field:ident($type:tt)),*
    }) => {
        impl ToRESTQuery for $T {
            fn to_rest_query_json(&self) -> serde_json::Value {
                let mut map = serde_json::Map::new();
                map.insert("@type".to_string(), std::convert::Into::into(stringify!($T)));

                match self {
                    $(
                        Self::$field(value) => {
                            map.insert(
                                stringify!($field).replace("r#", "").to_string(),
                                impl_to_rest_query!(value: $type)
                            );
                        }
                    )*
                }

                serde_json::Value::Object(map)
            }
        }
    };

    // for structs

    ($self:ident.$field:ident: String) => {
        std::convert::Into::into($self.$field.clone())
    };

    ($self:ident.$field:ident: $type:ty) => {
        $self.$field.to_rest_query_json()
    };

    // enum

    ($value:ident: String) => {
        std::convert::Into::into($value.clone())
    };

    ($value:ident: $type:ty) => {
        $value.to_rest_query_json()
    };
}

// use crate::prelude::*;
// use proc_macro2::{Ident, Span};
use std::fmt::Debug;

// marker trait
pub trait TypeStateParam: Debug + Clone + Eq + PartialEq + Send + Sync + Default {
    type Union: TypeStateParamUnion + From<Self>;

    fn name() -> String {
        format!("{:?}", Self::default())
    }

    // fn as_ident() -> Ident {
    //     Ident::new(Self::name().as_str(), Span::call_site())
    // }

    fn as_union() -> Self::Union {
        Self::default().into()
    }
}

pub trait TypeStateParamUnion: Debug + Clone + Eq + PartialEq + Send + Sync {
    fn list() -> Vec<Self>;

    // fn param_ident(&self) -> Ident;
    fn param(&self) -> &str;
}

#[macro_export]
macro_rules! make_type_param {
    ($name:ident($custommarkertrait:ident) => $($paramname:ident),*) => {
        // // marker trait
        // pub trait $typ: Debug + Clone + Eq + PartialEq + Send + Sync + Default {}

        $(
            #[derive(Debug, Eq, PartialEq, Clone/*, schemars::JsonSchema*/)]
            pub struct $paramname;

            impl $paramname {
                pub fn to_string() -> &'static str {
                    stringify!($paramname)
                }

                pub fn union() -> $name {
                    Self::default().into()
                }
            }

            // custom marker trait for caller
            impl $custommarkertrait for $paramname {}

            // generic marker trait
            impl TypeStateParam for $paramname {
                type Union = $name;
            }

            // needed for conversion to string
            impl Default for $paramname {
                fn default() -> Self {
                    Self {}
                }
            }

            impl From<$paramname> for $name {
                fn from(param: $paramname) -> Self {
                    $name::$paramname
                }
            }

            // impl Into<$name> for $paramname {
            //     fn into(self) -> $name {
            //         $name::$paramname
            //     }
            // }
        )*

        #[derive(Debug, Copy, Clone, Eq, PartialEq, /*enum_variant_macros::FromVariants*/)]
        pub enum $name {
            $($paramname),*
        }

        impl TypeStateParamUnion for $name {
            fn list() -> Vec<Self> {
                vec!(
                    $(Self::$paramname),*
                )
            }

            // fn param_ident(&self) -> Ident {
            //     match self {
            //         $(
            //             Self::$paramname => $paramname::as_ident()
            //         ),*
            //     }
            // }

            fn param(&self) -> &str {
                match self {
                    $(
                        Self::$paramname => stringify!($paramname)
                    ),*
                }
            }
        }
    }
}

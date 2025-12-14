pub use darling::FromDeriveInput;
pub use darling::FromField;
pub use proc_macro::TokenStream;
pub use quote::{quote, ToTokens};
pub use syn::spanned::Spanned;
pub use syn::{parse_macro_input, Data, DataEnum, DataStruct, DeriveInput, Fields, FieldsNamed};

pub use crate::args::*;

/// Check if a type is an Option<T>
pub fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        if typepath.path.segments.len() == 1 {
            let segment = &typepath.path.segments[0];
            return segment.ident == "Option";
        }
    }
    false
}

/// Check if a type is ServerIDFor<T>
pub fn is_server_id_for_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        // Check last segment (handles both ServerIDFor and terminusdb_schema::ServerIDFor)
        if let Some(segment) = typepath.path.segments.last() {
            return segment.ident == "ServerIDFor";
        }
    }
    false
}

/// Check if a type is PhantomData<T>
pub fn is_phantom_data_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        // Check last segment (handles both PhantomData and std::marker::PhantomData)
        if let Some(segment) = typepath.path.segments.last() {
            return segment.ident == "PhantomData";
        }
    }
    false
}

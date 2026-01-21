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

/// Check if a type is EntityIDFor<Self> specifically
/// This is used for auto-detection of id_field
pub fn is_entity_id_for_self(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        if let Some(segment) = typepath.path.segments.last() {
            if segment.ident == "EntityIDFor" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(syn::Type::Path(inner_path)) = arg {
                            if let Some(ident) = inner_path.path.get_ident() {
                                return ident == "Self";
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a type is PrimaryKey!() macro invocation
/// This is used for auto-detection of id_field
pub fn is_primary_key_type(ty: &syn::Type) -> bool {
    // Check for PrimaryKey!() macro invocation
    if let syn::Type::Macro(type_macro) = ty {
        if let Some(segment) = type_macro.mac.path.segments.last() {
            return segment.ident == "PrimaryKey";
        }
    }
    false
}

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

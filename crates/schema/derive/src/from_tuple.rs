//! `#[derive(FromTuple)]` — build a struct from a tuple of the same arity by
//! converting each element into its field type via
//! `terminusdb_schema::IntoField`.
//!
//! Generated for `struct S { f0: T0, f1: T1, … }`:
//!
//! ```ignore
//! impl<P0, P1, …> From<(P0, P1, …)> for S
//! where P0: IntoField<T0>, P1: IntoField<T1>, …
//! {
//!     fn from(t: (P0, P1, …)) -> Self {
//!         Self { f0: t.0.into_field(), f1: t.1.into_field(), … }
//!     }
//! }
//! ```
//!
//! `Self` stays valid inside the impl, so field types that mention it (e.g.
//! `EntityIDFor<Self>`) need no rewriting.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, Index};

pub fn derive(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;

    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &input.generics,
            "FromTuple does not support generic structs",
        )
        .to_compile_error();
    }

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "FromTuple only supports structs with named fields",
                )
                .to_compile_error()
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "FromTuple only supports structs")
                .to_compile_error()
        }
    };

    if fields.is_empty() {
        return syn::Error::new_spanned(name, "FromTuple requires at least one field")
            .to_compile_error();
    }

    let params: Vec<Ident> = (0..fields.len())
        .map(|i| Ident::new(&format!("P{i}"), Span::call_site()))
        .collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();
    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let indices: Vec<Index> = (0..fields.len()).map(Index::from).collect();

    // Trailing comma makes the single-field case a 1-tuple `(P0,)`.
    quote! {
        // (1) Build one model from a tuple, converting each element into its field
        //     type. `Self` is valid here, so `EntityIDFor<Self>` needs no rewriting.
        impl<#(#params),*> ::core::convert::From<(#(#params,)*)> for #name
        where
            #(#params: ::terminusdb_schema::IntoField<#field_types>,)*
        {
            fn from(__tuple: (#(#params,)*)) -> Self {
                Self {
                    #(#field_names: ::terminusdb_schema::IntoField::into_field(__tuple.#indices),)*
                }
            }
        }

        // (2) A whole model value converts into a link (`Ref<Self>` /
        //     `TdbLazy<Self>`), so a reference field can be given a NESTED model,
        //     not just an id. A concrete (non-blanket) impl, so it cannot collide
        //     with the `&str -> link` blanket in terminusdb-schema.
        impl ::terminusdb_schema::IntoField<::terminusdb_schema::TdbLazy<#name>> for #name {
            fn into_field(self) -> ::terminusdb_schema::TdbLazy<#name> {
                ::terminusdb_schema::TdbLazy::from(self)
            }
        }

        // (3) Build a `Vec<Self>` from an array / iterator of tuples (or models):
        //     each element is converted via its `Into<Self>` (the impl from (1)).
        impl #name {
            pub fn from_tuples<__I>(__items: __I) -> ::std::vec::Vec<Self>
            where
                __I: ::core::iter::IntoIterator,
                __I::Item: ::core::convert::Into<Self>,
            {
                __items
                    .into_iter()
                    .map(::core::convert::Into::into)
                    .collect()
            }
        }
    }
}

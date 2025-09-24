//! Relation trait implementation generation for TerminusDBModel derive macro

use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::FieldsNamed;
use heck::AsUpperCamelCase;

/// Detect the crate context and generate appropriate module paths
fn get_woql_path() -> TokenStream {
    // Check if we're inside woql2 crate by looking at CARGO_CRATE_NAME
    if let Ok(crate_name) = std::env::var("CARGO_CRATE_NAME") {
        if crate_name == "terminusdb_woql2" {
            quote! { crate }
        } else {
            quote! { terminusdb_woql2 }
        }
    } else {
        quote! { terminusdb_woql2 }
    }
}

fn get_relation_path() -> Option<TokenStream> {
    // Check if we're inside woql2 crate - if so, don't generate relation code at all
    if let Ok(crate_name) = std::env::var("CARGO_CRATE_NAME") {
        if crate_name == "terminusdb_woql2" {
            // Return None - woql2 shouldn't generate relation code
            return None;
        }
    }
    Some(quote! { terminusdb_relation })
}


/// Generate RelationTo implementations for ALL struct fields
/// 
/// This function generates RelationTo implementations for every field in the struct,
/// regardless of field type. The TerminusDBModel constraint in the RelationTo trait
/// will cause compile errors at usage time for non-model types, while the blanket
/// implementations for container types (Option, Vec, Box) will handle those cases
/// automatically via Rust's trait resolution.
pub fn generate_relation_impls(
    struct_name: &syn::Ident,
    fields_named: &FieldsNamed,
    impl_generics: &TokenStream,
    ty_generics: &TokenStream,
    where_clause: &Option<syn::WhereClause>,
) -> TokenStream {
    // Check if we should generate relations at all
    let relation_path = match get_relation_path() {
        Some(path) => path,
        None => {
            // Don't generate relation code for woql2 crate to avoid circular dependency
            return quote! {};
        }
    };
    
    let woql_path = get_woql_path();
    let mut impls = vec![];
    let mut marker_types = vec![];
    
    // Generate RelationTo implementations for ALL fields
    for field in &fields_named.named {
        let field_ident = match &field.ident {
            Some(ident) => ident,
            None => continue, // Skip tuple struct fields
        };
        
        let field_name = field_ident.to_string();
        let field_name_upper = AsUpperCamelCase(&field_name);
        let field_type = &field.ty;
        
        // Generate marker type for this field
        let marker_type_name = format_ident!("{}{field_name_upper}Relation", struct_name);
        
        // Define marker type and implement RelationField from external crate
        marker_types.push(quote! {
            /// Marker type for the #field_name relation field
            pub struct #marker_type_name;
            
            impl #relation_path::RelationField for #marker_type_name {
                fn field_name() -> &'static str {
                    #field_name
                }
            }
        });
        
        // Generate RelationTo implementation using external trait
        let explicit_impl = if let Some(clause) = where_clause {
            quote! {
                impl #impl_generics #relation_path::RelationTo<#field_type, #marker_type_name> for #struct_name #ty_generics 
                #clause
                {
                    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> #woql_path::prelude::Query {
                        // Generate WOQL constraints inline - use context-aware paths
                        // Use the helper function that works with string parameters
                        #relation_path::generate_relation_constraints(
                            #field_name,
                            &<#struct_name as terminusdb_schema::ToSchemaClass>::to_class(),
                            stringify!(#field_type), // Convert type to string for now
                            source_var,
                            target_var,
                            false
                        )
                    }
                }
            }
        } else {
            quote! {
                impl #impl_generics #relation_path::RelationTo<#field_type, #marker_type_name> for #struct_name #ty_generics {
                    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> #woql_path::prelude::Query {
                        // Generate WOQL constraints inline - use context-aware paths
                        // Use the helper function that works with string parameters
                        #relation_path::generate_relation_constraints(
                            #field_name,
                            &<#struct_name as terminusdb_schema::ToSchemaClass>::to_class(),
                            stringify!(#field_type), // Convert type to string for now
                            source_var,
                            target_var,
                            false
                        )
                    }
                }
            }
        };
        impls.push(explicit_impl);
    }
    
    quote! {
        #(#marker_types)*
        #(#impls)*
    }
}
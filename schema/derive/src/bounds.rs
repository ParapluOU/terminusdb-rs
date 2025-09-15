#[cfg(feature = "generic-derive")]
use crate::prelude::*;
#[cfg(feature = "generic-derive")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "generic-derive")]
use syn::{
    GenericArgument, GenericParam, Generics, Ident, PathArguments, Type, TypePath, WherePredicate,
};

/// Analyzes field types and collects required trait bounds for generic parameters
#[cfg(feature = "generic-derive")]
pub fn collect_type_param_bounds(
    fields: &syn::FieldsNamed,
    generics: &Generics,
    struct_name: &Ident,
) -> HashMap<Ident, Vec<String>> {
    let mut bounds = HashMap::new();

    // Collect generic parameter names
    let generic_params: HashSet<Ident> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(type_param) => Some(type_param.ident.clone()),
            _ => None,
        })
        .collect();

    // Build the struct name with generic parameters
    let struct_name_with_generics = if generic_params.is_empty() {
        struct_name.to_string()
    } else {
        let generic_names: Vec<String> = generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
                _ => None,
            })
            .collect();
        format!("{}<{}>", struct_name, generic_names.join(", "))
    };

    // Analyze each field - only checking top-level types
    for field in &fields.named {
        if let Some(_field_name) = &field.ident {
            analyze_type_for_bounds(
                &field.ty,
                &generic_params,
                &struct_name_with_generics,
                &mut bounds,
            );
        }
    }

    bounds
}

/// Analyzes a type to determine if it's a generic parameter that needs bounds
#[cfg(feature = "generic-derive")]
fn analyze_type_for_bounds(
    ty: &Type,
    generic_params: &HashSet<Ident>,
    struct_name_with_generics: &str,
    bounds: &mut HashMap<Ident, Vec<String>>,
) {
    // Only check top-level types - no recursion needed
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(ident) = path.get_ident() {
            // Check if this is a generic parameter
            if generic_params.contains(ident) {
                let param_bounds = bounds.entry(ident.clone()).or_insert_with(Vec::new);
                
                // Add the single trait alias bound that combines all requirements
                let bound = format!(
                    "terminusdb_schema::TerminusDBField<{}>",
                    struct_name_with_generics
                );
                
                if !param_bounds.contains(&bound) {
                    param_bounds.push(bound);
                }
            }
        }
        
        // Special case: Check if this is EntityIDFor<T> where T is a generic parameter
        if let Some(last_segment) = path.segments.last() {
            let is_entity_id_for = last_segment.ident == "EntityIDFor" || 
                (path.segments.len() > 1 && last_segment.ident == "EntityIDFor");
                
            if is_entity_id_for {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(Type::Path(TypePath { path: inner_path, .. })) = arg {
                            if let Some(ident) = inner_path.get_ident() {
                                if generic_params.contains(ident) {
                                    let param_bounds = bounds
                                        .entry(ident.clone())
                                        .or_insert_with(Vec::new);
                                    // EntityIDFor<T> requires ToTDBSchema on T for the struct definition
                                    // and ToSchemaClass for the ToSchemaProperty implementation
                                    let bounds = vec![
                                        "terminusdb_schema::ToTDBSchema".to_string(),
                                        "terminusdb_schema::ToSchemaClass".to_string(),
                                    ];
                                    for bound in bounds {
                                        if !param_bounds.contains(&bound) {
                                            param_bounds.push(bound);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Builds where clause predicates from collected bounds
#[cfg(feature = "generic-derive")]
pub fn build_where_predicates(
    type_param_bounds: &HashMap<Ident, Vec<String>>,
) -> Vec<WherePredicate> {
    let mut predicates = Vec::new();

    for (param, bounds_list) in type_param_bounds {
        if !bounds_list.is_empty() {
            // Build the bounds string
            let bounds_str = bounds_list.join(" + ");
            // Parse it as a TokenStream
            let bounds: proc_macro2::TokenStream = bounds_str.parse().unwrap();
            let predicate: WherePredicate = syn::parse_quote! {
                #param: #bounds
            };
            predicates.push(predicate);
        }
    }

    predicates
}

/// Combines existing where clause with new predicates
#[cfg(feature = "generic-derive")]
pub fn combine_where_clauses(
    existing: Option<&syn::WhereClause>,
    new_predicates: Vec<WherePredicate>,
) -> Option<syn::WhereClause> {
    if new_predicates.is_empty() && existing.is_none() {
        return None;
    }

    let mut combined_predicates = Vec::new();

    // Add existing predicates
    if let Some(where_clause) = existing {
        combined_predicates.extend(where_clause.predicates.iter().cloned());
    }

    // Add new predicates
    combined_predicates.extend(new_predicates);

    if combined_predicates.is_empty() {
        None
    } else {
        Some(syn::parse_quote! {
            where #(#combined_predicates),*
        })
    }
}

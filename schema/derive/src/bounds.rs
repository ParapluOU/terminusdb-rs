#[cfg(feature = "generic-derive")]
use crate::prelude::*;
#[cfg(feature = "generic-derive")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "generic-derive")]
use syn::{GenericArgument, GenericParam, Generics, Ident, PathArguments, Type, TypePath, WherePredicate};

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
    
    // Analyze each field
    for field in &fields.named {
        if let Some(field_name) = &field.ident {
            analyze_type_for_bounds(
                &field.ty,
                &generic_params,
                &struct_name_with_generics,
                &mut bounds,
                false, // not inside a container yet
            );
        }
    }
    
    
    bounds
}

/// Recursively analyzes a type to determine required trait bounds
#[cfg(feature = "generic-derive")]
fn analyze_type_for_bounds(
    ty: &Type,
    generic_params: &HashSet<Ident>,
    struct_name_with_generics: &str,
    bounds: &mut HashMap<Ident, Vec<String>>,
    inside_container: bool,
) {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            if let Some(ident) = path.get_ident() {
                // Check if this is a generic parameter
                if generic_params.contains(ident) {
                    let param_bounds = bounds.entry(ident.clone()).or_insert_with(Vec::new);
                    
                    // Add basic trait requirements
                    param_bounds.push("terminusdb_schema::ToTDBSchema".to_string());
                    param_bounds.push("terminusdb_schema::ToMaybeTDBSchema".to_string());
                    param_bounds.push("std::fmt::Debug".to_string());
                    param_bounds.push("Clone".to_string());
                    
                    // If used directly as a field (not inside Option/Vec), add more bounds
                    if !inside_container {
                        param_bounds.push("terminusdb_schema::ToSchemaClass".to_string());
                        param_bounds.push(format!("terminusdb_schema::ToSchemaProperty<{}>", struct_name_with_generics));
                        param_bounds.push(format!("terminusdb_schema::ToInstanceProperty<{}>", struct_name_with_generics));
                        param_bounds.push("terminusdb_schema::FromInstanceProperty".to_string());
                        param_bounds.push("terminusdb_schema::ToTDBInstance".to_string());
                        param_bounds.push("terminusdb_schema::ToTDBInstances".to_string());
                        param_bounds.push("terminusdb_schema::FromTDBInstance".to_string());
                        param_bounds.push("terminusdb_schema::InstanceFromJson".to_string());
                    }
                }
            }
            
            // Check if this is a container type like Option<T>, Vec<T>, or EntityIDFor<T>
            if let Some(last_segment) = path.segments.last() {
                let segment_name = last_segment.ident.to_string();
                let is_container = matches!(
                    segment_name.as_str(),
                    "Option" | "Vec" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet"
                );
                
                // EntityIDFor requires its type parameter to implement ToTDBSchema
                let is_entity_id_for = segment_name == "EntityIDFor";
                
                // Also check if this is a path that ends with EntityIDFor (e.g., terminusdb_schema::EntityIDFor)
                let is_entity_id_for_qualified = path.segments.iter()
                    .last()
                    .map(|s| s.ident.to_string() == "EntityIDFor")
                    .unwrap_or(false);
                
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(inner_ty) = arg {
                            // For EntityIDFor<T>, we need minimal bounds on T
                            if is_entity_id_for || is_entity_id_for_qualified {
                                // Check if the inner type is a generic parameter
                                if let Type::Path(TypePath { path: inner_path, .. }) = inner_ty {
                                    if let Some(ident) = inner_path.get_ident() {
                                        if generic_params.contains(ident) {
                                            let param_bounds = bounds.entry(ident.clone()).or_insert_with(Vec::new);
                                            // EntityIDFor<T> requires several traits for T
                                            let required_traits = vec![
                                                "terminusdb_schema::ToTDBSchema",
                                                "terminusdb_schema::ToSchemaClass",
                                                "terminusdb_schema::ToTDBInstance", 
                                                "terminusdb_schema::FromTDBInstance",
                                                "terminusdb_schema::InstanceFromJson",
                                                "Clone",
                                                "Send",
                                                "Sync",
                                                "std::fmt::Debug",
                                            ];
                                            for trait_name in required_traits {
                                                if !param_bounds.contains(&trait_name.to_string()) {
                                                    param_bounds.push(trait_name.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                // For other containers, analyze recursively
                                analyze_type_for_bounds(
                                    inner_ty,
                                    generic_params,
                                    struct_name_with_generics,
                                    bounds,
                                    is_container,
                                );
                            }
                        }
                    }
                }
            }
        }
        Type::Reference(type_ref) => {
            analyze_type_for_bounds(&type_ref.elem, generic_params, struct_name_with_generics, bounds, inside_container);
        }
        Type::Slice(type_slice) => {
            analyze_type_for_bounds(&type_slice.elem, generic_params, struct_name_with_generics, bounds, true);
        }
        Type::Array(type_array) => {
            analyze_type_for_bounds(&type_array.elem, generic_params, struct_name_with_generics, bounds, true);
        }
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                analyze_type_for_bounds(elem, generic_params, struct_name_with_generics, bounds, inside_container);
            }
        }
        _ => {} // Other type variants we don't need to handle
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
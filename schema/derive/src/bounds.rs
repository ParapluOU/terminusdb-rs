#[cfg(feature = "generic-derive")]
use crate::prelude::*;
#[cfg(feature = "generic-derive")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "generic-derive")]
use syn::{
    GenericArgument, GenericParam, Generics, Ident, PathArguments, Type, TypePath, WherePredicate,
};
#[cfg(feature = "generic-derive")]
use quote::quote;

/// Analyzes field types and collects required trait bounds for generic parameters
#[cfg(feature = "generic-derive")]
pub fn collect_type_param_bounds(
    fields: &syn::FieldsNamed,
    generics: &Generics,
    struct_name: &Ident,
) -> HashMap<Ident, Vec<String>> {
    let mut bounds = HashMap::new();
    
    // Check existing where clause for already-defined bounds
    let existing_model_params = check_existing_model_bounds(generics);
    let existing_field_params = check_existing_field_bounds(generics, struct_name);
    let existing_schema_class_params = check_existing_schema_class_bounds(generics);
    
    // Track which generic params are used in special contexts
    let mut model_params = HashSet::new();  // Params that need TerminusDBModel
    let mut entity_params = HashSet::new(); // Params that need ToTDBSchema + ToSchemaClass

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

    // First pass: identify special usage contexts
    for field in &fields.named {
        if let Some(_) = &field.ident {
            // Skip PhantomData fields entirely
            if crate::prelude::is_phantom_data_type(&field.ty) {
                continue;
            }
            identify_special_contexts(
                &field.ty,
                &generic_params,
                &mut model_params,
                &mut entity_params,
            );
        }
    }

    // Second pass: assign bounds to each generic parameter based on its usage
    for param in &generic_params {
        let param_bounds = bounds.entry(param.clone()).or_insert_with(Vec::new);
        
        // Determine bounds based on special usage contexts
        if model_params.contains(param) {
            // Used in TdbLazy<T> - needs TerminusDBModel + ToSchemaClass
            if !existing_model_params.contains(param) {
                param_bounds.push("terminusdb_schema::TerminusDBModel".to_string());
            }
            // TdbLazy always needs ToSchemaClass too
            if !existing_schema_class_params.contains(param) {
                param_bounds.push("terminusdb_schema::ToSchemaClass".to_string());
            }
        } else if entity_params.contains(param) {
            // Used in EntityIDFor<T> - needs entity-specific bounds
            param_bounds.push("terminusdb_schema::ToTDBSchema".to_string());
            param_bounds.push("terminusdb_schema::ToSchemaClass".to_string());
        } else if !existing_field_params.contains(param) && !existing_model_params.contains(param) {
            // Regular field usage - needs TerminusDBField
            // But we need to check if it's actually used as a field
            let mut used_as_field = false;
            for field in &fields.named {
                if let Some(_) = &field.ident {
                    // Skip PhantomData fields - they don't need bounds on their type parameters
                    if crate::prelude::is_phantom_data_type(&field.ty) {
                        continue;
                    }
                    if is_generic_param_used_as_field(&field.ty, param) {
                        used_as_field = true;
                        break;
                    }
                }
            }
            
            if used_as_field {
                let bound = format!(
                    "terminusdb_schema::TerminusDBField<{}>",
                    struct_name_with_generics
                );
                param_bounds.push(bound);
            }
        }
    }

    bounds
}

/// Checks if a generic parameter is used directly as a field type
#[cfg(feature = "generic-derive")]
fn is_generic_param_used_as_field(ty: &Type, param: &Ident) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        // Skip special container types
        if let Some(last_segment) = path.segments.last() {
            if last_segment.ident == "EntityIDFor" || last_segment.ident == "TdbLazy" {
                return false;
            }
        }
        
        // Check if this is the parameter itself
        if let Some(ident) = path.get_ident() {
            return ident == param;
        }
    }
    false
}

/// Identifies generic parameters used in special contexts (TdbLazy, EntityIDFor)
#[cfg(feature = "generic-derive")]
fn identify_special_contexts(
    ty: &Type,
    generic_params: &HashSet<Ident>,
    model_params: &mut HashSet<Ident>,
    entity_params: &mut HashSet<Ident>,
) {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(last_segment) = path.segments.last() {
            let is_entity_id_for = last_segment.ident == "EntityIDFor";
            let is_tdb_lazy = last_segment.ident == "TdbLazy";
                
            if is_entity_id_for || is_tdb_lazy {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(Type::Path(TypePath { path: inner_path, .. })) = arg {
                            if let Some(ident) = inner_path.get_ident() {
                                if generic_params.contains(ident) {
                                    if is_entity_id_for {
                                        entity_params.insert(ident.clone());
                                    } else if is_tdb_lazy {
                                        model_params.insert(ident.clone());
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

/// Check which generic parameters already have TerminusDBModel bounds
#[cfg(feature = "generic-derive")]
fn check_existing_model_bounds(generics: &Generics) -> HashSet<Ident> {
    let mut params = HashSet::new();
    
    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            if let WherePredicate::Type(type_predicate) = predicate {
                if let Type::Path(TypePath { path, .. }) = &type_predicate.bounded_ty {
                    if let Some(ident) = path.get_ident() {
                        // Check if any bound contains TerminusDBModel
                        for bound in &type_predicate.bounds {
                            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                                let path_str = quote!(#trait_bound).to_string();
                                if path_str.contains("TerminusDBModel") {
                                    params.insert(ident.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    params
}

/// Check which generic parameters already have ToSchemaClass bounds
#[cfg(feature = "generic-derive")]
fn check_existing_schema_class_bounds(generics: &Generics) -> HashSet<Ident> {
    let mut params = HashSet::new();
    
    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            if let WherePredicate::Type(type_predicate) = predicate {
                if let Type::Path(TypePath { path, .. }) = &type_predicate.bounded_ty {
                    if let Some(ident) = path.get_ident() {
                        // Check if any bound contains ToSchemaClass
                        for bound in &type_predicate.bounds {
                            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                                let path_str = quote!(#trait_bound).to_string();
                                if path_str.contains("ToSchemaClass") {
                                    params.insert(ident.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    params
}

/// Check which generic parameters already have TerminusDBField bounds
#[cfg(feature = "generic-derive")]
fn check_existing_field_bounds(generics: &Generics, struct_name: &Ident) -> HashSet<Ident> {
    let mut params = HashSet::new();
    
    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            if let WherePredicate::Type(type_predicate) = predicate {
                if let Type::Path(TypePath { path, .. }) = &type_predicate.bounded_ty {
                    if let Some(ident) = path.get_ident() {
                        // Check if any bound contains TerminusDBField
                        for bound in &type_predicate.bounds {
                            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                                let path_str = quote!(#trait_bound).to_string();
                                if path_str.contains("TerminusDBField") {
                                    params.insert(ident.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    params
}

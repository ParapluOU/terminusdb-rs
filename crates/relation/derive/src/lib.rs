//! Relation trait implementation generation for TerminusDBModel derive macro

use heck::AsUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{FieldsNamed, GenericArgument, PathArguments, Type};

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

/// Information about a TdbLazy<T> field that creates a document link
struct TdbLazyFieldInfo {
    /// The field name as string
    field_name: String,
    /// The marker type name (PascalCase)
    marker_type_name: syn::Ident,
    /// The target type T in TdbLazy<T>
    target_type: Type,
}

/// Information about an EntityIDFor<T> field (for BelongsTo generation)
struct EntityIdFieldInfo {
    /// The field identifier
    field_ident: syn::Ident,
    /// The marker type name (PascalCase)
    marker_type_name: syn::Ident,
    /// The target type T in EntityIDFor<T>
    target_type: Type,
    /// Whether the field is optional (Option<EntityIDFor<T>>)
    is_optional: bool,
}

/// Extract the inner type from Option<T>, returning (inner_type, true) or (original_type, false)
fn unwrap_option_type(ty: &Type) -> (Type, bool) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return (inner.clone(), true);
                    }
                }
            }
        }
    }
    (ty.clone(), false)
}

/// Extract the inner type from Vec<T>, returning (inner_type, true) or (original_type, false)
fn unwrap_vec_type(ty: &Type) -> (Type, bool) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return (inner.clone(), true);
                    }
                }
            }
        }
    }
    (ty.clone(), false)
}

/// Check if a type is TdbLazy<T> and extract T
fn extract_tdblazy_target(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "TdbLazy" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(target)) = args.args.first() {
                        return Some(target.clone());
                    }
                }
            }
        }
    }
    None
}

/// Check if a type is EntityIDFor<T> and extract T
fn extract_entity_id_target(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "EntityIDFor" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(target)) = args.args.first() {
                        return Some(target.clone());
                    }
                }
            }
        }
    }
    None
}

/// Analyze a field and extract TdbLazy relation information.
/// Only TdbLazy creates document links in TDB - used for ForwardRelation/ReverseRelation.
fn analyze_field_for_tdblazy(field: &syn::Field) -> Option<TdbLazyFieldInfo> {
    let field_ident = field.ident.as_ref()?;
    let field_name = field_ident.to_string();
    let field_name_upper = AsUpperCamelCase(&field_name);
    let marker_type_name = format_ident!("{}", field_name_upper.to_string());

    let ty = &field.ty;

    // Helper to create TdbLazyFieldInfo
    let make_info = |target: Type| TdbLazyFieldInfo {
        field_name: field_name.clone(),
        marker_type_name: marker_type_name.clone(),
        target_type: target,
    };

    // Try direct TdbLazy<T>
    if let Some(target) = extract_tdblazy_target(ty) {
        return Some(make_info(target));
    }

    // Try Option<TdbLazy<T>>
    let (unwrapped, is_option) = unwrap_option_type(ty);
    if is_option {
        if let Some(target) = extract_tdblazy_target(&unwrapped) {
            return Some(make_info(target));
        }
        // Try Option<Vec<TdbLazy<T>>>
        let (vec_unwrapped, is_vec) = unwrap_vec_type(&unwrapped);
        if is_vec {
            if let Some(target) = extract_tdblazy_target(&vec_unwrapped) {
                return Some(make_info(target));
            }
        }
    }

    // Try Vec<TdbLazy<T>>
    let (vec_unwrapped, is_vec) = unwrap_vec_type(ty);
    if is_vec {
        if let Some(target) = extract_tdblazy_target(&vec_unwrapped) {
            return Some(make_info(target));
        }
    }

    None
}

/// Analyze a field and extract EntityIDFor information for BelongsTo generation.
/// Only non-collection EntityIDFor<T> fields generate BelongsTo (provides typed ID access).
fn analyze_field_for_entity_id(field: &syn::Field) -> Option<EntityIdFieldInfo> {
    let field_ident = field.ident.as_ref()?;
    let field_name = field_ident.to_string();
    let field_name_upper = AsUpperCamelCase(&field_name);
    let marker_type_name = format_ident!("{}", field_name_upper.to_string());

    let ty = &field.ty;

    // Try direct EntityIDFor<T>
    if let Some(target) = extract_entity_id_target(ty) {
        return Some(EntityIdFieldInfo {
            field_ident: field_ident.clone(),
            marker_type_name,
            target_type: target,
            is_optional: false,
        });
    }

    // Try Option<EntityIDFor<T>>
    let (unwrapped, is_option) = unwrap_option_type(ty);
    if is_option {
        if let Some(target) = extract_entity_id_target(&unwrapped) {
            return Some(EntityIdFieldInfo {
                field_ident: field_ident.clone(),
                marker_type_name,
                target_type: target,
                is_optional: true,
            });
        }
    }

    // Vec<EntityIDFor<T>> does NOT get BelongsTo - no single parent ID
    None
}

/// Generate RelationTo implementations for ALL struct fields
///
/// This function generates:
/// 1. A nested `{StructName}Fields` module with marker types for each field
/// 2. A type alias `impl Struct { pub type Fields = StructFields; }` for ergonomic access
/// 3. `RelationTo<FieldType, StructFields::FieldName>` implementations for each field
///
/// The TerminusDBModel constraint in the RelationTo trait will cause compile errors
/// at usage time for non-model types, while the blanket implementations for container
/// types (Option, Vec, Box) will handle those cases automatically.
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
    let mut field_markers = vec![];
    let mut relation_to_impls = vec![];

    // Generate the Fields module name
    let fields_module_name = format_ident!("{}Fields", struct_name);

    // Generate marker types and RelationTo implementations for ALL fields
    for field in &fields_named.named {
        let field_ident = match &field.ident {
            Some(ident) => ident,
            None => continue, // Skip tuple struct fields
        };

        let field_name = field_ident.to_string();
        let field_name_upper = AsUpperCamelCase(&field_name);
        let field_type = &field.ty;

        // Generate marker type name (PascalCase of field name)
        let marker_type_name = format_ident!("{}", field_name_upper.to_string());

        // Define marker type inside the Fields module
        field_markers.push(quote! {
            /// Marker type for the `#field_name` relation field
            #[derive(Debug, Clone, Copy)]
            pub struct #marker_type_name;

            impl #relation_path::RelationField for #marker_type_name {
                fn field_name() -> &'static str {
                    #field_name
                }
            }
        });

        // Generate RelationTo implementation using the nested marker type
        let marker_path = quote! { #fields_module_name::#marker_type_name };

        let relation_to_impl = if let Some(clause) = where_clause {
            quote! {
                impl #impl_generics #relation_path::RelationTo<#field_type, #marker_path> for #struct_name #ty_generics
                #clause
                {
                    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> #woql_path::prelude::Query {
                        #relation_path::generate_relation_constraints(
                            #field_name,
                            &<#struct_name as terminusdb_schema::ToSchemaClass>::to_class(),
                            stringify!(#field_type),
                            source_var,
                            target_var,
                            false
                        )
                    }
                }
            }
        } else {
            quote! {
                impl #impl_generics #relation_path::RelationTo<#field_type, #marker_path> for #struct_name #ty_generics {
                    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> #woql_path::prelude::Query {
                        #relation_path::generate_relation_constraints(
                            #field_name,
                            &<#struct_name as terminusdb_schema::ToSchemaClass>::to_class(),
                            stringify!(#field_type),
                            source_var,
                            target_var,
                            false
                        )
                    }
                }
            }
        };
        relation_to_impls.push(relation_to_impl);
    }

    // =========================================================================
    // Generate BelongsTo impls for EntityIDFor<T> fields
    // (Provides typed ID access without implying TDB traversal)
    // =========================================================================

    let mut orm_relation_impls = vec![];

    for field in &fields_named.named {
        if let Some(entity_id_field) = analyze_field_for_entity_id(field) {
            let field_ident = &entity_id_field.field_ident;
            let marker_type_name = &entity_id_field.marker_type_name;
            let target_type = &entity_id_field.target_type;
            let is_optional = entity_id_field.is_optional;

            let marker_path = quote! { #fields_module_name::#marker_type_name };

            // Generate BelongsTo impl for EntityIDFor fields
            let belongs_to_impl = if is_optional {
                if let Some(clause) = where_clause {
                    quote! {
                        impl #impl_generics #relation_path::BelongsTo<#target_type, #marker_path> for #struct_name #ty_generics
                        #clause
                        {
                            fn parent_id(&self) -> Option<&terminusdb_schema::EntityIDFor<#target_type>> {
                                self.#field_ident.as_ref()
                            }
                        }
                    }
                } else {
                    quote! {
                        impl #impl_generics #relation_path::BelongsTo<#target_type, #marker_path> for #struct_name #ty_generics {
                            fn parent_id(&self) -> Option<&terminusdb_schema::EntityIDFor<#target_type>> {
                                self.#field_ident.as_ref()
                            }
                        }
                    }
                }
            } else {
                if let Some(clause) = where_clause {
                    quote! {
                        impl #impl_generics #relation_path::BelongsTo<#target_type, #marker_path> for #struct_name #ty_generics
                        #clause
                        {
                            fn parent_id(&self) -> Option<&terminusdb_schema::EntityIDFor<#target_type>> {
                                Some(&self.#field_ident)
                            }
                        }
                    }
                } else {
                    quote! {
                        impl #impl_generics #relation_path::BelongsTo<#target_type, #marker_path> for #struct_name #ty_generics {
                            fn parent_id(&self) -> Option<&terminusdb_schema::EntityIDFor<#target_type>> {
                                Some(&self.#field_ident)
                            }
                        }
                    }
                }
            };
            orm_relation_impls.push(belongs_to_impl);
        }
    }

    // =========================================================================
    // Generate ForwardRelation/ReverseRelation impls for TdbLazy<T> fields only
    // (TdbLazy creates actual document links in TDB schema)
    // =========================================================================

    // Track target types -> list of field names for DefaultField impl generation
    // We need to know if there's exactly one field per target to generate default_field_name()
    let mut target_field_info: HashMap<String, (Type, Vec<String>)> = HashMap::new();

    for field in &fields_named.named {
        if let Some(tdblazy_field) = analyze_field_for_tdblazy(field) {
            let field_name = &tdblazy_field.field_name;
            let marker_type_name = &tdblazy_field.marker_type_name;
            let target_type = &tdblazy_field.target_type;

            let marker_path = quote! { #fields_module_name::#marker_type_name };

            // Generate ReverseRelation impl (field-specific, for .with_via())
            let reverse_relation_impl = if let Some(clause) = where_clause {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #marker_path> for #struct_name #ty_generics
                    #clause
                    {}
                }
            } else {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #marker_path> for #struct_name #ty_generics {}
                }
            };
            orm_relation_impls.push(reverse_relation_impl);

            // Generate ForwardRelation impl
            let forward_relation_impl = if let Some(clause) = where_clause {
                quote! {
                    impl #impl_generics #relation_path::ForwardRelation<#target_type, #marker_path> for #struct_name #ty_generics
                    #clause
                    {}
                }
            } else {
                quote! {
                    impl #impl_generics #relation_path::ForwardRelation<#target_type, #marker_path> for #struct_name #ty_generics {}
                }
            };
            orm_relation_impls.push(forward_relation_impl);

            // Track target type -> field names for DefaultField impl
            let target_key = quote! { #target_type }.to_string();
            target_field_info
                .entry(target_key)
                .or_insert_with(|| (target_type.clone(), Vec::new()))
                .1
                .push(field_name.clone());
        }
    }

    // Generate ReverseRelation<Target, DefaultField> impls
    // When there's exactly ONE field for a target, include default_field_name() returning Some(field)
    // When there are multiple fields, use the trait default (returns None)
    for (target_type, field_names) in target_field_info.values() {
        let default_reverse_impl = if field_names.len() == 1 {
            // Single field - provide the actual field name for .with() to use
            let field_name = &field_names[0];
            if let Some(clause) = where_clause {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #relation_path::DefaultField> for #struct_name #ty_generics
                    #clause
                    {
                        fn default_field_name() -> Option<&'static str> {
                            Some(#field_name)
                        }
                    }
                }
            } else {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #relation_path::DefaultField> for #struct_name #ty_generics {
                        fn default_field_name() -> Option<&'static str> {
                            Some(#field_name)
                        }
                    }
                }
            }
        } else {
            // Multiple fields - use trait default (None) to indicate ambiguity
            if let Some(clause) = where_clause {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #relation_path::DefaultField> for #struct_name #ty_generics
                    #clause
                    {}
                }
            } else {
                quote! {
                    impl #impl_generics #relation_path::ReverseRelation<#target_type, #relation_path::DefaultField> for #struct_name #ty_generics {}
                }
            }
        };
        orm_relation_impls.push(default_reverse_impl);
    }

    // Generate the Fields module
    // Note: Using `{StructName}Fields` module directly instead of `StructName::Fields`
    // because inherent associated types are unstable and conflict with serde derives.
    // Users should use: `CarFields::FrontWheels` instead of `Car::Fields::FrontWheels`
    let fields_module = quote! {
        /// Field marker types for `#struct_name` relations.
        ///
        /// Use these markers with `.with_field::<Target, #fields_module_name::FieldName>()`
        /// to specify which relation field to traverse.
        pub mod #fields_module_name {
            use super::*;

            #(#field_markers)*
        }
    };

    quote! {
        #fields_module
        #(#relation_to_impls)*
        #(#orm_relation_impls)*
    }
}

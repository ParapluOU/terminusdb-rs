use crate::instance::{generate_totdbinstance_impl, process_fields_for_instance};
use crate::prelude::*;
use crate::schema::generate_totdbschema_impl;
#[cfg(feature = "generic-derive")]
use std::collections::HashMap;

/// Auto-detect id_field if not explicitly specified.
///
/// Rules:
/// 1. If id_field is already specified, return it unchanged
/// 2. Look for a field named "id"
/// 3. Check if that field has type EntityIDFor<Self> or PrimaryKey
/// 4. If found, return Some("id"), otherwise return None
fn auto_detect_id_field(fields_named: &FieldsNamed, opts: &TDBModelOpts) -> Option<String> {
    // If explicitly specified, use that
    if opts.id_field.is_some() {
        return opts.id_field.clone();
    }

    // Look for a field named "id"
    for field in &fields_named.named {
        if let Some(ident) = &field.ident {
            if ident == "id" {
                // Check if it's EntityIDFor<Self> or PrimaryKey
                if crate::prelude::is_entity_id_for_self(&field.ty)
                    || crate::prelude::is_primary_key_type(&field.ty)
                {
                    return Some("id".to_string());
                }
            }
        }
    }

    None
}

/// Validate that id_field exists when specified and has the correct type for the key strategy
fn validate_id_field_type(
    fields_named: &FieldsNamed,
    opts: &TDBModelOpts,
) -> Result<(), syn::Error> {
    // Only validate if id_field is specified
    if let Some(ref id_field_name) = opts.id_field {
        // Find the field with the matching name
        let field = fields_named
            .named
            .iter()
            .find(|f| f.ident.as_ref().map(|i| i.to_string()) == Some(id_field_name.clone()))
            .ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("id_field '{}' not found in struct", id_field_name),
                )
            })?;

        // Check if the key strategy is non-Random
        let key_strategy = opts.key.as_ref().map(|s| s.as_str());
        let is_non_random_key = match key_strategy {
            Some("random") | None => false, // Random is the default
            Some("lexical") | Some("hash") | Some("value_hash") => true,
            Some(other) => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Unknown key strategy: {}", other),
                ));
            }
        };

        // For non-Random keys, enforce that id_field is ServerIDFor<T>
        if is_non_random_key && !is_server_id_for_type(&field.ty) {
            return Err(syn::Error::new(
                field.ty.span(),
                format!(
                    "id_field '{}' must be of type ServerIDFor<Self> when using '{}' key strategy. \
                     ServerIDFor is required for server-generated IDs with non-Random key strategies.",
                    id_field_name,
                    key_strategy.unwrap()
                ),
            ));
        }
    }
    Ok(())
}

/// Generate implementation for structs (maps to Class in TerminusDB)
pub fn implement_for_struct(
    input: &DeriveInput,
    data_struct: &DataStruct,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;

    // Auto-detect id_field if not explicitly specified
    let effective_opts = match &data_struct.fields {
        Fields::Named(fields_named) => {
            let detected_id_field = auto_detect_id_field(fields_named, opts);
            if detected_id_field != opts.id_field {
                let mut new_opts = opts.clone();
                new_opts.id_field = detected_id_field;
                new_opts
            } else {
                opts.clone()
            }
        }
        _ => opts.clone(),
    };
    let opts = &effective_opts;

    // Generate class name that includes generic parameters
    let class_name = if let Some(explicit_class) = &opts.class_name {
        explicit_class.clone()
    } else {
        struct_name.to_string()
    };

    // For generics, we need to generate a dynamic class name
    #[cfg(feature = "generic-derive")]
    let class_name_expr = if !input.generics.params.is_empty() {
        // Generate a format string that includes generic types
        let mut format_str = class_name.clone();
        format_str.push('<');
        let generic_types: Vec<_> = input
            .generics
            .params
            .iter()
            .map(|param| {
                match param {
                    syn::GenericParam::Type(type_param) => {
                        let ident = &type_param.ident;
                        quote! {
                            {
                                // Inline prettify function
                                let full_name = std::any::type_name::<#ident>();
                                // Take only the last component after ::
                                full_name.rsplit("::").next().unwrap_or(full_name).to_string()
                            }
                        }
                    }
                    _ => quote! { "?" }, // Lifetime or const generics not supported
                }
            })
            .collect();

        if generic_types.is_empty() {
            quote! { #class_name }
        } else {
            quote! {
                {
                    let mut class_name = String::from(#class_name);
                    class_name.push('<');
                    let mut first = true;
                    #(
                        if !first { class_name.push_str(", "); }
                        class_name.push_str(&#generic_types);
                        first = false;
                    )*
                    class_name.push('>');
                    class_name
                }
            }
        }
    } else {
        quote! { #class_name }
    };

    #[cfg(not(feature = "generic-derive"))]
    let class_name_expr = quote! { #class_name };

    // Extract base generics for all impls
    let (base_impl_generics, base_ty_generics, base_where_clause) = input.generics.split_for_impl();

    // Generate bounds for each trait implementation if generics are enabled
    #[cfg(feature = "generic-derive")]
    let bounds_for_traits = if !input.generics.params.is_empty() {
        if let Fields::Named(fields_named) = &data_struct.fields {
            let mut trait_bounds = HashMap::new();

            // Collect bounds for each trait
            trait_bounds.insert(
                crate::bounds::TraitImplType::ToTDBSchema,
                crate::bounds::collect_bounds_for_impl(
                    fields_named,
                    &input.generics,
                    struct_name,
                    crate::bounds::TraitImplType::ToTDBSchema,
                ),
            );
            trait_bounds.insert(
                crate::bounds::TraitImplType::ToTDBInstance,
                crate::bounds::collect_bounds_for_impl(
                    fields_named,
                    &input.generics,
                    struct_name,
                    crate::bounds::TraitImplType::ToTDBInstance,
                ),
            );
            trait_bounds.insert(
                crate::bounds::TraitImplType::FromTDBInstance,
                crate::bounds::collect_bounds_for_impl(
                    fields_named,
                    &input.generics,
                    struct_name,
                    crate::bounds::TraitImplType::FromTDBInstance,
                ),
            );
            trait_bounds.insert(
                crate::bounds::TraitImplType::InstanceFromJson,
                crate::bounds::collect_bounds_for_impl(
                    fields_named,
                    &input.generics,
                    struct_name,
                    crate::bounds::TraitImplType::InstanceFromJson,
                ),
            );

            Some(trait_bounds)
        } else {
            None
        }
    } else {
        None
    };

    #[cfg(not(feature = "generic-derive"))]
    let bounds_for_traits: Option<
        std::collections::HashMap<String, std::collections::HashMap<syn::Ident, Vec<String>>>,
    > = None;

    // For non-generic code, use empty generics
    #[cfg(not(feature = "generic-derive"))]
    let (impl_generics, ty_generics, where_clause) =
        (quote! {}, quote! {}, None::<syn::WhereClause>);

    // For generic code, we'll use specific bounds for each impl
    #[cfg(feature = "generic-derive")]
    let (impl_generics, ty_generics, _) = (
        quote! { #base_impl_generics },
        quote! { #base_ty_generics },
        base_where_clause.cloned(),
    );

    // Generate the implementation for ToSchemaClass trait
    let schema_class_impl = {
        #[cfg(feature = "generic-derive")]
        let where_clause = if let Some(ref bounds_map) = bounds_for_traits {
            if let Some(bounds) = bounds_map.get(&crate::bounds::TraitImplType::ToTDBSchema) {
                let predicates = crate::bounds::build_where_predicates(bounds);
                crate::bounds::combine_where_clauses(base_where_clause, predicates)
            } else {
                base_where_clause.cloned()
            }
        } else {
            base_where_clause.cloned()
        };

        #[cfg(not(feature = "generic-derive"))]
        let where_clause: Option<syn::WhereClause> = None;

        quote! {
            impl #impl_generics terminusdb_schema::ToSchemaClass for #struct_name #ty_generics #where_clause {
                fn to_class() -> String {
                    #class_name_expr.to_string()
                }
            }
        }
    };

    // Process the struct fields to generate property definitions for schema
    let properties = match &data_struct.fields {
        Fields::Named(fields_named) => {
            // Validate id_field type if specified
            if let Err(e) = validate_id_field_type(fields_named, opts) {
                return e.to_compile_error();
            }
            process_named_fields(fields_named, struct_name, &ty_generics)
        }
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "TerminusDBModel derive macro only supports structs with named fields",
            )
            .to_compile_error();
        }
    };

    // Process the struct fields to generate instance conversions
    let fields_named = match &data_struct.fields {
        Fields::Named(fields) => fields,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "TerminusDBModel derive macro only supports structs with named fields for instance generation",
            )
            .to_compile_error();
        }
    };

    // Collect field types for to_schema_tree
    let field_types = match &data_struct.fields {
        Fields::Named(fields_named) => fields_named
            .named
            .iter()
            .filter(|field| {
                // Skip PhantomData fields - they don't contribute to schema tree
                !crate::prelude::is_phantom_data_type(&field.ty)
            })
            .map(|field| {
                let field_ty = &field.ty;
                field_ty
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    // Collect field identifiers for to_schema_tree_mut
    let field_idents = match &data_struct.fields {
        Fields::Named(fields_named) => fields_named
            .named
            .iter()
            .filter(|field| {
                // Skip PhantomData fields
                !crate::prelude::is_phantom_data_type(&field.ty)
            })
            .filter_map(|field| field.ident.as_ref().map(|ident| ident.clone()))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    // Generate the to_schema_tree implementation that collects schemas from all field types
    let to_schema_tree_impl = if field_types.is_empty() {
        quote! {
            fn to_schema_tree() -> Vec<terminusdb_schema::Schema> {
                vec![<Self as terminusdb_schema::ToTDBSchema>::to_schema()]
            }

            fn to_schema_tree_mut(collection: &mut std::collections::HashSet<terminusdb_schema::Schema>) {
                let schema = <Self as terminusdb_schema::ToTDBSchema>::to_schema();
                let class_name = schema.class_name().clone();

                // Only add if not already present (prevents recursion)
                if !collection.iter().any(|s| s.class_name() == &class_name) {
                    collection.insert(schema);
                    // No field types to process
                }
            }
        }
    } else {
        quote! {
            fn to_schema_tree() -> Vec<terminusdb_schema::Schema> {
                let mut collection = std::collections::HashSet::new();
                <Self as terminusdb_schema::ToTDBSchema>::to_schema_tree_mut(&mut collection);
                collection.into_iter().collect()
            }

            fn to_schema_tree_mut(collection: &mut std::collections::HashSet<terminusdb_schema::Schema>) {
                let schema = <Self as terminusdb_schema::ToTDBSchema>::to_schema();
                let class_name = schema.class_name().clone();

                // Only add if not already present (prevents recursion)
                if !collection.iter().any(|s| s.class_name() == &class_name) {
                    collection.insert(schema);

                    // Process field types statically
                    #(
                        <#field_types as terminusdb_schema::ToMaybeTDBSchema>::to_schema_tree_mut(collection);
                    )*
                }
            }
        }
    };

    // Generate the implementation for schema
    let schema_impl = {
        #[cfg(feature = "generic-derive")]
        let schema_where_clause = if let Some(ref bounds_map) = bounds_for_traits {
            if let Some(bounds) = bounds_map.get(&crate::bounds::TraitImplType::ToTDBSchema) {
                let predicates = crate::bounds::build_where_predicates(bounds);
                crate::bounds::combine_where_clauses(base_where_clause, predicates)
            } else {
                base_where_clause.cloned()
            }
        } else {
            base_where_clause.cloned()
        };

        #[cfg(not(feature = "generic-derive"))]
        let schema_where_clause: Option<syn::WhereClause> = None;

        generate_totdbschema_impl(
            struct_name,
            class_name_expr.clone(),
            opts,
            properties,
            quote! { SchemaTypeClass },
            to_schema_tree_impl,
            #[cfg(feature = "generic-derive")]
            (&impl_generics, &ty_generics, &schema_where_clause),
            #[cfg(not(feature = "generic-derive"))]
            (&quote! {}, &quote! {}, &None),
        )
    };

    // Generate the body code for the to_instance method for structs
    let properties_code = process_fields_for_instance(fields_named, struct_name);
    let instance_body_code = quote! {
        // Create a BTreeMap for properties
        let mut properties = std::collections::BTreeMap::new();

        // Convert each field to an InstanceProperty
        #properties_code

        // Construct the final Instance (optid_val is provided by the wrapper)
        terminusdb_schema::Instance {
            id: id.or( optid_val ).map(|v| schema.format_id(&v)),
            capture: false,
            ref_props: true,
            schema,
            properties,
        }
    };

    // Generate the implementation for instance using the simplified wrapper
    let instance_impl = {
        #[cfg(feature = "generic-derive")]
        let instance_where_clause = if let Some(ref bounds_map) = bounds_for_traits {
            if let Some(bounds) = bounds_map.get(&crate::bounds::TraitImplType::ToTDBInstance) {
                let predicates = crate::bounds::build_where_predicates(bounds);
                crate::bounds::combine_where_clauses(base_where_clause, predicates)
            } else {
                base_where_clause.cloned()
            }
        } else {
            base_where_clause.cloned()
        };

        #[cfg(not(feature = "generic-derive"))]
        let instance_where_clause: Option<syn::WhereClause> = None;

        generate_totdbinstance_impl(
            struct_name,
            instance_body_code, // Pass the generated body code
            opts.clone(),       // No longer pass None here
            #[cfg(feature = "generic-derive")]
            (&impl_generics, &ty_generics, &instance_where_clause),
            #[cfg(not(feature = "generic-derive"))]
            (&quote! {}, &quote! {}, &None),
            None, // No custom ID extraction for structs
        )
    };

    // Generate RelationTo implementations only if relations feature is enabled
    #[cfg(feature = "relations")]
    let relation_impls = terminusdb_relation_derive::generate_relation_impls(
        struct_name,
        fields_named,
        &impl_generics,
        &ty_generics,
        &base_where_clause.cloned(),
    );

    #[cfg(not(feature = "relations"))]
    let relation_impls = quote! {};

    // Generate Class marker trait implementation
    let class_marker_impl = quote! {
        impl #impl_generics terminusdb_schema::Class for #struct_name #ty_generics #base_where_clause {}
    };

    // Combine all implementations
    quote! {
        #schema_impl

        #instance_impl

        #schema_class_impl

        #class_marker_impl

        #relation_impls
    }
}

/// Process named fields from a struct to generate property definitions
pub fn process_named_fields(
    fields_named: &FieldsNamed,
    struct_name: &syn::Ident,
    ty_generics: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let fields = fields_named
        .named
        .iter()
        .filter(|field| {
            // Skip PhantomData fields - they're zero-sized and shouldn't appear in schema
            !crate::prelude::is_phantom_data_type(&field.ty)
        })
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_ty = &field.ty;
            let field_opts = TDBFieldOpts::from_field(field).unwrap();
            let property_name = field_opts.name.unwrap_or_else(|| field_name.to_string());

            // Extract subdocument value before quote
            // todo: adjust SchemaProperty to be able to encode subdocument requirements on the property level
            let subdocument = field_opts.subdocument;

            // Add class override if specified
            let classoverride = if let Some(class) = field_opts.class {
                quote! {
                    // terminusdb_schema::Property {
                    //     name: #property_name.to_string(),
                    //     r#type: None,
                    //     class: #class.to_string(),
                    // }
                    prop.class = #class.to_string();
                }
            }
            else {
                quote!{}
            };

            let mut property = quote! {
                // terminusdb_schema::Property {
                //     name: #property_name.to_string(),
                //     r#type: None,
                //     class: <#field_ty as terminusdb_schema::ToSchemaClass>::to_class().to_string(),
                // }

                {
                    let mut prop = <#field_ty as terminusdb_schema::ToSchemaProperty<#struct_name #ty_generics>>::to_property(#property_name);
                    #classoverride
                    prop
                }
            };

            property
        })
        .collect::<Vec<_>>();

    // Return the vector of properties
    quote! {
        Some(vec![
            #(#fields),*
        ])
    }
}

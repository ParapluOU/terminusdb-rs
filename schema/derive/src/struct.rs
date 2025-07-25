use crate::instance::{generate_totdbinstance_impl, process_fields_for_instance};
use crate::prelude::*;
use crate::schema::generate_totdbschema_impl;

/// Generate implementation for structs (maps to Class in TerminusDB)
pub fn implement_for_struct(
    input: &DeriveInput,
    data_struct: &DataStruct,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;
    let class_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| struct_name.to_string());

    // Generate the implementation for ToSchemaClass trait
    let schema_class_impl = quote! {
        impl terminusdb_schema::ToSchemaClass for #struct_name {
            fn to_class() -> &'static str {
                #struct_name
            }
        }
    };

    // Process the struct fields to generate property definitions for schema
    let properties = match &data_struct.fields {
        Fields::Named(fields_named) => process_named_fields(fields_named, struct_name),
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
    let schema_impl = generate_totdbschema_impl(
        struct_name,
        &class_name,
        opts,
        properties,
        quote! { SchemaTypeClass },
        to_schema_tree_impl,
    );

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
    let instance_impl = generate_totdbinstance_impl(
        struct_name,
        instance_body_code, // Pass the generated body code
        opts.clone(),       // No longer pass None here
    );

    // Combine both implementations
    quote! {
        #schema_impl

        #instance_impl

        // #schema_class_impl
    }
}

/// Process named fields from a struct to generate property definitions
pub fn process_named_fields(
    fields_named: &FieldsNamed,
    struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let fields = fields_named
        .named
        .iter()
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

                <#field_ty as terminusdb_schema::ToSchemaProperty<#struct_name>>::to_property(#property_name).tap_mut(|prop| {
                    #classoverride
                })
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

use log::trace;
use crate::prelude::*;
use crate::schema::generate_totdbschema_impl;
use crate::instance::{generate_totdbinstance_impl, process_enum_variants_for_instance};

/// Process a simple enum (without variants carrying values) to generate a TerminusDB Enum
pub fn implement_for_simple_enum(
    input: &DeriveInput,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let enum_name = &input.ident;
    let class_name = opts.class_name.clone().unwrap_or_else(|| enum_name.to_string());

    trace!("Processing SimpleEnum: {}", enum_name);
    
    // Get the rename strategy from opts
    let rename_strategy = opts.get_rename_strategy();
    
    // Extract the variant names from the enum
    let variant_values = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let variant_name_str = variant_name.to_string();
        
        // Ensure this is a simple variant without fields
        match &variant.fields {
            Fields::Unit => {
                // Apply rename strategy to variant name
                let renamed_variant = rename_strategy.apply(&variant_name_str);
                quote! { #renamed_variant.to_string() }
            },
            _ => {
                // This is not a simple enum variant - generate an error
                trace!("Error: Non-unit variant in simple enum {}: {}", enum_name, variant_name);
                syn::Error::new(
                    variant.span(),
                    "TerminusDBModel for Enum only supports simple enum variants without fields. Tagged enums should use TaggedUnion."
                ).to_compile_error()
            }
        }
    }).collect::<Vec<_>>();

    // Generate the values implementation for the enum
    let values_impl = quote! {
        Some(vec![#(#variant_values),*])
    };

    // Generate the to_schema_tree implementation that collects schemas from all field types
    let to_schema_tree_impl = quote! {
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
                // Enums don't need to process field types
            }
        }
    };

    // Generate the schema implementation with an Enum type
    let schema_impl = generate_totdbschema_impl(
        enum_name,
        &class_name,
        opts,
        values_impl,
        quote! { SchemaTypeEnum },
        to_schema_tree_impl
    );
    
    // Generate the body code for the to_instance method for simple enums
    let properties_code = process_enum_variants_for_instance(data_enum, enum_name, rename_strategy);
    let instance_body_code = quote! {
        // Create a BTreeMap for properties
        let mut properties = std::collections::BTreeMap::new();
        
        // Populate properties based on the enum variant
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
    
    // Generate the ToTDBInstance implementation using the simplified wrapper
    let instance_impl = generate_totdbinstance_impl(
        enum_name,
        instance_body_code, // Pass the generated body code
        opts.clone()
        // No longer pass Some(data_enum) here
    );
    
    // Generate the implementation for ToSchemaClass trait
    let schema_class_impl = quote! {
        impl terminusdb_schema::ToSchemaClass for #enum_name {
            fn to_class() -> &'static str {
                #enum_name
            }
        }
    };
    
    // Combine both implementations
    quote! {
        #schema_impl
        
        #instance_impl

        // #schema_class_impl
    }
}

// Implementation note: For simple enums, we use stringify! to convert variant names to strings
// This eliminates the need for qualified enum variant paths, avoiding the common error where
// unqualified enum variants are used in the generated code.

// todo: when an enum does not have tags, it is a simple enum
// in that case it has to derive a terminusdb_schema::Schema::Enum
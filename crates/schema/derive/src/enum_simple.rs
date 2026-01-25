use crate::instance::{generate_totdbinstance_impl, process_enum_variants_for_instance};
use crate::prelude::*;
use crate::schema::generate_totdbschema_impl;
use tracing::trace;

/// Process a simple enum (without variants carrying values) to generate a TerminusDB Enum
pub fn implement_for_simple_enum(
    input: &DeriveInput,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let enum_name = &input.ident;
    let class_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| enum_name.to_string());

    trace!("Processing SimpleEnum: {}", enum_name);

    // Get the rename strategy from opts, defaulting to lowercase for enum variants
    let rename_strategy = match opts.get_rename_strategy() {
        crate::args::RenameStrategy::None => crate::args::RenameStrategy::Lowercase,
        other => other,
    };

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
        quote! { #class_name },
        opts,
        values_impl,
        quote! { SchemaTypeEnum },
        to_schema_tree_impl,
        (&quote! {}, &quote! {}, &None), // No generics for enums currently
    );

    // Generate the body code for the to_instance method for simple enums
    let instance_rename_strategy = match opts.get_rename_strategy() {
        crate::args::RenameStrategy::None => crate::args::RenameStrategy::Lowercase,
        other => other,
    };
    let properties_code =
        process_enum_variants_for_instance(data_enum, enum_name, instance_rename_strategy);
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
        instance_body_code,              // Pass the generated body code
        opts.clone(),                    // No longer pass Some(data_enum) here
        (&quote! {}, &quote! {}, &None), // No generics for enums currently
        None,                            // No custom ID extraction for simple enums
    );

    // Generate the implementation for ToSchemaClass trait
    let schema_class_impl = quote! {
        impl terminusdb_schema::ToSchemaClass for #enum_name {
            fn to_class() -> String {
                stringify!(#enum_name).to_string()
            }
        }
    };

    // Generate Class marker trait implementation
    let class_marker_impl = quote! {
        impl terminusdb_schema::Class for #enum_name {}
    };

    // Generate TDBEnum trait implementation for enum deserialization
    let tdb_enum_impl = generate_tdbenum_impl(data_enum, enum_name, rename_strategy);

    // Combine all implementations
    quote! {
        #schema_impl

        #instance_impl

        #schema_class_impl

        #class_marker_impl

        #tdb_enum_impl
    }
}

// Implementation note: For simple enums, we use stringify! to convert variant names to strings
// This eliminates the need for qualified enum variant paths, avoiding the common error where
// unqualified enum variants are used in the generated code.

// todo: when an enum does not have tags, it is a simple enum
// in that case it has to derive a terminusdb_schema::Schema::Enum

/// Generate the TDBEnum trait implementation for a simple enum.
///
/// This provides bidirectional mapping between Rust enum variants and their
/// TerminusDB string representations, enabling proper deserialization.
fn generate_tdbenum_impl(
    data_enum: &DataEnum,
    enum_name: &syn::Ident,
    rename_strategy: crate::args::RenameStrategy,
) -> proc_macro2::TokenStream {
    // Collect variant info: (variant_ident, renamed_string)
    let variant_info: Vec<_> = data_enum
        .variants
        .iter()
        .filter_map(|variant| {
            match &variant.fields {
                Fields::Unit => {
                    let variant_ident = &variant.ident;
                    let variant_name_str = variant_ident.to_string();
                    let renamed = rename_strategy.apply(&variant_name_str);
                    Some((variant_ident.clone(), renamed))
                }
                _ => None, // Skip non-unit variants (they'll error elsewhere)
            }
        })
        .collect();

    // Generate variants() - returns Vec of all variants
    let variants_arms: Vec<_> = variant_info
        .iter()
        .map(|(ident, _)| quote! { #enum_name::#ident })
        .collect();

    // Generate to_tdb_value() - match self to renamed string
    let to_value_arms: Vec<_> = variant_info
        .iter()
        .map(|(ident, renamed)| {
            quote! { #enum_name::#ident => #renamed.to_string() }
        })
        .collect();

    // Generate from_tdb_value() - match string to variant
    let from_value_arms: Vec<_> = variant_info
        .iter()
        .map(|(ident, renamed)| {
            quote! { #renamed => Some(#enum_name::#ident) }
        })
        .collect();

    quote! {
        impl terminusdb_schema::TDBEnum for #enum_name {
            fn variants() -> Vec<Self> {
                vec![#(#variants_arms),*]
            }

            fn to_tdb_value(&self) -> String {
                match self {
                    #(#to_value_arms),*
                }
            }

            fn from_tdb_value(s: &str) -> Option<Self> {
                match s {
                    #(#from_value_arms,)*
                    _ => None,
                }
            }
        }
    }
}

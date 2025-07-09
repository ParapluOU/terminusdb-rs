use crate::instance::{
    generate_abstract_tagged_union_instance_logic, generate_totdbinstance_impl,
    process_tagged_enum_for_instance,
};
use crate::prelude::*;
use crate::r#struct::process_named_fields;
use crate::schema::generate_totdbschema_impl;
use log::{log, trace};
use quote::format_ident;

/// Process a tagged union enum (with variants carrying values) to generate a TerminusDB TaggedUnion
pub fn implement_for_tagged_enum(
    input: &DeriveInput,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let enum_name = &input.ident;
    let class_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| enum_name.to_string());

    trace!("Processing TaggedEnum: {}", enum_name);

    // Get the rename strategy from opts, defaulting to lowercase for enum variants
    let rename_strategy = match opts.get_rename_strategy() {
        crate::args::RenameStrategy::None => crate::args::RenameStrategy::Lowercase,
        other => other,
    };

    // Process enum variants to generate properties
    let variant_properties = process_enum_variants(data_enum, enum_name, rename_strategy);

    // Generate virtual structs for complex variants
    let virtual_structs = generate_virtual_structs(data_enum, enum_name, opts);

    trace!(
        "{} virtual_structs generated for {}: {}",
        virtual_structs.len(),
        enum_name,
        virtual_structs
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Generate to_schema_tree implementation that includes virtual struct schemas
    let to_schema_tree_impl = if virtual_structs.is_empty() {
        // If there are no complex variants, just return the main schema
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
                }
            }
        }
    } else {
        // Collect variant struct schema names for inclusion in the schema tree
        let variant_struct_names = virtual_structs.iter().map(|(struct_name, _)| {
            let struct_name_ident = format_ident!("{}", struct_name);
            quote! { 
                for variant_schema in <#struct_name_ident as terminusdb_schema::ToTDBSchema>::to_schema_tree() {
                    if !collection.iter().any(|s| s.class_name() == variant_schema.class_name()) {
                        collection.insert(variant_schema);
                    }
                }
            }
        }).collect::<Vec<_>>();

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

                    // Include schemas for all complex variants
                    #(#variant_struct_names)*
                }
            }
        }
    };

    // Generate the schema implementation
    let schema_impl = generate_totdbschema_impl(
        enum_name,
        &class_name,
        opts,
        variant_properties,
        quote! { SchemaTypeTaggedUnion },
        to_schema_tree_impl,
    );

    // Generate the body code for the to_instance method based on whether the enum is abstract
    let instance_body_code = if opts.abstract_class.unwrap_or(false) {
        // Abstract Tagged Union: Generate the match statement that delegates to inner type
        generate_abstract_tagged_union_instance_logic(data_enum, enum_name)
    } else {
        // Non-Abstract Tagged Union: Generate standard instance creation code
        let rename_strategy = match opts.get_rename_strategy() {
            crate::args::RenameStrategy::None => crate::args::RenameStrategy::Lowercase,
            other => other,
        };
        let properties_code =
            process_tagged_enum_for_instance(data_enum, enum_name, rename_strategy);
        quote! {
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
        }
    };

    // Generate the ToTDBInstance implementation using the simplified wrapper
    let instance_impl = generate_totdbinstance_impl(
        enum_name,
        instance_body_code, // Pass the generated body code
        opts.clone(),       // No longer pass Some(data_enum) here
    );

    // Extract the TokenStream from the second element of each tuple in virtual_structs
    let virtual_struct_impls = virtual_structs.iter().map(|(_, tokens)| tokens);

    // Generate the implementation for ToSchemaClass trait
    let schema_class_impl = quote! {
        impl terminusdb_schema::ToSchemaClass for #enum_name {
            fn to_class() -> &'static str {
                #enum_name
            }
        }
    };

    // Combine all the implementations
    quote! {
        #schema_impl

        #instance_impl

        // #schema_class_impl

        // Include virtual structs for complex variants
        #(#virtual_struct_impls)*
    }
}

/// Process enum variants to generate properties for a TaggedUnion
fn process_enum_variants(
    data_enum: &DataEnum,
    enum_name: &syn::Ident,
    rename_strategy: crate::args::RenameStrategy,
) -> proc_macro2::TokenStream {
    let properties = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let variant_name_str = variant_name.to_string();
        let variant_name_renamed = rename_strategy.apply(&variant_name_str);
        
        match &variant.fields {
            // For unit variants, use sys:Unit
            Fields::Unit => {
                quote! {
                    terminusdb_schema::Property {
                        name: #variant_name_renamed.to_string(),
                        class: "sys:Unit".to_string(),
                        r#type: None
                    }
                }
            },
            // For single-field variants (newtype), use the inner type
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = fields.unnamed.first().unwrap();
                let field_ty = &field.ty;
                
                quote! {
                    {
                        let mut prop = terminusdb_schema::Property {
                            name: #variant_name_renamed.to_string(),
                            class: <#field_ty as terminusdb_schema::ToSchemaClass>::to_class().to_string(),
                            r#type: None
                        };
                        prop
                    }
                }
            },
            // For multi-field variants, create a new class for them
            Fields::Unnamed(_) => {
                let variant_struct_name = format!("{}{}", enum_name, variant_name);
                
                quote! {
                    terminusdb_schema::Property {
                        name: #variant_name_renamed.to_string(),
                        class: #variant_struct_name.to_string(),
                        r#type: None
                    }
                }
            },
            // For named fields, create a new class for them
            Fields::Named(_) => {
                let variant_struct_name = format!("{}{}", enum_name, variant_name);
                
                quote! {
                    terminusdb_schema::Property {
                        name: #variant_name_renamed.to_string(),
                        class: #variant_struct_name.to_string(),
                        r#type: None
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

    quote! {
        Some(vec![
            #(#properties),*
        ])
    }
}

/// Generate virtual structs for complex enum variants
fn generate_virtual_structs(
    data_enum: &DataEnum,
    enum_name: &syn::Ident,
    parent_opts: &TDBModelOpts,
) -> Vec<(String, proc_macro2::TokenStream)> {
    let mut virtual_structs = Vec::new();

    for variant in &data_enum.variants {
        let variant_name = &variant.ident;
        let variant_name_str = variant_name.to_string();

        // Use consistent naming for all variant structs
        let variant_struct_name = format!("{}{}", enum_name, variant_name_str);

        let variant_struct_ident = format_ident!("{}", variant_struct_name);

        match &variant.fields {
            // Skip unit variants
            Fields::Unit => continue,

            // Skip single-field variants (newtypes)
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => continue,

            // Generate virtual struct for multi-field unnamed variants
            Fields::Unnamed(fields) => {
                // todo: seemingly redundant generation of field names

                // Create field definitions
                let field_defs = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        let field_name = format_ident!("_{}", i);
                        let field_ty = &field.ty;
                        quote! {
                            pub #field_name: #field_ty
                        }
                    })
                    .collect::<Vec<_>>();

                // Collect field identifiers for to_schema_tree_mut
                let field_idents = (0..fields.unnamed.len())
                    .map(|i| format_ident!("_{}", i))
                    .collect::<Vec<_>>();

                // Generate the struct definition
                let struct_def = quote! {
                    /// Generated virtual struct for enum variant
                    #[derive(Debug, Clone, terminusdb_schema_derive::TerminusDBModel, terminusdb_schema_derive::FromTDBInstance)]
                    #[allow(non_camel_case_types)]
                    struct #variant_struct_ident {
                        #(#field_defs),*
                    }
                };

                // Generate the schema implementation
                let schema_impl = generate_totdbschema_impl(
                    &variant_struct_ident,
                    &variant_struct_name,
                    &TDBModelOpts {
                        class_name: Some(variant_struct_name.to_string()),
                        base: None,
                        key: Some("value_hash".to_string()),
                        abstract_class: None,
                        unfoldable: None,
                        inherits: None,
                        doc: None,
                        original_input: None,
                        id_field: None,
                        rename_all: None,
                    },
                    quote! { None },
                    quote! { SchemaTypeClass },
                    quote! {
                        fn to_schema_tree() -> Vec<terminusdb_schema::Schema> {
                            // vec![<Self as terminusdb_schema::ToTDBSchema>::to_schema()]
                            unimplemented!()
                        }

                        fn to_schema_tree_mut(collection: &mut std::collections::HashSet<terminusdb_schema::Schema>) {
                            let schema = <Self as terminusdb_schema::ToTDBSchema>::to_schema();
                            let class_name = schema.class_name().clone();

                            // Only add if not already present (prevents recursion)
                            if !collection.iter().any(|s| s.class_name() == &class_name) {
                                collection.insert(schema);
                                // No field types to process
                            }

                            todo!("recursively add subschemas for enum virtual struct" )
                        }
                    },
                );

                // Generate the instance implementation
                let instance_impl = generate_totdbinstance_impl(
                    &variant_struct_ident,
                    quote! { // This is the instance_body_code for the virtual struct
                        let mut properties = std::collections::BTreeMap::new();
                        #(
                            properties.insert(
                                format!("_{}", #field_idents),
                                <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                                    self.#field_idents.clone(),
                                    format!("_{}", #field_idents),
                                    // 'schema' is provided by the wrapper impl
                                    &schema
                                )
                            );
                        )*
                        // Construct the final Instance (optid_val is provided by the wrapper)
                        terminusdb_schema::Instance {
                            id: id.or( optid_val ).map(|v| schema.format_id(&v)),
                            capture: false,
                            ref_props: true,
                            schema,
                            properties,
                        }
                    },
                    TDBModelOpts {
                        // These are the args for the virtual struct
                        class_name: Some(variant_struct_name.clone()),
                        base: None,
                        key: Some("value_hash".to_string()),
                        abstract_class: None,
                        unfoldable: None,
                        inherits: None,
                        doc: None,
                        original_input: None,
                        id_field: None,
                        rename_all: None,
                    },
                );

                // Combine the struct definition and implementations
                let full_impl = quote! {
                    #struct_def

                    // #schema_impl

                    // #instance_impl
                };

                virtual_structs.push((variant_struct_name, full_impl));
            }

            // Generate virtual struct for named fields variants
            Fields::Named(fields) => {
                // Create field definitions
                let field_defs = fields.named.iter().map(|field| {
                    let field_name = field.ident.as_ref().unwrap();
                    let field_ty = &field.ty;
                    quote! {
                        pub #field_name: #field_ty
                    }
                });

                // Generate the struct definition
                let struct_def = quote! {
                    /// Generated virtual struct for enum variant
                    #[derive(Debug, Clone, terminusdb_schema_derive::TerminusDBModel, terminusdb_schema_derive::FromTDBInstance)]
                    #[allow(non_camel_case_types)]
                    struct #variant_struct_ident {
                        #(#field_defs),*
                    }
                };

                // Create custom options for the variant struct
                let variant_opts = TDBModelOpts {
                    class_name: Some(variant_struct_name.clone()),
                    base: parent_opts.base.clone(),
                    key: Some("value_hash".to_string()), // Use ValueHash as default for virtual structs
                    abstract_class: None,
                    unfoldable: None,
                    inherits: None,
                    doc: Some(format!(
                        "Virtual struct for {} enum variant {}",
                        enum_name, variant_name
                    )),
                    original_input: None,
                    // todo: should this be configurable?
                    id_field: None,
                    rename_all: None,
                };

                // Process the struct fields to generate instance conversions
                let dummy_fields = syn::FieldsNamed {
                    brace_token: Default::default(),
                    named: fields.named.clone(),
                };

                // Collect field identifiers for to_schema_tree_mut
                let field_idents = fields
                    .named
                    .iter()
                    .filter_map(|field| field.ident.as_ref().map(|ident| ident.clone()))
                    .collect::<Vec<_>>();

                let properties_token = process_named_fields(&dummy_fields, &variant_struct_ident);

                // Generate to_schema_tree implementation for the virtual struct
                let field_types = fields
                    .named
                    .iter()
                    .map(|field| {
                        let field_ty = &field.ty;
                        field_ty
                    })
                    .collect::<Vec<_>>();

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

                                // Process field types, preventing recursion by checking collection
                                #(
                                    for field_schema in <#field_types as terminusdb_schema::ToTDBSchema>::to_schema_tree() {
                                        if !collection.iter().any(|s| s.class_name() == field_schema.class_name()) {
                                            collection.insert(field_schema);
                                        }
                                    }
                                )*
                            }
                        }
                    }
                };

                // Generate ToTDBSchema implementation for the virtual struct
                let schema_impl = generate_totdbschema_impl(
                    &variant_struct_ident,
                    &variant_struct_name,
                    &variant_opts,
                    properties_token,
                    quote! { SchemaTypeClass },
                    to_schema_tree_impl,
                );

                // Generate field conversions for ToTDBInstance
                let field_conversions = fields
                    .named
                    .iter()
                    .map(|field| {
                        let field_name = field.ident.as_ref().unwrap();
                        let field_name_str = field_name.to_string();

                        quote! {
                            properties.insert(
                                #field_name_str.to_string(),
                                <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                                    self.#field_name.clone(),
                                    #field_name_str,
                                    &schema
                                )
                            );
                        }
                    })
                    .collect::<Vec<_>>();

                // Generate ToTDBInstance implementation for the virtual struct
                let instance_impl = quote! {
                    impl terminusdb_schema::ToTDBInstance for #variant_struct_ident {
                        fn to_instance(&self, id: Option<String>) -> terminusdb_schema::Instance {
                            let schema = <Self as terminusdb_schema::ToTDBSchema>::to_schema();

                            // Create a BTreeMap for properties
                            let mut properties = std::collections::BTreeMap::new();

                            // Convert each field to an InstanceProperty
                            #(#field_conversions)*

                            terminusdb_schema::Instance {
                                schema,
                                id,
                                capture: false,
                                ref_props: true,
                                properties,
                            }
                        }
                    }

                    impl terminusdb_schema::ToTDBInstances for #variant_struct_ident {
                        fn to_instance_tree(&self) -> Vec<terminusdb_schema::Instance> {
                            let instance = self.to_instance(None);
                            terminusdb_schema::build_instance_tree(&instance)
                        }
                    }
                };

                // Combine the struct definition and implementations
                let full_impl = quote! {
                    #struct_def

                    // #schema_impl
                    //
                    // #instance_impl
                };

                virtual_structs.push((variant_struct_name, full_impl));
            }
        }
    }

    virtual_structs
}

// todo: when an enum DOES have tags, it is a union
// in that case it has to derive a terminusdb_schema::Schema::TaggedUnion

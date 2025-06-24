use crate::prelude::*;
use quote::format_ident;
// No longer need DataEnum here
// use syn::DataEnum;

/// Generate implementation for ToTDBInstance trait for both structs and enums.
/// Assumes the `instance_body_code` contains the complete logic for the `to_instance` method body.
pub fn generate_totdbinstance_impl(
    type_name: &syn::Ident,
    instance_body_code: proc_macro2::TokenStream, // Renamed from fields_code
    // struct/enum level derive arguments
    args: TDBModelOpts,
    // Removed data_enum_opt argument
) -> proc_macro2::TokenStream {
    let optid = match args.id_field.as_ref() {
        None => {quote!{None::<String>}}
        Some(field_name) => {
            let ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            quote!{
                Some( self.#ident.clone().into() )
            }
        }
    };
    
    // This now simply wraps the provided instance_body_code within the impl
    quote! {
        impl terminusdb_schema::ToTDBInstance for #type_name {
            fn to_instance(&self, id: Option<String>) -> terminusdb_schema::Instance {
                // Get schema info needed by some body implementations
                let schema = <Self as terminusdb_schema::ToTDBSchema>::to_schema();
                // Get optional ID from struct field if configured
                let optid_val = #optid;
                
                // The entire logic for generating the instance is now contained here
                #instance_body_code
            }
        }
        
        // Use the helper function from the traits module
        impl terminusdb_schema::ToTDBInstances for #type_name {
            fn to_instance_tree(&self) -> Vec<terminusdb_schema::Instance> {
                let instance = self.to_instance(None);
                terminusdb_schema::build_instance_tree(&instance)
            }
        }
    }
}

/// Process struct fields to convert them to InstanceProperty values
/// (Used to generate part of the instance_body_code for structs)
pub fn process_fields_for_instance(
    fields_named: &syn::FieldsNamed,
    _struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let field_conversions = fields_named.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let field_opts = TDBFieldOpts::from_field(field).unwrap();
        let property_name = field_opts.name.unwrap_or_else(|| field_name.to_string());
        
        // Extract subdocument value before quote
        let subdocument = field_opts.subdocument;

        /*

        // Generate code to check if the field type is a simple enum
        let enum_check_code = quote! {
            // Check if the type of this field corresponds to a Schema::Enum
            // This uses the ToTDBSchema trait bound on the field's type T
            if let Some(field_schema) = <#field_ty as terminusdb_schema::ToMaybeTDBSchema>::to_schema() {
                if let terminusdb_schema::Schema::Enum {..} = field_schema {
                    // It's an enum, serialize as string
                    properties.insert(
                        #property_name.to_string(),
                        terminusdb_schema::InstanceProperty::Primitive(
                            terminusdb_schema::PrimitiveValue::String(format!("{:?}", self.#field_name))
                        )
                    );
                } else {
                    // Not an enum, use the default ToInstanceProperty
                    properties.insert(
                        #property_name.to_string(),
                        <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                            self.#field_name.clone(),
                            &#property_name,
                            &schema // 'schema' is the schema of the containing struct
                        )
                    );
                }
            } else {
                 // No schema found for this type, assume it's a relation or needs default handling
                 properties.insert(
                    #property_name.to_string(),
                    <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                        self.#field_name.clone(),
                        &#property_name,
                        &schema
                    )
                );
            }
        };

        // Use the conditional code generation
        enum_check_code

         */

        quote! {
            properties.insert(
                #property_name.to_string(),
                <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                    self.#field_name.clone(),
                    &#property_name,
                    &schema
                )
            );
        }

    }).collect::<Vec<_>>();
    
    quote! {
        #(#field_conversions)*
    }
}

/// Process enum variants for simple (unit-only) enums
pub fn process_enum_variants_for_instance(
    data_enum: &syn::DataEnum,
    enum_name: &syn::Ident,
    rename_strategy: crate::args::RenameStrategy,
) -> proc_macro2::TokenStream {
    let variants = data_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string();
        let renamed_variant = rename_strategy.apply(&variant_name_str);
        
        quote! {
            #enum_name::#variant_ident => {
                properties.insert(
                    #renamed_variant.to_string(),
                    terminusdb_schema::InstanceProperty::Primitive(
                        terminusdb_schema::PrimitiveValue::Unit
                    )
                );
            }
        }
    }).collect::<Vec<_>>();
    
    quote! {
        match self {
            #(#variants)*
        }
    }
}

/// Process tagged union enum variants
pub fn process_tagged_enum_for_instance(
    data_enum: &syn::DataEnum,
    enum_name: &syn::Ident,
    rename_strategy: crate::args::RenameStrategy,
) -> proc_macro2::TokenStream {
    let variants = data_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string();
        let renamed_variant = rename_strategy.apply(&variant_name_str);
        
        match &variant.fields {
            Fields::Unit => {
                // Unit variant
                quote! {
                    #enum_name::#variant_ident => {
                        properties.insert(
                            #renamed_variant.to_string(),
                            terminusdb_schema::InstanceProperty::Primitive(
                                terminusdb_schema::PrimitiveValue::Unit
                            )
                        );
                    }
                }
            },
            Fields::Unnamed(fields_unnamed) => {
                // Tuple variant
                if fields_unnamed.unnamed.len() == 1 {
                    // Single field tuple variant
                    quote! {
                        #enum_name::#variant_ident(value) => {
                            properties.insert(
                                #renamed_variant.to_string(),
                                <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                                    value.clone(),
                                    #renamed_variant,
                                    &schema
                                )
                            );
                        }
                    }
                } else {
                    // Multi-field tuple variant (not really supported well in TerminusDB)
                    let field_indices: Vec<syn::Index> = (0..fields_unnamed.unnamed.len())
                        .map(syn::Index::from)
                        .collect();
                    
                    // Generate variable names like field_0, field_1, etc.
                    let field_vars = field_indices.iter().enumerate()
                        .map(|(i, _)| format_ident!("field_{}", i))
                        .collect::<Vec<_>>();
                    
                    quote! {
                        #enum_name::#variant_ident(#(ref #field_vars),*) => {
                            properties.insert(
                                #renamed_variant.to_string(),
                                terminusdb_schema::InstanceProperty::Primitive(
                                    terminusdb_schema::PrimitiveValue::String(
                                        format!("Complex variant data not supported directly: {}", #renamed_variant)
                                    )
                                )
                            );
                        }
                    }
                }
            },
            Fields::Named(fields_named) => {
                // Struct variant
                // Collect field names into a Vec to avoid the moved value error
                let field_names: Vec<_> = fields_named.named.iter()
                    .map(|field| field.ident.as_ref().expect("Named fields should have identifiers"))
                    .collect();
                
                quote! {
                    #enum_name::#variant_ident { #(ref #field_names),* } => {
                        // Create a sub-instance for the struct variant
                        let mut sub_properties = std::collections::BTreeMap::new();
                        
                        #(
                            sub_properties.insert(
                                stringify!(#field_names).to_string(),
                                <_ as terminusdb_schema::ToInstanceProperty<Self>>::to_property(
                                    #field_names.clone(),
                                    stringify!(#field_names),
                                    &schema
                                )
                            );
                        )*
                        
                        properties.insert(
                            #renamed_variant.to_string(),
                            terminusdb_schema::InstanceProperty::Relation(
                                terminusdb_schema::RelationValue::One(
                                    terminusdb_schema::Instance {
                                        schema: terminusdb_schema::Schema::Class {
                                            id: format!("{}_{}", Self::schema_name(), #renamed_variant),
                                            base: None,
                                            key: terminusdb_schema::Key::ValueHash,
                                            documentation: None,
                                            subdocument: true,
                                            r#abstract: false,
                                            inherits: vec![],
                                            unfoldable: false,
                                            properties: vec![],
                                        },
                                        id: None,
                                        capture: false,
                                        ref_props: true,
                                        properties: sub_properties,
                                    }
                                )
                            )
                        );
                    }
                }
            }
        }
    }).collect::<Vec<_>>();
    
    quote! {
        match self {
            #(#variants)*
        }
    }
}

// --- Helper functions for generating instance_body_code ---

/// Generates the instance body logic for an abstract tagged union.
/// This creates a match statement that calls `to_instance` on the inner value.
pub fn generate_abstract_tagged_union_instance_logic(
    data_enum: &syn::DataEnum,
    type_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut match_arms = Vec::new(); // Initialize vector for match arms
                    
    for variant in data_enum.variants.iter() {
        let variant_ident = &variant.ident;
        let arm = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                // Arm for single unnamed field: call inner_value.to_instance
                quote! {
                    #type_name::#variant_ident(inner_value) => {
                        // 'id' here is the Option<String> argument to the outer to_instance function
                        // The caller `impl` must provide `id` in its scope.
                        inner_value.to_instance(id.clone())
                    }
                }
            },
            _ => {
                // Arm for other variants: panic
                quote! {
                    #type_name::#variant_ident { .. } => {
                        panic!("Abstract tagged union variant '{}' must have exactly one unnamed field", stringify!(#variant_ident));
                    }
                }
            }
        };
        match_arms.push(arm); // Add generated arm to vector
    }

    // Generate the match expression TokenStream
    quote! {
        match self {
            #(#match_arms),*
        }
    }
} 
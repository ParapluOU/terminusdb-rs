use darling::FromField;
use proc_macro2;
use quote::{quote, format_ident};
use syn::{self, Data, DataEnum, DataStruct, Fields, FieldsNamed, Visibility};
use syn::spanned::Spanned;
use anyhow::Context;

use crate::args::TDBFieldOpts;
use anyhow::*;

/// Enum type for different kinds of enums
enum EnumType {
    Simple,
    TaggedUnion,
}

/// Entry point for the FromTDBInstance derive macro
pub(crate) fn derive_from_terminusdb_instance(input: syn::DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
    let struct_name = &input.ident;

    let result = match &input.data {
        Data::Struct(data_struct) => {
            match &data_struct.fields {
                Fields::Named(fields_named) => {
                    // Generate implementation for struct with named fields
                    implement_from_instance_for_struct(struct_name, data_struct)
                }
                _ => {
                    return Err(syn::Error::new(proc_macro2::Span::call_site(),
                                               "FromTDBInstance derive macro only supports structs with named fields"));
                }
            }
        }
        Data::Enum(data_enum) => {
            // Generate implementation for enums
            match detect_enum_type(data_enum) {
                EnumType::Simple => implement_from_instance_for_simple_enum(struct_name, data_enum),
                EnumType::TaggedUnion => implement_from_instance_for_tagged_enum(struct_name, data_enum),
            }
        }
        _ => {
            return Err(syn::Error::new(proc_macro2::Span::call_site(),
                                       "FromTDBInstance derive macro only supports structs and enums"));
        }
    };

    Result::Ok(result)
}

/// Detect the type of enum (simple enum with only unit variants or tagged union)
fn detect_enum_type(data_enum: &DataEnum) -> EnumType {
    let has_only_unit_variants = data_enum.variants.iter().all(|variant| {
        match &variant.fields {
            Fields::Unit => true,
            _ => false,
        }
    });

    if has_only_unit_variants {
        EnumType::Simple
    } else {
        EnumType::TaggedUnion
    }
}

/// Generate implementation for FromTDBInstance trait for structs
fn implement_from_instance_for_struct(
    struct_name: &syn::Ident,
    data_struct: &DataStruct,
) -> proc_macro2::TokenStream {
    // Process named fields
    let (fields_code, field_names) = match &data_struct.fields {
        Fields::Named(fields_named) => {
            process_named_fields_for_deserialization(fields_named, struct_name)
        }
        _ => {
            return syn::Error::new(proc_macro2::Span::call_site(),
                                   "FromTDBInstance derive macro only supports structs with named fields")
                .to_compile_error();
        }
    };

    // Generate the implementation
    quote! {
        impl terminusdb_schema::FromTDBInstance for #struct_name {
            fn from_instance(instance: &terminusdb_schema::Instance) -> Result<Self, anyhow::Error> {
                use anyhow::*;

                // Check that the schema type matches
                if !instance.is_of_type::<Self>() {
                    return Err(anyhow::anyhow!("Instance type mismatch, expected {}", stringify!(#struct_name)));
                }
                
                #fields_code
                
                Result::Ok(Self {
                    #(#field_names),*
                })
            }
            
            fn from_instance_tree(instances: &[terminusdb_schema::Instance]) -> Result<Self, anyhow::Error> {
                if instances.is_empty() {
                    return Err(anyhow::anyhow!("Empty instance tree"));
                }
                
                // Find the root instance with the matching type
                for instance in instances {
                    if instance.is_of_type::<Self>() {
                        return Self::from_instance(instance);
                    }
                }
                
                Err(anyhow::anyhow!("No instance with type {} found in tree", stringify!(#struct_name)))
            }
        }
    }
}

/// Process named fields for deserialization
fn process_named_fields_for_deserialization<'a>(
    fields_named: &'a FieldsNamed,
    struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, Vec<&'a syn::Ident>) {
    let field_names = fields_named.named.iter()
        .map(|field| field.ident.as_ref().expect("Named fields should have an identifier"))
        .collect::<Vec<_>>();

    let field_parsers = fields_named.named.iter().map(|field| {
        let field_ident = field.ident.as_ref().expect("Named fields should have an identifier");
        let field_name_str = field_ident.to_string();

        // Parse field options using darling
        let field_opts = match TDBFieldOpts::from_field(field) {
            Result::Ok(opts) => opts,
            Result::Err(err) => {
                return syn::Error::new(field.span(), err.to_string()).to_compile_error();
            }
        };

        // Use custom name if provided
        let property_name = field_opts.name.unwrap_or_else(|| field_name_str.clone());
        let field_ty = &field.ty;

        // ALWAYS use from_maybe_property, passing the Option<&InstanceProperty> directly.
        // The trait implementation for T or Option<T> will handle missing/null/present cases.
        quote! {
            let #field_ident = <#field_ty as terminusdb_schema::FromInstanceProperty>::from_maybe_property(
                &instance.get_property(#property_name).cloned()
            ).with_context(|| format!("Failed to deserialize field '{}' for struct '{}'", #property_name, stringify!(#struct_name)))?;
        }
    }).collect::<Vec<_>>();

    let field_parsers_token = quote! {
        #(#field_parsers)*
    };

    (field_parsers_token, field_names)
}

/// Check if a type is an Option<T>
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        if typepath.path.segments.len() == 1 {
            let segment = &typepath.path.segments[0];
            return segment.ident == "Option";
        }
    }
    false
}

/// Extract the inner type T from Option<T>
fn extract_inner_type_from_option(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::Path(typepath) = ty {
        if let Some(segment) = typepath.path.segments.first() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return inner_type;
                    }
                }
            }
        }
    }
    // If we can't extract the inner type, return the original type
    ty
}

/// Check if a type is a TdbLazy field
fn is_tdblazy_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(typepath) = ty {
        if typepath.path.segments.len() == 1 {
            let segment = &typepath.path.segments[0];
            return segment.ident == "TdbLazy";
        }
    }
    false
}

/// Extract the inner type T from TdbLazy<T>
fn extract_inner_type_from_tdblazy(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::Path(typepath) = ty {
        if let syn::PathArguments::AngleBracketed(args) = &typepath.path.segments[0].arguments {
            if let syn::GenericArgument::Type(inner_ty) = &args.args[0] {
                return inner_ty;
            }
        }
    }
    panic!("Expected TdbLazy<T> type");
}

/// Generate implementation for FromTDBInstance trait for simple enums
fn implement_from_instance_for_simple_enum(
    enum_name: &syn::Ident,
    data_enum: &DataEnum,
) -> proc_macro2::TokenStream {
    let variant_matchers = data_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string().to_lowercase(); // Using lowercase for consistency

        quote! {
            if instance.properties.contains_key(#variant_name_str) {
                return Result::Ok(#enum_name::#variant_ident);
            }
        }
    }).collect::<Vec<_>>();

    quote! {
        impl terminusdb_schema::FromTDBInstance for #enum_name {
            fn from_instance(instance: &terminusdb_schema::Instance) -> Result<Self, anyhow::Error> {
                // Check that the schema is an Enum and matches the expected type
                match &instance.schema {
                    terminusdb_schema::Schema::Enum { id, .. } if id == stringify!(#enum_name) => {
                        // Check which variant is present
                        #(#variant_matchers)*
                        
                        Err(anyhow::anyhow!("No matching variant found for enum {}", stringify!(#enum_name)))
                    },
                    _ => Err(anyhow::anyhow!("Instance is not an enum or type does not match, expected {}", stringify!(#enum_name))),
                }
            }

            fn from_instance_tree(instances: &[terminusdb_schema::Instance]) -> Result<Self, anyhow::Error> {
                if instances.is_empty() {
                    return Err(anyhow::anyhow!("Empty instance tree"));
                }
                
                // Find the root instance with the matching type
                for instance in instances {
                    if let terminusdb_schema::Schema::Enum { id, .. } = &instance.schema {
                        if id == stringify!(#enum_name) {
                            return Self::from_instance(instance);
                        }
                    }
                }
                
                Err(anyhow::anyhow!("No instance with type {} found in tree", stringify!(#enum_name)))
            }
        }
    }
}

/// Generate implementation for FromTDBInstance trait for tagged union enums
fn implement_from_instance_for_tagged_enum(
    enum_name: &syn::Ident,
    data_enum: &DataEnum,
) -> proc_macro2::TokenStream {
    let variant_matchers = data_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string().to_lowercase(); // Using lowercase for variant property names
        let variant_name_cap = variant_ident.to_string(); // Original capitalized variant name

        match &variant.fields {
            Fields::Unit => {
                // Unit variant
                quote! {
                    if instance.properties.contains_key(#variant_name_str) {
                        if let Some(prop) = instance.properties.get(#variant_name_str) {
                            if let terminusdb_schema::InstanceProperty::Primitive(terminusdb_schema::PrimitiveValue::Unit) = prop {
                                return Result::Ok(#enum_name::#variant_ident);
                            }
                        }
                    }
                }
            }
            Fields::Unnamed(fields_unnamed) => {
                // Tuple variant
                let token_stream = if fields_unnamed.unnamed.len() == 1 {
                    // Single field tuple variant
                    let field_ty = &fields_unnamed.unnamed[0].ty;

                    // Use compile-time primitive detection with MaybeIsPrimitive trait
                    quote! {
                        if let Some(prop) = instance.properties.get(#variant_name_str) {
                            if <#field_ty as terminusdb_schema::MaybeIsPrimitive>::is_primitive() {
                                // Direct primitive deserialization for primitive types
                                match <#field_ty as terminusdb_schema::FromInstanceProperty>::from_property(prop) {
                                    Result::Ok(value) => return Result::Ok(#enum_name::#variant_ident(value)),
                                    Err(e) => return Err(anyhow::anyhow!("Failed to deserialize variant {}: {}", #variant_name_str, e)),
                                }
                            } else {
                                // Complex type deserialization
                                match <#field_ty as terminusdb_schema::MaybeFromTDBInstance>::maybe_from_property(prop)? {
                                    Some(value) => return Result::Ok(#enum_name::#variant_ident(value)),
                                    None => return Err(anyhow::anyhow!("Failed to deserialize complex variant {}", #variant_name_str)),
                                }
                            }
                        }
                        // Also check capitalized variant name
                        if let Some(prop) = instance.properties.get(#variant_name_cap) {
                            if <#field_ty as terminusdb_schema::MaybeIsPrimitive>::is_primitive() {
                                // Direct primitive deserialization for primitive types
                                match <#field_ty as terminusdb_schema::FromInstanceProperty>::from_property(prop) {
                                    Result::Ok(value) => return Result::Ok(#enum_name::#variant_ident(value)),
                                    Err(e) => return Err(anyhow::anyhow!("Failed to deserialize variant {}: {}", #variant_name_cap, e)),
                                }
                            } else {
                                // Complex type deserialization
                                match <#field_ty as terminusdb_schema::MaybeFromTDBInstance>::maybe_from_property(prop)? {
                                    Some(value) => return Result::Ok(#enum_name::#variant_ident(value)),
                                    None => return Err(anyhow::anyhow!("Failed to deserialize complex variant {}", #variant_name_cap)),
                                }
                            }
                        }
                    }
                } else {
                    // Multi-field tuple variant (not supported well in TerminusDB)
                    quote! {
                        if instance.properties.contains_key(#variant_name_str) {
                            return Err(anyhow::anyhow!("Complex tuple variants not fully supported: {}", #variant_name_str));
                        }
                    }
                };

                token_stream
            }
            Fields::Named(fields_named) => {
                // Struct variant
                let field_names: Vec<_> = fields_named.named.iter()
                    .map(|field| field.ident.as_ref().expect("Named fields should have identifiers"))
                    .collect();

                let field_types: Vec<_> = fields_named.named.iter()
                    .map(|field| &field.ty)
                    .collect();

                let field_strings: Vec<_> = field_names.iter()
                    .map(|name| name.to_string())
                    .collect();

                let field_parsers = field_names.iter().zip(field_types.iter()).zip(field_strings.iter())
                    .map(|((field_name, field_type), field_string)| {
                        // Use compile-time primitive detection with MaybeIsPrimitive trait
                        quote! {
                            let #field_name = match sub_instance.get_property(#field_string) {
                                Some(field_prop) => {
                                    if <#field_type as terminusdb_schema::MaybeIsPrimitive>::is_primitive() {
                                        // Direct primitive deserialization for primitive types
                                        match <#field_type as terminusdb_schema::FromInstanceProperty>::from_property(field_prop) {
                                            Result::Ok(value) => value,
                                            Err(e) => return Err(anyhow::anyhow!("Failed to deserialize field {} in variant {}: {}", #field_string, #variant_name_str, e)),
                                        }
                                    } else {
                                        // Complex type deserialization
                                        match <#field_type as terminusdb_schema::MaybeFromTDBInstance>::maybe_from_property(field_prop)? {
                                            Some(value) => value,
                                            None => return Err(anyhow::anyhow!("Failed to deserialize complex field {} in variant {}", #field_string, #variant_name_str)),
                                        }
                                    }
                                },
                                None => return Err(anyhow::anyhow!("Field {} missing in variant {}", #field_string, #variant_name_str)),
                            };
                        }
                    })
                    .collect::<Vec<_>>();

                quote! {
                    if let Some(prop) = instance.properties.get(#variant_name_str) {
                        if let terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(sub_instance)) = prop {
                            #(#field_parsers)*
                            
                            return Result::Ok(#enum_name::#variant_ident { #(#field_names),* });
                        }
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

    quote! {
        impl terminusdb_schema::FromTDBInstance for #enum_name {
            fn from_instance(instance: &terminusdb_schema::Instance) -> Result<Self, anyhow::Error> {
                // Check that the schema is a TaggedUnion and matches the expected type
                match &instance.schema {
                    terminusdb_schema::Schema::TaggedUnion { id, .. } if id == stringify!(#enum_name) => {
                        // Check each variant
                        #(#variant_matchers)*
                        
                        Err(anyhow::anyhow!("No matching variant found for tagged union {}", stringify!(#enum_name)))
                    },
                    _ => Err(anyhow::anyhow!("Instance is not a tagged union or type does not match, expected {}", stringify!(#enum_name))),
                }
            }

            fn from_instance_tree(instances: &[terminusdb_schema::Instance]) -> Result<Self, anyhow::Error> {
                if instances.is_empty() {
                    return Err(anyhow::anyhow!("Empty instance tree"));
                }
                
                // Find the root instance with the matching type
                for instance in instances {
                    if let terminusdb_schema::Schema::TaggedUnion { id, .. } = &instance.schema {
                        if id == stringify!(#enum_name) {
                            return Self::from_instance(instance);
                        }
                    }
                }
                
                Err(anyhow::anyhow!("No instance with type {} found in tree", stringify!(#enum_name)))
            }
        }
    }
}

 
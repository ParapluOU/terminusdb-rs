use crate::args::{TDBFieldOpts, TDBModelOpts};
use crate::prelude::*;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DataEnum, DeriveInput, Field, Fields, FieldsNamed, GenericArgument,
    Ident, Path, PathArguments, Type, TypePath,
};

/// Enum type for different kinds of enums
enum EnumType {
    Simple,
    TaggedUnion,
}

/// Detect the type of enum (simple enum with only unit variants or tagged union)
fn detect_enum_type(data_enum: &DataEnum) -> EnumType {
    let has_only_unit_variants = data_enum
        .variants
        .iter()
        .all(|variant| match &variant.fields {
            Fields::Unit => true,
            _ => false,
        });

    if has_only_unit_variants {
        EnumType::Simple
    } else {
        EnumType::TaggedUnion
    }
}

/// Derives the `InstanceFromJson` trait implementation.
pub fn derive_instance_from_json_impl(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let type_name = &input.ident;
    let opts = TDBModelOpts::from_derive_input(&input)?;

    match &input.data {
        Data::Struct(data_struct) => {
            // Handle struct implementation
            implement_instance_from_json_for_struct(
                type_name, 
                data_struct, 
                &opts,
                #[cfg(feature = "generic-derive")]
                &input.generics,
            )
        }
        Data::Enum(data_enum) => {
            // Determine if this is a simple enum or a tagged union
            let enum_type = detect_enum_type(data_enum);

            match enum_type {
                EnumType::Simple => {
                    implement_instance_from_json_for_simple_enum(type_name, data_enum, &opts)
                }
                EnumType::TaggedUnion => {
                    implement_instance_from_json_for_tagged_enum(type_name, data_enum, &opts)
                }
            }
        }
        Data::Union(_) => Err(syn::Error::new_spanned(
            input,
            "InstanceFromJson cannot be derived for unions",
        )),
    }
}

/// Implements InstanceFromJson for struct types
fn implement_instance_from_json_for_struct(
    struct_name: &Ident,
    data_struct: &syn::DataStruct,
    opts: &TDBModelOpts,
    #[cfg(feature = "generic-derive")]
    generics: &syn::Generics,
) -> Result<TokenStream, syn::Error> {
    // Extract generic parameters
    #[cfg(feature = "generic-derive")]
    let (impl_generics, ty_generics, where_clause) = {
        if !generics.params.is_empty() {
            let (syn_impl_generics, syn_ty_generics, syn_where_clause) = generics.split_for_impl();
            (quote! { #syn_impl_generics }, quote! { #syn_ty_generics }, syn_where_clause.cloned())
        } else {
            (quote!{}, quote!{}, None)
        }
    };
    
    #[cfg(not(feature = "generic-derive"))]
    let (impl_generics, ty_generics, where_clause) = (quote!{}, quote!{}, None::<syn::WhereClause>);
    // For generics, we need to use the schema name which includes generic parameters
    let expected_type_name_expr = {
        #[cfg(feature = "generic-derive")]
        {
            if !generics.params.is_empty() {
                quote! { <#struct_name #ty_generics as terminusdb_schema::ToTDBSchema>::schema_name() }
            } else {
                let static_name = opts
                    .class_name
                    .clone()
                    .unwrap_or_else(|| struct_name.to_string());
                quote! { #static_name }
            }
        }
        #[cfg(not(feature = "generic-derive"))]
        {
            let static_name = opts
                .class_name
                .clone()
                .unwrap_or_else(|| struct_name.to_string());
            quote! { #static_name }
        }
    };

    let fields = match &data_struct.fields {
        Fields::Named(fields) => fields,
        _ => {
            return Err(syn::Error::new_spanned(
                data_struct.fields.clone(),
                "InstanceFromJson requires named fields",
            ))
        }
    };

    let field_deserializers =
        generate_field_deserializers(fields, struct_name, opts, &ty_generics)?;

    let expanded = quote! {
        impl #impl_generics terminusdb_schema::json::InstanceFromJson for #struct_name #ty_generics #where_clause {
            #[allow(unused_variables)] // json_map might be unused if struct has no fields
            fn instance_from_json(json: serde_json::Value) -> ::core::result::Result<terminusdb_schema::Instance, anyhow::Error> {
                use terminusdb_schema::{Instance, InstanceProperty, ToTDBInstance, Schema, ToTDBSchema};
                use terminusdb_schema::json::{InstanceFromJson, InstancePropertyFromJson};
                use serde_json::{Value, Map};
                use std::collections::HashMap;
                use anyhow::{Context, anyhow, Result};
                use std::convert::TryInto;
                use std::collections::BTreeMap;

                let mut json_map = match json {
                    Value::Object(map) => map,
                    _ => return Err(anyhow!("Expected a JSON object for instance deserialization, found {:?}", json)),
                };

                // Extract @id
                // Use remove to take ownership, simplifying later property iteration
                let id = json_map.remove("@id")
                    .and_then(|v| v.as_str().map(String::from));

                // Extract and verify @type
                let type_name = json_map.remove("@type")
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| anyhow!("Missing or invalid '@type' field in JSON instance"))?;

                let expected_type_name = #expected_type_name_expr;
                if type_name != expected_type_name {
                     return Err(anyhow!("Mismatched '@type': expected '{}', found '{}'", expected_type_name, type_name));
                }

                let mut _properties: BTreeMap<String, InstanceProperty> = BTreeMap::new();

                #field_deserializers

                ::core::result::Result::Ok(Instance {
                    id,
                    schema: <#struct_name #ty_generics as ToTDBSchema>::to_schema(),
                    capture: false,
                    ref_props: false,
                    properties: _properties, // Use filled properties map
                })
            }
        }
    };

    Result::Ok(expanded)
}

/// Implements InstanceFromJson for simple enums (unit variants only)
fn implement_instance_from_json_for_simple_enum(
    enum_name: &Ident,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> Result<TokenStream, syn::Error> {

    let variant_matchers = data_enum
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let variant_name_str = variant_ident.to_string().to_lowercase();

            quote! {
                if json_map.contains_key(#variant_name_str) {
                    json_map.remove(#variant_name_str);
                    let mut properties = std::collections::BTreeMap::new();
                    properties.insert(
                        #variant_name_str.to_string(),
                        terminusdb_schema::InstanceProperty::Primitive(
                            terminusdb_schema::PrimitiveValue::Unit
                        )
                    );
                    return ::core::result::Result::Ok(terminusdb_schema::Instance {
                        id,
                        schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                        capture: false,
                        ref_props: false,
                        properties,
                    });
                }
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        impl terminusdb_schema::json::InstanceFromJson for #enum_name {
            fn instance_from_json(json: serde_json::Value) -> ::core::result::Result<terminusdb_schema::Instance, anyhow::Error> {
                use terminusdb_schema::{Instance, InstanceProperty, PrimitiveValue, Schema, ToTDBSchema};
                use serde_json::{Value, Map};
                use anyhow::{Context, anyhow, Result};
                use std::collections::BTreeMap;

                // Handle both object format ({"@type": "Status", "active": null}) and string format ("active")
                let mut json_map = match json {
                    Value::String(variant_name) => {
                        // Direct string deserialization for standalone enums
                        let variant_lower = variant_name.to_lowercase();
                        let mut properties = std::collections::BTreeMap::new();
                        properties.insert(
                            variant_lower.clone(),
                            terminusdb_schema::InstanceProperty::Primitive(
                                terminusdb_schema::PrimitiveValue::Unit
                            )
                        );
                        return ::core::result::Result::Ok(terminusdb_schema::Instance {
                            id: None,
                            schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                            capture: false,
                            ref_props: false,
                            properties,
                        });
                    },
                    Value::Object(map) => map,
                    _ => return Err(anyhow!("Expected a JSON object or string for enum deserialization, found {:?}", json)),
                };

                // Extract @id
                let id = json_map.remove("@id")
                    .and_then(|v| v.as_str().map(String::from));

                // Extract and verify @type
                let type_name = json_map.remove("@type")
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| anyhow!("Missing or invalid '@type' field in JSON instance"))?;

                let expected_type_name = <#enum_name as terminusdb_schema::ToTDBSchema>::schema_name();
                if type_name != expected_type_name {
                    return Err(anyhow!("Mismatched '@type': expected '{}', found '{}'", expected_type_name, type_name));
                }

                // Check each variant
                #(#variant_matchers)*

                Err(anyhow!("No valid enum variant found in JSON"))
            }
        }
    };

    Result::Ok(expanded)
}

/// Implements InstanceFromJson for tagged union enums
fn implement_instance_from_json_for_tagged_enum(
    enum_name: &Ident,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> Result<TokenStream, syn::Error> {

    // Build a list of type checks for variant-to-union deserialization
    // We only generate type checks for variants where the inner type is likely a custom struct type
    // (not a primitive like String, i32, bool, or standard types)
    let variant_type_checks = data_enum.variants.iter().filter_map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string();
        let variant_name_lower = variant_name_str.to_lowercase();

        // Only handle single-field variants (newtype variants with a named type)
        if let Fields::Unnamed(fields) = &variant.fields {
            if fields.unnamed.len() == 1 {
                let field_ty = &fields.unnamed[0].ty;

                // Check if this is likely a custom type (not a primitive or standard type)
                // We do this by checking if the type is a simple Path (not Option, Vec, etc.)
                // and doesn't start with a known primitive prefix
                let is_likely_custom_type = if let syn::Type::Path(type_path) = field_ty {
                    if let Some(segment) = type_path.path.segments.last() {
                        let type_name = segment.ident.to_string();
                        // Skip known primitives and standard types
                        !matches!(
                            type_name.as_str(),
                            "String" | "str" | "bool" | "i8" | "i16" | "i32" | "i64" | "i128" |
                            "u8" | "u16" | "u32" | "u64" | "u128" | "f32" | "f64" | "usize" | "isize" |
                            "Option" | "Vec" | "Box" | "Rc" | "Arc" | "XSDAnySimpleType"
                        )
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_likely_custom_type {
                    return Some(quote! {
                        // Check if @type matches this variant's inner type
                        if type_name == <#field_ty as terminusdb_schema::ToSchemaClass>::to_class() {
                            // The entire JSON object is the variant's value
                            // We need to re-insert @type and @id since they were removed
                            let mut variant_value_map = json_map.clone();
                            variant_value_map.insert("@type".to_string(), Value::String(type_name.clone()));
                            if let Some(ref id_val) = id {
                                variant_value_map.insert("@id".to_string(), Value::String(id_val.clone()));
                            }

                            // Wrap the variant value in the union property
                            json_map.clear();
                            json_map.insert(#variant_name_lower.to_string(), Value::Object(variant_value_map));

                            // Update type_name to match the union type
                            type_name = expected_type_name.clone();
                            is_variant_remapped = true;
                        }
                    });
                }
            }
        }
        None
    }).collect::<Vec<_>>();

    let variant_matchers = data_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name_str = variant_ident.to_string();
        let variant_name_lower = variant_name_str.to_lowercase();
        
        match &variant.fields {
            // Unit variant
            Fields::Unit => {
                quote! {
                    if json_map.contains_key(#variant_name_lower) {
                        json_map.remove(#variant_name_lower);
                        let mut properties = std::collections::BTreeMap::new();
                        properties.insert(
                            #variant_name_lower.to_string(),
                            terminusdb_schema::InstanceProperty::Primitive(
                                terminusdb_schema::PrimitiveValue::Unit
                            )
                        );
                        return ::core::result::Result::Ok(terminusdb_schema::Instance {
                            id,
                            schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                            capture: false,
                            ref_props: false,
                            properties,
                        });
                    }
                }
            },
            // Single field variant (newtype)
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field_ty = &fields.unnamed[0].ty;
                quote! {
                    if let Some(value) = json_map.remove(#variant_name_lower) {
                        let property = <#field_ty as terminusdb_schema::json::InstancePropertyFromJson<#enum_name>>::property_from_json(value)
                            // .with_context(|| format!("Failed to deserialize variant '{}' for enum '{}'", #variant_name_lower, #expected_type_name))?;
                            .context("implement_instance_from_json_for_tagged_enum()")?;

                        let mut properties = std::collections::BTreeMap::new();
                        properties.insert(#variant_name_lower.to_string(), property);
                        
                        return ::core::result::Result::Ok(terminusdb_schema::Instance {
                            id,
                            schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                            capture: false,
                            ref_props: false,
                            properties,
                        });
                    }
                }
            },
            // Multi-field tuple variant
            Fields::Unnamed(fields) => {
                // Get the variant struct name (used for virtual structs)
                let variant_struct_name = format!("{}{}", enum_name, variant_name_str);
                
                quote! {
                    if let Some(Value::Array(nested_values)) = json_map.remove(#variant_name_lower) {
                        // Create a virtual struct instance representing the tuple variant
                        let mut sub_properties = std::collections::BTreeMap::new();
                        
                        // Process each field in the array
                        for (i, value) in nested_values.iter().enumerate() {
                            sub_properties.insert(
                                format!("_{}", i),
                                terminusdb_schema::deserialize_property(value.clone())?
                            );
                        }
                        
                        let mut properties = std::collections::BTreeMap::new();
                        properties.insert(
                            #variant_name_lower.to_string(),
                            terminusdb_schema::InstanceProperty::Relation(
                                terminusdb_schema::RelationValue::One(
                                    terminusdb_schema::Instance {
                                        schema: terminusdb_schema::Schema::Class {
                                            id: #variant_struct_name.to_string(),
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
                        
                        return ::core::result::Result::Ok(terminusdb_schema::Instance {
                            id,
                            schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                            capture: false,
                            ref_props: false,
                            properties,
                        });
                    }
                }
            },
            // Named fields variant
            Fields::Named(fields_named) => {
                // Get the variant struct name (used for virtual structs)
                let variant_struct_name = format!("{}{}", enum_name, variant_name_str);
                
                quote! {
                    if let Some(Value::Object(mut nested_map)) = json_map.remove(#variant_name_lower) {
                        let mut sub_properties = std::collections::BTreeMap::new();
                        
                        // Process each field in the object
                        for (key, value) in nested_map {
                            sub_properties.insert(
                                key,
                                terminusdb_schema::deserialize_property(value)?
                            );
                        }
                        
                        let mut properties = std::collections::BTreeMap::new();
                        properties.insert(
                            #variant_name_lower.to_string(),
                            terminusdb_schema::InstanceProperty::Relation(
                                terminusdb_schema::RelationValue::One(
                                    terminusdb_schema::Instance {
                                        schema: terminusdb_schema::Schema::Class {
                                            id: #variant_struct_name.to_string(),
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
                        
                        return ::core::result::Result::Ok(terminusdb_schema::Instance {
                            id,
                            schema: <#enum_name as terminusdb_schema::ToTDBSchema>::to_schema(),
                            capture: false,
                            ref_props: false,
                            properties,
                        });
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

    let expanded = quote! {
        impl terminusdb_schema::json::InstanceFromJson for #enum_name {
            fn instance_from_json(json: serde_json::Value) -> ::core::result::Result<terminusdb_schema::Instance, anyhow::Error> {
                use terminusdb_schema::{Instance, InstanceProperty, PrimitiveValue, Schema, Key, RelationValue, ToTDBSchema};
                use terminusdb_schema::json::{InstancePropertyFromJson};
                use serde_json::{Value, Map};
                use anyhow::{Context, anyhow, Result};
                use std::collections::BTreeMap;

                let mut json_map = match json {
                    Value::Object(map) => map,
                    _ => return Err(anyhow!("Expected a JSON object for tagged enum deserialization, found {:?}", json)),
                };

                // Extract @id
                let id = json_map.remove("@id")
                    .and_then(|v| v.as_str().map(String::from));

                // Extract and verify @type
                let mut type_name = json_map.remove("@type")
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| anyhow!("Missing or invalid '@type' field in JSON instance"))?;

                let expected_type_name = <#enum_name as terminusdb_schema::ToTDBSchema>::schema_name();
                let mut is_variant_remapped = false;

                // Check if @type matches any variant's inner type (variant-to-union deserialization)
                #(#variant_type_checks)*

                // Verify @type matches the union type (either originally or after remapping)
                if type_name != expected_type_name {
                    return Err(anyhow!("Mismatched '@type': expected '{}', found '{}'", expected_type_name, type_name));
                }

                // Check each variant
                #(#variant_matchers)*

                Err(anyhow!("No valid enum variant found in JSON for tagged union {}", <#enum_name as terminusdb_schema::ToTDBSchema>::schema_name()))
            }
        }
    };

    Result::Ok(expanded)
}

/// Generates the code snippets for deserializing each field from the JSON map.
fn generate_field_deserializers(
    fields: &FieldsNamed,
    struct_name: &Ident,
    opts: &TDBModelOpts,
    ty_generics: &proc_macro2::TokenStream,
) -> Result<TokenStream, syn::Error> {
    let mut deserializers: Vec<TokenStream> = Vec::new();

    for field in &fields.named {
        // Skip PhantomData fields - they don't need JSON deserialization
        if crate::prelude::is_phantom_data_type(&field.ty) {
            continue;
        }
        
        let field_ident = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field, "InstanceFromJson requires named fields")
        })?;
        let field_ty = &field.ty;
        let field_opts = TDBFieldOpts::from_field(field)?;
        let json_key_name = field_opts.name.unwrap_or_else(|| field_ident.to_string());

        // Check if this field is the id_field
        let is_id_field = opts.id_field.as_ref().map(|id_field| id_field == &field_ident.to_string()).unwrap_or(false);

        // Use property_from_maybe_json for all fields, letting the trait implementation
        // handle the differences between Option and non-Option types
        let deserializer = if is_id_field {
            // Special handling for id_field - use the extracted @id value
            quote_spanned! {field.span()=>
                // For id_field, create a Value::String from the extracted @id
                let json_value = id.as_ref().map(|id_str| Value::String(id_str.clone()));

                // Use property_from_maybe_json for the id field
                let _prop = <#field_ty as terminusdb_schema::json::InstancePropertyFromJson<#struct_name #ty_generics>>::property_from_maybe_json(
                    json_value.clone()
                )
                .context("generate_field_deserializers() - id_field");
                ;

                if let Err(ref e) = _prop {
                    ::tracing::error!("failed to deserialize id_field '{}' of type {}: {}. payload: {:#?}", #json_key_name, stringify!(#field_ty), e, json_value);
                }

                _properties.insert(#json_key_name.to_string(), _prop?);
            }
        } else {
            quote_spanned! {field.span()=>
                // let err = concat!("Failed to deserialize field '{}' for type '{}'", #json_key_name, #expected_type_name);

                let json_value = json_map.remove(#json_key_name);

                // Use property_from_maybe_json for all fields
                let _prop = <#field_ty as terminusdb_schema::json::InstancePropertyFromJson<#struct_name #ty_generics>>::property_from_maybe_json(
                    json_value.clone()
                )
                // .context(&err)?;
                .context("generate_field_deserializers()");
                ;

                if let Err(ref e) = _prop {
                    ::tracing::error!("failed to deserialize field '{}' of type {}: {}. payload: {:#?}", #json_key_name, stringify!(#field_ty), e, json_value);
                }

                _properties.insert(#json_key_name.to_string(), _prop?);
            }
        };

        deserializers.push(deserializer);
    }

    Result::Ok(quote! {
        #(#deserializers)*
    })
}

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
            implement_instance_from_json_for_struct(type_name, data_struct, &opts)
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
) -> Result<TokenStream, syn::Error> {
    let expected_type_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| struct_name.to_string());

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
        generate_field_deserializers(fields, struct_name, &expected_type_name, opts)?;

    let expanded = quote! {
        impl terminusdb_schema::json::InstanceFromJson for #struct_name {
            #[allow(unused_variables)] // json_map might be unused if struct has no fields
            fn instance_from_json(json: serde_json::Value) -> anyhow::Result<terminusdb_schema::Instance> {
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

                let expected_type_name = #expected_type_name;
                if type_name != expected_type_name {
                     return Err(anyhow!("Mismatched '@type': expected '{}', found '{}'", expected_type_name, type_name));
                }

                let mut _properties: BTreeMap<String, InstanceProperty> = BTreeMap::new();

                #field_deserializers

                Result::Ok(Instance {
                    id,
                    schema: <#struct_name as ToTDBSchema>::to_schema(),
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
    let expected_type_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| enum_name.to_string());

    let variant_matchers = data_enum
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let variant_name_str = variant_ident.to_string();

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
                    return Result::Ok(terminusdb_schema::Instance {
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
            fn instance_from_json(json: serde_json::Value) -> anyhow::Result<terminusdb_schema::Instance> {
                use terminusdb_schema::{Instance, InstanceProperty, PrimitiveValue, Schema, ToTDBSchema};
                use serde_json::{Value, Map};
                use anyhow::{Context, anyhow, Result};
                use std::collections::BTreeMap;

                let mut json_map = match json {
                    Value::Object(map) => map,
                    _ => return Err(anyhow!("Expected a JSON object for enum deserialization, found {:?}", json)),
                };

                // Extract @id
                let id = json_map.remove("@id")
                    .and_then(|v| v.as_str().map(String::from));

                // Extract and verify @type
                let type_name = json_map.remove("@type")
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| anyhow!("Missing or invalid '@type' field in JSON instance"))?;

                let expected_type_name = #expected_type_name;
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
    let expected_type_name = opts
        .class_name
        .clone()
        .unwrap_or_else(|| enum_name.to_string());

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
                        return Result::Ok(terminusdb_schema::Instance {
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
                        
                        return Result::Ok(terminusdb_schema::Instance {
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
                        
                        return Result::Ok(terminusdb_schema::Instance {
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
                        
                        return Result::Ok(terminusdb_schema::Instance {
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
            fn instance_from_json(json: serde_json::Value) -> anyhow::Result<terminusdb_schema::Instance> {
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
                let type_name = json_map.remove("@type")
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| anyhow!("Missing or invalid '@type' field in JSON instance"))?;

                let expected_type_name = #expected_type_name;
                if type_name != expected_type_name {
                    return Err(anyhow!("Mismatched '@type': expected '{}', found '{}'", expected_type_name, type_name));
                }

                // Check each variant
                #(#variant_matchers)*

                Err(anyhow!("No valid enum variant found in JSON for tagged union {}", #expected_type_name))
            }
        }
    };

    Result::Ok(expanded)
}

/// Generates the code snippets for deserializing each field from the JSON map.
fn generate_field_deserializers(
    fields: &FieldsNamed,
    struct_name: &Ident,
    expected_type_name: &str,
    opts: &TDBModelOpts,
) -> Result<TokenStream, syn::Error> {
    let mut deserializers: Vec<TokenStream> = Vec::new();

    for field in &fields.named {
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
                let _prop = <#field_ty as terminusdb_schema::json::InstancePropertyFromJson<#struct_name>>::property_from_maybe_json(
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
                let _prop = <#field_ty as terminusdb_schema::json::InstancePropertyFromJson<#struct_name>>::property_from_maybe_json(
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

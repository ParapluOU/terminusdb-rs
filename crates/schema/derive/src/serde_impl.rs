//! Generates serde Serialize and Deserialize implementations for TerminusDBModel types.
//!
//! These implementations use the TDB JSON-LD format, delegating to the existing
//! ToTDBInstance/FromTDBInstance infrastructure.

use crate::prelude::*;

/// Generates both Serialize and Deserialize implementations.
///
/// # Arguments
/// * `input` - The derive input for the type
/// * `is_simple_enum` - Whether this is a simple enum (all unit variants)
pub fn generate_serde_impls(
    input: &DeriveInput,
    is_simple_enum: bool,
) -> proc_macro2::TokenStream {
    let serialize_impl = generate_serialize_impl(input);
    let deserialize_impl = generate_deserialize_impl(input, is_simple_enum);

    quote! {
        #serialize_impl
        #deserialize_impl
    }
}

/// Generates serde::Serialize implementation that produces TDB JSON-LD format.
///
/// This works for all types (structs, simple enums, tagged unions) because
/// Instance::to_json() already handles type-specific serialization:
/// - Structs/Tagged unions: returns `{"@type": "...", ...}`
/// - Simple enums: returns `"variant_name"` (string)
fn generate_serialize_impl(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;

    quote! {
        impl ::serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                let json = <Self as terminusdb_schema::ToTDBInstance>::to_json(self);
                json.serialize(serializer)
            }
        }
    }
}

/// Generates serde::Deserialize implementation that expects TDB JSON-LD format.
///
/// For simple enums, expects a string value and uses TDBEnum::from_tdb_value().
/// For structs and tagged unions, expects JSON object with @type field and uses
/// InstanceFromJson + FromTDBInstance.
fn generate_deserialize_impl(
    input: &DeriveInput,
    is_simple_enum: bool,
) -> proc_macro2::TokenStream {
    let name = &input.ident;

    if is_simple_enum {
        // Simple enums serialize as strings, so deserialize from string
        quote! {
            impl<'de> ::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'de>,
                {
                    let s = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                    <Self as terminusdb_schema::TDBEnum>::from_tdb_value(&s)
                        .ok_or_else(|| ::serde::de::Error::custom(
                            format!("invalid enum variant '{}' for {}", s, stringify!(#name))
                        ))
                }
            }
        }
    } else {
        // Structs and tagged unions use the full Instance machinery
        quote! {
            impl<'de> ::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'de>,
                {
                    use ::serde::de::Error as DeError;

                    // First deserialize to serde_json::Value
                    let value = <::serde_json::Value as ::serde::Deserialize>::deserialize(deserializer)?;

                    // Then use existing InstanceFromJson infrastructure
                    let instance = <Self as terminusdb_schema::json::InstanceFromJson>::instance_from_json(value)
                        .map_err(DeError::custom)?;

                    // Finally use FromTDBInstance to convert to the target type
                    <Self as terminusdb_schema::FromTDBInstance>::from_instance(&instance)
                        .map_err(DeError::custom)
                }
            }
        }
    }
}

//! Code generation for GraphQL filter types.
//!
//! This module parses GraphQL SDL and generates Rust structs for filter types.
//!
//! # Usage in build.rs
//!
//! ```ignore
//! use terminusdb_gql::{generate_gql_schema, generate_filter_types, generate_filter_impls};
//! use std::fs;
//!
//! fn main() {
//!     let sdl = generate_gql_schema::<(Project, Ticket)>();
//!
//!     // Generate filter struct definitions
//!     let filters = generate_filter_types(&sdl).unwrap();
//!
//!     // Generate TdbGQLFilter/TdbGQLOrdering trait impls
//!     // These connect filters to their model types
//!     let impls = generate_filter_impls(&[
//!         ("Project", "my_domain::Project"),
//!         ("Ticket", "my_domain::Ticket"),
//!     ]);
//!
//!     let out_dir = std::env::var("OUT_DIR").unwrap();
//!     fs::write(format!("{}/filters.rs", out_dir), filters).unwrap();
//!     fs::write(format!("{}/filter_impls.rs", out_dir), impls).unwrap();
//! }
//! ```

use graphql_parser::schema::{
    parse_schema, Definition, Document, EnumType, InputObjectType, Type,
    TypeDefinition as GqlTypeDefinition,
};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::collections::HashSet;

/// Generate Rust code for filter types from GraphQL SDL.
///
/// This function parses the SDL and generates Rust structs for all input types
/// (filter types). The generated code includes:
/// - Struct definitions with `#[derive(Default, Clone, Debug, Serialize, Deserialize)]`
/// - Optional fields with proper Option wrapping
/// - Serde rename attributes for fields starting with underscore
///
/// # Example
///
/// ```ignore
/// use terminusdb_gql::codegen::generate_filter_types;
///
/// let sdl = generate_gql_schema::<(Project, Ticket)>();
/// let rust_code = generate_filter_types(&sdl)?;
/// // Write rust_code to a file in OUT_DIR
/// ```
pub fn generate_filter_types(sdl: &str) -> Result<String, String> {
    let doc = parse_schema::<String>(sdl).map_err(|e| format!("Failed to parse SDL: {}", e))?;

    let tokens = generate_from_document(&doc);
    Ok(tokens.to_string())
}

/// Generate TokenStream from a parsed GraphQL document.
fn generate_from_document(doc: &Document<String>) -> TokenStream {
    let mut generated = Vec::new();

    // Collect all type names for reference resolution
    let input_type_names: HashSet<_> = doc
        .definitions
        .iter()
        .filter_map(|def| {
            if let Definition::TypeDefinition(GqlTypeDefinition::InputObject(input)) = def {
                Some(input.name.clone())
            } else {
                None
            }
        })
        .collect();

    let enum_names: HashSet<_> = doc
        .definitions
        .iter()
        .filter_map(|def| {
            if let Definition::TypeDefinition(GqlTypeDefinition::Enum(e)) = def {
                Some(e.name.clone())
            } else {
                None
            }
        })
        .collect();

    // Generate enums (like Ordering)
    for def in &doc.definitions {
        if let Definition::TypeDefinition(GqlTypeDefinition::Enum(e)) = def {
            generated.push(generate_enum(e));
        }
    }

    // Generate structs for input types (filters and ordering)
    for def in &doc.definitions {
        if let Definition::TypeDefinition(GqlTypeDefinition::InputObject(input)) = def {
            generated.push(generate_input_object(input, &input_type_names, &enum_names));
        }
    }

    quote! {
        use serde::{Deserialize, Serialize};
        use bon::Builder;

        #(#generated)*
    }
}

/// Generate a Rust enum for a GraphQL enum type.
fn generate_enum(e: &EnumType<String>) -> TokenStream {
    let enum_name = sanitize_type_name(&e.name);
    let enum_ident = Ident::new(&enum_name, Span::call_site());

    let variants: Vec<TokenStream> = e
        .values
        .iter()
        .map(|v| {
            let variant_name = &v.name;
            let rust_variant_name = sanitize_enum_variant(variant_name);
            let variant_ident = Ident::new(&rust_variant_name, Span::call_site());

            // Add serde rename if the name changed
            if rust_variant_name != *variant_name {
                quote! {
                    #[serde(rename = #variant_name)]
                    #variant_ident,
                }
            } else {
                quote! {
                    #variant_ident,
                }
            }
        })
        .collect();

    quote! {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
        pub enum #enum_ident {
            #(#variants)*
        }
    }
}

/// Sanitize an enum variant name to be a valid Rust identifier.
/// Converts to PascalCase and handles reserved keywords.
fn sanitize_enum_variant(name: &str) -> String {
    // Handle reserved Rust keywords
    let rust_name = match name {
        "in" => "In",
        "notin" => "NotIn",
        "type" => "Type",
        "self" => "Self_",
        "super" => "Super",
        "crate" => "Crate",
        "mod" => "Mod",
        "fn" => "Fn",
        "let" => "Let",
        "const" => "Const",
        "static" => "Static",
        "mut" => "Mut",
        "ref" => "Ref",
        "match" => "Match",
        "if" => "If",
        "else" => "Else",
        "loop" => "Loop",
        "while" => "While",
        "for" => "For",
        "break" => "Break",
        "continue" => "Continue",
        "return" => "Return",
        "true" => "True",
        "false" => "False",
        _ => {
            // Convert to PascalCase
            return to_pascal_case(name);
        }
    };
    rust_name.to_string()
}

/// Convert a string to PascalCase.
fn to_pascal_case(s: &str) -> String {
    // Handle already PascalCase names
    if s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !s.contains('_') {
        return s.to_string();
    }

    // Split by underscores and capitalize each word
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Generate a Rust struct for a GraphQL input object type.
fn generate_input_object(
    input: &InputObjectType<String>,
    known_inputs: &HashSet<String>,
    known_enums: &HashSet<String>,
) -> TokenStream {
    let struct_name = sanitize_type_name(&input.name);
    let struct_ident = Ident::new(&struct_name, Span::call_site());

    // First pass: collect all sanitized field names to detect conflicts
    let sanitized_names: HashSet<String> = input
        .fields
        .iter()
        .filter(|f| !f.name.starts_with('_')) // Only non-underscore fields
        .map(|f| sanitize_field_name(&f.name))
        .collect();

    let fields: Vec<TokenStream> = input
        .fields
        .iter()
        .map(|field| {
            let field_name = &field.name;
            let rust_field_name = sanitize_field_name_with_conflicts(field_name, &sanitized_names);
            let field_ident = Ident::new(&rust_field_name, Span::call_site());

            let field_type =
                graphql_type_to_rust(&field.value_type, known_inputs, known_enums, &struct_name);

            // Add serde rename if the field name was sanitized or starts with underscore
            let serde_attr = if rust_field_name != *field_name {
                quote! { #[serde(rename = #field_name, skip_serializing_if = "Option::is_none")] }
            } else {
                quote! { #[serde(skip_serializing_if = "Option::is_none")] }
            };

            quote! {
                #serde_attr
                pub #field_ident: #field_type,
            }
        })
        .collect();

    quote! {
        #[derive(Default, Clone, Debug, Serialize, Deserialize, bon::Builder)]
        #[builder(on(_, into), finish_fn = build)]
        pub struct #struct_ident {
            #(#fields)*
        }
    }
}

/// Convert a GraphQL type to a Rust type.
fn graphql_type_to_rust(
    ty: &Type<String>,
    known_inputs: &HashSet<String>,
    known_enums: &HashSet<String>,
    current_struct: &str,
) -> TokenStream {
    match ty {
        Type::NamedType(name) => {
            let rust_type = graphql_scalar_to_rust(name, known_inputs, known_enums, current_struct);
            // All filter fields are optional
            quote! { Option<#rust_type> }
        }
        Type::NonNullType(inner) => {
            // For non-null, we still wrap in Option for filter inputs
            // (filters are typically all optional)
            graphql_type_to_rust(inner, known_inputs, known_enums, current_struct)
        }
        Type::ListType(inner) => {
            let inner_type =
                graphql_type_to_rust_unwrapped(inner, known_inputs, known_enums, current_struct);
            quote! { Option<Vec<#inner_type>> }
        }
    }
}

/// Convert a GraphQL type to Rust without wrapping in Option (for list elements).
fn graphql_type_to_rust_unwrapped(
    ty: &Type<String>,
    known_inputs: &HashSet<String>,
    known_enums: &HashSet<String>,
    current_struct: &str,
) -> TokenStream {
    match ty {
        Type::NamedType(name) => {
            graphql_scalar_to_rust(name, known_inputs, known_enums, current_struct)
        }
        Type::NonNullType(inner) => {
            graphql_type_to_rust_unwrapped(inner, known_inputs, known_enums, current_struct)
        }
        Type::ListType(inner) => {
            let inner_type =
                graphql_type_to_rust_unwrapped(inner, known_inputs, known_enums, current_struct);
            quote! { Vec<#inner_type> }
        }
    }
}

/// Convert a GraphQL scalar/type name to Rust type.
fn graphql_scalar_to_rust(
    name: &str,
    known_inputs: &HashSet<String>,
    known_enums: &HashSet<String>,
    current_struct: &str,
) -> TokenStream {
    match name {
        "String" => quote! { String },
        "Int" => quote! { i32 },
        "Float" => quote! { f64 },
        "Boolean" => quote! { bool },
        "ID" => quote! { String },
        "BigInt" => quote! { String },   // BigInt as string
        "BigFloat" => quote! { String }, // BigFloat as string
        "DateTime" => quote! { String }, // DateTime as ISO string
        "JSON" => quote! { serde_json::Value },
        _ => {
            // It's a custom type - check if it's a known input or enum type
            let rust_name = sanitize_type_name(name);
            let type_ident = Ident::new(&rust_name, Span::call_site());

            // For self-referential types (like _not: ProjectFilter), use Box
            if rust_name == current_struct {
                quote! { Box<#type_ident> }
            } else if known_inputs.contains(name) || known_enums.contains(name) {
                quote! { #type_ident }
            } else {
                // Unknown type, treat as String
                quote! { String }
            }
        }
    }
}

/// Sanitize a GraphQL type name to be a valid Rust identifier.
/// Converts underscores to CamelCase style.
fn sanitize_type_name(name: &str) -> String {
    // Convert names like "Project_Filter" to "ProjectFilter"
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Sanitize a GraphQL field name to be a valid Rust identifier.
fn sanitize_field_name(name: &str) -> String {
    if name.starts_with('_') {
        // Remove leading underscore for Rust field name
        // e.g., "_id" -> "id", "_and" -> "and"
        name.trim_start_matches('_').to_string()
    } else {
        name.to_string()
    }
}

/// Sanitize a field name while avoiding conflicts with existing field names.
/// If removing the underscore would cause a conflict, prefix with "tdb_".
fn sanitize_field_name_with_conflicts(name: &str, existing_names: &HashSet<String>) -> String {
    if name.starts_with('_') {
        let stripped = name.trim_start_matches('_');
        // If the stripped name conflicts with an existing field, use "tdb_" prefix
        if existing_names.contains(stripped) {
            format!("tdb_{}", stripped)
        } else {
            stripped.to_string()
        }
    } else {
        name.to_string()
    }
}

/// Configuration for a single model's filter generation.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// The model's simple name (e.g., "Project")
    pub name: String,
    /// The full path to the model type (e.g., "crate::models::Project")
    pub path: String,
}

impl ModelConfig {
    /// Create a new model configuration.
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }
}

/// Generate TdbGQLFilter and TdbGQLOrdering trait implementations for the given models.
///
/// This function generates impl blocks that connect each filter/ordering type to its
/// corresponding model. The filter and ordering type names are derived from the
/// model name using the conventions `{ModelName}Filter` and `{ModelName}Ordering`.
///
/// # Arguments
///
/// * `models` - Slice of (model_name, model_path) tuples
///
/// # Example
///
/// ```ignore
/// let impls = generate_filter_impls(&[
///     ("Project", "crate::models::Project"),
///     ("Ticket", "crate::models::Ticket"),
/// ]);
/// ```
///
/// This generates:
/// ```ignore
/// impl terminusdb_schema::TdbGQLFilter<crate::models::Project> for ProjectFilter {}
/// impl terminusdb_schema::TdbGQLOrdering<crate::models::Project> for ProjectOrdering {}
/// impl terminusdb_schema::TdbGQLFilter<crate::models::Ticket> for TicketFilter {}
/// impl terminusdb_schema::TdbGQLOrdering<crate::models::Ticket> for TicketOrdering {}
/// ```
pub fn generate_filter_impls(models: &[(&str, &str)]) -> String {
    let impls: Vec<TokenStream> = models
        .iter()
        .flat_map(|(name, path)| {
            let filter_name = format!("{}Filter", name);
            let filter_ident = Ident::new(&filter_name, Span::call_site());

            let ordering_name = format!("{}Ordering", name);
            let ordering_ident = Ident::new(&ordering_name, Span::call_site());

            // Parse the path as a token stream
            let path_tokens: TokenStream = path.parse().unwrap_or_else(|_| {
                let ident = Ident::new(path, Span::call_site());
                quote! { #ident }
            });

            vec![
                quote! {
                    impl terminusdb_schema::TdbGQLFilter<#path_tokens> for #filter_ident {}
                },
                quote! {
                    impl terminusdb_schema::TdbGQLOrdering<#path_tokens> for #ordering_ident {}
                },
            ]
        })
        .collect();

    let tokens = quote! {
        #(#impls)*
    };

    tokens.to_string()
}

/// Deprecated: Use `generate_filter_impls` instead.
///
/// This function is kept for backwards compatibility but will be removed in a future version.
#[deprecated(since = "0.2.0", note = "Use generate_filter_impls instead")]
pub fn generate_model_impls(models: &[(&str, &str)]) -> String {
    generate_filter_impls(models)
}

/// Generate both filter types and filter/ordering trait impls from SDL.
///
/// This is a convenience function that combines `generate_filter_types` and
/// `generate_filter_impls` into a single call.
///
/// # Returns
///
/// A tuple of (filter_types_code, filter_impls_code).
pub fn generate_all(sdl: &str, models: &[(&str, &str)]) -> Result<(String, String), String> {
    let filters = generate_filter_types(sdl)?;
    let impls = generate_filter_impls(models);
    Ok((filters, impls))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_type_name() {
        assert_eq!(sanitize_type_name("Project_Filter"), "ProjectFilter");
        assert_eq!(sanitize_type_name("StringFilter"), "StringFilter");
        assert_eq!(
            sanitize_type_name("Project_Collection_Filter"),
            "ProjectCollectionFilter"
        );
    }

    #[test]
    fn test_sanitize_field_name() {
        assert_eq!(sanitize_field_name("_id"), "id");
        assert_eq!(sanitize_field_name("_and"), "and");
        assert_eq!(sanitize_field_name("name"), "name");
    }

    #[test]
    fn test_sanitize_field_name_with_conflicts() {
        let existing: HashSet<String> = ["id", "name"].iter().map(|s| s.to_string()).collect();

        // No conflict - strips underscore
        assert_eq!(sanitize_field_name_with_conflicts("_and", &existing), "and");

        // Conflict - uses tdb_ prefix
        assert_eq!(
            sanitize_field_name_with_conflicts("_id", &existing),
            "tdb_id"
        );

        // Regular field - unchanged
        assert_eq!(
            sanitize_field_name_with_conflicts("name", &existing),
            "name"
        );
    }

    #[test]
    fn test_generate_filter_with_id_conflict() {
        // Test case where model has both `id` field and `_id` field
        let sdl = r#"
            input StringFilter {
                eq: String
            }

            input UserFilter {
                id: StringFilter
                _id: ID
                name: StringFilter
            }
        "#;

        let code = generate_filter_types(sdl).unwrap();

        // Should have both id and tdb_id fields
        assert!(code.contains("pub id"), "Should contain id field");
        assert!(
            code.contains("pub tdb_id"),
            "Should contain tdb_id field (from _id to avoid conflict)"
        );
        assert!(
            code.contains(r#"rename = "_id""#),
            "Should have serde rename for tdb_id"
        );
    }

    #[test]
    fn test_generate_filter_types() {
        let sdl = r#"
            input StringFilter {
                eq: String
                ne: String
            }

            input ProjectFilter {
                name: StringFilter
                _id: ID
            }
        "#;

        let code = generate_filter_types(sdl).unwrap();

        assert!(
            code.contains("struct StringFilter"),
            "Should contain StringFilter struct"
        );
        assert!(
            code.contains("struct ProjectFilter"),
            "Should contain ProjectFilter struct"
        );
        assert!(code.contains("pub eq"), "Should contain eq field");
        assert!(
            code.contains("pub id"),
            "Should contain id field (sanitized from _id)"
        );
    }

    #[test]
    fn test_generate_self_referential_filter() {
        let sdl = r#"
            input ProjectFilter {
                name: String
                _and: [ProjectFilter!]
                _not: ProjectFilter
            }
        "#;

        let code = generate_filter_types(sdl).unwrap();

        // Self-referential _not should use Box
        assert!(
            code.contains("Box < ProjectFilter >") || code.contains("Box<ProjectFilter>"),
            "Should use Box for self-referential _not field. Generated code: {}",
            code
        );
    }

    #[test]
    fn test_generate_filter_impls() {
        let impls = generate_filter_impls(&[
            ("Project", "crate::models::Project"),
            ("Ticket", "my_crate::Ticket"),
        ]);

        // Check TdbGQLFilter impls
        assert!(
            impls.contains("impl terminusdb_schema :: TdbGQLFilter < crate :: models :: Project > for ProjectFilter"),
            "Should contain ProjectFilter impl. Generated:\n{}",
            impls
        );
        assert!(
            impls.contains("impl terminusdb_schema :: TdbGQLFilter < my_crate :: Ticket > for TicketFilter"),
            "Should contain TicketFilter impl. Generated:\n{}",
            impls
        );

        // Check TdbGQLOrdering impls
        assert!(
            impls.contains("impl terminusdb_schema :: TdbGQLOrdering < crate :: models :: Project > for ProjectOrdering"),
            "Should contain ProjectOrdering impl. Generated:\n{}",
            impls
        );
        assert!(
            impls.contains("impl terminusdb_schema :: TdbGQLOrdering < my_crate :: Ticket > for TicketOrdering"),
            "Should contain TicketOrdering impl. Generated:\n{}",
            impls
        );
    }

    #[test]
    fn test_full_pipeline() {
        use crate::generate_gql_schema;
        use terminusdb_schema::{Key, Property, Schema, ToTDBSchemas, TypeFamily};

        // Define test schemas
        struct TestModels;

        // @claude todo: we should never manually implement ToSchema. just derive TerminusDBModel
        impl ToTDBSchemas for TestModels {
            fn to_schemas() -> Vec<Schema> {
                vec![
                    Schema::Class {
                        id: "Project".to_string(),
                        base: None,
                        key: Key::Lexical(vec!["name".to_string()]),
                        documentation: None,
                        subdocument: false,
                        r#abstract: false,
                        inherits: vec![],
                        unfoldable: false,
                        properties: vec![
                            Property {
                                name: "name".to_string(),
                                r#type: None,
                                class: "xsd:string".to_string(),
                            },
                            Property {
                                name: "active".to_string(),
                                r#type: Some(TypeFamily::Optional),
                                class: "xsd:boolean".to_string(),
                            },
                        ],
                    },
                    Schema::Enum {
                        id: "Status".to_string(),
                        base: None,
                        values: vec!["Open".to_string(), "Closed".to_string()],
                        documentation: None,
                    },
                ]
            }
        }

        // Generate SDL from models
        let sdl = generate_gql_schema::<TestModels>();

        // Verify SDL contains expected types
        assert!(
            sdl.contains("type Project"),
            "SDL should contain Project type"
        );
        assert!(
            sdl.contains("input Project_Filter"),
            "SDL should contain Project_Filter"
        );
        assert!(
            sdl.contains("enum Status"),
            "SDL should contain Status enum"
        );

        // Generate Rust code from SDL
        let rust_code = generate_filter_types(&sdl).unwrap();

        // Verify generated Rust code
        assert!(
            rust_code.contains("struct ProjectFilter"),
            "Should generate ProjectFilter"
        );
        assert!(
            rust_code.contains("struct StringFilter"),
            "Should generate StringFilter"
        );
        assert!(
            rust_code.contains("struct BooleanFilter"),
            "Should generate BooleanFilter"
        );
        assert!(
            rust_code.contains("pub name"),
            "ProjectFilter should have name field"
        );
        assert!(
            rust_code.contains("pub active"),
            "ProjectFilter should have active field"
        );
    }

    #[test]
    fn test_generate_filter_with_relations() {
        use crate::generate_gql_schema;
        use terminusdb_schema::{Key, Property, Schema, ToTDBSchemas};

        struct RelationModels;

        // @claude todo: we should never manually implement ToSchema. just derive TerminusDBModel
        impl ToTDBSchemas for RelationModels {
            fn to_schemas() -> Vec<Schema> {
                vec![
                    Schema::Class {
                        id: "Project".to_string(),
                        base: None,
                        key: Key::Lexical(vec!["name".to_string()]),
                        documentation: None,
                        subdocument: false,
                        r#abstract: false,
                        inherits: vec![],
                        unfoldable: false,
                        properties: vec![Property {
                            name: "name".to_string(),
                            r#type: None,
                            class: "xsd:string".to_string(),
                        }],
                    },
                    Schema::Class {
                        id: "Ticket".to_string(),
                        base: None,
                        key: Key::Lexical(vec!["title".to_string()]),
                        documentation: None,
                        subdocument: false,
                        r#abstract: false,
                        inherits: vec![],
                        unfoldable: false,
                        properties: vec![
                            Property {
                                name: "title".to_string(),
                                r#type: None,
                                class: "xsd:string".to_string(),
                            },
                            Property {
                                name: "project".to_string(),
                                r#type: None,
                                class: "Project".to_string(),
                            },
                        ],
                    },
                ]
            }
        }

        let sdl = generate_gql_schema::<RelationModels>();
        let rust_code = generate_filter_types(&sdl).unwrap();

        // Struct names are sanitized (Ticket_Filter -> TicketFilter)
        assert!(
            rust_code.contains("struct TicketFilter"),
            "Should have TicketFilter"
        );
        assert!(
            rust_code.contains("struct ProjectFilter"),
            "Should have ProjectFilter"
        );

        // TicketFilter should have project field referencing ProjectFilter
        assert!(
            rust_code.contains("project : Option < ProjectFilter"),
            "TicketFilter should have project: Option<ProjectFilter>"
        );
    }

    #[test]
    fn test_generate_enum_types() {
        let sdl = r#"
            enum Ordering {
                Asc
                Desc
            }

            input Project_Ordering {
                name: Ordering
                created_at: Ordering
            }
        "#;

        let code = generate_filter_types(sdl).unwrap();

        // Should generate Ordering enum
        assert!(
            code.contains("pub enum Ordering"),
            "Should contain Ordering enum"
        );
        assert!(code.contains("Asc"), "Should contain Asc variant");
        assert!(code.contains("Desc"), "Should contain Desc variant");

        // Should generate ordering struct with Ordering fields
        assert!(
            code.contains("struct ProjectOrdering"),
            "Should contain ProjectOrdering struct"
        );
        assert!(
            code.contains("name : Option < Ordering"),
            "ProjectOrdering should have name: Option<Ordering>. Code: {}",
            code
        );
    }

    #[test]
    fn test_generate_enum_with_keywords() {
        let sdl = r#"
            enum FilterOperator {
                equals
                notequals
                in
                notin
            }
        "#;

        let code = generate_filter_types(sdl).unwrap();

        // Should handle reserved keyword 'in'
        assert!(
            code.contains("pub enum FilterOperator"),
            "Should contain FilterOperator enum"
        );
        assert!(
            code.contains("Equals"),
            "Should contain Equals variant (from equals)"
        );
        assert!(
            code.contains("Notequals"),
            "Should contain Notequals variant"
        );
        assert!(code.contains("In"), "Should contain In variant (from 'in')");
        assert!(
            code.contains("NotIn"),
            "Should contain NotIn variant (from notin)"
        );
        assert!(
            code.contains(r#"rename = "in""#),
            "Should rename In back to 'in'"
        );
    }

    #[test]
    fn test_sanitize_enum_variant() {
        // PascalCase stays the same
        assert_eq!(sanitize_enum_variant("Asc"), "Asc");
        assert_eq!(sanitize_enum_variant("Desc"), "Desc");

        // lowercase becomes PascalCase
        assert_eq!(sanitize_enum_variant("equals"), "Equals");
        assert_eq!(sanitize_enum_variant("space"), "Space");

        // snake_case becomes PascalCase
        assert_eq!(sanitize_enum_variant("not_equals"), "NotEquals");

        // Keywords are handled
        assert_eq!(sanitize_enum_variant("in"), "In");
        assert_eq!(sanitize_enum_variant("notin"), "NotIn");
    }
}

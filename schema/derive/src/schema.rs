use crate::prelude::*;

/// Generate the ToTDBSchema trait implementation with a specific schema type
pub fn generate_totdbschema_impl(
    struct_name: &syn::Ident,
    class_name_expr: proc_macro2::TokenStream,
    opts: &TDBModelOpts,
    properties_or_values: proc_macro2::TokenStream,
    schema_type_param: proc_macro2::TokenStream,
    to_schema_tree_impl: proc_macro2::TokenStream,
    generics: (&proc_macro2::TokenStream, &proc_macro2::TokenStream, &Option<syn::WhereClause>),
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics;
    // Generate the base implementation
    let base = if let Some(base) = &opts.base {
        quote! { Some(#base.to_string()) }
    } else {
        quote! { None }
    };

    // Generate the key implementation
    let key = if let Some(key) = &opts.key {
        match key.as_str() {
            "random" => quote! { terminusdb_schema::Key::Random },
            "hash" => {
                if let Some(fields) = opts.get_key_fields() {
                    let field_strings = fields.iter().map(|f| quote! { #f.to_string() });
                    quote! { terminusdb_schema::Key::Hash(vec![#(#field_strings),*]) }
                } else {
                    quote! { terminusdb_schema::Key::Hash(vec!["id".to_string()]) }
                }
            }
            "value_hash" => quote! { terminusdb_schema::Key::ValueHash },
            "lexical" => {
                if let Some(fields) = opts.get_key_fields() {
                    let field_strings = fields.iter().map(|f| quote! { #f.to_string() });
                    quote! { terminusdb_schema::Key::Lexical(vec![#(#field_strings),*]) }
                } else {
                    quote! { terminusdb_schema::Key::Lexical(vec!["id".to_string()]) }
                }
            }
            _ => quote! { terminusdb_schema::Key::Random },
        }
    } else {
        quote! { terminusdb_schema::Key::Random }
    };

    // Generate the abstract implementation
    let abstract_class = if let Some(abstract_class) = opts.abstract_class {
        quote! { Some(#abstract_class) }
    } else {
        quote! { None }
    };

    // Generate the inherits implementation
    let inherits = if let Some(inherits_from) = &opts.inherits {
        // Parse comma-separated class names
        let inherits_list = inherits_from
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| quote! { #s.to_string() })
            .collect::<Vec<_>>();

        quote! { Some(vec![#(#inherits_list),*]) }
    } else {
        quote! { None }
    };

    // Default unfoldable to false to match the test expectations
    let unfoldable = opts.unfoldable.unwrap_or(false);

    // Handle subdocument attribute
    let subdocument = if let Some(subdocument) = opts.subdocument {
        quote! { Some(#subdocument) }
    } else {
        quote! { None }
    };

    // Use provided doc attribute or extract from doc comments
    let documentation = if let Some(doc) = &opts.doc {
        quote! {
            Some(terminusdb_schema::ClassDocumentation {
                comment: #doc.to_string(),
                properties_or_values: std::collections::BTreeMap::new()
            })
        }
    } else if let Some(doc_str) = opts.extract_doc_string() {
        quote! {
            Some(terminusdb_schema::ClassDocumentation {
                comment: #doc_str.to_string(),
                properties_or_values: std::collections::BTreeMap::new()
            })
        }
    } else {
        quote! { None }
    };

    // Determine whether properties_or_values is for properties or values
    // based on the schema_type_param
    let properties_method = if schema_type_param.to_string().contains("SchemaTypeEnum") {
        quote! {
            fn properties() -> Option<Vec<terminusdb_schema::Property>> {
                None
            }
        }
    } else {
        quote! {
            fn properties() -> Option<Vec<terminusdb_schema::Property>> {
                use tap::prelude::*;

                #properties_or_values
            }
        }
    };

    // Determine whether properties_or_values is for properties or values
    // based on the schema_type_param
    let values_method = if schema_type_param.to_string().contains("SchemaTypeEnum") {
        quote! {
            fn values() -> Option<Vec<terminusdb_schema::URI>> {
                #properties_or_values
            }
        }
    } else {
        quote! {
            fn values() -> Option<Vec<terminusdb_schema::URI>> {
                None
            }
        }
    };

    // Use the existing schema type instead of creating a custom one
    quote! {
        impl #impl_generics terminusdb_schema::ToTDBSchema for #struct_name #ty_generics #where_clause {
            type Type = terminusdb_schema::#schema_type_param;

            fn id() -> Option<String> {
                Some(#class_name_expr.to_string())
            }

            fn base() -> Option<String> {
                #base
            }

            fn key() -> terminusdb_schema::Key {
                #key
            }

            fn subdocument() -> Option<bool> {
                #subdocument
            }

            fn abstractdocument() -> Option<bool> {
                #abstract_class
            }

            fn inherits() -> Option<Vec<String>> {
                #inherits
            }

            fn unfoldable() -> bool {
                #unfoldable
            }

            fn documentation() -> Option<terminusdb_schema::ClassDocumentation> {
                #documentation
            }

            #properties_method

            #values_method

            #to_schema_tree_impl
        }

        // ToSchemaClass is implemented separately in struct.rs with proper generics
    }
}

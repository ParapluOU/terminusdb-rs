use crate::prelude::*;

/// Container for attributes specified on the struct or enum
#[derive(Debug, FromDeriveInput, Clone, Default)]
#[darling(
    attributes(tdb),
    supports(struct_named, enum_unit, enum_newtype, enum_any)
)]
pub struct TDBModelOpts {
    /// Optional custom class name for the schema
    #[darling(default)]
    pub(crate) class_name: Option<String>,

    /// Optional base URI
    #[darling(default)]
    pub(crate) base: Option<String>,

    /// Optional - define a custom key strategy
    #[darling(default)]
    pub(crate) key: Option<String>,

    /// Whether this is an abstract class
    #[darling(default)]
    pub(crate) abstract_class: Option<bool>,

    /// Whether this is unfoldable
    #[darling(default)]
    pub(crate) unfoldable: Option<bool>,

    /// List of class names this inherits from
    #[darling(default)]
    pub(crate) inherits: Option<String>,

    /// Documentation for the class
    #[darling(default)]
    pub(crate) doc: Option<String>,

    /// Original input for extracting doc comments
    #[darling(skip)]
    pub(crate) original_input: Option<DeriveInput>,

    /// optional name of the field that should be mapped to the @id property
    #[darling(default)]
    pub(crate) id_field: Option<String>,

    /// Rename strategy for enum variants (e.g., "lowercase", "UPPERCASE", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case")
    #[darling(default)]
    pub(crate) rename_all: Option<String>,

    /// Fields to use for Lexical or Hash key strategies
    #[darling(default)]
    pub(crate) key_fields: Option<String>,
}

/// Rename case conversion strategies
#[derive(Debug, Clone, Copy)]
pub enum RenameStrategy {
    /// Keep original name
    None,
    /// Convert to lowercase
    Lowercase,
    /// Convert to UPPERCASE  
    Uppercase,
    /// Convert to PascalCase
    PascalCase,
    /// Convert to camelCase
    CamelCase,
    /// Convert to snake_case
    SnakeCase,
    /// Convert to SCREAMING_SNAKE_CASE
    ScreamingSnakeCase,
    /// Convert to kebab-case
    KebabCase,
}

impl RenameStrategy {
    /// Parse rename strategy from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lowercase" => Some(RenameStrategy::Lowercase),
            "UPPERCASE" => Some(RenameStrategy::Uppercase),
            "PascalCase" => Some(RenameStrategy::PascalCase),
            "camelCase" => Some(RenameStrategy::CamelCase),
            "snake_case" => Some(RenameStrategy::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Some(RenameStrategy::ScreamingSnakeCase),
            "kebab-case" => Some(RenameStrategy::KebabCase),
            _ => None,
        }
    }

    /// Apply the rename strategy to a string
    pub fn apply(self, input: &str) -> String {
        match self {
            RenameStrategy::None => input.to_string(),
            RenameStrategy::Lowercase => input.to_lowercase(),
            RenameStrategy::Uppercase => input.to_uppercase(),
            RenameStrategy::PascalCase => to_pascal_case(input),
            RenameStrategy::CamelCase => to_camel_case(input),
            RenameStrategy::SnakeCase => to_snake_case(input),
            RenameStrategy::ScreamingSnakeCase => to_snake_case(input).to_uppercase(),
            RenameStrategy::KebabCase => to_kebab_case(input),
        }
    }
}

impl TDBModelOpts {
    /// Get the rename strategy for enum variants
    pub fn get_rename_strategy(&self) -> RenameStrategy {
        match &self.rename_all {
            Some(strategy_str) => {
                RenameStrategy::from_str(strategy_str).unwrap_or(RenameStrategy::None)
            }
            None => RenameStrategy::None,
        }
    }

    /// Parse key_fields string into a vector of field names
    pub fn get_key_fields(&self) -> Option<Vec<String>> {
        self.key_fields.as_ref().map(|fields_str| {
            fields_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    }

    /// Extracts docstring from struct or enum doc comments
    pub fn extract_doc_string(&self) -> Option<String> {
        // Attempt to extract doc string from attributes
        let attrs = self
            .original_input
            .as_ref()?
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"))
            .collect::<Vec<_>>();

        if attrs.is_empty() {
            return None;
        }

        // Combine all doc comment segments
        let mut doc_string = String::new();
        for attr in attrs {
            if let Result::Ok(doc_meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(expr_lit) = &doc_meta.value {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        let doc_segment = lit_str.value();
                        if !doc_string.is_empty() {
                            doc_string.push_str("\n");
                        }
                        doc_string.push_str(doc_segment.trim());
                    }
                }
            }
        }

        if doc_string.is_empty() {
            None
        } else {
            Some(doc_string)
        }
    }
}

/// Container for attributes specified on struct fields
#[derive(Debug, FromField)]
#[darling(attributes(tdb))]
pub struct TDBFieldOpts {
    /// The original field from the struct
    pub(crate) ident: Option<syn::Ident>,

    /// The field type
    pub(crate) ty: syn::Type,

    /// Optional custom name for this field in the schema
    #[darling(default)]
    pub(crate) name: Option<String>,

    /// Optional custom class name for this field
    #[darling(default)]
    pub(crate) class: Option<String>,

    /// Documentation for this field
    #[darling(default)]
    pub(crate) doc: Option<String>,

    /// Whether this field is a subdocument
    #[darling(default)]
    pub(crate) subdocument: Option<bool>,
}

/// Container for attributes specified on enum variants
#[derive(Debug, Clone, Default)]
pub struct TDBVariantOpts {
    /// Optional custom name for this variant in the schema
    pub(crate) rename: Option<String>,

    /// Documentation for this variant
    pub(crate) doc: Option<String>,
}

impl TDBVariantOpts {
    /// Parse TDB options from variant attributes
    /// For now, we'll implement a simple version focusing on enum-level rename_all
    pub fn from_variant(_variant: &syn::Variant) -> Result<Self, String> {
        // For the initial implementation, we'll just return default options
        // Individual variant renaming can be added in a future enhancement
        Ok(TDBVariantOpts::default())
    }

    /// Get the effective name for this variant, considering both rename and rename_all
    pub fn get_effective_name(
        &self,
        original_name: &str,
        rename_strategy: RenameStrategy,
    ) -> String {
        if let Some(ref renamed) = self.rename {
            // Individual rename takes precedence
            renamed.clone()
        } else {
            // Apply enum-level rename strategy
            rename_strategy.apply(original_name)
        }
    }
}

/// Helper function to convert to PascalCase
fn to_pascal_case(input: &str) -> String {
    input
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

/// Helper function to convert to camelCase
fn to_camel_case(input: &str) -> String {
    let pascal = to_pascal_case(input);
    let mut chars = pascal.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Helper function to convert to snake_case
fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
    }

    result
}

/// Helper function to convert to kebab-case
fn to_kebab_case(input: &str) -> String {
    to_snake_case(input).replace('_', "-")
}

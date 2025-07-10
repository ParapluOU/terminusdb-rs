#![allow(warnings)]

mod args;
mod enum_simple;
mod enum_union;
mod from_instance;
mod instance;
mod json_deserialize;
mod prelude;
mod schema;
mod r#struct;

use crate::enum_simple::implement_for_simple_enum;
use crate::enum_union::implement_for_tagged_enum;
use crate::prelude::*;
use crate::r#struct::implement_for_struct;
use tracing::trace;

/// Determine whether an enum is a simple enum or a tagged union, and delegate to the appropriate implementation
fn implement_for_enum(
    input: &DeriveInput,
    data_enum: &DataEnum,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    // Check if all variants are simple (unit variants without data)
    let all_variants_are_simple = data_enum
        .variants
        .iter()
        .all(|variant| matches!(variant.fields, Fields::Unit));

    if all_variants_are_simple {
        // This is a simple enum, use the simple enum implementation
        implement_for_simple_enum(input, data_enum, opts)
    } else {
        // This is a tagged union enum, use the tagged union implementation
        implement_for_tagged_enum(input, data_enum, opts)
    }
}

/// Automatically derives the `ToTDBSchema` trait for a struct or enum.
///
/// This macro generates TerminusDB schema definitions from Rust data structures.
/// It can be used on both structs (maps to TerminusDB Classes) and enums (maps to TerminusDB TaggedUnions).
///
/// # Struct Attributes
///
/// The following attributes can be used on structs or enums:
///
/// - `#[tdb(class_name = "CustomName")]` - Specify a custom class name (defaults to struct/enum name).
/// - `#[tdb(base = "http://example.com/")]` - Specify a base URI.
/// - `#[tdb(key = "value_hash")]` - Key strategy (one of "random", "value_hash", "hash", "lexical").
/// - `#[tdb(subdocument = true)]` - Mark as a subdocument.
/// - `#[tdb(abstract_class = true)]` - Mark as an abstract class.
/// - `#[tdb(unfoldable = false)]` - Control whether the class is unfoldable.
/// - `#[tdb(inherits = "BaseClass")]` - Specify inheritance.
/// - `#[tdb(doc = "Class documentation")]` - Provide documentation.
/// - `#[tdb(rename_all = "lowercase")]` - Rename all enum variants (for enums only). Supported values:
///   "lowercase", "UPPERCASE", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case".
///
/// # Field Attributes
///
/// The following attributes can be used on struct fields:
///
/// - `#[tdb(name = "customName")]` - Custom property name (defaults to field name).
/// - `#[tdb(class = "xsd:string")]` - Specify a custom class for this field.
/// - `#[tdb(doc = "Property documentation")]` - Provide documentation.
///
/// # Examples
///
/// ## Basic Struct Example
///
/// ```rust
/// use terminusdb_schema_derive::TerminusDBModel;
/// use terminusdb_schema::ToTDBSchema;
/// use terminusdb_schema::ToTDBInstance;
/// use terminusdb_schema::FromTDBInstance;
///
/// #[derive(TerminusDBModel)]
/// struct Person {
///     id: String,
///     name: String,
///     age: i32,
///     email: Option<String>,
/// }
///
/// // Now you can use Person::to_schema() to get the TerminusDB schema
/// // and Person::from_instance() to deserialize from TerminusDB instances
/// ```
///
/// ## Enum Example (TaggedUnion)
///
/// ```rust
/// use terminusdb_schema_derive::TerminusDBModel;
/// use terminusdb_schema::ToTDBSchema;
/// use terminusdb_schema::ToTDBInstance;
/// use terminusdb_schema::FromTDBInstance;
///
/// #[derive(TerminusDBModel)]
/// #[tdb(class_name = "ContentType")]
/// enum Content {
///     TextContent,
///     ImageContent,
///     VideoContent,
/// }
///
/// // Now you can use Content::to_schema() to get the TerminusDB schema
/// // and Content::from_instance() to deserialize from TerminusDB instances
/// ```
#[proc_macro_derive(TerminusDBModel, attributes(tdb))]
pub fn derive_terminusdb_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Add debug message to help diagnose
    // trace!("Processing TerminusDBModel derive for: {}", input.ident);

    // Parse the attributes using darling
    let mut opts = match TDBModelOpts::from_derive_input(&input) {
        Result::Ok(opts) => opts,
        Err(err) => {
            // Convert darling::Error to a compile error
            let error_string = err.to_string();
            // trace!("Error parsing attributes: {}", error_string);
            return syn::Error::new(proc_macro2::Span::call_site(), error_string)
                .to_compile_error()
                .into();
        }
    };

    // Store the original input for doc extraction
    opts.original_input = Some(input.clone());

    // Generate implementation based on whether this is a struct or enum
    let expanded = match &input.data {
        Data::Struct(data_struct) => {
            trace!("Implementing for struct: {}", input.ident);
            implement_for_struct(&input, data_struct, &opts)
        }
        Data::Enum(data_enum) => {
            trace!("Implementing for enum: {}", input.ident);
            implement_for_enum(&input, data_enum, &opts)
        }
        Data::Union(_) => {
            trace!("Error: Unions not supported for {}", input.ident);
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "TerminusDBModel derive macro does not support unions",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate InstanceFromJson implementation separately
    // Need to clone `input` as `derive_instance_from_json_impl` might consume it
    let instance_from_json_impl =
        match json_deserialize::derive_instance_from_json_impl(input.clone()) {
            Result::Ok(ts) => ts,
            Err(err) => return err.to_compile_error().into(),
        };

    // Generate FromTDBInstance implementation
    let from_instance_impl = match from_instance::derive_from_terminusdb_instance(input.clone()) {
        Result::Ok(ts) => ts,
        Err(err) => return err.to_compile_error().into(),
    };

    // Combine the results
    let final_output = quote! {
        #expanded
        #instance_from_json_impl
        #from_instance_impl
    };

    final_output.into()
}

/// Automatically derives the `FromTDBInstance` trait for a struct or enum.
///
/// **NOTE: This derive is now a no-op!**
/// The `FromTDBInstance` functionality has been integrated into `TerminusDBModel`.
/// Simply derive `TerminusDBModel` instead to get both serialization and deserialization.
///
/// This derive is kept for backward compatibility and will not generate any code.
/// It may be removed in future versions.
///
/// # Migration
///
/// Change this:
/// ```rust
/// #[derive(TerminusDBModel, FromTDBInstance)]
/// struct Person { ... }
/// ```
///
/// To this:
/// ```rust
/// #[derive(TerminusDBModel)]
/// struct Person { ... }
/// ```
#[proc_macro_derive(FromTDBInstance, attributes(tdb))]
pub fn derive_from_terminusdb_instance(input: TokenStream) -> TokenStream {
    // This is now a no-op - all functionality has been moved to TerminusDBModel
    quote! {
        // FromTDBInstance derive is now a no-op
        // All functionality has been integrated into TerminusDBModel
    }
    .into()
}

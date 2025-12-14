//! Proc macros for TerminusDB testing.
//!
//! Provides `#[terminusdb_test::test]` macro for ergonomic test database setup.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, FnArg, Ident, ItemFn, LitStr, Pat, Token,
};

/// Arguments for the test macro: `#[test(db = "prefix")]`
struct TestArgs {
    db_prefix: LitStr,
}

impl Parse for TestArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident != "db" {
            return Err(syn::Error::new(ident.span(), "expected `db`"));
        }
        let _eq: Token![=] = input.parse()?;
        let db_prefix: LitStr = input.parse()?;
        Ok(TestArgs { db_prefix })
    }
}

/// A test macro that sets up a temporary TerminusDB database.
///
/// # Usage
///
/// ```ignore
/// use terminusdb_test::test;
///
/// #[test(db = "my_test")]
/// async fn test_database_ops(client: _, spec: _) -> anyhow::Result<()> {
///     // client: TerminusDBHttpClient
///     // spec: BranchSpec for the test database
///
///     client.insert_schema(...).await?;
///     Ok(())
/// }
/// ```
///
/// This expands to:
///
/// ```ignore
/// #[tokio::test]
/// async fn test_database_ops() -> anyhow::Result<()> {
///     terminusdb_test::with_test_db("my_test", |client, spec| async move {
///         // your test code here
///         Ok(())
///     }).await
/// }
/// ```
///
/// # Parameters
///
/// The test function can declare these parameters:
/// - `client` - A `TerminusDBHttpClient` connected to the test database
/// - `spec` - A `BranchSpec` for the test database (db name + main branch)
///
/// Both parameters are optional. Use `_` for the type as it will be inferred.
#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as TestArgs);
    let input_fn = parse_macro_input!(item as ItemFn);

    let db_prefix = &args.db_prefix;
    let fn_name = &input_fn.sig.ident;
    let fn_body = &input_fn.block;
    let fn_attrs = &input_fn.attrs;
    let fn_vis = &input_fn.vis;

    // Extract parameter names from the function signature
    let mut param_names: Vec<Ident> = Vec::new();
    for arg in &input_fn.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                param_names.push(pat_ident.ident.clone());
            }
        }
    }

    // Generate closure parameters based on what the function declares
    let closure_params = if param_names.is_empty() {
        quote! { _client, _spec }
    } else if param_names.len() == 1 {
        let p1 = &param_names[0];
        quote! { #p1, _spec }
    } else {
        let p1 = &param_names[0];
        let p2 = &param_names[1];
        quote! { #p1, #p2 }
    };

    let expanded = quote! {
        #(#fn_attrs)*
        #[::tokio::test]
        #fn_vis async fn #fn_name() -> ::anyhow::Result<()> {
            ::terminusdb_test::with_test_db(#db_prefix, |#closure_params| async move
                #fn_body
            ).await
        }
    };

    TokenStream::from(expanded)
}

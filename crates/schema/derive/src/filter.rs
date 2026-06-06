//! Generation of `{Model}Filter` structs from the `TerminusDBModel` derive.
//!
//! When the `filters` feature is enabled on this crate AND a model is annotated
//! `#[tdb(filter)]`, the derive emits a typed GraphQL filter struct co-located
//! with the model, plus the `Filterable` and `TdbGQLFilter` impls that connect
//! it to the ORM. No SDL or separate codegen crate is involved — the filter
//! type for each field is resolved by the compiler via
//! `<FieldTy as terminusdb_schema::Filterable>::Filter`.

use crate::prelude::*;
use quote::format_ident;

/// Entry point: emit filter code for a model, or nothing if filters are
/// disabled / the model didn't opt in / the shape is unsupported.
pub fn generate_filter(input: &DeriveInput, opts: &TDBModelOpts) -> proc_macro2::TokenStream {
    // Opt-out, scoped per-crate: filters are generated for every model in a
    // crate that sets `TERMINUSDB_DERIVE_FILTERS=1` for its own compilation
    // (via its `build.rs`), unless the model opted out with `#[tdb(no_filter)]`.
    //
    // We gate on a build-time env var rather than a Cargo feature on purpose:
    // a Cargo feature unifies across the whole dependency graph, which would
    // force filter generation onto unrelated model crates (e.g. terminusdb-woql2)
    // whose exotic types don't generate clean filters. The env var is set only
    // for the opting-in crate's rustc invocation, so it scopes cleanly.
    if std::env::var("TERMINUSDB_DERIVE_FILTERS").is_err() || opts.no_filter {
        return quote! {};
    }
    // Generic models would need generic filter types — out of scope.
    if !input.generics.params.is_empty() {
        return quote! {};
    }

    match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(fields) => generate_struct_filter(&input.ident, fields, opts),
            // tuple/unit structs aren't filterable models, but other models may
            // reference them — give them an opaque filter so those compile.
            _ => opaque_filterable(&input.ident),
        },
        Data::Enum(de) => {
            let simple = de.variants.iter().all(|v| matches!(v.fields, Fields::Unit));
            if simple {
                generate_enum_filter(&input.ident, de, opts)
            } else {
                // Tagged unions don't have a real filter shape yet; opaque so
                // models with a tagged-union field still compile.
                opaque_filterable(&input.ident)
            }
        }
        _ => quote! {},
    }
}

/// Emit only `Filterable for #model` mapping to `OpaqueFilter`, for model
/// shapes we don't generate a real filter for (so referencing models compile).
fn opaque_filterable(model: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        impl terminusdb_schema::Filterable for #model {
            type Filter = terminusdb_schema::OpaqueFilter;
        }
    }
}

/// `{Model}Filter` for a struct: one `Option<<FieldTy as Filterable>::Filter>`
/// per property field, plus the `_id`/`_ids`/`_and`/`_or`/`_not` envelope.
fn generate_struct_filter(
    model: &syn::Ident,
    fields: &FieldsNamed,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let filter_ident = format_ident!("{}Filter", model);
    let id_field_name = opts.id_field.as_deref();

    // One pass yields, per property field, both its struct declaration and its
    // `to_gql` render statement (so the two never drift out of sync).
    let parts = fields
        .named
        .iter()
        // PhantomData isn't a real property.
        .filter(|f| !is_phantom_data_type(&f.ty))
        // The explicit id field (if any) is represented by the `_id` envelope.
        .filter(|f| match (f.ident.as_ref(), id_field_name) {
            (Some(id), Some(idf)) => *id != idf,
            _ => true,
        })
        .map(|f| {
            let fname = f.ident.as_ref().unwrap();
            let fty = &f.ty;
            let fopts = TDBFieldOpts::from_field(f).unwrap();
            let prop_name = fopts.name.unwrap_or_else(|| fname.to_string());
            // GraphQL property name must match TDB's schema; rename if needed.
            let rename = if prop_name != fname.to_string() {
                quote! { #[serde(rename = #prop_name)] }
            } else {
                quote! {}
            };
            // Relation links (TdbLazy/EntityIDFor) filter by the *target's* filter,
            // which can be recursive (e.g. Project.forked_from -> Project, or
            // mutual A<->B references) and would make the filter struct infinitely
            // sized. Box those to break the layout cycle. Box is serde-transparent.
            let ty_str = quote! { #fty }.to_string();
            let is_link = ty_str.contains("TdbLazy") || ty_str.contains("EntityIDFor");
            let filter_ty = if is_link {
                quote! { Box<<#fty as terminusdb_schema::Filterable>::Filter> }
            } else {
                quote! { <#fty as terminusdb_schema::Filterable>::Filter }
            };
            let decl = quote! {
                #rename
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #fname: Option<#filter_ty>,
            };
            // Each field renders itself via its own `ToGql` (strings quoted,
            // enums bare, nested filters recursive) — no enum/string guessing.
            let render = quote! {
                if let Some(v) = &self.#fname {
                    parts.push(format!("{}: {}", #prop_name, terminusdb_schema::ToGql::to_gql(v)));
                }
            };
            (decl, render)
        })
        .collect::<Vec<_>>();
    let prop_fields = parts.iter().map(|(decl, _)| decl);
    let prop_renders = parts.iter().map(|(_, render)| render);

    quote! {
        #[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
        pub struct #filter_ident {
            #(#prop_fields)*
            // `tdb_`-prefixed rust names avoid collisions with model fields
            // literally named `id`/`and`/`or`/`not`; serde renames to TDB's form.
            #[serde(
                rename = "_id",
                skip_serializing_if = "Option::is_none",
                serialize_with = "terminusdb_schema::serialize_opt_entity_id_iri"
            )]
            pub tdb_id: Option<terminusdb_schema::EntityIDFor<#model>>,
            #[serde(
                rename = "_ids",
                skip_serializing_if = "Option::is_none",
                serialize_with = "terminusdb_schema::serialize_opt_entity_ids_iri"
            )]
            pub tdb_ids: Option<Vec<terminusdb_schema::EntityIDFor<#model>>>,
            #[serde(rename = "_and", skip_serializing_if = "Option::is_none")]
            pub tdb_and: Option<Vec<Box<#filter_ident>>>,
            #[serde(rename = "_or", skip_serializing_if = "Option::is_none")]
            pub tdb_or: Option<Vec<Box<#filter_ident>>>,
            #[serde(rename = "_not", skip_serializing_if = "Option::is_none")]
            pub tdb_not: Option<Box<#filter_ident>>,
        }

        impl terminusdb_schema::Filterable for #model {
            type Filter = #filter_ident;
        }
        impl terminusdb_schema::TdbGQLFilter<#model> for #filter_ident {}

        impl terminusdb_schema::ToGql for #filter_ident {
            fn to_gql(&self) -> String {
                let mut parts: Vec<String> = Vec::new();
                #(#prop_renders)*
                // Envelope: `_id`/`_ids` are quoted IRIs (EntityIDFor::to_gql),
                // `_and`/`_or` are lists of nested filters, `_not` one nested.
                if let Some(v) = &self.tdb_id { parts.push(format!("_id: {}", terminusdb_schema::ToGql::to_gql(v))); }
                if let Some(v) = &self.tdb_ids { parts.push(format!("_ids: {}", terminusdb_schema::ToGql::to_gql(v))); }
                if let Some(v) = &self.tdb_and { parts.push(format!("_and: {}", terminusdb_schema::ToGql::to_gql(v))); }
                if let Some(v) = &self.tdb_or { parts.push(format!("_or: {}", terminusdb_schema::ToGql::to_gql(v))); }
                if let Some(v) = &self.tdb_not { parts.push(format!("_not: {}", terminusdb_schema::ToGql::to_gql(v))); }
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
}

/// `{Enum}Filter` for a simple (unit-variant) enum: `{ eq, ne }` over the enum,
/// mirroring TDB's `*_Enum_Filter`. Also emits `ToGql` for the enum itself,
/// rendering each variant to its **bare** TDB value (the same value the schema
/// uses — `rename_strategy.apply`, defaulting to lowercase).
fn generate_enum_filter(
    model: &syn::Ident,
    de: &syn::DataEnum,
    opts: &TDBModelOpts,
) -> proc_macro2::TokenStream {
    let filter_ident = format_ident!("{}Filter", model);

    // Mirror enum_simple.rs: default the rename strategy to lowercase.
    let rename_strategy = match opts.get_rename_strategy() {
        crate::args::RenameStrategy::None => crate::args::RenameStrategy::Lowercase,
        other => other,
    };
    let value_arms = de.variants.iter().map(|variant| {
        let vident = &variant.ident;
        let rendered = rename_strategy.apply(&vident.to_string());
        quote! { #model::#vident => #rendered.to_string() }
    });

    quote! {
        #[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
        pub struct #filter_ident {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub eq: Option<#model>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ne: Option<#model>,
        }

        impl terminusdb_schema::Filterable for #model {
            type Filter = #filter_ident;
        }
        impl terminusdb_schema::TdbGQLFilter<#model> for #filter_ident {}

        impl terminusdb_schema::ToGql for #model {
            fn to_gql(&self) -> String {
                match self { #(#value_arms),* }
            }
        }
        impl terminusdb_schema::ToGql for #filter_ident {
            fn to_gql(&self) -> String {
                let mut parts: Vec<String> = Vec::new();
                if let Some(v) = &self.eq { parts.push(format!("eq: {}", terminusdb_schema::ToGql::to_gql(v))); }
                if let Some(v) = &self.ne { parts.push(format!("ne: {}", terminusdb_schema::ToGql::to_gql(v))); }
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
}

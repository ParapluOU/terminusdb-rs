use crate::prelude::*;

/// Check if the input has generic parameters and handle accordingly based on feature flag
pub fn check_generics(input: &DeriveInput) -> Result<(), syn::Error> {
    if !input.generics.params.is_empty() {
        #[cfg(not(feature = "generic-derive"))]
        {
            return Err(syn::Error::new(
                input.generics.span(),
                "Generic types are not yet supported in TerminusDBModel derive macro. \
                 To experiment with generic support, enable the 'generic-derive' feature flag. \
                 Note: Generic support is experimental and may have limitations.",
            ));
        }
        
        #[cfg(feature = "generic-derive")]
        {
            // When feature is enabled, we allow generics
            // Additional validation can be added here
            validate_generic_params(input)?;
        }
    }
    Ok(())
}

#[cfg(feature = "generic-derive")]
fn validate_generic_params(input: &DeriveInput) -> Result<(), syn::Error> {
    use syn::{GenericParam, TypeParam};
    
    // Check for unsupported generic parameter types
    for param in &input.generics.params {
        match param {
            GenericParam::Type(TypeParam { .. }) => {
                // Type parameters are supported
            }
            GenericParam::Lifetime(_) => {
                return Err(syn::Error::new(
                    param.span(),
                    "Lifetime parameters are not yet supported in generic TerminusDBModel types",
                ));
            }
            GenericParam::Const(_) => {
                return Err(syn::Error::new(
                    param.span(),
                    "Const generics are not yet supported in generic TerminusDBModel types",
                ));
            }
        }
    }
    
    Ok(())
}

#[cfg(feature = "generic-derive")]
pub fn extract_generic_bounds(input: &DeriveInput) -> Vec<syn::WherePredicate> {
    let mut bounds = Vec::new();
    
    // Extract existing where clause predicates
    if let Some(where_clause) = &input.generics.where_clause {
        bounds.extend(where_clause.predicates.iter().cloned());
    }
    
    // For each type parameter, we'll need to add TerminusDB-specific bounds
    // This is a placeholder - actual implementation would analyze field usage
    for param in &input.generics.params {
        if let syn::GenericParam::Type(type_param) = param {
            let ident = &type_param.ident;
            
            // Add basic trait bounds that all generic types need
            let bound: syn::WherePredicate = syn::parse_quote! {
                #ident: terminusdb_schema::ToTDBSchema + 
                        terminusdb_schema::ToTDBInstance + 
                        terminusdb_schema::FromTDBInstance +
                        terminusdb_schema::InstanceFromJson +
                        std::fmt::Debug
            };
            bounds.push(bound);
        }
    }
    
    bounds
}
# Supporting Generic Types in TerminusDBModel Derive Macro

## Overview
This document provides a comprehensive analysis of supporting generic type parameters in the `#[derive(TerminusDBModel)]` macro, tracing the complete flow from derive input to generated trait implementations.

## Complete Execution Flow Trace

### 1. Entry Point: `derive_terminusdb_model` (lib.rs:155)
```rust
#[proc_macro_derive(TerminusDBModel, attributes(tdb))]
pub fn derive_terminusdb_model(input: TokenStream) -> TokenStream
```

**Flow:**
1. Parse input as `DeriveInput`
2. Parse attributes using `TDBModelOpts::from_derive_input`
3. Check for generics (currently errors without feature flag)
4. Route to struct/enum implementation
5. Generate three trait implementations:
   - Main traits (ToTDBSchema, ToTDBInstance, ToTDBInstances)
   - InstanceFromJson
   - FromTDBInstance

### 2. Attribute Processing (args.rs)

**`TDBModelOpts`** - Container for struct/enum-level attributes:
- `class_name`: Custom schema name
- `key`: Key strategy (random, hash, value_hash, lexical)
- `key_fields`: Fields for lexical/hash keys
- `id_field`: Field mapped to @id property
- `base`, `inherits`, `doc`, etc.

**`TDBFieldOpts`** - Container for field-level attributes:
- `name`: Custom property name
- `class`: Override field type class
- `doc`: Field documentation

### 3. Struct Processing Flow

#### 3.1 Schema Generation (`generate_totdbschema_impl` in schema.rs)

**Input:** Struct metadata, parsed options
**Output:** `impl ToTDBSchema for StructName`

**Process:**
1. Generate schema metadata (base, key, inheritance, etc.)
2. Process fields via `process_named_fields` (struct.rs:217)
   - For each field:
     - Get field type and options
     - Generate property using `ToSchemaProperty` trait
     - Apply class overrides if specified
3. Generate `to_schema_tree_mut` implementation
   - Collects schemas from all field types recursively
   - Uses `ToMaybeTDBSchema` to handle types that may not have schemas

**Generated Code Pattern:**
```rust
impl ToTDBSchema for StructName {
    fn to_schema() -> Schema {
        Schema::Class {
            properties: Some(vec![
                <FieldType as ToSchemaProperty<StructName>>::to_property("field_name"),
                // ... more fields
            ]),
            // ... other schema fields
        }
    }
    
    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        // Add self schema
        // Recursively add field type schemas
        <FieldType as ToMaybeTDBSchema>::to_schema_tree_mut(collection);
    }
}
```

#### 3.2 Instance Generation (`generate_totdbinstance_impl` in instance.rs)

**Input:** Type name, instance body code, options
**Output:** `impl ToTDBInstance + ToTDBInstances`

**Process:**
1. Handle `id_field` if specified
   - Extract ID value using `ToInstanceProperty`
   - Convert to `Option<String>`
2. Generate instance with properties from `process_fields_for_instance`
   - Each field converted via `ToInstanceProperty<Parent>`
3. Implement `ToTDBInstances` using helper function

**Generated Code Pattern:**
```rust
impl ToTDBInstance for StructName {
    fn to_instance(&self, id: Option<String>) -> Instance {
        let schema = Self::to_schema();
        let optid_val = /* extract from id_field if configured */;
        
        let mut properties = BTreeMap::new();
        properties.insert(
            "field_name".to_string(),
            <FieldType as ToInstanceProperty<Self>>::to_property(
                self.field_name.clone(),
                "field_name",
                &schema
            )
        );
        
        Instance { id: id.or(optid_val), schema, properties, ... }
    }
}
```

#### 3.3 JSON Deserialization (`derive_instance_from_json_impl` in json_deserialize.rs)

**Process:**
1. Generate field deserializers
2. Handle @type validation
3. Convert JSON to Instance with proper types

#### 3.4 FromTDBInstance Generation (`derive_from_terminusdb_instance` in from_instance.rs)

**Process:**
1. Validate instance schema matches expected type
2. Extract properties for each field
3. Use `FromInstanceProperty` trait for conversion

### 4. Trait Dependencies and Field Type Requirements

#### For Schema Generation (ToTDBSchema)
Each field type `T` must implement:
- `ToSchemaProperty<Parent>` - Generates property definition
- `ToMaybeTDBSchema` - Allows recursive schema collection
- `ToSchemaClass` (for determining property class)

#### For Instance Serialization (ToTDBInstance)
Each field type `T` must implement:
- `ToInstanceProperty<Parent>` - Converts value to property
- Additional traits depending on usage

#### For Deserialization (FromTDBInstance)
Each field type `T` must implement:
- `FromInstanceProperty` - Converts property back to value

### 5. Generic Type Challenges

#### 5.1 Trait Bound Analysis
For generic struct `Container<T>`, the derive macro must determine which traits `T` needs:

**Field Usage Analysis Required:**
```rust
struct Container<T> {
    value: T,           // Needs: ToInstanceProperty<Container<T>>, FromInstanceProperty
    items: Vec<T>,      // Needs: T to have certain traits for Vec impl
    optional: Option<T> // Needs: T traits for Option impl
}
```

#### 5.2 Schema Generation Issues
- `ToSchemaClass::to_class()` returns `&'static str` - cannot be determined for generic `T`
- Schema properties need concrete type information
- `to_schema_tree_mut` must handle generic type parameters

#### 5.3 Property Conversion Complexity
- `ToInstanceProperty<Parent>` creates circular dependencies with generic parents
- Need to propagate parent type information through conversions

### 6. Critical Implementation Details

#### 6.1 How Field Types are Processed

**In `process_named_fields` (struct.rs:217):**
```rust
<#field_ty as ToSchemaProperty<#struct_name>>::to_property(#property_name)
```

This means for generic `Container<T>`, field type `T` must implement `ToSchemaProperty<Container<T>>`.

**In `process_fields_for_instance` (instance.rs:73):**
```rust
<_ as ToInstanceProperty<Self>>::to_property(
    self.#field_name.clone(),
    &#property_name,
    &schema
)
```

The `Self` here refers to the struct being processed, creating the circular dependency.

#### 6.2 The ToMaybeTDBSchema Trait

This trait (impl/generic.rs) provides a default "no schema" implementation for all types:
```rust
impl<T> ToMaybeTDBSchema for T {
    default fn to_schema() -> Option<Schema> { None }
}

// Specialized for types that do have schemas
impl<T: ToTDBSchema> ToMaybeTDBSchema for T {
    fn to_schema() -> Option<Schema> { Some(T::to_schema()) }
}
```

This allows the macro to call `to_schema_tree_mut` on any field type without knowing if it has a schema.

#### 6.3 Specialization and Default Implementations

The codebase heavily uses Rust's specialization feature:
- Generic blanket implementations with `default` methods
- More specific implementations override defaults
- Example: `ToInstanceProperty` for all `T: ToTDBInstance` (generic.rs:38)

### 7. Implementation Strategy for Generics

#### Phase 1: Modify Struct Processing
```rust
// In struct.rs, add at the beginning of implement_for_struct:
let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

// Analyze fields to collect required bounds
let mut type_param_bounds = HashMap::new();
for field in &fields_named.named {
    collect_generic_bounds(&field.ty, &input.generics, &mut type_param_bounds);
}

// Build where clause predicates
let additional_predicates = build_where_predicates(&type_param_bounds, struct_name);
```

#### Phase 2: Bound Collection Algorithm
```rust
fn collect_generic_bounds(
    ty: &Type,
    generics: &Generics,
    bounds: &mut HashMap<Ident, Vec<TokenStream>>
) {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            if let Some(ident) = path.get_ident() {
                // Check if this is a generic parameter
                if generics.params.iter().any(|p| {
                    matches!(p, GenericParam::Type(t) if t.ident == *ident)
                }) {
                    // Add required bounds for direct usage
                    bounds.entry(ident.clone()).or_default().extend(vec![
                        quote! { ToTDBSchema },
                        quote! { ToMaybeTDBSchema },
                        quote! { ToSchemaClass },
                        quote! { ToSchemaProperty<#struct_name> },
                        quote! { ToInstanceProperty<#struct_name> },
                        quote! { FromInstanceProperty },
                        quote! { InstanceFromJson },
                        quote! { std::fmt::Debug },
                    ]);
                }
            }
            
            // Recursively check generic arguments
            if let Some(last_segment) = path.segments.last() {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(inner_ty) = arg {
                            collect_generic_bounds(inner_ty, generics, bounds);
                        }
                    }
                }
            }
        }
        // Handle other type variants...
    }
}
```

#### Phase 3: Modify All Trait Implementations
Each generated impl block needs generic parameters:

```rust
// In generate_totdbschema_impl:
quote! {
    impl #impl_generics ToTDBSchema for #struct_name #ty_generics
    #where_clause
    where
        #(#additional_predicates,)*
    {
        // ... existing implementation
    }
}
```

### 8. Specific Issues and Solutions

#### Issue 1: Static Schema Names
**Problem:** `ToSchemaClass::to_class()` returns `&'static str`, but generic `T` class name isn't known at compile time.

**Solution:** For generic parameters, generate a runtime schema name:
```rust
impl<T> ToSchemaClass for Container<T> {
    fn to_class() -> &'static str {
        // This won't work - need runtime solution
        // Perhaps change trait to return String instead
    }
}
```

#### Issue 2: Circular Trait Dependencies
**Problem:** `T: ToInstanceProperty<Container<T>>` creates circular dependency.

**Solution:** May need to modify trait design or use associated types:
```rust
trait ToInstanceProperty {
    type Parent;
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty;
}
```

#### Issue 3: Schema Tree Collection
**Problem:** Generic types don't know their concrete schemas at compile time.

**Solution:** The `ToMaybeTDBSchema` trait already handles this by returning `Option<Schema>`.
```

## Current State Analysis

### Trait Requirements
The `TerminusDBModel` trait requires implementing several traits:
1. **`ToTDBSchema`** - Converts type to TerminusDB schema
2. **`ToTDBInstance`** - Converts instance to TerminusDB instance format
3. **`ToTDBInstances`** - Handles instance trees (collections of related instances)
4. **`FromTDBInstance`** - Deserializes from TerminusDB instance
5. **`InstanceFromJson`** - JSON deserialization support
6. **`std::fmt::Debug`** - Debug formatting

### Current Implementation Limitations
The derive macro currently:
- Only handles concrete types (no generic parameters)
- Generates static schema definitions at compile time
- Doesn't propagate trait bounds for generic parameters
- Assumes all field types have specific trait implementations

### How Generics Work in Manual Implementations
Looking at `Option<T>` implementation in `schema/src/impl/opt.rs`:
```rust
impl<T: ToTDBSchema> ToTDBSchema for Option<T> {
    fn to_schema() -> Schema {
        T::to_schema()
    }
}

impl<T: ToTDBInstance, S> ToInstanceProperty<S> for Option<T> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        // Implementation details
    }
}
```

Key observations:
- Generic parameters require trait bounds (`T: ToTDBSchema`)
- Implementations delegate to the inner type's implementations
- Uses Rust's specialization feature for more specific implementations

## Challenges with Generic Types

### 1. Trait Bound Propagation
**Challenge**: The derive macro must generate appropriate trait bounds for all generic parameters.

**Example**: For `struct Container<T> { value: T }`, the generated code must include:
```rust
impl<T> ToTDBSchema for Container<T> 
where
    T: ToTDBSchema + ToSchemaClass + ToSchemaProperty<Container<T>>
{
    // ...
}
```

### 2. Schema Generation at Compile Time
**Challenge**: Schema must be generated statically, but generic types are not known until instantiation.

**Issues**:
- Cannot determine concrete schema for `T` at macro expansion time
- Schema depends on the actual type used for `T`
- Field properties need to delegate to `T`'s schema information

### 3. Field Type Analysis
**Challenge**: The macro analyzes field types to generate property definitions. With generics:
- `field_ty` might be `T`, `Vec<T>`, `Option<T>`, etc.
- Need to determine which traits `T` must implement
- Complex nested generics (e.g., `HashMap<String, Vec<T>>`)

### 4. Associated Types and Schema Class
**Challenge**: `ToSchemaClass` trait has associated constants that must be known at compile time.

```rust
pub trait ToSchemaClass {
    fn to_class() -> &'static str;
}
```

For generic types, this can't be determined statically.

### 5. Instance Property Conversion
**Challenge**: `ToInstanceProperty<Parent>` trait requires parent type information.

For generic struct `Foo<T>`:
- Fields of type `T` need `T: ToInstanceProperty<Foo<T>>`
- This creates complex recursive bounds

## Design Approach

### Phase 1: Basic Generic Support
1. **Parse generic parameters** from the struct definition
2. **Collect required trait bounds** by analyzing field usage
3. **Generate implementations** with appropriate where clauses
4. **Delegate to inner type implementations** for schema and instance conversion

### Phase 2: Implementation Strategy

#### 1. Modify Derive Input Processing
```rust
// In lib.rs
let generics = &input.generics;
let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
```

#### 2. Analyze Field Types for Required Bounds
Create a bound collector that examines each field type:
- Direct generic usage: `T` → requires all traits
- Wrapped generics: `Option<T>` → requires subset of traits
- Complex nesting: analyze recursively

#### 3. Generate Trait Implementations with Bounds
```rust
impl #impl_generics ToTDBSchema for #struct_name #ty_generics 
#where_clause
where
    #(#additional_bounds,)*
{
    // Implementation
}
```

#### 4. Handle Schema Generation
For generic types, schema generation must:
- Delegate to concrete type's schema
- Use phantom data or type information at runtime
- Consider making schema generation fallible for unsupported combinations

### Phase 3: Complex Cases

#### Handling Multiple Generic Parameters
```rust
struct Pair<T, U> {
    first: T,
    second: U,
}
```
Each parameter needs independent bound analysis.

#### Handling Constrained Generics
```rust
struct NumericContainer<T: std::ops::Add> {
    value: T,
}
```
Must preserve existing bounds and add TerminusDB-specific ones.

#### Handling Associated Types
May need to generate additional bounds for associated types used in fields.

## Implementation Plan

### Step 1: Minimal Viable Implementation
1. Support single generic parameter `<T>`
2. Require `T` to implement all necessary traits
3. Generate simple delegating implementations
4. Test with basic types

### Step 2: Bound Optimization
1. Analyze actual trait usage per field
2. Generate minimal required bounds
3. Support standard container types

### Step 3: Advanced Features
1. Multiple generic parameters
2. Lifetime parameters
3. Const generics (future)
4. Complex nested generics

## Unclarities and Open Questions

### 1. Schema Identity
**Question**: How should generic types be identified in the schema?
- Use type name with parameters? (`Container<String>`)
- Generate separate schemas per instantiation?
- Runtime schema generation?

### 2. Specialization Requirements
**Question**: Will we need specialization for common patterns?
- `Vec<T>` vs `Vec<String>` might need different handling
- Performance implications

### 3. Backwards Compatibility
**Question**: How to ensure existing code continues to work?
- Non-generic types should work exactly as before
- Migration path for users

### 4. Error Messages
**Question**: How to provide helpful error messages?
- Missing trait implementations on generic parameters
- Invalid generic usage patterns
- Clear diagnostics

### 5. Performance Impact
**Question**: What's the compile-time cost?
- More complex trait resolution
- Potential for exponential bound growth
- Monomorphization concerns

## Feature Flag Strategy

### Gradual Rollout with Feature Flag
To safely experiment with generic support without breaking existing functionality:

1. **Add feature flag to schema-derive/Cargo.toml**:
```toml
[features]
generic-derive = []
```

2. **Conditionally compile generic support**:
```rust
#[cfg(feature = "generic-derive")]
fn handle_generics(input: &DeriveInput) -> TokenStream {
    // New generic-aware implementation
}

#[cfg(not(feature = "generic-derive"))]
fn handle_generics(input: &DeriveInput) -> TokenStream {
    // Return error if generics detected
    if !input.generics.params.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "Generic types are not yet supported. Enable 'generic-derive' feature to experiment with generic support."
        ).to_compile_error();
    }
    // Continue with existing implementation
}
```

3. **Test with feature flag**:
```bash
cargo test --features generic-derive
```

This approach allows:
- Existing code continues to work unchanged
- Developers can opt-in to test generic support
- Easy rollback if issues are found
- Gradual migration once stable

## Testing Strategy

### Unit Tests
1. Simple generic struct with one parameter
2. Multiple generic parameters
3. Nested generics (generic fields containing generics)
4. Generic enums
5. Mixed concrete and generic fields

### Integration Tests
1. Full round-trip serialization/deserialization
2. Schema generation and validation
3. Database operations with generic types
4. Performance benchmarks

### Edge Cases
1. Recursive generic types
2. Zero-sized types
3. Lifetime parameters
4. Associated type projections

## Example Generated Code

### Input
```rust
#[derive(TerminusDBModel)]
struct Container<T> {
    id: String,
    value: T,
    items: Vec<T>,
}
```

### Expected Generated Output
```rust
impl<T> ToTDBSchema for Container<T>
where
    T: ToTDBSchema + ToSchemaClass + ToMaybeTDBSchema,
    Vec<T>: ToSchemaClass,
{
    fn to_schema() -> Schema {
        Schema::Class {
            id: "Container".to_string(),
            properties: Some(vec![
                Property {
                    name: "id".to_string(),
                    class: "xsd:string".to_string(),
                    r#type: None,
                },
                <T as ToSchemaProperty<Container<T>>>::to_property("value"),
                <Vec<T> as ToSchemaProperty<Container<T>>>::to_property("items"),
            ]),
            // ... other schema fields
        }
    }
    
    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        let schema = Self::to_schema();
        if !collection.contains(&schema) {
            collection.insert(schema);
            T::to_schema_tree_mut(collection);
            Vec::<T>::to_schema_tree_mut(collection);
        }
    }
}

impl<T> ToTDBInstance for Container<T>
where
    T: ToTDBSchema + ToTDBInstance + ToInstanceProperty<Container<T>>,
    Vec<T>: ToInstanceProperty<Container<T>>,
{
    fn to_instance(&self, id: Option<String>) -> Instance {
        let schema = Self::to_schema();
        let mut properties = BTreeMap::new();
        
        properties.insert(
            "id".to_string(),
            self.id.to_property("id", &schema)
        );
        properties.insert(
            "value".to_string(),
            self.value.to_property("value", &schema)
        );
        properties.insert(
            "items".to_string(),
            self.items.to_property("items", &schema)
        );
        
        Instance {
            id: id.or_else(|| Some(self.hash_key_id())),
            schema,
            properties,
            // ... other instance fields
        }
    }
}

impl<T> FromTDBInstance for Container<T>
where
    T: FromTDBInstance,
{
    fn from_instance(instance: &Instance) -> anyhow::Result<Self> {
        Ok(Container {
            id: String::from_property(
                instance.get_property("id")
                    .ok_or_else(|| anyhow::anyhow!("Missing field: id"))?
            )?,
            value: T::from_property(
                instance.get_property("value")
                    .ok_or_else(|| anyhow::anyhow!("Missing field: value"))?
            )?,
            items: Vec::<T>::from_property(
                instance.get_property("items")
                    .ok_or_else(|| anyhow::anyhow!("Missing field: items"))?
            )?,
        })
    }
}
```

## Testing the Implementation

### Without Feature Flag (Default)
```bash
# This will show error for generic types
cargo test -p terminusdb-schema-derive

# Expected: Compilation error with helpful message for generic structs
```

### With Feature Flag
```bash
# This enables generic support and runs generic tests
cargo test -p terminusdb-schema-derive --features generic-derive

# Run specific generic test
cargo test -p terminusdb-schema-derive --features generic-derive generic_test
```

### Integration Testing
```bash
# Test that non-generic code still works with feature enabled
cargo test -p terminusdb-schema-derive --all-features

# Verify no regression in existing functionality
cargo test -p terminusdb-schema
```

## Current Status

As of this analysis:
1. **Created**: Comprehensive analysis document (`.tasks/support-generic.md`)
2. **Added**: Feature flag `generic-derive` to control experimental support
3. **Implemented**: Basic generic checking in derive macro
4. **Created**: Test file `generic_test.rs` with examples of expected functionality
5. **Added**: Error handling for generics when feature is disabled

### Next Steps for Implementation
1. Implement actual generic support in `struct.rs` when feature is enabled
2. Add trait bound analysis and generation
3. Update schema generation to handle generic parameters
4. Implement instance conversion for generic types
5. Add comprehensive test coverage

## Conclusion

Supporting generics in `TerminusDBModel` derive macro is complex but achievable. The main challenges are:
1. Proper trait bound generation and propagation
2. Static schema generation for dynamic types
3. Maintaining backwards compatibility
4. Providing good error messages

The implementation should proceed incrementally, starting with simple cases and gradually adding complexity. Each phase should be thoroughly tested before moving to the next. The feature flag approach allows safe experimentation without breaking existing code.
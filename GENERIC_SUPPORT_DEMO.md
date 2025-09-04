# Generic Type Support in TerminusDBModel

The `#[derive(TerminusDBModel)]` macro now supports generic type parameters, including multiple generic parameters like `Struct<T1, T2>`.

## Important: Required Trait Bounds

When using `EntityIDFor<T>` in generic structs, you must add trait bounds to the struct definition. This is a Rust requirement - the struct fields must be valid at the point of definition.

### Single Generic Parameter

```rust
use terminusdb_schema::{EntityIDFor, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T: ToTDBSchema> {
    id: String,
    referenced: EntityIDFor<T>,
}
```

### Multiple Generic Parameters

```rust
#[derive(Debug, Clone, TerminusDBModel)]  
struct Pair<T1: ToTDBSchema, T2: ToTDBSchema> {
    id: String,
    first: EntityIDFor<T1>,
    second: EntityIDFor<T2>,
}
```

### Using Type Aliases (Recommended)

To avoid repeating bounds, you can create type aliases:

```rust
// Define a trait alias for all required bounds
trait Model: ToTDBSchema + Clone + Send + Sync + Debug {}

// Implement for all types that meet the requirements
impl<T> Model for T where T: ToTDBSchema + Clone + Send + Sync + Debug {}

// Now you can use the simpler bound
#[derive(Debug, Clone, TerminusDBModel)]
struct Container<T: Model> {
    id: String,
    items: Vec<EntityIDFor<T>>,
}
```

## How It Works

1. **Struct Definition**: You add minimal bounds (like `T: ToTDBSchema`) to make the struct fields valid
2. **Derive Macro**: The macro automatically adds all additional bounds needed for the trait implementations
3. **Runtime Type Names**: Generic types generate dynamic class names like `"Pair<Person, Product>"`

## Example Usage

```rust
#[derive(Debug, Clone, TerminusDBModel)]
struct User {
    id: String,
    name: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Product {
    id: String,
    title: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T: ToTDBSchema> {
    id: String,
    target: EntityIDFor<T>,
}

// Usage
let user_ref = Reference::<User> {
    id: "ref-1".to_string(),
    target: EntityIDFor::new("User/user-123").unwrap(),
};

// Different instantiations have different schema names
assert_eq!(Reference::<User>::to_class(), "Reference<User>");
assert_eq!(Reference::<Product>::to_class(), "Reference<Product>");
```

## Feature Flag

Generic support requires the `generic-derive` feature flag:

```toml
[dependencies]
terminusdb-schema-derive = { version = "0.1", features = ["generic-derive"] }
```

## Limitations

- Const generics and lifetime parameters are not supported
- All generic type parameters used with `EntityIDFor<T>` must have at least `T: ToTDBSchema` bound
- The derive macro cannot add bounds to the struct definition itself (Rust limitation)
# TerminusDB Schema Derive

This crate provides a procedural macro for deriving TerminusDB schema
definitions from Rust structs and enums.

## Overview

The `TerminusDBModel` derive macro allows you to automatically generate
TerminusDB schema classes from your Rust data structures. This simplifies the
creation and maintenance of your graph database schema.

## Usage

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
parture_terminusdb_schema = "0.1.0"
parture_terminusdb_schema_derive = "0.1.0"
```

### Basic Example

```rust
use parture_terminusdb_schema::{Schema, ToTDBSchema};
use parture_terminusdb_schema_derive::TerminusDBModel;

/// A simple person model
#[derive(TerminusDBModel)]
#[tdb(class_name = "Person")]
struct Person {
    /// The person's name
    #[tdb(property_type = "String")]
    name: String,
    
    /// The person's age
    #[tdb(property_type = "Integer")]
    age: u32,
    
    /// Whether the person is active
    #[tdb(property_type = "Boolean")]
    active: bool,
}

fn main() {
    // Generate a TerminusDB schema from the Person model
    let schema = Person::schema();
    
    // Use the schema with your TerminusDB client
    // ...
}
```

## Attributes

### Struct/Enum Attributes

- `class_name`: The name of the class in TerminusDB (default: struct/enum name)
- `inherits`: Parent class(es) to inherit from
- `abstract_class`: Whether the class is abstract (default: false)
- `doc`: Documentation for the class (uses doc comments by default)

### Field Attributes

- `property_type`: The TerminusDB data type for the property
  - Built-in types: "String", "Integer", "Decimal", "Boolean", "DateTime"
  - Custom class types: use the class name
- `name`: Override the property name in TerminusDB (default: field name)
- `optional`: Whether the property is optional (default: false)
- `is_set`: Whether the property is a set of values (default: false)
- `doc`: Documentation for the property (uses doc comments by default)

## Advanced Example

You can create class hierarchies and relationships between classes:

```rust
#[derive(TerminusDBModel)]
#[tdb(class_name = "Entity", abstract_class = true)]
struct Entity {
    #[tdb(property_type = "String")]
    id: String,
    
    #[tdb(property_type = "DateTime")]
    created_at: String,
}

#[derive(TerminusDBModel)]
#[tdb(class_name = "Person", inherits = "Entity")]
struct Person {
    #[tdb(property_type = "String")]
    name: String,
    
    #[tdb(property_type = "String", optional = true)]
    email: Option<String>,
}

#[derive(TerminusDBModel)]
#[tdb(class_name = "Organization", inherits = "Entity")]
struct Organization {
    #[tdb(property_type = "String")]
    name: String,
    
    #[tdb(property_type = "Person", is_set = true)]
    members: Vec<String>, // IDs of Person objects
}
```

## Limitations

- Currently only supports structs with named fields
- Does not support tuple structs or newtype structs
- Does not currently support unit structs
- Does not support unions

Check the examples directory for more complete examples.

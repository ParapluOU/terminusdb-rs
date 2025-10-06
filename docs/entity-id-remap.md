# EntityIDFor Type-Safe Remap Guide

## Overview

The `EntityIDFor` type now supports type-safe remapping with specialized behavior for TaggedUnion variants. This allows you to convert entity IDs between related types while preserving the full typed path when needed.

## What Changed

### New Marker Traits

Two new marker traits have been added:

```rust
/// Marker for TaggedUnion enum types
pub trait TaggedUnion: ToTDBSchema {}

/// Marker for types that are variants of a TaggedUnion
/// Generic over the parent union to support multi-union variants
pub trait TaggedUnionVariant<Union: TaggedUnion>: ToTDBSchema {}
```

These traits are **automatically implemented** by the `#[derive(TerminusDBModel)]` macro for TaggedUnions and their variants.

### Enhanced Remap Functionality

The `EntityIDFor::remap()` method now uses a new `EntityIDRemap<To>` trait with specialization:

- **Default behavior (regular types)**: Uses only the ID segment
  ```rust
  // "MyModel/123" → "OtherModel/123"
  EntityIDFor::<MyModel>::new("MyModel/123")?.remap::<OtherModel>()
  ```

- **Specialized behavior (TaggedUnion variants)**: Preserves full typed path + base URI
  ```rust
  // "VariantType/123" → "VariantType/123" (preserves variant type!)
  EntityIDFor::<VariantType>::new("VariantType/123")?.remap::<UnionType>()
  ```

## When to Use Remap

### ✅ Use Case: Converting Variant IDs to Union IDs

When you have an ID for a specific variant type and need to use it as the parent TaggedUnion type:

```rust
#[derive(TerminusDBModel)]
enum PaymentMethod {
    CreditCard { card_number: String, cvv: String },
    BankTransfer { account: String, routing: String },
}

// You have a variant ID from somewhere (e.g., query result)
let variant_id: EntityIDFor<PaymentMethodCreditCard> =
    EntityIDFor::new("PaymentMethodCreditCard/abc123")?;

// Convert to union type while preserving the variant information
let union_id: EntityIDFor<PaymentMethod> = variant_id.remap();

// union_id.typed() == "PaymentMethodCreditCard/abc123"
// union_id.id() == "abc123"
// union_id.get_type_name() == "PaymentMethodCreditCard"
```

### ✅ Use Case: Generic Functions with Type Relationships

```rust
fn process_variant<V, U>(variant_id: EntityIDFor<V>) -> EntityIDFor<U>
where
    V: TaggedUnionVariant<U>,
    U: TaggedUnion,
{
    variant_id.remap() // Type-safe, compiler enforced!
}
```

### ✅ Use Case: Single-Field Model Variants

TaggedUnions with single-field variants that wrap model types:

```rust
#[derive(TerminusDBModel)]
struct UserLoginEvent {
    user_id: String,
    timestamp: String,
}

#[derive(TerminusDBModel)]
enum ActivityEvent {
    UserLogin(UserLoginEvent),           // ✅ Model type → gets TaggedUnionVariant
    SystemShutdown { reason: String },   // ✅ Multi-field → gets TaggedUnionVariant
}

let login_id: EntityIDFor<UserLoginEvent> = EntityIDFor::new("UserLoginEvent/xyz789")?;
let event_id: EntityIDFor<ActivityEvent> = login_id.remap(); // Works!
```

## Limitations

### ❌ Primitive Single-Field Variants

TaggedUnions with single-field variants that wrap **primitives** do NOT get `TaggedUnionVariant`:

```rust
#[derive(TerminusDBModel)]
enum Source {
    Post(String),    // ❌ Primitive → filtered out
    Url(String),     // ❌ Primitive → filtered out
}

// This will NOT compile:
// let string_id: EntityIDFor<String> = ...;
// let source_id = string_id.remap::<Source>(); // ERROR: String doesn't implement TaggedUnionVariant<Source>
```

**Why?** The same primitive (e.g., `String`) can appear in multiple TaggedUnions, which would violate Rust's trait coherence rules.

**Filtered primitives:**
- Standard types: `String`, `&str`, `bool`, `char`, numeric types
- TerminusDB primitives: `XSDAnySimpleType`, `DateTime`, `NaiveTime`, `Uuid`

### Workaround for Primitive Variants

For primitive variants, construct the ID directly:

```rust
// Instead of remapping, construct directly
let source_id: EntityIDFor<Source> = EntityIDFor::new("Post/my-id")?;
```

## Auto-Implementation Rules

The derive macro automatically implements marker traits based on variant structure:

| Variant Type | Example | Gets `TaggedUnionVariant`? |
|--------------|---------|---------------------------|
| Multi-field named | `CreditCard { card: String, cvv: String }` | ✅ Yes (via virtual struct) |
| Multi-field tuple | `Point(f64, f64)` | ✅ Yes (via virtual struct) |
| Single-field model | `UserLogin(UserLoginEvent)` | ✅ Yes (if not a known primitive) |
| Single-field primitive | `Node(String)` | ❌ No (filtered by heuristic) |
| Unit variant | `Cash` | ❌ No (no associated data) |

## Migration Guide

### No Breaking Changes

Existing code continues to work without modification:

```rust
// Old code still works exactly as before
let old_id: EntityIDFor<TypeA> = EntityIDFor::new("TypeA/123")?;
let new_id: EntityIDFor<TypeB> = old_id.remap();
// Behavior unchanged for non-TaggedUnion types
```

### New Capabilities

If you're working with TaggedUnions, you can now:

1. **Type-safe variant-to-union conversion**
   ```rust
   fn handle_payment_variant<V>(id: EntityIDFor<V>)
   where V: TaggedUnionVariant<PaymentMethod>
   {
       let payment_id: EntityIDFor<PaymentMethod> = id.remap();
       // ...
   }
   ```

2. **Preserve variant type information**
   ```rust
   let variant_id: EntityIDFor<CreditCardVariant> = get_from_db()?;
   let union_id = variant_id.remap::<PaymentMethod>();
   // union_id preserves "CreditCardVariant/..." not just the ID
   ```

## Examples

### Complete Example: Order Processing

```rust
use terminusdb_schema::{EntityIDFor, TaggedUnion, TaggedUnionVariant};

#[derive(TerminusDBModel)]
struct Order {
    items: Vec<String>,
    payment: EntityIDFor<PaymentMethod>,  // Stores as union
}

#[derive(TerminusDBModel)]
enum PaymentMethod {
    CreditCard { card_number: String, cvv: String },
    BankTransfer { account: String, routing: String },
    Cash,
}

// Process a credit card payment
async fn process_credit_card(
    client: &TerminusDBClient,
    card_id: EntityIDFor<PaymentMethodCreditCard>,
) -> Result<EntityIDFor<PaymentMethod>> {
    // Validate the card details
    let card: PaymentMethodCreditCard = client.get_instance(&card_id).await?;
    validate_card(&card)?;

    // Convert to union type for storage
    let payment_id: EntityIDFor<PaymentMethod> = card_id.remap();

    Ok(payment_id)
}

// Generic function that works with any payment variant
fn store_payment<V>(
    variant_id: EntityIDFor<V>,
) -> EntityIDFor<PaymentMethod>
where
    V: TaggedUnionVariant<PaymentMethod>
{
    variant_id.remap()
}
```

## Technical Details

### How It Works

1. **Marker Traits**: The derive macro analyzes enum variants and generates appropriate trait implementations
2. **Specialization**: Rust's specialization feature allows different remap behaviors based on trait bounds
3. **Heuristic Filtering**: Type name matching filters out known primitives to avoid coherence conflicts

### Performance

- **Zero runtime cost**: All type checking happens at compile time
- **No allocations**: Remapping reuses the existing IRI string
- **Inline-friendly**: Small functions marked with `#[inline]`

## Troubleshooting

### Compile Error: "the trait bound `X: TaggedUnionVariant<Y>` is not satisfied"

**Cause**: The type doesn't implement `TaggedUnionVariant` for the target union.

**Solutions**:
1. Check if it's a primitive type → use direct ID construction instead
2. Check if it's actually a variant of that union → verify your enum definition
3. Check if it's a custom type not in the primitive filter list → may need to add it

### Remap Preserving Wrong Type

**Expected**: `variant_id.remap()` preserves variant type
**Got**: Generic type name instead

**Cause**: Using regular remap instead of TaggedUnion-aware remap.

**Solution**: Ensure the type implements `TaggedUnionVariant<Union>` - check it's not filtered as a primitive.

## Questions?

For issues or questions:
- Check the test file: `schema/derive/tests/tagged_union_remap_test.rs`
- File an issue on GitHub
- Review the implementation: `schema/src/id.rs` (lines 196-213)

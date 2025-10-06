# Migration Guide: EntityIDFor Remap Enhancement

## TL;DR

`EntityIDFor::remap()` now has type-safe specialization for TaggedUnion variants. **No breaking changes** - existing code works unchanged. New capability: variant-to-union ID conversion preserves full typed paths.

## What You Get

### Before
```rust
let variant_id: EntityIDFor<VariantType> = EntityIDFor::new("VariantType/123")?;
let union_id: EntityIDFor<UnionType> = variant_id.remap();
// Lost variant type info! union_id.typed() == "UnionType/123" ❌
```

### After
```rust
let variant_id: EntityIDFor<VariantType> = EntityIDFor::new("VariantType/123")?;
let union_id: EntityIDFor<UnionType> = variant_id.remap();
// Preserves variant type! union_id.typed() == "VariantType/123" ✅
```

## Quick Start

### Works Automatically

Multi-field TaggedUnion variants automatically support remap:

```rust
#[derive(TerminusDBModel)]
enum PaymentMethod {
    CreditCard { card_number: String, cvv: String },
    BankTransfer { account: String, routing: String },
}

// Just works! ✅
let card_id: EntityIDFor<PaymentMethodCreditCard> = ...;
let payment_id: EntityIDFor<PaymentMethod> = card_id.remap();
```

Single-field model variants also work:

```rust
#[derive(TerminusDBModel)]
struct UserLoginEvent { user_id: String }

#[derive(TerminusDBModel)]
enum ActivityEvent {
    UserLogin(UserLoginEvent),  // ✅ Model type works
    SystemShutdown { reason: String },
}

let login_id: EntityIDFor<UserLoginEvent> = ...;
let event_id: EntityIDFor<ActivityEvent> = login_id.remap(); // ✅
```

### Limitation: Primitives Filtered Out

Single-field primitive variants don't support remap:

```rust
#[derive(TerminusDBModel)]
enum Source {
    Post(String),  // ❌ Primitive - can't use remap
    Url(String),
}

// Won't compile:
// let id: EntityIDFor<String> = ...;
// let source_id = id.remap::<Source>(); // ERROR

// Workaround - construct directly:
let source_id: EntityIDFor<Source> = EntityIDFor::new("Post/id")?; // ✅
```

## Type Safety Benefits

### Compile-Time Guarantees

```rust
// Only compiles if V is actually a variant of U
fn convert<V, U>(v: EntityIDFor<V>) -> EntityIDFor<U>
where
    V: TaggedUnionVariant<U>,
    U: TaggedUnion,
{
    v.remap() // Type-checked! ✅
}
```

### Prevents Mistakes

```rust
enum PaymentMethod { ... }
enum OrderStatus { ... }

let payment_id: EntityIDFor<PaymentMethodCreditCard> = ...;

// This won't compile - different unrelated types:
// let status_id: EntityIDFor<OrderStatus> = payment_id.remap(); // ERROR ✅
```

## Common Patterns

### Pattern 1: Store Variant as Union

```rust
async fn save_payment<V>(
    client: &TerminusDBClient,
    variant: V,
) -> Result<EntityIDFor<PaymentMethod>>
where
    V: TaggedUnionVariant<PaymentMethod> + TerminusDBModel
{
    let variant_id = client.insert_instance(&variant).await?;
    Ok(variant_id.remap()) // Convert to union for storage
}
```

### Pattern 2: Generic Processing

```rust
fn process_any_payment_variant<V>(id: EntityIDFor<V>)
where
    V: TaggedUnionVariant<PaymentMethod>
{
    let union_id: EntityIDFor<PaymentMethod> = id.remap();
    // Now can use with functions expecting PaymentMethod
}
```

### Pattern 3: Preserve Variant Info

```rust
// Before: lost which variant it was
let union_id = variant_id.remap();
// Had to query to find out variant type

// After: variant type preserved in ID
let union_id = variant_id.remap();
assert_eq!(union_id.get_type_name(), "PaymentMethodCreditCard");
// No query needed! ✅
```

## Checklist for Migration

- [ ] **No action required** - existing code works unchanged
- [ ] **Optional**: Update TaggedUnion usage to use type-safe remap
- [ ] **Optional**: Remove workarounds for variant-to-union conversion
- [ ] **Note**: Primitive single-field variants still need manual ID construction

## Need More Details?

See full documentation: [`docs/entity-id-remap.md`](./docs/entity-id-remap.md)

## Questions?

- Example tests: `schema/derive/tests/tagged_union_remap_test.rs`
- Implementation: `schema/src/id.rs` (lines 196-213)
- File issues on GitHub

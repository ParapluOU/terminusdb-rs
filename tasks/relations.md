# TerminusDB Relations: High-Level Query Macro System

## Overview

This document describes the design and implementation of a compositional relation system for TerminusDB that enables type-safe, high-level query construction through Rust's type system.

## Problem Statement

TerminusDB is a triple store that uses a datalog variant for queries. While powerful, writing WOQL queries manually requires:
- Explicit triple patterns for every relation
- Type constraints for each variable
- Manual handling of optional relations
- Repetitive boilerplate for common patterns

The goal is to leverage Rust's type system to automatically generate these constraints from the model structure.

## Core Concepts

### 1. Relations as First-Class Citizens

In TerminusDB models, relations are represented through specific field types:
- `TdbLazy<T>` - Lazy-loaded relation to another model
- `EntityIDFor<T>` - Direct ID reference to another model
- `Option<TdbLazy<T>>` - Optional relation
- `Vec<TdbLazy<T>>` - One-to-many relation

The key insight is that these types already encode the relation semantics - we just need to extract and use this information.

### 2. Const Generic Field Discrimination

When a model has multiple relations to the same target type, we need to distinguish them:

```rust
struct User {
    manager: Option<TdbLazy<User>>,      // Self-relation 1
    reports: Vec<TdbLazy<User>>,         // Self-relation 2
    posts: Vec<TdbLazy<Post>>,           // Only relation to Post
}
```

Using const generics with field names as discriminators:
```rust
trait RelationTo<Target, const FIELD_NAME: &'static str = "default">
```

This allows:
- `User` implements `RelationTo<User, "manager">`
- `User` implements `RelationTo<User, "reports">`
- `User` implements `RelationTo<Post, "posts">`
- `User` implements `RelationTo<Post>` (automatic default)

### 3. Bidirectional Relations

The system supports both forward and reverse traversal:
- `RelationTo<Target, FIELD>` - Forward relation (A has relation to B)
- `RelationFrom<Target, FIELD>` - Reverse relation (A is related from B)

Key insight: `RelationFrom` is automatically implemented for any `RelationTo`:
```rust
impl<Source, Target, const FIELD: &'static str> RelationFrom<Source, FIELD> for Target
where
    Source: RelationTo<Target, FIELD> + TerminusDBModel,
    Target: TerminusDBModel,
{
    fn constraints_with_vars(source_var: &str, target_var: &str) -> Query {
        <Source as RelationTo<Target, FIELD>>::constraints_with_vars(target_var, source_var)
    }
}
```

### 4. Automatic Default Relations

When a model has only one relation to a target type, it becomes the default automatically:
- No need for explicit marking
- Reduces boilerplate
- Natural ergonomics

When multiple relations exist to the same target:
- User can mark one as `#[tdb(default_relation)]`
- Or must specify field name explicitly in queries

## Architecture Decision: Separate Crate with Integrated Derive

### The Challenge: Circular Dependencies

Initial approach created a circular dependency:
```
terminusdb-schema → terminusdb-woql2 → terminusdb-schema
```

Also faced Rust limitations with const generics for `&'static str` parameters.

### Solution: Modular Architecture with Marker Types

1. **Separate relation crate** (`terminusdb-relation`):
   - Contains traits and macros
   - Depends on both schema and woql2
   - No circular dependency

2. **Derive helper crate** (`terminusdb-relation-derive`):
   - Regular library (not proc-macro)
   - Contains relation generation logic
   - Imported by schema/derive

3. **Single derive macro**:
   - Users only use `#[derive(TerminusDBModel)]`
   - Internally calls relation generation
   - Seamless integration

### Benefits

1. **Clean dependency graph**: No circular dependencies
2. **Modular code**: Each crate has clear responsibility
3. **Single entry point**: User experience unchanged
4. **Optional feature**: Can be feature-gated if needed
5. **Testability**: Each component can be tested independently

## Implementation Details

### Constraint Generation

For a relation `User.posts → Post`, generate:
```rust
and!(
    triple!(var!("User"), "posts", var!("Post")),
    type_!(var!("User"), User),
    type_!(var!("Post"), Post)
)
```

For optional relations, wrap in `optional!()`:
```rust
optional!(and!(
    triple!(var!("User"), "manager", var!("Manager")),
    type_!(var!("User"), User),
    type_!(var!("Manager"), User)
))
```

### Field Type Detection

The derive macro inspects field types to identify relations:
1. Check if type is `TdbLazy<T>` or `EntityIDFor<T>`
2. Check if wrapped in `Option<>` (optional relation)
3. Check if wrapped in `Vec<>` (collection relation)
4. Extract target type `T`
5. Generate appropriate `RelationTo` implementation

### Marker Type Implementation

Instead of const generics with strings, we generate unique marker types for each relation field:

```rust
// For User.posts field, generates:
pub struct UserPostsRelation;

impl RelationField for UserPostsRelation {
    fn field_name() -> &'static str {
        "posts"
    }
}

// And the RelationTo implementation:
impl RelationTo<Post, UserPostsRelation> for User {
    fn constraints_with_vars(source_var: &str, target_var: &str) -> Query {
        // Generate WOQL constraints
    }
}
```

This avoids Rust's const generic limitations while providing the same functionality.

### from_path! Macro Design (Future Work)

The macro will provide compositional path building:
```rust
from_path! {
    ReviewSession => Publication <= Chunk => Annotation
}
```

Key features:
- `=>` for forward relations (A relates to B)
- `<=` for reverse relations (B relates to A)
- `.field` for explicit field specification
- Automatic default relation selection

Expansion strategy:
1. Parse tokens left-to-right
2. Each segment generates constraints
3. Chain constraints with `and!()`
4. Support custom variables: `$var:Type`

## Usage Patterns

### Simple Forward Path
```rust
let query = from_path! {
    User => Post => Comment
};
```
Uses default relations throughout.

### Mixed Directions
```rust
let query = from_path! {
    User => Post <= Comment => User
};
```
- User has posts
- Comments belong to posts
- Comments have authors (users)

### Explicit Fields
```rust
let query = from_path! {
    User.manager => User.reports => User
};
```
Navigate through specific relations when multiple exist.

### Custom Variables
```rust
let query = from_path! {
    $u:User => $p:Post.comments => $c:Comment
};
```
Control variable naming for complex queries.

## Future Extensions

### 1. Filtering Support
```rust
from_path! {
    User => Post where Post.published == true => Comment
}
```

### 2. Aggregations
```rust
from_path! {
    User => count(Post) as post_count
}
```

### 3. Type-Safe Query Builder API
```rust
PathBuilder::<User>::new()
    .to::<Post>()
    .where(|p| p.published.eq(true))
    .to::<Comment>()
    .build()
```

### 4. Compile-Time Validation
- Ensure relation paths are valid
- Check field names exist
- Verify type compatibility

## Design Principles

1. **Type Safety**: Leverage Rust's type system for compile-time guarantees
2. **Zero Cost**: All abstractions compile away to basic WOQL
3. **Compositionality**: Build complex queries from simple parts
4. **Ergonomics**: Intuitive syntax that matches mental model
5. **Extensibility**: Easy to add new features without breaking changes

## Conclusion

This relation system transforms TerminusDB's triple-based queries into a type-safe, compositional system that feels natural in Rust. By encoding relations in the type system and using const generics for disambiguation, we achieve both safety and ergonomics while maintaining the full power of the underlying triple store.
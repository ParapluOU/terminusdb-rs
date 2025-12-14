# terminusdb-orm

An ActiveRecord-style ORM layer for TerminusDB with compile-time type safety and efficient batch loading of related entities.

## Features

- **Type-safe queries**: `EntityIDFor<T>` ensures you can only query with IDs of the correct type
- **Automatic relation detection**: Derive macro generates relation traits from `TdbLazy<T>` and `EntityIDFor<T>` fields
- **Compile-time relation validation**: Invalid relations fail at compile time, not runtime
- **Efficient batch loading**: Always exactly 2 database calls regardless of query complexity (no N+1)
- **GraphQL-based relation traversal**: Uses TerminusDB's auto-generated reverse fields

## Quick Start

```rust
use terminusdb_orm::prelude::*;
use terminusdb_schema::TdbLazy;
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Serialize, Deserialize};

// Define models with relations
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Writer {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct BlogPost {
    pub title: String,
    /// TdbLazy creates a document link, enabling reverse relations
    pub writer: TdbLazy<Writer>,
}

// Query with eager loading
let writer_id = EntityIDFor::<Writer>::new("writer1")?;
let result = Writer::find(writer_id)
    .with::<BlogPost>()  // Load all BlogPosts by this writer
    .with_client(&client)
    .execute(&spec)
    .await?;

// Access results by type
let writers: Vec<Writer> = result.get()?;
let posts: Vec<BlogPost> = result.get()?;
```

## Defining Relations

### Document Links with `TdbLazy<T>`

`TdbLazy<T>` creates an actual document link in the TerminusDB schema. This enables:
- Forward traversal (BlogPost -> Writer)
- Reverse traversal via auto-generated fields (Writer -> all BlogPosts)

```rust
#[derive(TerminusDBModel)]
pub struct BlogPost {
    pub title: String,
    pub writer: TdbLazy<Writer>,  // Document link
}
```

### ID References with `EntityIDFor<T>`

`EntityIDFor<T>` stores a string ID reference. It provides compile-time type safety for IDs but **does not create schema links** - the field is stored as a plain string in TerminusDB:

```rust
#[derive(TerminusDBModel)]
pub struct Comment {
    pub text: String,
    pub post_id: EntityIDFor<Post>,    // Stored as string, no GraphQL traversal
    pub author_id: EntityIDFor<User>,  // Stored as string, no GraphQL traversal
}
```

> **When to use each**:
> - Use `TdbLazy<T>` when you need relation traversal (`.with()`, `.with_field()`)
> - Use `EntityIDFor<T>` when you only need type-safe ID storage/validation

### Auto-Generated Traits

The `TerminusDBModel` derive macro automatically generates:

- `{Struct}Fields` module with marker types for each field
- `ReverseRelation<T, Field>` for each `TdbLazy<T>` or `EntityIDFor<T>` field
- `ForwardRelation<T, Field>` for forward traversal
- `ReverseRelation<T, DefaultField>` for `.with::<T>()` without specifying a field

## Query API

### Finding Entities

```rust
// Find by typed ID
let id = EntityIDFor::<Comment>::new("comment1")?;
let query = Comment::find(id);

// Find multiple by typed IDs
let ids = vec![
    EntityIDFor::<Comment>::new("1")?,
    EntityIDFor::<Comment>::new("2")?,
];
let query = Comment::find_all(ids);

// Find by string ID (bypasses type checking)
let query = Comment::find_by_string("Comment/1");
let query = Comment::find_all_by_strings(["Comment/1", "Comment/2"]);
```

### Loading Relations

#### Reverse Relations: `.with::<T>()`

Load entities that reference the queried entity:

```rust
// BlogPost has: writer: TdbLazy<Writer>
// Load Writer and all BlogPosts that reference it
let result = Writer::find(id)
    .with::<BlogPost>()
    .execute(&spec).await?;
```

#### Reverse Relations with Field: `.with_via::<T, Field>()`

When a type has multiple fields referencing the same target, specify which one:

```rust
#[derive(TerminusDBModel)]
pub struct Document {
    pub author: TdbLazy<User>,    // User who wrote it
    pub reviewer: TdbLazy<User>,  // User who reviewed it
}

// Load only documents where user is the author
let result = User::find(id)
    .with_via::<Document, DocumentFields::Author>()
    .execute(&spec).await?;

// Load only documents where user is the reviewer
let result = User::find(id)
    .with_via::<Document, DocumentFields::Reviewer>()
    .execute(&spec).await?;
```

#### Forward Relations: `.with_field::<T, Field>()`

Load entities that the queried entity references. **Requires `TdbLazy<T>` fields** (not `EntityIDFor<T>`):

```rust
#[derive(TerminusDBModel)]
pub struct Car {
    pub front_left: TdbLazy<Wheel>,   // Document link - forward traversal works
    pub front_right: TdbLazy<Wheel>,
    pub back_left: TdbLazy<Wheel>,
    pub back_right: TdbLazy<Wheel>,
}

// Load all wheels for a car
let result = Car::find(id)
    .with_field::<Wheel, CarFields::FrontLeft>()
    .with_field::<Wheel, CarFields::FrontRight>()
    .with_field::<Wheel, CarFields::BackLeft>()
    .with_field::<Wheel, CarFields::BackRight>()
    .execute(&spec).await?;
```

> **Note**: Forward relations with `EntityIDFor<T>` fields will compile but fail at runtime because `EntityIDFor` creates string fields, not document links. GraphQL cannot traverse into strings.

#### Nested Relations: `.with_nested::<T>()`

Load relations of relations:

```rust
let result = Writer::find(id)
    .with_nested::<BlogPost>(|b| {
        b.with::<Comment>()  // Comments on each BlogPost
         .with::<Like>()     // Likes on each BlogPost
    })
    .execute(&spec).await?;
```

### Executing Queries

```rust
// Execute and get OrmResult containing all types
let result = query.execute(&spec).await?;

// Execute and get only the primary type
let writers: Vec<Writer> = query.execute_primary(&spec).await?;

// Execute and get exactly one result (errors if 0 or >1)
let writer: Writer = query.execute_one(&spec).await?;
```

### Accessing Results

```rust
let result = Writer::find(id)
    .with::<BlogPost>()
    .execute(&spec).await?;

// Get all entities of a type
let writers: Vec<Writer> = result.get()?;
let posts: Vec<BlogPost> = result.get()?;

// Get a single entity (errors if >1)
let writer: Option<Writer> = result.get_one()?;

// Metadata
println!("Total docs: {}", result.len());
println!("Types: {:?}", result.class_names());
println!("Counts: {:?}", result.count_by_class());
```

## How It Works

The ORM uses a **two-phase loading** strategy that always results in exactly 2 database calls:

1. **Phase 1: GraphQL ID Collection**
   - Generates a GraphQL query that traverses all requested relations
   - Uses TerminusDB's auto-generated reverse fields (e.g., `_writer_of_BlogPost`)
   - Collects only `_id` values, not full documents

2. **Phase 2: Batch Document Fetch**
   - Fetches all collected IDs in a single batch request
   - Returns documents of all types in one response

Example generated GraphQL for `Writer::find(id).with::<BlogPost>()`:

```graphql
query {
  Writer(id: "terminusdb:///data/Writer/123") {
    _id
    _writer_of_BlogPost {
      _id
    }
  }
}
```

## Compile-Time Safety

Invalid relations fail at compile time:

```rust
// This compiles: BlogPost has TdbLazy<Writer>
Writer::find(id).with::<BlogPost>();

// This does NOT compile: Writer has no TdbLazy<BlogPost>
BlogPost::find(id).with::<Writer>();  // Error: Writer: ReverseRelation<BlogPost> not satisfied

// This compiles: Car has EntityIDFor<Wheel> fields
Car::find(id).with_field::<Wheel, CarFields::FrontLeft>();

// This does NOT compile: wrong field type
Car::find(id).with_field::<Wheel, UserFields::Name>();  // Error: type mismatch
```

## Testing

The ORM includes a convenient `with_test_db` helper for integration tests:

```rust
use terminusdb_orm::testing::with_test_db;

#[tokio::test]
async fn test_something() -> anyhow::Result<()> {
    with_test_db("my_test", |client, spec| async move {
        // Insert schema
        client.insert_entity_schema::<MyModel>((&spec).into()).await?;

        // Insert data, run queries...

        Ok(())
    }).await
}
```

This automatically:
1. Gets or creates a shared in-memory test server
2. Creates a uniquely-named database
3. Runs your test closure
4. Cleans up the database when done (even on failure)

## Current Limitations

### Not Yet Implemented

- **Filtered relations**: `.with::<Post>().filter(|p| p.published)`
- **Ordered relations**: `.with::<Post>().order_by("created_at")`
- **Paginated relations**: `.with::<Post>().limit(10).offset(0)`
- **Lazy loading**: On-demand relation loading when accessed

### Known Constraints

- **`TdbLazy<T>` is required for relation traversal** - both forward (`.with_field`) and reverse (`.with`) relations rely on GraphQL traversal, which requires actual document links in the schema
- `EntityIDFor<T>` fields store string IDs only - useful for type-safe ID handling but cannot be traversed via GraphQL
- For `EntityIDFor<T>` fields, you must manually read the ID and fetch the related document separately

## Running Tests

```bash
# Run all ORM tests (uses embedded in-memory TerminusDB)
cargo test -p terminusdb-orm --features testing

# Run a specific test
cargo test -p terminusdb-orm --features testing --test typed_api_test
```

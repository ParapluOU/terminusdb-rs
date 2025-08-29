# Implementation Summary: ServerIDFor Support

## Overview
Successfully implemented support for retrieving models with server-generated IDs after insertion. The key insight was that TerminusDB returns the ID in the `@id` field but not in custom fields, requiring special handling during deserialization.

## Changes Made

### 1. Schema Derive Modifications (`schema/derive/src/json_deserialize.rs`)
- Modified `generate_field_deserializers` to accept `opts` parameter containing model attributes
- Added special handling for fields marked with `id_field` attribute
- When deserializing, the id_field is populated from the extracted `@id` value instead of expecting it in the JSON

### 2. Value Hash Key Strategy (`schema/src/instance/instance.rs`)
- Implemented the previously unimplemented `ValueHash` key strategy
- Also implemented the `Hash(fields)` key strategy
- Both return `None` to let the server generate the ID

### 3. String-to-Number Conversion (`schema/src/json/impls.rs`)
- Added handling for TerminusDB's behavior of sometimes returning numeric values as strings
- Updated the `impl_int_deserialization!` macro to parse strings as numbers when needed

### 4. Client Methods (`client/src/http/instance.rs`)
- Already had `insert_instance_and_retrieve` and `insert_instances_and_retrieve` methods
- These now work correctly with the id_field population

## Usage Example

```rust
#[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct User {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// Create a user without ID
let user = User {
    id: ServerIDFor::new(), // None
    email: "test@example.com".to_string(),
    name: "Test User".to_string(),
};

// Insert and retrieve with populated ID
let (saved_user, commit_id) = client.insert_instance_and_retrieve(&user, args).await?;

// The ID is now populated
assert!(saved_user.id.is_some());
println!("Generated ID: {}", saved_user.id.as_ref().unwrap().id());
```

## Test Results
- ✓ Simple insert and retrieve test passes
- ✓ Lexical key strategy works
- ✓ Value hash key strategy works
- ✓ Multiple insert and retrieve works (order not guaranteed by TerminusDB)
- ✓ Mixed key strategies work

## Key Technical Details

1. **ID Field Population**: The derive macro now checks if a field is the `id_field` and creates a JSON value from the extracted `@id` during deserialization

2. **TerminusDB Response Format**: TerminusDB only returns `@id` field, not custom id fields:
   ```json
   {
     "@id": "User/test@example.com",
     "@type": "User",
     "email": "test@example.com",
     "name": "Test User"
   }
   ```

3. **Order Not Guaranteed**: TerminusDB doesn't guarantee order for bulk inserts, so the `insert_instances_and_retrieve` method returns results in arbitrary order.

4. **Numeric String Handling**: TerminusDB sometimes returns numbers as strings (e.g., `"42"` instead of `42`), which is now handled transparently.

## Future Considerations

1. The test suite has some unrelated failing tests that should be addressed separately
2. Consider adding more robust error handling for edge cases
3. Documentation could be enhanced with more examples
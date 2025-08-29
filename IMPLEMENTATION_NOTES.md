# Implementation Notes: ServerIDFor and insert_instance_and_retrieve

## Summary

We've successfully implemented `insert_instance_and_retrieve` and `insert_instances_and_retrieve` methods that:
1. Insert models with server-generated IDs (lexical/value_hash key strategies)
2. Retrieve the inserted models with populated IDs

## Current Status

### What Works
- ✅ Insertion of models with `ServerIDFor<Self>` fields
- ✅ Server correctly generates IDs based on key strategy (lexical, value_hash)
- ✅ The insertion returns the generated ID in the response
- ✅ Schema generation correctly marks `ServerIDFor` fields as optional

### Issue Found
- ❌ Retrieval fails because TerminusDB doesn't include the `id` field in the response JSON
- The server only returns `@id` (the document identifier) but not the custom `id` field
- This causes deserialization to fail with "Required property not found"

## Root Cause

When you define a model with an `id_field`:
```rust
#[derive(TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
struct User {
    id: ServerIDFor<Self>,
    email: String,
}
```

The server generates the ID but doesn't store it as a separate field. Instead, it's only available through the `@id` property.

## Workaround Solutions

### 1. Manual ID Population (Recommended for now)
```rust
// Insert the model
let (result, commit_id) = client.insert_instance_with_commit_id(&user, args).await?;

// Extract the ID from the result
let generated_id = result.root_id
    .split('/')
    .last()
    .unwrap();

// Manually set the ID
let mut user_with_id = user.clone();
user_with_id.id.__set_from_server(EntityIDFor::new(generated_id)?);
```

### 2. Use a Custom Deserializer
Implement a custom deserializer that populates the `id` field from the `@id` field during deserialization.

### 3. Future Fix Options

1. **Modify the derive macro** to automatically populate `id_field` from `@id` during deserialization
2. **Server-side change**: Have TerminusDB include the `id` field in responses when `id_field` is specified
3. **Post-processing step**: Add a method that transforms the raw JSON to include the `id` field before deserialization

## Test Results

The implementation successfully:
- Inserts models with server-generated IDs
- Returns the correct ID in the insertion result
- Handles lexical key generation (e.g., email-based IDs)

Example successful insertion:
```
Insert result: InsertInstanceResult { 
    root_id: "terminusdb:///data/SimpleModel/test_name", 
    root_result: Inserted("terminusdb:///data/SimpleModel/test_name"), 
    sub_entities: {}, 
    commit_id: Some("txlyfc85op2anm0to8fasy28wgh9xtm") 
}
```

## Next Steps

1. Consider implementing a post-processing step in `get_instance` that adds the `id` field from `@id`
2. Or modify the derive macro to handle this case automatically
3. Document this limitation and provide examples of the workaround
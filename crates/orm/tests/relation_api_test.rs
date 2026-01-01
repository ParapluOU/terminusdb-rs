//! Comprehensive tests for ORM relation API
//!
//! This file defines all the syntax we want to support for relation loading.
//! Tests are organized by feature and include both compile-time and runtime checks.

use terminusdb_orm::prelude::*;

// Required for TerminusDBModel derive
use terminusdb_schema as terminusdb_schema;
use terminusdb_schema::{TdbLazy, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;

use serde::{Deserialize, Serialize};

// ============================================================================
// Test Models - Define a realistic domain model
// ============================================================================

/// A user in the system
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct User {
    pub name: String,
    pub email: String,
}

/// A blog post authored by a user
/// Uses TdbLazy<User> to create a document link (enables reverse relations)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct Post {
    pub title: String,
    pub content: String,
    /// The author of this post (document link to User, enables reverse relation)
    pub user: TdbLazy<User>,
}

/// A comment on a post, also by a user
/// Uses TdbLazy to create document links (enables reverse relations)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct Comment {
    pub text: String,
    /// The post this comment belongs to (document link)
    pub post: TdbLazy<Post>,
    /// The user who wrote this comment (document link)
    pub author: TdbLazy<User>,
}

/// A document with multiple user references (author and reviewer)
/// Uses TdbLazy<User> to create document links (enables reverse relations)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct Document {
    pub title: String,
    /// Primary author (document link)
    pub author: TdbLazy<User>,
    /// Reviewer (document link, different from author)
    pub reviewer: TdbLazy<User>,
}

/// A car with multiple wheel references (forward relations)
/// Uses TdbLazy to create document links (enables forward relation traversal)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct Car {
    pub model: String,
    pub front_left: TdbLazy<Wheel>,
    pub front_right: TdbLazy<Wheel>,
    pub back_left: TdbLazy<Wheel>,
    pub back_right: TdbLazy<Wheel>,
}

/// A wheel (referenced by Car)
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Wheel {
    pub size: u32,
    pub brand: String,
}

// ============================================================================
// Trait Implementations - NOW AUTOMATICALLY DERIVED!
// ============================================================================
//
// The TerminusDBModel derive macro now automatically generates relation traits
// for TdbLazy<T> fields only. EntityIDFor<T> is just a typed string ID and
// doesn't create document links in TDB, so no relation traits are generated.
//
// For each `TdbLazy<T>` field:
// - `ReverseRelation<T, StructFields::FieldName>` - enables `.with_via::<Self, Field>()`
// - `ForwardRelation<T, StructFields::FieldName>` - enables `.with_field::<T, Field>()`
//
// For each unique target type T (via TdbLazy):
// - `ReverseRelation<T, DefaultField>` - enables `.with::<Self>()` on T queries
//
// No manual implementations needed!

// ============================================================================
// Test: Basic Query Building
// ============================================================================

#[test]
fn test_find_single_id() {
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id);
    assert_eq!(query.len(), 1);
}

#[test]
fn test_find_multiple_ids() {
    let id1 = EntityIDFor::<User>::new("user1").unwrap();
    let id2 = EntityIDFor::<User>::new("user2").unwrap();
    let query = User::find_all([id1, id2]);
    assert_eq!(query.len(), 2);
}

#[test]
fn test_find_by_string() {
    let query = User::find_by_string("User/user1");
    assert_eq!(query.len(), 1);
}

#[test]
fn test_find_all_by_strings() {
    let query = User::find_all_by_strings(["User/user1", "User/user2"]);
    assert_eq!(query.len(), 2);
}

// ============================================================================
// Test: Reverse Relations - .with::<T>()
// ============================================================================

#[test]
fn test_with_reverse_relation_single_field() {
    // Post has one TdbLazy<User> field (user)
    // .with::<Post>() should automatically use the "user" field name
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id).with::<Post>();

    // Check that the relation was registered with the correct field name
    assert_eq!(query.relations().len(), 1);
    match &query.relations()[0].direction {
        RelationDirection::Reverse { via_field } => {
            // Single-field relations now automatically specify the field name
            assert_eq!(via_field.as_deref(), Some("user"), "with::<T>() should auto-detect single field");
        }
        _ => panic!("Expected Reverse direction"),
    }
}

#[test]
fn test_with_reverse_relation_multiple_fields() {
    // Document has TWO TdbLazy<User> fields (author, reviewer)
    // .with::<Document>() should load Documents where EITHER field matches
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id).with::<Document>();

    assert_eq!(query.relations().len(), 1);
    match &query.relations()[0].direction {
        RelationDirection::Reverse { via_field } => {
            assert!(via_field.is_none(), "with::<T>() loads via any field");
        }
        _ => panic!("Expected Reverse direction"),
    }
}

#[test]
fn test_with_multiple_reverse_relations() {
    // Load both Posts and Comments for a User
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id)
        .with::<Post>()
        .with::<Comment>();

    assert_eq!(query.relations().len(), 2);
}

#[test]
fn test_with_chained_reverse_relations() {
    // Post -> Comments (nested relation)
    let id = EntityIDFor::<Post>::new("post1").unwrap();
    let query = Post::find(id).with::<Comment>();

    assert_eq!(query.relations().len(), 1);
}

// ============================================================================
// Test: Reverse Relations with Field - .with_via::<T, Field>()
// ============================================================================

#[test]
fn test_with_via_specific_field() {
    // Load only Documents where user is the AUTHOR (not reviewer)
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id)
        .with_via::<Document, DocumentFields::Author>();

    assert_eq!(query.relations().len(), 1);
    match &query.relations()[0].direction {
        RelationDirection::Reverse { via_field } => {
            assert_eq!(via_field.as_deref(), Some("author"));
        }
        _ => panic!("Expected Reverse direction"),
    }
}

#[test]
fn test_with_via_different_field() {
    // Load only Documents where user is the REVIEWER
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id)
        .with_via::<Document, DocumentFields::Reviewer>();

    assert_eq!(query.relations().len(), 1);
    match &query.relations()[0].direction {
        RelationDirection::Reverse { via_field } => {
            assert_eq!(via_field.as_deref(), Some("reviewer"));
        }
        _ => panic!("Expected Reverse direction"),
    }
}

#[test]
fn test_with_via_both_fields_separately() {
    // Load Documents where user is author AND where user is reviewer (separate queries)
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id)
        .with_via::<Document, DocumentFields::Author>()
        .with_via::<Document, DocumentFields::Reviewer>();

    // Both relations are registered
    assert_eq!(query.relations().len(), 2);
}

// ============================================================================
// Test: Forward Relations - .with_field::<T, Field>()
// ============================================================================

#[test]
fn test_with_field_forward_relation() {
    // Car has explicit wheel fields - must specify which one
    let id = EntityIDFor::<Car>::new("car1").unwrap();
    let query = Car::find(id)
        .with_field::<Wheel, CarFields::FrontLeft>();

    assert_eq!(query.relations().len(), 1);
    match &query.relations()[0].direction {
        RelationDirection::Forward { field_name } => {
            assert_eq!(field_name, "front_left");
        }
        _ => panic!("Expected Forward direction"),
    }
}

#[test]
fn test_with_field_multiple_forward_relations() {
    // Load all four wheels
    let id = EntityIDFor::<Car>::new("car1").unwrap();
    let query = Car::find(id)
        .with_field::<Wheel, CarFields::FrontLeft>()
        .with_field::<Wheel, CarFields::FrontRight>()
        .with_field::<Wheel, CarFields::BackLeft>()
        .with_field::<Wheel, CarFields::BackRight>();

    assert_eq!(query.relations().len(), 4);
}

// ============================================================================
// Test: Compile-time Safety (these are compile-time tests)
// ============================================================================

#[test]
fn test_compile_time_safety_documentation() {
    // The following would NOT compile - documented here for reference:

    // 1. Forward relation without field marker:
    // Car::find(id).with::<Wheel>();
    // Error: Wheel: ReverseRelation<Car> is not satisfied

    // 2. Reverse relation with wrong parent:
    // Post::find(id).with::<User>();
    // Error: User: ReverseRelation<Post> is not satisfied
    // (User doesn't have BelongsTo<Post>)

    // 3. Forward relation with wrong field:
    // Car::find(id).with_field::<Wheel, UserFields::Name>();
    // Error: Car: ForwardRelation<Wheel, UserFields::Name> is not satisfied

    // 4. Type-safe IDs:
    // let user_id = EntityIDFor::<User>::new("u1").unwrap();
    // Post::find(user_id);
    // Error: expected EntityIDFor<Post>, found EntityIDFor<User>
}

// ============================================================================
// Test: Query Options
// ============================================================================

#[test]
fn test_query_with_unfold() {
    let id = EntityIDFor::<User>::new("user1").unwrap();
    let query = User::find(id).unfold();
    // unfold is set (we can't easily check this without accessing private fields)
    assert_eq!(query.len(), 1);
}

// ============================================================================
// Test: Mixed Forward and Reverse Relations
// ============================================================================

#[test]
fn test_mixed_relation_types() {
    // This would be a complex query combining both relation types
    // For now, just verify they can be chained (even if semantically unusual)

    // Example: If we had a model with both forward and reverse relations
    // we could chain both types of with calls
}

// ============================================================================
// Integration Tests (use embedded in-memory TerminusDB server)
// ============================================================================

// These tests verify actual query execution and data loading

#[cfg(feature = "testing")]
mod integration {
    use super::*;
    use terminusdb_test::test as db_test;
    use terminusdb_client::DocumentInsertArgs;

    #[db_test(db = "orm_reverse_relation_test")]
    async fn test_execute_with_reverse_relation(client: _, spec: _) -> anyhow::Result<()> {
        // Insert schemas
        let schema_args = DocumentInsertArgs {
            spec: spec.clone(),
            ..Default::default()
        };

        client.insert_schema(&User::to_schema(), schema_args.clone()).await?;
        client.insert_schema(&Post::to_schema(), schema_args.clone()).await?;

        // Insert a user
        let user = User {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };
        let user_result = client.save_instance(&user, schema_args.clone()).await?;
        let user_id = user_result.root_id.clone();

        // Insert posts by that user (using TdbLazy)
        let post1 = Post {
            title: "First Post".to_string(),
            content: "Hello world".to_string(),
            user: TdbLazy::new_id(&user_id)?,
        };
        let post2 = Post {
            title: "Second Post".to_string(),
            content: "Another post".to_string(),
            user: TdbLazy::new_id(&user_id)?,
        };
        client.save_instance(&post1, schema_args.clone()).await?;
        client.save_instance(&post2, schema_args.clone()).await?;

        // Query user with posts using the TdbLazy field name
        let result = User::find_by_string(&user_id)
            .with_via::<Post, PostFields::User>()
            .with_client(&client)
            .execute(&spec)
            .await?;

        // Verify results
        let users: Vec<User> = result.get()?;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Alice");

        // Verify posts are loaded via reverse relation
        let posts: Vec<Post> = result.get()?;
        assert_eq!(posts.len(), 2, "Expected 2 posts via reverse relation");
        Ok(())
    }

    #[db_test(db = "orm_with_via_test")]
    async fn test_execute_with_via_specific_field(client: _, spec: _) -> anyhow::Result<()> {
        // Insert schemas
        let schema_args = DocumentInsertArgs {
            spec: spec.clone(),
            ..Default::default()
        };

        client.insert_schema(&User::to_schema(), schema_args.clone()).await?;
        client.insert_schema(&Document::to_schema(), schema_args.clone()).await?;

        // Insert two users
        let alice = User { name: "Alice".to_string(), email: "alice@example.com".to_string() };
        let bob = User { name: "Bob".to_string(), email: "bob@example.com".to_string() };

        let alice_result = client.save_instance(&alice, schema_args.clone()).await?;
        let bob_result = client.save_instance(&bob, schema_args.clone()).await?;

        let alice_id = alice_result.root_id.clone();
        let bob_id = bob_result.root_id.clone();

        // Insert document: Alice is author, Bob is reviewer (using TdbLazy)
        let doc = Document {
            title: "Important Document".to_string(),
            author: TdbLazy::new_id(&alice_id)?,
            reviewer: TdbLazy::new_id(&bob_id)?,
        };
        client.save_instance(&doc, schema_args.clone()).await?;

        // Query Alice's authored documents (should find the doc)
        let result = User::find_by_string(&alice_id)
            .with_via::<Document, DocumentFields::Author>()
            .with_client(&client)
            .execute(&spec)
            .await?;

        // Verify doc is in result
        let docs: Vec<Document> = result.get()?;
        assert_eq!(docs.len(), 1, "Expected 1 document where Alice is author");
        assert_eq!(docs[0].title, "Important Document");

        // Query Alice's reviewed documents (should find nothing)
        let result2 = User::find_by_string(&alice_id)
            .with_via::<Document, DocumentFields::Reviewer>()
            .with_client(&client)
            .execute(&spec)
            .await?;

        // Verify no docs in result (Alice is not reviewer of any doc)
        let docs2: Vec<Document> = result2.get()?;
        assert_eq!(docs2.len(), 0, "Expected 0 documents where Alice is reviewer");
        Ok(())
    }
}

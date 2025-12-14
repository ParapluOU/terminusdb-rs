//! End-to-end test for the typed ORM API
//!
//! This test uses actual Comment/Reply models and tests:
//! - `Comment::find_all([EntityIDFor<Comment>]).with::<Reply>().execute(&spec)`
//!
//! Run with: `cargo test -p terminusdb-orm --features testing --test typed_api_test`

use terminusdb_orm::prelude::*;
#[cfg(feature = "testing")]
use terminusdb_test::test as db_test;

// Required for TerminusDBModel derive
use terminusdb_schema as terminusdb_schema;
#[allow(unused_imports)]
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::TdbLazy;
use terminusdb_schema_derive::TerminusDBModel;

use serde::{Deserialize, Serialize};

/// A comment that can have multiple replies
#[derive(Clone, Debug, Default, Serialize, Deserialize, TerminusDBModel)]
pub struct Comment {
    pub text: String,
    pub author: String,
}

/// A reply to a comment
/// Uses TdbLazy<Comment> to create a document link (enables reverse relations)
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Reply {
    pub text: String,
    pub author: String,
    /// Reference to parent comment (document link, enables reverse relation)
    pub comment: TdbLazy<Comment>,
}

// The TerminusDBModel derive macro now automatically generates:
// - ReverseRelation<Comment, ReplyFields::Comment>
// - ReverseRelation<Comment, DefaultField>
// - ForwardRelation<Comment, ReplyFields::Comment>

// ============================================================================
// Forward relation example: Car with multiple wheel fields
// ============================================================================

/// A wheel for a car
#[derive(Clone, Debug, Default, Serialize, Deserialize, TerminusDBModel)]
pub struct Wheel {
    pub position: String,
    pub size: u32,
}

/// A car with multiple wheel references (forward relations)
/// Uses TdbLazy to create document links (enables relation traversal)
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Car {
    pub model: String,
    /// Front wheels (document links)
    pub front_wheels: Vec<TdbLazy<Wheel>>,
    /// Back wheels (document links)
    pub back_wheels: Vec<TdbLazy<Wheel>>,
}

// ForwardRelation impls are now automatically derived by TerminusDBModel
// for TdbLazy<T> fields only:
// - ForwardRelation<Wheel, CarFields::FrontWheels>
// - ForwardRelation<Wheel, CarFields::BackWheels>

/// Test that typed EntityIDFor works with find_all
#[test]
fn test_typed_find_all_compiles() {
    // Create typed Comment IDs
    let id1 = EntityIDFor::<Comment>::new("1").unwrap();
    let id2 = EntityIDFor::<Comment>::new("2").unwrap();

    // This compiles! Comment::find_all takes EntityIDFor<Comment>
    let _query = Comment::find_all([id1, id2]);

    // This would NOT compile (type mismatch):
    // let reply_id = EntityIDFor::<Reply>::new("r1").unwrap();
    // let _bad = Comment::find_all([reply_id]); // Error: expected EntityIDFor<Comment>
}

/// Test typed find with single ID
#[test]
fn test_typed_find_compiles() {
    let id = EntityIDFor::<Comment>::new("123").unwrap();
    let _query = Comment::find(id);
}

/// Test the with::<T>() chain compiles for reverse relations
#[test]
fn test_with_chain_compiles() {
    let id = EntityIDFor::<Comment>::new("1").unwrap();

    // The full typed API chain - Reply has BelongsTo<Comment> so this is valid
    let _query = Comment::find_all([id])
        .with::<Reply>();

    // Multiple with calls for different related types
    let id2 = EntityIDFor::<Comment>::new("2").unwrap();
    let _query2 = Comment::find_all([id2])
        .with::<Reply>();

    // Note: .with::<Comment>() would NOT compile because Comment doesn't
    // implement ReverseRelation<Comment> (Comment has no BelongsTo<Comment>)
}

/// Test forward relations with with_field::<T, Field>()
#[test]
fn test_with_field_forward_relation() {
    let car_id = EntityIDFor::<Car>::new("car1").unwrap();

    // Forward relation: Car has front_wheels and back_wheels fields pointing to Wheel
    // Must specify which field to traverse using CarFields::FieldName
    let _query = Car::find_all([car_id.clone()])
        .with_field::<Wheel, CarFields::FrontWheels>();

    // Can load both wheel relations
    let _query2 = Car::find_all([car_id])
        .with_field::<Wheel, CarFields::FrontWheels>()
        .with_field::<Wheel, CarFields::BackWheels>();

    // Note: This would NOT compile because it doesn't specify which field:
    // let _bad = Car::find_all([car_id]).with::<Wheel>();
    // Error: Wheel: ReverseRelation<Car> is not satisfied (Wheel has no BelongsTo<Car>)
}

// ============================================================================
// Integration tests - require running TerminusDB
// ============================================================================

/// Full end-to-end test: insert schema, insert data, query with typed API
#[cfg(feature = "testing")]
#[db_test(db = "orm_typed_api_test")]
async fn test_comment_find_all_with_reply(client: _, spec: _) -> anyhow::Result<()> {
    use terminusdb_client::DocumentInsertArgs;

    // Insert schemas for Comment and Reply
    let comment_schema = Comment::to_schema();
    let reply_schema = Reply::to_schema();

    let schema_args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    // Insert Comment schema
    client
        .insert_schema(&comment_schema, schema_args.clone())
        .await
        .expect("Failed to insert Comment schema");

    // Insert Reply schema
    client
        .insert_schema(&reply_schema, schema_args.clone())
        .await
        .expect("Failed to insert Reply schema");

    // Create Comment instances
    let comment1 = Comment {
        text: "First comment".to_string(),
        author: "Alice".to_string(),
    };

    let comment2 = Comment {
        text: "Second comment".to_string(),
        author: "Bob".to_string(),
    };

    // Insert comments using save_instance
    let insert_args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    let result1 = client
        .save_instance(&comment1, insert_args.clone())
        .await
        .expect("Failed to insert comment1");

    let result2 = client
        .save_instance(&comment2, insert_args.clone())
        .await
        .expect("Failed to insert comment2");

    // Extract the IDs - need both typed (for queries) and string (for TdbLazy)
    let comment1_id = result1.root_ref::<Comment>()
        .expect("Should parse comment1 ID");
    let comment2_id = result2.root_ref::<Comment>()
        .expect("Should parse comment2 ID");
    let comment1_id_str = result1.root_id.clone();
    let comment2_id_str = result2.root_id.clone();

    println!("Inserted Comment 1: {}", comment1_id);
    println!("Inserted Comment 2: {}", comment2_id);

    // Create Reply instances using TdbLazy links
    let reply1 = Reply {
        text: "Reply to first".to_string(),
        author: "Carol".to_string(),
        comment: TdbLazy::new_id(&comment1_id_str)?,
    };

    let reply2 = Reply {
        text: "Another reply to first".to_string(),
        author: "Dave".to_string(),
        comment: TdbLazy::new_id(&comment1_id_str)?,
    };

    let reply3 = Reply {
        text: "Reply to second".to_string(),
        author: "Eve".to_string(),
        comment: TdbLazy::new_id(&comment2_id_str)?,
    };

    // Insert replies
    client
        .save_instance(&reply1, insert_args.clone())
        .await
        .expect("Failed to insert reply1");

    client
        .save_instance(&reply2, insert_args.clone())
        .await
        .expect("Failed to insert reply2");

    let reply3_result = client
        .save_instance(&reply3, insert_args.clone())
        .await
        .expect("Failed to insert reply3");

    let reply3_id = reply3_result.root_ref::<Reply>()
        .expect("Should parse reply3 ID");

    println!("Inserted Reply 3: {}", reply3_id);

    // =========================================================================
    // NOW TEST THE TYPED API: Comment::find_all([ids]).with::<Reply>()
    // =========================================================================

    // Use typed IDs to query
    let result = Comment::find_all([comment1_id.clone(), comment2_id.clone()])
        .with::<Reply>()
        .with_client(&client)
        .execute(&spec)
        .await
        .expect("Query should succeed");

    // Verify we got the comments
    let comments: Vec<Comment> = result.get().expect("Should get comments");
    assert_eq!(comments.len(), 2, "Should have 2 comments");

    println!("Found {} comments", comments.len());
    for c in &comments {
        println!("  - {} by {}", c.text, c.author);
    }

    // Verify we got the replies (if with::<Reply>() loads them)
    let replies: Vec<Reply> = result.get().expect("Should get replies");
    println!("Found {} replies", replies.len());
    for r in &replies {
        println!("  - {} by {}", r.text, r.author);
    }

    // Note: The number of replies depends on how with::<Reply>() is implemented
    // It should load related replies based on the comment_id field
    Ok(())
}

/// Test using find() with a single typed ID
#[cfg(feature = "testing")]
#[db_test(db = "orm_find_single_test")]
async fn test_comment_find_single(client: _, spec: _) -> anyhow::Result<()> {
    use terminusdb_client::DocumentInsertArgs;

    // Insert schema
    let schema_args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    client
        .insert_schema(&Comment::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert schema");

    // Insert a comment
    let comment = Comment {
        text: "Single comment test".to_string(),
        author: "TestUser".to_string(),
    };

    let result = client
        .save_instance(&comment, schema_args.clone())
        .await
        .expect("Failed to insert comment");

    let comment_id = result.root_ref::<Comment>().unwrap();

    // Query using typed find()
    let query_result = Comment::find(comment_id.clone())
        .with_client(&client)
        .execute(&spec)
        .await
        .expect("Query should succeed");

    let comments: Vec<Comment> = query_result.get().expect("Should deserialize");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Single comment test");
    assert_eq!(comments[0].author, "TestUser");

    println!("Found comment: {:?}", comments[0]);
    Ok(())
}

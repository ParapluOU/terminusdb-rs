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
use terminusdb_schema;
use terminusdb_schema::TdbLazy;
#[allow(unused_imports)]
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::TerminusDBModel;

use serde::{Deserialize, Serialize};

/// A comment that can have multiple replies
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Comment {
    pub text: String,
    pub author: String,
}

/// A reply to a comment
/// Uses TdbLazy<Comment> to create a document link (enables reverse relations)
#[derive(Clone, Debug, TerminusDBModel)]
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
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Wheel {
    pub position: String,
    pub size: u32,
}

/// A car with multiple wheel references (forward relations)
/// Uses TdbLazy to create document links (enables relation traversal)
#[derive(Clone, Debug, TerminusDBModel)]
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
    let _query = Comment::find_all([id]).with::<Reply>();

    // Multiple with calls for different related types
    let id2 = EntityIDFor::<Comment>::new("2").unwrap();
    let _query2 = Comment::find_all([id2]).with::<Reply>();

    // Note: .with::<Comment>() would NOT compile because Comment doesn't
    // implement ReverseRelation<Comment> (Comment has no BelongsTo<Comment>)
}

/// Test forward relations with with_field::<T, Field>()
#[test]
fn test_with_field_forward_relation() {
    let car_id = EntityIDFor::<Car>::new("car1").unwrap();

    // Forward relation: Car has front_wheels and back_wheels fields pointing to Wheel
    // Must specify which field to traverse using CarFields::FieldName
    let _query = Car::find_all([car_id.clone()]).with_field::<Wheel, CarFields::FrontWheels>();

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
    let comment1_id = result1
        .root_ref::<Comment>()
        .expect("Should parse comment1 ID");
    let comment2_id = result2
        .root_ref::<Comment>()
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

    let reply3_id = reply3_result
        .root_ref::<Reply>()
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

/// Test that EntityIDFor created from a bare ID (not server response) works correctly.
///
/// This is a regression test for the bug where `.iri()` returned `Type/id` instead of
/// `terminusdb:///data/Type/id` when EntityIDFor was created from a bare ID.
/// GraphQL queries require the full IRI format.
#[cfg(feature = "testing")]
#[db_test(db = "orm_bare_id_query_test")]
async fn test_find_with_bare_id(client: _, spec: _) -> anyhow::Result<()> {
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
        text: "Bare ID test comment".to_string(),
        author: "BareIdTester".to_string(),
    };

    let result = client
        .save_instance(&comment, schema_args.clone())
        .await
        .expect("Failed to insert comment");

    // Get the server-returned ID and extract just the bare ID part
    let server_id = result.root_ref::<Comment>().unwrap();
    let bare_id = server_id.id(); // Just the UUID part, without Type/ prefix

    println!("Server returned ID: {}", server_id);
    println!("Bare ID extracted: {}", bare_id);

    // Create a NEW EntityIDFor from the bare ID - this is the scenario that was buggy
    // Previously, .iri() would return "Comment/{uuid}" instead of "terminusdb:///data/Comment/{uuid}"
    let reconstructed_id =
        EntityIDFor::<Comment>::new(bare_id).expect("Should create EntityIDFor from bare ID");

    println!("Reconstructed ID typed(): {}", reconstructed_id.typed());
    println!("Reconstructed ID iri(): {}", reconstructed_id.iri());

    // Verify the IRI has the correct format for GraphQL
    assert!(
        reconstructed_id
            .iri()
            .to_string()
            .starts_with("terminusdb:///data/"),
        "IRI should start with 'terminusdb:///data/' but got: {}",
        reconstructed_id.iri()
    );

    // NOW THE CRITICAL TEST: Query using the reconstructed ID
    // This would fail if .iri() returned the wrong format
    let query_result = Comment::find(reconstructed_id)
        .with_client(&client)
        .execute(&spec)
        .await
        .expect("Query with bare ID should succeed - this fails if IRI format is wrong");

    let comments: Vec<Comment> = query_result.get().expect("Should deserialize");
    assert_eq!(comments.len(), 1, "Should find exactly one comment");
    assert_eq!(comments[0].text, "Bare ID test comment");
    assert_eq!(comments[0].author, "BareIdTester");

    println!("Successfully queried with bare ID: {:?}", comments[0]);
    Ok(())
}

/// Test that `.with::<R>()` works for single-field relations without needing `with_via`.
///
/// This is a regression test for the DefaultField resolution fix.
/// When `Reply` has exactly one `TdbLazy<Comment>` field, `.with::<Reply>()`
/// should automatically use the correct field name in the GraphQL query.
#[cfg(feature = "testing")]
#[db_test(db = "orm_default_field_single_relation")]
async fn test_with_default_field_single_relation(client: _, spec: _) -> anyhow::Result<()> {
    use terminusdb_client::DocumentInsertArgs;

    // Insert schemas
    let schema_args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    client
        .insert_schema(&Comment::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert Comment schema");

    client
        .insert_schema(&Reply::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert Reply schema");

    // Insert a comment
    let comment = Comment {
        text: "Parent comment for default field test".to_string(),
        author: "TestAuthor".to_string(),
    };

    let comment_result = client
        .save_instance(&comment, schema_args.clone())
        .await
        .expect("Failed to insert comment");

    let comment_id = comment_result.root_ref::<Comment>().unwrap();
    let comment_id_str = comment_result.root_id.clone();

    println!("Inserted comment: {}", comment_id);

    // Insert replies referencing this comment
    let reply1 = Reply {
        text: "First reply".to_string(),
        author: "Replier1".to_string(),
        comment: TdbLazy::new_id(&comment_id_str)?,
    };

    let reply2 = Reply {
        text: "Second reply".to_string(),
        author: "Replier2".to_string(),
        comment: TdbLazy::new_id(&comment_id_str)?,
    };

    client
        .save_instance(&reply1, schema_args.clone())
        .await
        .expect("Failed to insert reply1");

    client
        .save_instance(&reply2, schema_args.clone())
        .await
        .expect("Failed to insert reply2");

    // THE KEY TEST: Use .with::<Reply>() WITHOUT specifying the field
    // This should work because Reply has only one TdbLazy<Comment> field
    let result = Comment::find(comment_id.clone())
        .with::<Reply>()  // Should auto-use "comment" field name
        .with_client(&client)
        .execute(&spec)
        .await
        .expect("Query with .with::<Reply>() should succeed - DefaultField should resolve 'comment' field");

    let comments: Vec<Comment> = result.get().expect("Should deserialize comments");
    assert_eq!(comments.len(), 1, "Should find exactly one comment");
    assert_eq!(comments[0].text, "Parent comment for default field test");

    let replies: Vec<Reply> = result.get().expect("Should deserialize replies");
    assert_eq!(replies.len(), 2, "Should find exactly two replies");

    println!(
        "Found {} comments and {} replies using .with::<Reply>()",
        comments.len(),
        replies.len()
    );
    for reply in &replies {
        println!("  Reply: {} by {}", reply.text, reply.author);
    }

    Ok(())
}

// ============================================================================
// Subdocument unfolding test models
// ============================================================================

/// A date range subdocument
#[derive(Clone, Debug, Default, TerminusDBModel, PartialEq)]
#[tdb(unfoldable = true, key = "value_hash")]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

/// A session with a subdocument field
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Session {
    pub name: String,
    /// This is a subdocument - should be unfolded, not returned as a reference string
    #[tdb(subdocument = true)]
    pub date_range: DateRange,
}

/// A task that references a session (for testing relation queries with subdocuments)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct Task {
    pub title: String,
    /// Reference to parent session (document link)
    pub session: TdbLazy<Session>,
}

/// Test that relation queries properly unfold subdocuments in the fetched documents.
///
/// This is a regression test for the bug where ORM batch document fetch (Phase 2)
/// returned subdocument references as strings instead of unfolded data.
///
/// Before the fix:
///   date_range: "Session/.../date_range/DateRange/0S6lxALEPvP5jZBZ"
///
/// After the fix:
///   date_range: { start: "2024-01-01", end: "2024-12-31" }
#[cfg(feature = "testing")]
#[db_test(db = "orm_subdocument_unfold_test")]
async fn test_relation_query_unfolds_subdocuments(client: _, spec: _) -> anyhow::Result<()> {
    use terminusdb_client::DocumentInsertArgs;

    // Insert schemas
    let schema_args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    client
        .insert_schema(&DateRange::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert DateRange schema");

    client
        .insert_schema(&Session::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert Session schema");

    client
        .insert_schema(&Task::to_schema(), schema_args.clone())
        .await
        .expect("Failed to insert Task schema");

    // Insert a session with a subdocument
    let session = Session {
        name: "Q1 Review".to_string(),
        date_range: DateRange {
            start: "2024-01-01".to_string(),
            end: "2024-03-31".to_string(),
        },
    };

    let session_result = client
        .save_instance(&session, schema_args.clone())
        .await
        .expect("Failed to insert session");

    let session_id = session_result.root_ref::<Session>().unwrap();
    let session_id_str = session_result.root_id.clone();

    println!("Inserted session: {}", session_id);

    // Insert a task referencing the session
    let task = Task {
        title: "Complete review".to_string(),
        session: TdbLazy::new_id(&session_id_str)?,
    };

    client
        .save_instance(&task, schema_args.clone())
        .await
        .expect("Failed to insert task");

    // THE KEY TEST: Query session with related tasks
    // This triggers Phase 2 batch fetch, which should now unfold subdocuments
    let result = Session::find(session_id.clone())
        .with::<Task>() // This triggers relation loading (Phase 2)
        .with_client(&client)
        .execute(&spec)
        .await
        .expect("Query should succeed");

    let sessions: Vec<Session> = result.get().expect("Should deserialize sessions");
    assert_eq!(sessions.len(), 1, "Should find exactly one session");

    let fetched_session = &sessions[0];
    println!("Fetched session: {:?}", fetched_session);
    println!("  name: {}", fetched_session.name);
    println!("  date_range.start: {}", fetched_session.date_range.start);
    println!("  date_range.end: {}", fetched_session.date_range.end);

    // Verify the subdocument was properly unfolded (not a reference string)
    assert_eq!(
        fetched_session.date_range.start, "2024-01-01",
        "date_range.start should be '2024-01-01', not a reference string"
    );
    assert_eq!(
        fetched_session.date_range.end, "2024-03-31",
        "date_range.end should be '2024-03-31', not a reference string"
    );

    // Also verify the session name is correct
    assert_eq!(fetched_session.name, "Q1 Review");

    // Verify the task was also loaded
    let tasks: Vec<Task> = result.get().expect("Should deserialize tasks");
    assert_eq!(tasks.len(), 1, "Should find exactly one task");
    assert_eq!(tasks[0].title, "Complete review");

    println!("SUCCESS: Subdocument was properly unfolded in relation query!");

    Ok(())
}

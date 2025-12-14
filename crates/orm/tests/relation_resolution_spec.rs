//! ORM Relation Resolution Specification
//!
//! This file serves as a TDD specification for all the relation types and
//! resolution patterns the ORM should support.
//!
//! # Relation Types
//!
//! Note: All relations use TdbLazy<T> to create actual document links in TDB.
//! EntityIDFor<T> is just a typed string ID and doesn't enable relation traversal.
//!
//! 1. **One-to-One** (HasOne)
//!    - User has one Profile (via Profile.user: TdbLazy<User>)
//!    - Profile belongs to User
//!
//! 2. **One-to-Many** (HasMany)
//!    - User has many Posts (via Post.author: TdbLazy<User>)
//!    - Post belongs to User
//!
//! 3. **Many-to-Many** (through TdbLazy Vec)
//!    - Post has many Tags (via Vec<TdbLazy<Tag>>)
//!    - Tag appears in many Posts (reverse query)
//!
//! 4. **Self-Referential**
//!    - User has optional Manager (Option<TdbLazy<User>>)
//!    - User has many DirectReports (via reverse relation)
//!
//! 5. **Polymorphic** (via enum or trait object)
//!    - Comment can belong to Post or Video
//!
//! 6. **Nested/Deep Relations**
//!    - User -> Posts -> Comments -> Replies (multi-level traversal)
//!
//! # Resolution Patterns
//!
//! 1. **Eager Loading** - Load relations upfront with the query
//! 2. **Lazy Loading** - Load relations on-demand when accessed
//! 3. **Batch Loading** - Load relations for multiple parents efficiently
//! 4. **Filtered Loading** - Load only relations matching criteria
//! 5. **Ordered Loading** - Load relations in specific order
//! 6. **Paginated Loading** - Load limited subset of relations

use terminusdb_orm::prelude::*;

// Required for TerminusDBModel derive
use terminusdb_schema as terminusdb_schema;
#[allow(unused_imports)]
use terminusdb_schema::{ToTDBInstance, TdbLazy};
use terminusdb_schema_derive::TerminusDBModel;

use serde::{Deserialize, Serialize};

// ============================================================================
// Domain Models - Comprehensive Example
// ============================================================================

/// A user in the system - demonstrates multiple relation types
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct User {
    pub username: String,
    pub email: String,
    /// Self-referential: User's manager (optional one-to-one)
    pub manager: Option<TdbLazy<User>>,
}

/// User profile - one-to-one with User
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Profile {
    pub bio: String,
    pub avatar_url: Option<String>,
    /// Belongs to exactly one User (document link)
    pub user: TdbLazy<User>,
}

/// A blog post - one-to-many with User, many-to-many with Tag
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Post {
    pub title: String,
    pub content: String,
    pub published: bool,
    /// Belongs to one author (document link)
    pub author: TdbLazy<User>,
    /// Has many tags (many-to-many via direct Vec of document links)
    pub tags: Vec<TdbLazy<Tag>>,
}

/// A tag for categorizing posts
#[derive(Clone, Debug, Default, Serialize, Deserialize, TerminusDBModel)]
pub struct Tag {
    pub name: String,
    pub slug: String,
}

/// A comment on a post - demonstrates nested relations
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct Comment {
    pub text: String,
    /// The post this comment is on (document link)
    pub post: TdbLazy<Post>,
    /// The user who wrote this comment (document link)
    pub author: TdbLazy<User>,
    /// Optional parent comment for nested replies (document link)
    pub parent_comment: Option<TdbLazy<Comment>>,
}

/// A "like" on a post - simple join entity
#[derive(Clone, Debug, Serialize, Deserialize, TerminusDBModel)]
pub struct PostLike {
    pub post: TdbLazy<Post>,
    pub user: TdbLazy<User>,
}

// ============================================================================
// Test: One-to-One Relations
// ============================================================================

mod one_to_one {
    use super::*;

    /// User.profile() -> Option<Profile>
    /// Profile.user() -> User
    #[test]
    fn test_has_one_profile_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // Forward: User -> Profile via Profile.user
        // This is actually a reverse lookup (find Profile where user = this user)
        let query = User::find(user_id)
            .with::<Profile>(); // Loads the user's profile

        assert_eq!(query.relations().len(), 1);
    }

    /// Get Profile and eagerly load its User
    #[test]
    fn test_belongs_to_user_syntax() {
        let profile_id = EntityIDFor::<Profile>::new("profile1").unwrap();

        // Forward: Profile -> User via profile.user field
        let query = Profile::find(profile_id)
            .with_field::<User, ProfileFields::User>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Forward { field_name } => {
                assert_eq!(field_name, "user");
            }
            _ => panic!("Expected Forward direction"),
        }
    }
}

// ============================================================================
// Test: One-to-Many Relations
// ============================================================================

mod one_to_many {
    use super::*;

    /// User.posts() -> Vec<Post>
    #[test]
    fn test_has_many_posts_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // Reverse: Find all Posts where author = this user
        let query = User::find(user_id)
            .with::<Post>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Reverse { via_field } => {
                // via_field is None because we're loading via any TdbLazy<User> field
                assert!(via_field.is_none());
            }
            _ => panic!("Expected Reverse direction"),
        }
    }

    /// Post.author() -> User
    #[test]
    fn test_belongs_to_author_syntax() {
        let post_id = EntityIDFor::<Post>::new("post1").unwrap();

        // Forward: Post -> User via post.author field
        let query = Post::find(post_id)
            .with_field::<User, PostFields::Author>();

        assert_eq!(query.relations().len(), 1);
    }

    /// User.comments() -> Vec<Comment>
    /// (Comments authored by this user, not comments on user's posts)
    #[test]
    fn test_has_many_comments_via_specific_field() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // Reverse with specific field: Find Comments where author = this user
        let query = User::find(user_id)
            .with_via::<Comment, CommentFields::Author>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Reverse { via_field } => {
                assert_eq!(via_field.as_deref(), Some("author"));
            }
            _ => panic!("Expected Reverse direction"),
        }
    }
}

// ============================================================================
// Test: Many-to-Many Relations
// ============================================================================

mod many_to_many {
    use super::*;

    /// Post.tags() -> Vec<Tag>
    #[test]
    fn test_post_has_many_tags() {
        let post_id = EntityIDFor::<Post>::new("post1").unwrap();

        // Forward: Post -> Tags via post.tags field (Vec<TdbLazy<Tag>>)
        let query = Post::find(post_id)
            .with_field::<Tag, PostFields::Tags>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Forward { field_name } => {
                assert_eq!(field_name, "tags");
            }
            _ => panic!("Expected Forward direction"),
        }
    }

    /// Tag.posts() -> Vec<Post>
    /// Find all posts that have this tag
    #[test]
    fn test_tag_has_many_posts_reverse() {
        let tag_id = EntityIDFor::<Tag>::new("tag1").unwrap();

        // Reverse: Find all Posts where tags contains this tag
        // This requires the ORM to understand Vec<TdbLazy<T>> as a many-to-many
        let query = Tag::find(tag_id)
            .with::<Post>(); // Should find Posts that reference this Tag

        assert_eq!(query.relations().len(), 1);
    }
}

// ============================================================================
// Test: Self-Referential Relations
// ============================================================================

mod self_referential {
    use super::*;

    /// User.manager() -> Option<User>
    #[test]
    fn test_self_ref_belongs_to_manager() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // Forward: User -> User via manager field
        let query = User::find(user_id)
            .with_field::<User, UserFields::Manager>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Forward { field_name } => {
                assert_eq!(field_name, "manager");
            }
            _ => panic!("Expected Forward direction"),
        }
    }

    /// User.direct_reports() -> Vec<User>
    /// Find all users whose manager is this user
    #[test]
    fn test_self_ref_has_many_direct_reports() {
        let manager_id = EntityIDFor::<User>::new("manager1").unwrap();

        // Reverse: Find all Users where manager = this user
        let query = User::find(manager_id)
            .with_via::<User, UserFields::Manager>();

        assert_eq!(query.relations().len(), 1);
        match &query.relations()[0].direction {
            RelationDirection::Reverse { via_field } => {
                assert_eq!(via_field.as_deref(), Some("manager"));
            }
            _ => panic!("Expected Reverse direction"),
        }
    }
}

// ============================================================================
// Test: Nested/Deep Relations
// ============================================================================

mod nested_relations {
    use super::*;

    /// Load User with Posts, and each Post with its Comments
    #[test]
    fn test_nested_eager_loading_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // TODO: This syntax needs to be designed
        // Option 1: Nested .with() calls
        // let query = User::find(user_id)
        //     .with::<Post>()
        //     .nested::<Comment>();  // Load Comments for each Post
        //
        // Option 2: Path-based syntax
        // let query = User::find(user_id)
        //     .with_path::<(Post, Comment)>();
        //
        // Option 3: Builder pattern
        // let query = User::find(user_id)
        //     .with::<Post>(|posts| posts.with::<Comment>());

        // For now, verify we can at least chain multiple top-level relations
        let query = User::find(user_id)
            .with::<Post>()
            .with::<Comment>(); // Comments authored by user (not nested)

        assert_eq!(query.relations().len(), 2);
    }

    /// Comment.replies() -> Vec<Comment>
    /// Self-referential nested comments
    #[test]
    fn test_comment_replies() {
        let comment_id = EntityIDFor::<Comment>::new("comment1").unwrap();

        // Find all comments whose parent_comment = this comment
        let query = Comment::find(comment_id)
            .with_via::<Comment, CommentFields::ParentComment>();

        assert_eq!(query.relations().len(), 1);
    }
}

// ============================================================================
// Test: Batch Loading (N+1 Prevention)
// ============================================================================

mod batch_loading {
    use super::*;

    /// Load multiple users and their posts in batches
    #[test]
    fn test_batch_load_multiple_parents() {
        let user_ids = vec![
            EntityIDFor::<User>::new("user1").unwrap(),
            EntityIDFor::<User>::new("user2").unwrap(),
            EntityIDFor::<User>::new("user3").unwrap(),
        ];

        // Should generate efficient batch query, not N+1 queries
        let query = User::find_all(user_ids)
            .with::<Post>();

        assert_eq!(query.len(), 3);
        assert_eq!(query.relations().len(), 1);
    }
}

// ============================================================================
// Test: Filtered Relations (Future)
// ============================================================================

mod filtered_relations {
    use super::*;

    /// Load only published posts for a user
    #[test]
    #[ignore = "Filtered relations not yet implemented"]
    fn test_filtered_relation_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // TODO: Design filtering syntax
        // Option 1: Closure-based
        // let query = User::find(user_id)
        //     .with::<Post>()
        //     .filter(|post| post.published == true);
        //
        // Option 2: Method chaining
        // let query = User::find(user_id)
        //     .with_where::<Post>(PostFilters::published(true));
        //
        // Option 3: WOQL-based
        // let query = User::find(user_id)
        //     .with::<Post>()
        //     .where_eq("published", true);
    }
}

// ============================================================================
// Test: Ordered Relations (Future)
// ============================================================================

mod ordered_relations {
    use super::*;

    /// Load posts ordered by creation date
    #[test]
    #[ignore = "Ordered relations not yet implemented"]
    fn test_ordered_relation_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // TODO: Design ordering syntax
        // let query = User::find(user_id)
        //     .with::<Post>()
        //     .order_by("created_at", Order::Desc);
    }
}

// ============================================================================
// Test: Paginated Relations (Future)
// ============================================================================

mod paginated_relations {
    use super::*;

    /// Load first 10 posts for a user
    #[test]
    #[ignore = "Paginated relations not yet implemented"]
    fn test_paginated_relation_syntax() {
        let user_id = EntityIDFor::<User>::new("user1").unwrap();

        // TODO: Design pagination syntax
        // let query = User::find(user_id)
        //     .with::<Post>()
        //     .limit(10)
        //     .offset(0);
    }
}

// ============================================================================
// Test: Result Access Patterns
// ============================================================================

mod result_access {
    use super::*;

    /// Verify the result container provides typed access
    #[test]
    fn test_result_type_access_design() {
        // After executing a query, we should be able to:
        //
        // 1. Get all entities of a type:
        //    let users: Vec<User> = result.get::<User>()?;
        //    let posts: Vec<Post> = result.get::<Post>()?;
        //
        // 2. Get entities by parent ID (grouped):
        //    let user_posts: HashMap<EntityIDFor<User>, Vec<Post>> = result.get_grouped::<User, Post>()?;
        //
        // 3. Get a single related entity:
        //    let profile: Option<Profile> = result.get_one_related::<User, Profile>(user_id)?;
        //
        // 4. Iterate with relations:
        //    for user in result.iter::<User>() {
        //        let posts = user.related::<Post>();
        //    }

        // This is a design test - actual implementation will follow
    }
}

// ============================================================================
// Test: Query Execution Strategies
// ============================================================================

mod execution_strategies {
    use super::*;

    /// The ORM should support different execution strategies
    #[test]
    fn test_execution_strategy_design() {
        // 1. Single Query (GraphQL-style join)
        //    - Generate a single complex query that fetches all data
        //    - Pros: Single round-trip
        //    - Cons: Complex query, might fetch duplicates
        //
        // 2. Multi-Query (DataLoader-style)
        //    - First query: Fetch primary entities
        //    - Second query: Batch fetch related entities by collected IDs
        //    - Pros: Simpler queries, no duplicates
        //    - Cons: Multiple round-trips
        //
        // 3. Lazy Loading
        //    - Fetch primary entities only
        //    - Fetch relations on-demand when accessed
        //    - Pros: Only fetches what's needed
        //    - Cons: Potential N+1 if not careful
        //
        // The ORM should allow configuration:
        // let query = User::find(id)
        //     .with::<Post>()
        //     .strategy(LoadStrategy::SingleQuery); // or BatchQuery, Lazy
    }
}

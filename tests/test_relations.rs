use terminusdb_schema::{TdbLazy, EntityIDFor, TerminusDBModel};
use terminusdb_relation::{RelationTo, RelationFrom};

#[derive(Debug, Clone, TerminusDBModel)]
struct User {
    id: String,
    name: String,
    // Single relation to Post - should be default
    posts: Vec<TdbLazy<Post>>,
    // Multiple relations to User - need explicit default
    manager: Option<TdbLazy<User>>,
    #[tdb(default_relation)]
    reports: Vec<TdbLazy<User>>,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Post {
    id: String,
    title: String,
    content: String,
    author: TdbLazy<User>,
    comments: Vec<TdbLazy<Comment>>,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Comment {
    id: String,
    text: String,
    author: TdbLazy<User>,
    post: TdbLazy<Post>,
}

#[test]
fn test_single_relation_default() {
    // User has single relation to Post, so it should be the default
    let query = <User as RelationTo<Post>>::constraints();
    println!("User -> Post (default): {:?}", query);
}

#[test]
fn test_explicit_field_relation() {
    // Test explicit field access
    let query = <User as RelationTo<Post, UserPostsRelation>>::constraints();
    println!("User -> Post (explicit posts): {:?}", query);
}

#[test]
fn test_multiple_relations_with_default() {
    // User has multiple relations to User, but reports is marked as default
    let query = <User as RelationTo<User>>::constraints();
    println!("User -> User (default reports): {:?}", query);
}

#[test]
fn test_reverse_relation() {
    // Test automatic RelationFrom implementation
    let query = <Post as RelationFrom<User, UserPostsRelation>>::constraints();
    println!("Post <- User (posts): {:?}", query);
}

#[test]
fn test_custom_variables() {
    let query = <Post as RelationTo<Comment, PostCommentsRelation>>::constraints_with_vars("p", "c");
    println!("Post -> Comment with custom vars: {:?}", query);
}

#[test]
fn test_bidirectional_relations() {
    // Comment has relations to both User and Post
    let user_query = <Comment as RelationTo<User, CommentAuthorRelation>>::constraints();
    let post_query = <Comment as RelationTo<Post, CommentPostRelation>>::constraints();

    println!("Comment -> User: {:?}", user_query);
    println!("Comment -> Post: {:?}", post_query);

    // And reverse relations should work
    let from_user = <Comment as RelationFrom<User, UserReportsRelation>>::constraints();
    println!("Comment <- User: {:?}", from_user);
}
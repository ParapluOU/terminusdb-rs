#![cfg(feature = "generic-derive")]

use serde::{Deserialize, Serialize};
use terminusdb_schema::{TerminusDBField, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete TerminusDBModel types
#[derive(Debug, Clone, TerminusDBModel)]
struct User {
    id: String,
    name: String,
    email: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Article {
    id: String,
    title: String,
    content: String,
}

// A generic wrapper that works with TerminusDBModel types
#[derive(Debug, Clone, TerminusDBModel)]
struct Wrapper<T>
where
    T: TerminusDBField<Wrapper<T>>,
{
    id: String,
    content: T,
    created_at: String,
}

#[test]
fn generic_wrapper_with_models() {
    // Create a wrapper containing a User
    let user_wrapper = Wrapper::<User> {
        id: "wrapper-1".to_string(),
        content: User {
            id: "user-1".to_string(),
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        },
        created_at: "2024-01-01".to_string(),
    };

    // Verify we can get the schema
    let user_schema = <Wrapper<User> as ToTDBSchema>::to_schema();
    assert_eq!(user_schema.class_name(), "Wrapper<User>");

    // Create a wrapper containing an Article
    let article_wrapper = Wrapper::<Article> {
        id: "wrapper-2".to_string(),
        content: Article {
            id: "article-1".to_string(),
            title: "Generic Types in Rust".to_string(),
            content: "An introduction to generics...".to_string(),
        },
        created_at: "2024-01-02".to_string(),
    };

    let article_schema = <Wrapper<Article> as ToTDBSchema>::to_schema();
    assert_eq!(article_schema.class_name(), "Wrapper<Article>");

    // Verify we can convert to instances
    let _user_instance = user_wrapper.to_instance(None);
    let _article_instance = article_wrapper.to_instance(None);

    println!("✅ Generic Wrapper<T> successfully works with TerminusDBModel types!");
}

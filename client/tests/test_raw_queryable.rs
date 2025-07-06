use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::{vars, WoqlBuilder};
use terminusdb_woql2::prelude::Query;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Test model for raw query testing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Book {
    id: EntityIDFor<Self>,
    title: String,
    author: String,
    pages: i32,
}

/// Custom result type for finding books by a specific author
#[derive(Debug, Deserialize)]
struct BooksByAuthor {
    title: String,
    pages: i32,
}

/// Raw query implementation for finding books by author
struct BooksByAuthorQuery {
    author: String,
}

impl RawQueryable for BooksByAuthorQuery {
    type Result = BooksByAuthor;
    
    fn query(&self) -> Query {
        WoqlBuilder::new()
            .triple(vars!("Book"), "rdf:type", "@schema:Book")
            .triple(vars!("Book"), "@schema:author", vars!("Author"))
            .triple(vars!("Book"), "@schema:title", vars!("Title"))
            .triple(vars!("Book"), "@schema:pages", vars!("Pages"))
            .eq(vars!("Author"), self.author.clone())
            .select(vec![vars!("Title"), vars!("Pages")])
            .finalize()
    }
    
    fn extract_result(&self, mut binding: HashMap<String, serde_json::Value>) -> anyhow::Result<Self::Result> {
        let title = binding.remove("Title")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing title field"))?;
            
        let pages = binding.remove("Pages")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<i32>(v).ok())
            .unwrap_or(0);
        
        Ok(BooksByAuthor {
            title,
            pages,
        })
    }
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");
    
    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Book>(args).await.ok();
    
    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_raw_queryable_with_filter() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert test data
    let books = vec![
        Book {
            id: EntityIDFor::new("book1").unwrap(),
            title: "The Rust Programming Language".to_string(),
            author: "Steve Klabnik".to_string(),
            pages: 552,
        },
        Book {
            id: EntityIDFor::new("book2").unwrap(),
            title: "Programming Rust".to_string(),
            author: "Jim Blandy".to_string(),
            pages: 622,
        },
        Book {
            id: EntityIDFor::new("book3").unwrap(),
            title: "Rust in Action".to_string(),
            author: "Tim McNamara".to_string(),
            pages: 456,
        },
        Book {
            id: EntityIDFor::new("book4").unwrap(),
            title: "The Book of Rust".to_string(),
            author: "Steve Klabnik".to_string(),
            pages: 300,
        },
    ];
    
    for book in &books {
        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(book, args).await?;
    }
    
    // Execute raw query to find books by Steve Klabnik
    let query = BooksByAuthorQuery {
        author: "Steve Klabnik".to_string(),
    };
    let steve_books = client.execute_raw_query(&spec, query).await?;
    
    println!("Books by Steve Klabnik:");
    for book in &steve_books {
        println!("  {} - {} pages", book.title, book.pages);
    }
    
    // Verify results
    assert_eq!(steve_books.len(), 2, "Steve Klabnik should have 2 books");
    
    // Check the books
    let rust_lang = steve_books.iter()
        .find(|b| b.title == "The Rust Programming Language")
        .expect("Should find The Rust Programming Language");
    assert_eq!(rust_lang.pages, 552);
    
    let book_of_rust = steve_books.iter()
        .find(|b| b.title == "The Book of Rust")
        .expect("Should find The Book of Rust");
    assert_eq!(book_of_rust.pages, 300);
    
    Ok(())
}

/// Simple custom query that just selects specific fields
#[derive(Debug, Deserialize)]
struct BookSummary {
    title: String,
    author: String,
}

struct BookSummaryQuery;

impl RawQueryable for BookSummaryQuery {
    type Result = BookSummary;
    
    fn query(&self) -> Query {
        WoqlBuilder::new()
            .triple(vars!("Book"), "rdf:type", "@schema:Book")
            .triple(vars!("Book"), "@schema:title", vars!("Title"))
            .triple(vars!("Book"), "@schema:author", vars!("Author"))
            .select(vec![vars!("Title"), vars!("Author")])
            .finalize()
    }
    
    fn extract_result(&self, mut binding: HashMap<String, serde_json::Value>) -> anyhow::Result<Self::Result> {
        let title = binding.remove("Title")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing title field"))?;
            
        let author = binding.remove("Author")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing author field"))?;
        
        Ok(BookSummary { title, author })
    }
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_raw_queryable_simple_projection() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert a test book
    let book = Book {
        id: EntityIDFor::new("test_book").unwrap(),
        title: "Test Book".to_string(),
        author: "Test Author".to_string(),
        pages: 100,
    };
    
    let args = DocumentInsertArgs::from(spec.clone());
    client.save_instance(&book, args).await?;
    
    // Execute raw query
    let summaries = client.execute_raw_query(&spec, BookSummaryQuery).await?;
    
    println!("Book summaries:");
    for summary in &summaries {
        println!("  {} by {}", summary.title, summary.author);
    }
    
    // Verify we got our book
    let our_book = summaries.iter()
        .find(|s| s.title == "Test Book")
        .expect("Should find our test book");
    
    assert_eq!(our_book.author, "Test Author");
    
    Ok(())
}
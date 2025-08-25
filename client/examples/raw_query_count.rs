use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use terminusdb_client::{RawQueryable, TerminusDBHttpClient, BranchSpec};
use terminusdb_woql2::prelude::Query;
use terminusdb_woql_builder::{builder::WoqlBuilder, vars};

/// Example demonstrating the count functionality for RawQueryable
#[derive(Debug, Deserialize)]
struct BookInfo {
    title: String,
    pages: i32,
}

/// Query to find all books by a specific author
struct BooksByAuthorQuery {
    author: String,
}

impl RawQueryable for BooksByAuthorQuery {
    type Result = BookInfo;

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

    fn extract_result(
        &self,
        mut binding: HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Self::Result> {
        let title = binding
            .remove("Title")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing title field"))?;

        let pages = binding
            .remove("Pages")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<i32>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing pages field"))?;

        Ok(BookInfo { title, pages })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to local TerminusDB instance
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::with_branch("bookstore", "main");

    // Create a query for books by a specific author
    let query = BooksByAuthorQuery {
        author: "J.K. Rowling".to_string(),
    };

    // Get the count of books by this author
    match query.count(&client, &spec).await {
        Ok(count) => {
            println!("Found {} books by {}", count, query.author);
            
            // If there are books, fetch them
            if count > 0 {
                match query.apply(&client, &spec).await {
                    Ok(books) => {
                        println!("\nBook details:");
                        for book in books {
                            println!("- {} ({} pages)", book.title, book.pages);
                        }
                    }
                    Err(e) => eprintln!("Error fetching books: {}", e),
                }
            }
        }
        Err(e) => eprintln!("Error counting books: {}", e),
    }

    // Demonstrate the query_count() method
    println!("\n--- Demonstrating query_count() ---");
    let count_query = query.query_count();
    match count_query {
        Query::Count(_) => println!("✓ query_count() correctly wraps the query in a Count operation"),
        _ => println!("✗ query_count() did not produce a Count query"),
    }

    Ok(())
}